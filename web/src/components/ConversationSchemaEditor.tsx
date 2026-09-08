import { useEffect, useRef, useState } from "react";
import { api, ApiRequestError } from "../api";
import type { ConversationSchemaDraft } from "../types";
import { ConversationBuilderCard } from "./ConversationBuilderCard";
import type { OpenConversationSample } from "./ConversationSampleCard";
import { ConversationSchemaFields } from "./ConversationSchemaFields";

/** A semantic draft is not the Project schema, a published workflow or a Run. */
type SchemaEditorProps = {
  project: string; conversation: string; task: string; call?: string; schemaId?: string; onDirtyChange: (dirty: boolean) => void; onAssistance?:()=>void; onSample: OpenConversationSample; readOnly?: boolean; onDraftChange?: (draft: ConversationSchemaDraft) => void;
};
export function ConversationSchemaEditor(props: SchemaEditorProps) {
  return <SchemaEditor key={`${props.project}:${props.conversation}:${props.task}:${props.schemaId ?? props.call}`} {...props} />;
}
function SchemaEditor({ project, conversation, task, call, schemaId, onDirtyChange, onSample, onAssistance, onDraftChange, readOnly = false }: SchemaEditorProps) {
  const [draft, setDraft] = useState<ConversationSchemaDraft>();
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
    const load = schemaId ? api.conversationSchemaDraft(project, schemaId, controller.signal) : call ? api.conversationSchemaDraftForCall(project, conversation, task, call, controller.signal) : Promise.reject(new Error("Schema source is missing"));
    void load.then((saved) => {
      if (saved && saved.task_id !== task) throw new Error("Schema Draft belongs to another task");
      if (!controller.signal.aborted) { setDraft(saved ?? undefined); setReady(true); if (saved) onDraftChange?.(saved); }
    }).catch((error: Error) => { if (!controller.signal.aborted) setError(error.message); });
    return () => { alive.current = false; controller.abort(); };
  }, [project, conversation, task, call, schemaId]);
  async function save() {
    if (pending.current || readOnly) return;
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
        : call ? await api.saveConversationSchemaDraft(project, conversation, task, call) : (()=>{throw new Error("Schema source is missing");})();
      if (alive.current) { setDraft(saved); setEditing(false); setUncertain(false); frozen.current = undefined; onDraftChange?.(saved); }
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
      const saved = schemaId ? await api.conversationSchemaDraft(project,schemaId) : call ? await api.conversationSchemaDraftForCall(project,conversation,task,call) : undefined;
      if (saved && saved.task_id !== task) throw new Error("Schema Draft belongs to another task");
      if (alive.current && saved) { setDraft(saved); setError(""); onDraftChange?.(saved); }
    } catch (error) { if (alive.current) setError((error as Error).message); }
    finally { pending.current = false; if (alive.current) setBusy(false); }
  }
  return <section className="conversation-schema-editor" aria-label="Saved label draft">
    {draft && <p>{draft.definition.task.kind === "bounding_box" ? "Object boxes" : "Whole-image categories"} · Label draft only</p>}
    <p role="status">{busy ? "Saving Schema Draft…" : !ready ? "Loading saved label draft…" : uncertain ? "Save outcome unknown; retry the same edit" : dirty ? `Unsaved edits · Based on revision ${draft?.revision}` : draft ? `Schema Draft saved · Revision ${draft.revision}` : "The model proposal is saved; its editable Draft still needs to be recovered locally. No extra model call is needed."}</p>
    {draft && !editing && <><ul>{draft.definition.task.labels.map((label) => <li key={label}>{label}</li>)}</ul><button disabled={readOnly} onClick={() => { setLabels(draft.definition.task.labels.join("\n")); setRules(draft.definition.boundary_rules.join("\n")); setEditing(true); }}>Edit labels and boundary rules</button></>}
    {editing && <><ConversationSchemaFields value={{ goal: draft?.definition.goal ?? "", kind: draft?.definition.task.kind ?? "bounding_box", labels, rules }} disabled={busy || uncertain || readOnly} onChange={value => { setLabels(value.labels); setRules(value.rules); }} /><small>Edits belong to this Schema Draft only. They do not change published workflows or accept annotations.</small></>}
    {(!draft || editing) && <div className="button-row">{editing && <button disabled={busy || uncertain} onClick={() => { setEditing(false); setError(""); }}>Cancel edit</button>}<button disabled={!ready || busy || readOnly || (editing && !labels.trim())} onClick={() => void save()}>{uncertain ? "Retry same Schema save" : draft ? "Save Schema changes" : "Save as editable Schema Draft"}</button></div>}
    {error && <><p role="alert">{error} Your entered labels remain here.</p>{editing && !uncertain && <button disabled={busy} onClick={() => void reloadBase()}>Load latest revision; keep my edits</button>}</>}
    {draft && <ConversationBuilderCard project={project} conversation={conversation} task={task} schema={draft} editing={editing || busy || uncertain || readOnly} onSample={onSample} onAssistance={onAssistance} />}
  </section>;
}
