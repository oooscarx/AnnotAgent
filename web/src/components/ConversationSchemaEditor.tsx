import { useEffect, useId, useRef, useState } from "react";
import { api, ApiRequestError } from "../api";
import type { ConversationSchemaDraft } from "../types";

/** A semantic draft is not the Project schema, a published workflow or a Run. */
export function ConversationSchemaEditor({ project, conversation, task, call, onDirtyChange }: {
  project: string; conversation: string; task: string; call: string; onDirtyChange: (dirty: boolean) => void;
}) {
  const [draft, setDraft] = useState<ConversationSchemaDraft>();
  const fieldId = useId();
  const [labels, setLabels] = useState("");
  const [rules, setRules] = useState("");
  const [editing, setEditing] = useState(false);
  const [ready, setReady] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [uncertain, setUncertain] = useState(false);
  const pending = useRef(false);
  const alive = useRef(true);
  const frozen = useRef<Parameters<typeof api.editConversationSchemaDraft>[2] | undefined>(undefined);
  const dirty = busy || uncertain || (editing && (labels !== draft?.definition.task.labels.join("\n") || rules !== draft?.definition.boundary_rules.join("\n")));
  useEffect(() => { onDirtyChange(dirty); return () => onDirtyChange(false); }, [dirty, onDirtyChange]);
  useEffect(() => {
    alive.current = true; const controller = new AbortController();
    void api.conversationSchemaDraftForCall(project, conversation, task, call, controller.signal).then((saved) => {
      if (!controller.signal.aborted) { setDraft(saved ?? undefined); setReady(true); }
    }).catch((error: Error) => { if (!controller.signal.aborted) setError(error.message); });
    return () => { alive.current = false; controller.abort(); };
  }, [project, conversation, task, call]);
  async function save() {
    if (pending.current) return;
    pending.current = true; setBusy(true); setError("");
    try {
      if (draft) frozen.current ??= {
        request_id: crypto.randomUUID(), expected_revision: draft.revision,
        decision: { decision: "draft", kind: draft.definition.task.kind,
          labels: labels.split("\n").map((label) => label.trim()).filter(Boolean),
          multi_label: draft.definition.task.multi_label, attributes: draft.definition.task.attributes,
          boundary_rules: rules.split("\n").map((rule) => rule.trim()).filter(Boolean), rationale: "User-edited annotation semantics" },
      };
      const saved = draft
        ? await api.editConversationSchemaDraft(project, draft.id, frozen.current!)
        : await api.saveConversationSchemaDraft(project, conversation, task, call);
      if (alive.current) { setDraft(saved); setEditing(false); setUncertain(false); frozen.current = undefined; }
    } catch (error) {
      if (alive.current) {
        setError((error as Error).message);
        const rejected = error instanceof ApiRequestError && [400,401,403,404,409,422].includes(error.status);
        setUncertain(!rejected);
        if (rejected) frozen.current = undefined;
      }
    } finally { pending.current = false; if (alive.current) setBusy(false); }
  }
  async function reloadBase() {
    if (pending.current) return;
    pending.current = true; setBusy(true);
    try {
      const saved = await api.conversationSchemaDraftForCall(project,conversation,task,call);
      if (alive.current && saved) { setDraft(saved); setError(""); }
    } catch (error) { if (alive.current) setError((error as Error).message); }
    finally { pending.current = false; if (alive.current) setBusy(false); }
  }
  return <section className="conversation-schema-editor" aria-label="Saved label draft">
    <p role="status">{busy ? "Saving Schema Draft…" : uncertain ? "Save outcome unknown; retry the same edit" : dirty ? `Unsaved edits · Based on revision ${draft?.revision}` : draft ? `Schema Draft saved · Revision ${draft.revision}` : "This proposal has not yet become an editable draft"}</p>
    {draft && !editing && <><ul>{draft.definition.task.labels.map((label) => <li key={label}>{label}</li>)}</ul><button onClick={() => { setLabels(draft.definition.task.labels.join("\n")); setRules(draft.definition.boundary_rules.join("\n")); setEditing(true); }}>Edit labels and boundary rules</button></>}
    {editing && <><label htmlFor={`${fieldId}-labels`}>Labels · one per line</label><textarea id={`${fieldId}-labels`} rows={3} value={labels} disabled={busy || uncertain} onChange={(event) => setLabels(event.target.value)} /><label htmlFor={`${fieldId}-rules`}>Boundary rules · one per line</label><textarea id={`${fieldId}-rules`} rows={3} value={rules} disabled={busy || uncertain} onChange={(event) => setRules(event.target.value)} /><small>Edits belong to this Schema Draft only. They do not change published workflows or accept annotations.</small></>}
    {(!draft || editing) && <div className="button-row">{editing && <button disabled={busy || uncertain} onClick={() => { setEditing(false); setError(""); }}>Cancel edit</button>}<button disabled={!ready || busy || (editing && !labels.trim())} onClick={() => void save()}>{uncertain ? "Retry same Schema save" : draft ? "Save Schema changes" : "Save as editable Schema Draft"}</button></div>}
    {error && <><p role="alert">{error} Your entered labels remain here.</p>{editing && !uncertain && <button disabled={busy} onClick={() => void reloadBase()}>Load latest revision; keep my edits</button>}</>}
  </section>;
}
