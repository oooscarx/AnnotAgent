import type { ConversationMessage } from "./types";
import { isAnnotationGoalMessage } from "./conversation-control";

export interface HistoryReader {
  latest(): Promise<ConversationMessage[]>;
  forward(after: number): Promise<ConversationMessage[]>;
  exact(id: string): Promise<ConversationMessage>;
}

// Visible history and task context are intentionally separate. Paging must never
// replace an old task's source with the first message in the visible page.
export async function loadConversationHistory(reader: HistoryReader, sourceId: string | undefined, referenceId: string | undefined, signal: AbortSignal) {
  const check = () => { if (signal.aborted) throw new DOMException("Aborted", "AbortError"); };
  check();
  const messages = await reader.latest();
  check();
  const context: ConversationMessage[] = [];
  let referenceError: string | undefined;
  for (const id of new Set([sourceId, referenceId].filter((id): id is string => Boolean(id)))) {
    try {
      const message = messages.find(message => message.input.id === id) ?? await reader.exact(id);
      check();
      context.push(message);
    } catch (error) {
      check();
      if (id === sourceId) throw error;
      // A bad optional deep link must not strand the valid Project in Loading.
      // The canvas's exact-reference guard still refuses to substitute a result.
      referenceError = (error as Error).message;
    }
  }
  let defaultGoal: ConversationMessage | undefined;
  if (!sourceId) {
    // Preserve the existing oldest-unscoped-message fallback, including journals
    // starting with stop/candidate notes. Do not silently choose a newer goal.
    let after = 0;
    while (true) {
      check();
      const page = await reader.forward(after);
      check();
      defaultGoal = page.find(isAnnotationGoalMessage);
      if (defaultGoal || page.length < 100) break;
      const next = page.at(-1)!.sequence;
      if (next <= after) throw new Error("Conversation history cursor did not advance.");
      after = next;
    }
  }
  return { messages, context, defaultGoal, referenceError };
}
