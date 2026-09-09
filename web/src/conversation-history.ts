import type { ConversationMessage } from "./types";

export interface HistoryReader {
  latest(): Promise<ConversationMessage[]>;
  firstGoal(): Promise<ConversationMessage | undefined>;
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
    // Preserve the oldest unscoped goal without downloading every preceding note.
    defaultGoal = await reader.firstGoal();
    check();
  }
  return { messages, context, defaultGoal, referenceError };
}
