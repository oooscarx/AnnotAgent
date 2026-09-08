import { useCallback, useEffect, useRef, useState } from "react";
import { ApiRequestError } from "../api";
import { futureSchemaApi, type FutureSchemaState } from "../conversation-future-schema-api";
import { futureSchemaDefinition, futureSchemaDiff, futureSchemaLocalConflicts, futureSchemaSourceMatches, makeFutureSchemaInput, makeFutureSchemaLocal, mergeFutureSchemaState, parseFutureSchemaLocal, sameSavedFutureSchemaInput, schemaFields, type FutureSchemaLocal, type SchemaDefinition } from "../conversation-future-schema";
import type { ConversationSchemaDraft } from "../types";
import { ConversationSchemaFields } from "./ConversationSchemaFields";
import { ConversationSchemaEditor } from "./ConversationSchemaEditor";
import type { OpenConversationSample } from "./ConversationSampleCard";

export function FutureSchemaDiff({ before, after }: { before: SchemaDefinition; after: SchemaDefinition }) {
  const changes = futureSchemaDiff(before, after);
  return <section className="conversation-future-schema-diff" aria-label="Future rule changes"><h4>Changes from the tested rules</h4>
    {!changes.length ? <p>No semantic changes yet.</p> : changes.map(change => <div key={change.field}><h5>{change.field}</h5><div className="conversation-future-schema-comparison"><div><strong>Tested rules</strong>{change.before.length ? <ul>{change.before.map((text, index) => <li key={index}>{text}</li>)}</ul> : <p>None</p>}</div><div><strong>Proposed rules</strong>{change.after.length ? <ul>{change.after.map((text, index) => <li key={index}>{text}</li>)}</ul> : <p>None</p>}</div></div>{change.added && <small>{change.added.length} added · {change.removed?.length ?? 0} removed{!change.added.length && !change.removed?.length ? " · Order changed" : ""}</small>}</div>)}
  </section>;
}

/** A human-authored fork of sealed semantics, not a model call or a global apply. */
export function ConversationFutureSchemaCard({ project, conversation, task, call, sourceAnswer, cancelled, onDirtyChange, onSample, onAssistance }: {
  project: string; conversation: string; task: string; call: string; sourceAnswer: string; cancelled: boolean;
  onDirtyChange?: (dirty: boolean) => void; onSample: OpenConversationSample; onAssistance?: () => void;
}) {
  const storageKey = `annotagent.future-schema:${project}:${conversation}:${task}:${call}`;
  const [state, setState] = useState<FutureSchemaState>();
  const [local, setLocal] = useState<FutureSchemaLocal>();
  const [opened, setOpened] = useState(false), [busy, setBusy] = useState(false), [ready, setReady] = useState(false), [durable, setDurable] = useState(true), [error, setError] = useState("");
  const alive = useRef(true), pending = useRef(false), revision = useRef(0), currentLocal = useRef(local);
  const dirtySources = useRef(new Set<string>()), dirtyCallback = useRef(onDirtyChange);
  dirtyCallback.current = onDirtyChange; currentLocal.current = local;
  const setDirty = useCallback((source: string, dirty: boolean) => { if (dirty) dirtySources.current.add(source); else dirtySources.current.delete(source); dirtyCallback.current?.(dirtySources.current.size > 0); }, []);
  const editorDirty = useCallback((dirty: boolean) => setDirty("editor", dirty), [setDirty]);
  const editorChanged = useCallback((schema: ConversationSchemaDraft) => {
    setState(current => current?.record?.schema_id === schema.id && schema.task_id === task && (!current.schema || schema.revision > current.schema.revision) ? { ...current, schema } : current);
  }, [task]);
  const stale = Boolean(state && local && !futureSchemaSourceMatches(local, state));
  const conflict = futureSchemaLocalConflicts(local, state);
  const changed = Boolean(state && local && JSON.stringify(local.fields) !== JSON.stringify(schemaFields(state.base_schema.definition)));

  function retain(next: FutureSchemaLocal) {
    currentLocal.current = next; setLocal(next);
    try { sessionStorage.setItem(storageKey, JSON.stringify(next)); setDurable(true); setDirty("proposal", false); }
    catch { setDurable(false); setDirty("proposal", true); }
  }
  function clearLocal() {
    currentLocal.current = undefined; setLocal(undefined); setDirty("proposal", false);
    try { sessionStorage.removeItem(storageKey); } catch { /* A saved server record remains authoritative. */ }
  }
  function apply(value: FutureSchemaState) {
    if (value.scope !== "future_tasks_only" || value.base_schema.task_id !== task || value.source.scope_answer_command_id !== sourceAnswer || (value.record && (value.record.input.feedback_call_id !== call || value.record.input.scope_answer_command_id !== sourceAnswer || value.record.input.context_digest !== value.source.context_digest || value.record.input.base_schema_id !== value.base_schema.id || value.record.input.base_schema_revision !== value.base_schema.revision)) || (value.schema && (value.schema.task_id !== task || value.schema.id !== value.record?.schema_id || value.schema.id === value.base_schema.id))) throw new Error("The future rule source does not match this feedback task. No other Schema was substituted.");
    setState(previous => mergeFutureSchemaState(previous, value));
    const previous = currentLocal.current;
    if (value.record && value.schema) {
      setOpened(true);
      if (!previous || (previous.frozen && sameSavedFutureSchemaInput(previous.frozen, value.record.input, call))) clearLocal();
    }
  }
  useEffect(() => {
    alive.current = true; const controller = new AbortController(), ticket = ++revision.current;
    try { const saved = parseFutureSchemaLocal(sessionStorage.getItem(storageKey), call); if (saved) { currentLocal.current = saved; setLocal(saved); setOpened(true); } }
    catch { setDurable(false); }
    void futureSchemaApi.read(project, conversation, task, call, controller.signal).then(value => {
      if (controller.signal.aborted || ticket !== revision.current) return;
      apply(value); setReady(true);
    }).catch((reason: Error) => { if (!controller.signal.aborted && ticket === revision.current) { setError(reason.message); setReady(true); } });
    return () => { alive.current = false; revision.current++; controller.abort(); dirtyCallback.current?.(false); };
  }, [storageKey]);
  async function reload(open = false) {
    if (pending.current) return; pending.current = true; setBusy(true); setError(""); const ticket = ++revision.current;
    try {
      const value = await futureSchemaApi.read(project, conversation, task, call);
      if (!alive.current || ticket !== revision.current) return;
      apply(value); setReady(true);
      if (open) { setOpened(true); if (!currentLocal.current && !value.record) retain(makeFutureSchemaLocal(call, value)); }
    } catch (reason) { if (alive.current && ticket === revision.current) setError((reason as Error).message); }
    finally { pending.current = false; if (alive.current) setBusy(false); }
  }
  async function save() {
    if (pending.current || cancelled || !state || !local || stale || state.record) return;
    const command = local.frozen ?? makeFutureSchemaInput(crypto.randomUUID(), state, local.fields);
    pending.current = true; setBusy(true); setError(""); const ticket = ++revision.current;
    retain({ ...local, frozen: command });
    try {
      const value = await futureSchemaApi.save(project, conversation, task, call, command);
      if (!alive.current || ticket !== revision.current) return;
      apply(value);
      if (!value.record || !sameSavedFutureSchemaInput(command, value.record.input, call)) throw new Error("A different future rule draft is saved. Your original proposal was not substituted.");
      onAssistance?.();
    } catch (reason) {
      if (!alive.current || ticket !== revision.current) return;
      setError((reason as Error).message);
      try {
        const recovered = await futureSchemaApi.read(project, conversation, task, call);
        if (!alive.current || ticket !== revision.current) return;
        apply(recovered);
        if (recovered.record && sameSavedFutureSchemaInput(command, recovered.record.input, call)) { setError(""); onAssistance?.(); }
        else if (!recovered.record && reason instanceof ApiRequestError && [400, 401, 403, 404, 409, 410, 422, 429].includes(reason.status)) retain({ ...local, frozen: undefined });
      } catch { /* Unknown outcome preserves the exact command; no new POST is implicit. */ }
    } finally { pending.current = false; if (alive.current) setBusy(false); }
  }
  const proposed = state && local ? futureSchemaDefinition(state.base_schema, local.fields) : state?.schema?.definition;
  const diff = state && proposed ? futureSchemaDiff(state.base_schema.definition, proposed) : [];
  return <section className="conversation-future-schema" aria-label="Future rule draft">
    <h4>Rules for future work</h4>
    <p>Create a separate, human-edited label draft. This does not ask a model to propose rules or apply a rule across the project.</p>
    {!ready && <p role="status">Checking saved future rule draft…</p>}
    {!opened && !cancelled && <button disabled={!ready || busy} onClick={() => void reload(true)}>Edit future rule draft</button>}
    {opened && <><p>The original Schema, tested workflow, sample results, published versions, existing Runs and annotations remain unchanged. A new plan and sample test require fresh authorization before any model call.</p>
      {state && <small>Based on tested Schema {state.base_schema.id} · Revision {state.base_schema.revision}. Task identity, multiple-label behavior and attributes are preserved.</small>}
      {state?.record && state.schema && <><p role="status">Future rule draft saved</p><p>New Schema {state.schema.id} · Revision {state.schema.revision} · Human-authored, no model call.</p></>}
      {local && (!state?.record || conflict) && <form onSubmit={event => { event.preventDefault(); void save(); }}>
        <ConversationSchemaFields includeGoalAndKind value={local.fields} disabled={busy || cancelled || stale || Boolean(local.frozen) || Boolean(state?.record)} onChange={fields => { retain({ ...local, fields }); setError(""); }} />
        {state && proposed && <FutureSchemaDiff before={state.base_schema.definition} after={proposed} />}
        {stale && <p role="alert">The saved source differs from this browser proposal. Your fields are retained, but cannot be saved against a different Schema or scope answer.</p>}
        {conflict && <><p role="alert">A different future rule draft is already saved. Your original browser proposal is retained read-only and has not overwritten it.</p><button type="button" disabled={busy} onClick={() => { clearLocal(); setError(""); }}>Discard browser proposal; open saved draft</button></>}
        {!state?.record && !cancelled && !stale && <div className="conversation-feedback-actions"><button type="button" disabled={busy || Boolean(local.frozen)} onClick={() => { clearLocal(); setOpened(false); setError(""); }}>Discard unsaved future rule edits</button><button className="primary" type="submit" disabled={!ready || busy || !local.fields.goal.trim() || !local.fields.labels.trim() || !diff.length}>{local.frozen ? "Retry same future rule save" : "Save future rule draft"}</button></div>}
        {local.frozen && !state?.record && <p role="status">Save acknowledgement is not confirmed. The exact command and original rules are retained; retry does not create another proposal or call a model.</p>}
        {changed && !state?.record && <small>{durable ? "Not saved to the server · Edits are kept in this browser tab." : "Browser storage is unavailable. These edits exist only on this page; leaving requires confirmation."}</small>}
      </form>}
      {!local && state?.schema && <><FutureSchemaDiff before={state.base_schema.definition} after={state.schema.definition} /><ConversationSchemaEditor key={state.schema.id} project={project} conversation={conversation} task={task} schemaId={state.schema.id} onDirtyChange={editorDirty} onDraftChange={editorChanged} onSample={onSample} onAssistance={onAssistance} readOnly={cancelled} /></>}
    </>}
    {cancelled && <p role="status">This feedback action is cancelled. Any saved future rule draft and browser edits are read-only here; nothing was applied or undone.</p>}
    {error && <p role="alert">{error} Saved drafts and the original results remain on the server. Reload only reads saved state.</p>}
    {(opened || error) && <button disabled={busy} onClick={() => void reload()}>Reload saved future rule draft</button>}
  </section>;
}
