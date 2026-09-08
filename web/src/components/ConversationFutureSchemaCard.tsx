import { useCallback, useEffect, useRef, useState } from "react";
import { ApiRequestError } from "../api";
import { futureSchemaApi, type FutureSchemaState } from "../conversation-future-schema-api";
import { futureSchemaDefinition, futureSchemaDiff, futureSchemaLocalConflicts, futureSchemaSourceMatches, makeFutureSchemaInput, makeFutureSchemaLocal, mergeFutureSchemaState, parseFutureSchemaLocal, sameSavedFutureSchemaInput, schemaFields, type FutureSchemaLocal } from "../conversation-future-schema";
import type { ConversationSchemaDraft } from "../types";
import { ConversationSchemaFields } from "./ConversationSchemaFields";
import { ConversationSchemaEditor } from "./ConversationSchemaEditor";
import { ConversationFutureSchemaProposal } from "./ConversationFutureSchemaProposal";
import { futureProposalDefinition, futureProposalSourceMatches, mergeFutureProposal } from "../conversation-future-proposal";
import type { FutureProposalStatus } from "../conversation-future-proposal-api";
import type { OpenConversationSample } from "./ConversationSampleCard";
import { FutureSchemaDiff } from "./FutureSchemaDiff";

/** Human-reviewed forks of sealed semantics; model suggestions have separate authority. */
export function ConversationFutureSchemaCard({ project, conversation, task, call, sourceAnswer, cancelled, onDirtyChange, onSample, onAssistance, onSetup }: {
  project: string; conversation: string; task: string; call: string; sourceAnswer: string; cancelled: boolean;
  onDirtyChange?: (dirty: boolean) => void; onSample: OpenConversationSample; onAssistance?: () => void; onSetup?: () => void;
}) {
  const storageKey = `annotagent.future-schema:${project}:${conversation}:${task}:${call}`;
  const [state, setState] = useState<FutureSchemaState>();
  const [local, setLocal] = useState<FutureSchemaLocal>();
  const [modelProposal, setModelProposal] = useState<FutureProposalStatus>();
  const [opened, setOpened] = useState(false), [busy, setBusy] = useState(false), [ready, setReady] = useState(false), [durable, setDurable] = useState(true), [error, setError] = useState("");
  const alive = useRef(true), pending = useRef(false), revision = useRef(0), currentLocal = useRef(local);
  const dirtySources = useRef(new Set<string>()), dirtyCallback = useRef(onDirtyChange);
  dirtyCallback.current = onDirtyChange; currentLocal.current = local;
  const setDirty = useCallback((source: string, dirty: boolean) => { if (dirty) dirtySources.current.add(source); else dirtySources.current.delete(source); dirtyCallback.current?.(dirtySources.current.size > 0); }, []);
  const editorDirty = useCallback((dirty: boolean) => setDirty("editor", dirty), [setDirty]);
  const modelDirty = useCallback((dirty: boolean) => setDirty("model", dirty), [setDirty]);
  const modelChanged = useCallback((value: FutureProposalStatus) => setModelProposal(current => mergeFutureProposal(current, value)), []);
  const editorChanged = useCallback((schema: ConversationSchemaDraft) => {
    setState(current => current?.record?.schema_id === schema.id && schema.task_id === task && (!current.schema || schema.revision > current.schema.revision) ? { ...current, schema } : current);
  }, [task]);
  const stale = Boolean(state && local && !futureSchemaSourceMatches(local, state));
  const conflict = futureSchemaLocalConflicts(local, state);
  const changed = Boolean(state && local && futureSchemaDiff(state.base_schema.definition, futureSchemaDefinition(state.base_schema, local.fields, local.proposal)).length);

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
  async function save(value = local) {
    if (pending.current || cancelled || !state || !value || !futureSchemaSourceMatches(value, state) || state.record) return;
    if (value.proposal && (!modelProposal || modelProposal.cancelled || modelProposal.authorization.consent.call_id !== value.proposal.proposal_call_id || modelProposal.proposal_digest !== value.proposal.proposal_digest)) { setError("Reload the exact saved model proposal before saving these reviewed rules. It may have been cancelled or changed."); return; }
    const command = value.frozen ?? makeFutureSchemaInput(crypto.randomUUID(), state, value.fields, value.proposal);
    pending.current = true; setBusy(true); setError(""); const ticket = ++revision.current;
    retain({ ...value, frozen: command });
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
        else if (!recovered.record && reason instanceof ApiRequestError && [400, 401, 403, 404, 409, 410, 422, 429].includes(reason.status)) retain({ ...value, frozen: undefined });
      } catch { /* Unknown outcome preserves the exact command; no new POST is implicit. */ }
    } finally { pending.current = false; if (alive.current) setBusy(false); }
  }
  async function useProposal(value: FutureProposalStatus, edit: boolean) {
    if (pending.current || cancelled || !state || state.record || currentLocal.current) throw new Error("Your existing future-rule edits are retained. Save or explicitly discard them before using a model suggestion.");
    const proposal = value.proposal && "Ok" in value.proposal ? value.proposal.Ok : undefined;
    const source = { ...state.source, feedback_call_id: call, base_schema_id: state.base_schema.id, base_schema_revision: state.base_schema.revision };
    if (value.cancelled || !proposal || !value.proposal_digest || value.receipt?.status !== "completed" || !futureProposalSourceMatches(value.authorization.source, source)) throw new Error("This saved suggestion is not available for this exact tested source.");
    const definition = proposal.decision.decision === "draft" ? futureProposalDefinition(state.base_schema, proposal) : structuredClone(state.base_schema.definition);
    const next: FutureSchemaLocal = { ...makeFutureSchemaLocal(call, state), fields: schemaFields(definition), proposal: { proposal_call_id: value.authorization.consent.call_id, proposal_digest: value.proposal_digest, definition } };
    retain(next); setOpened(true); setError("");
    if (!edit) await save(next);
  }
  const proposed = state && local ? futureSchemaDefinition(state.base_schema, local.fields, local.proposal) : state?.schema?.definition;
  const proposalBlocked = Boolean(local?.proposal && (!modelProposal || modelProposal.cancelled || modelProposal.authorization.consent.call_id !== local.proposal.proposal_call_id || modelProposal.proposal_digest !== local.proposal.proposal_digest));
  const diff = state && proposed ? futureSchemaDiff(state.base_schema.definition, proposed) : [];
  return <section className="conversation-future-schema" aria-label="Future rule draft">
    <h4>Rules for future work</h4>
    <p>Review a model suggestion or define the rules yourself. Saving creates a separate label draft for future work, not a rule applied across the project.</p>
    {!ready && <p role="status">Checking saved future rule draft…</p>}
    {!opened && !cancelled && <button disabled={!ready || busy} onClick={() => void reload(true)}>Edit future rule draft</button>}
    {state && <ConversationFutureSchemaProposal key={`${call}:${sourceAnswer}`} project={project} conversation={conversation} task={task} source={{ ...state.source, feedback_call_id: call, base_schema_id: state.base_schema.id, base_schema_revision: state.base_schema.revision }} cancelled={cancelled} adopted={Boolean(state.record)} editing={Boolean(local) || busy} onUse={useProposal} onDirtyChange={modelDirty} onStatus={modelChanged} onSetup={onSetup} onAssistance={onAssistance} />}
    {opened && <><p>The original Schema, tested workflow, sample results, published versions, existing Runs and annotations remain unchanged. A new plan and sample test require fresh authorization before any model call.</p>
      {state && <small>Based on tested Schema {state.base_schema.id} · Revision {state.base_schema.revision}. Task identity is preserved. Label and attribute changes are shown in the comparison.</small>}
      {state?.record && state.schema && <><p role="status">Future rule draft saved</p><p>New Schema {state.schema.id} · Revision {state.schema.revision} · {state.record.input.proposal_call_id ? "Model-assisted and human-reviewed. Saving did not call the model again." : "Human-authored, no model call."}</p></>}
      {local && (!state?.record || conflict) && <form onSubmit={event => { event.preventDefault(); void save(); }}>
        <ConversationSchemaFields includeGoalAndKind value={local.fields} disabled={busy || cancelled || stale || proposalBlocked || Boolean(local.frozen) || Boolean(state?.record)} onChange={fields => { retain({ ...local, fields }); setError(""); }} />
        {local.proposal && <p>Model-proposed multiple-label behavior and attributes are preserved while editing these fields. Their full changes are listed below; this form does not edit attribute definitions.</p>}
        {proposalBlocked && <p role="status">Checking the original model proposal, or its cancellation. These fields are retained and cannot be saved until the exact proposal is available.</p>}
        {state && proposed && <FutureSchemaDiff before={state.base_schema.definition} after={proposed} />}
        {stale && <p role="alert">The saved source differs from this browser proposal. Your fields are retained, but cannot be saved against a different Schema or scope answer.</p>}
        {conflict && <><p role="alert">A different future rule draft is already saved. Your original browser proposal is retained read-only and has not overwritten it.</p><button type="button" disabled={busy} onClick={() => { clearLocal(); setError(""); }}>Discard browser proposal; open saved draft</button></>}
        {!state?.record && !cancelled && !stale && <div className="conversation-feedback-actions"><button type="button" disabled={busy || Boolean(local.frozen)} onClick={() => { clearLocal(); setOpened(false); setError(""); }}>Discard unsaved future rule edits</button><button className="primary" type="submit" disabled={!ready || busy || proposalBlocked || !local.fields.goal.trim() || !local.fields.labels.trim() || !diff.length}>{local.frozen ? "Retry same future rule save" : "Save future rule draft"}</button></div>}
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
