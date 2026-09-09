import { useEffect, useRef, useState } from "react";
import { api } from "../api";
import type { ProviderProfile, RegistryModelProfile } from "../types";
import "./agent-model-picker.css";

type Preference = { revision: number; model_profile_id: string | null };
type Command = { conversation: string; input: { request_id: string; expected_revision: number; model_profile_id: string | null } };

/** Registry-backed next-authorization choice. No probes, credentials or model calls. */
export function AgentModelPicker({ project, conversation, onConversation, onSettings, onPreference }: {
  project: string; conversation?: string; onConversation: (id: string) => void; onSettings: () => void;
  onPreference: (value: Preference | undefined) => void;
}) {
  const [models, setModels] = useState<RegistryModelProfile[]>([]);
  const [providers, setProviders] = useState<ProviderProfile[]>([]);
  const [preference, setPreference] = useState<Preference>();
  const [search, setSearch] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [pending, setPending] = useState(false);
  const [open, setOpen] = useState(false);
  const command = useRef<Command | undefined>(undefined);
  const alive = useRef(true);
  const details = useRef<HTMLDetailsElement>(null);
  const summary = useRef<HTMLElement>(null);
  const searchInput = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (!open) return;
    searchInput.current?.focus();
    const outside = (event: PointerEvent) => {
      if (!details.current?.contains(event.target as Node) && details.current) details.current.open = false;
    };
    document.addEventListener("pointerdown", outside);
    return () => document.removeEventListener("pointerdown", outside);
  }, [open]);
  useEffect(() => {
    alive.current = true;
    onPreference(undefined);
    const controller = new AbortController();
    void Promise.all([api.modelProfiles(), api.providers(), conversation ? api.conversationAgentModel(project, conversation, controller.signal) : Promise.resolve({revision:0,model_profile_id:null})]).then(([registry, accounts, choice]) => {
      if (controller.signal.aborted) return;
      setModels(registry.models); setProviders(accounts.providers); setPreference(choice); onPreference(choice);
    }).catch(reason => { if (!controller.signal.aborted) setError((reason as Error).message); });
    return () => { alive.current = false; controller.abort(); };
  }, [project, conversation]);
  const compatible = (model: RegistryModelProfile) => model.input_modalities.includes("text") && model.task_capabilities.includes("text_generation") && model.protocol_features.tool_calls && model.protocol_features.structured_output;
  const selected = models.find(model => model.id === preference?.model_profile_id);
  const close = () => { if (details.current) details.current.open = false; summary.current?.focus(); };
  async function choose(id: string | null) {
    if (busy || !preference) return;
    setBusy(true); setError(""); onPreference(undefined);
    try {
      if (!command.current) {
        const owner = conversation ?? (await api.createConversation(project)).conversation_id;
        if (!alive.current) return;
        command.current = {conversation:owner,input:{request_id:crypto.randomUUID(),expected_revision:preference.revision,model_profile_id:id}};
        setPending(true);
      }
      const current = command.current;
      const result = await api.selectConversationAgentModel(project, current.conversation, current.input);
      if (!alive.current) return;
      setPreference(result); onPreference(result); command.current = undefined; setPending(false);
      onConversation(current.conversation);
    } catch (reason) { if (alive.current) setError((reason as Error).message); }
    finally { if (alive.current) setBusy(false); }
  }
  async function reload() {
    if (busy) return;
    const owner = command.current?.conversation ?? conversation;
    if (!owner) return;
    setBusy(true);
    try {
      const current = await api.conversationAgentModel(project, owner);
      if (!alive.current) return;
      setPreference(current); onPreference(current); command.current = undefined; setPending(false); setError(""); onConversation(owner);
    } catch (reason) { if (alive.current) setError((reason as Error).message); }
    finally { if (alive.current) setBusy(false); }
  }
  const query = search.trim().toLocaleLowerCase();
  return <details ref={details} className="agent-model-picker" onToggle={event=>setOpen(event.currentTarget.open)} onKeyDown={event => {
    if (event.nativeEvent.isComposing || event.keyCode === 229) return;
    if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); close(); }
    if (event.key === "Enter" && event.target instanceof HTMLInputElement) { event.preventDefault(); event.stopPropagation(); }
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      const choices = Array.from(details.current?.querySelectorAll<HTMLButtonElement>("button[data-model-choice]:not(:disabled)") ?? []);
      if (!choices.length) return;
      event.preventDefault();
      const index = choices.indexOf(document.activeElement as HTMLButtonElement);
      choices[(index + (event.key === "ArrowDown" ? 1 : -1) + choices.length) % choices.length]?.focus();
    }
  }}>
    <summary ref={summary} aria-label="Choose Agent model">{selected ? `${selected.display_name} · ${providers.find(provider => provider.id === selected.provider_id)?.display_name ?? "Missing Provider"}` : (preference?.model_profile_id ? "Unavailable model reference" : "Project Agent default")}</summary>
    {open && <section aria-label="Agent model selection" className="agent-model-panel">
      <strong>Agent model · next request</strong>
      <label><span className="sr-only">Search models</span><input ref={searchInput} type="search" placeholder="Search models" value={search} onChange={event => setSearch(event.target.value)} /></label>
      <button type="button" data-model-choice aria-pressed={!preference?.model_profile_id} disabled={!preference || busy || pending} onClick={()=>void choose(null)}>Use Project Agent default{!preference?.model_profile_id && <span aria-hidden="true">✓</span>}</button>
      {providers.map(provider => {
        const visible=models.filter(model=>model.provider_id===provider.id && `${provider.display_name} ${model.display_name} ${model.remote_model_id}`.toLocaleLowerCase().includes(query));
        if (!visible.length) return null;
        return <section className="agent-model-group" key={provider.id} aria-label={provider.display_name}>
          <h3>{provider.display_name}</h3>
          {visible.map(model=>{
            const reason=!compatible(model) ? "Requires text, tools and structured output" : !model.enabled ? "Model disabled" : !provider.enabled ? "Provider disabled" : provider.adapter==="mock" ? "Test-only Provider" : model.status==="unavailable" || model.status==="disabled" ? "Model unavailable" : undefined;
            return <button key={model.id} type="button" data-model-choice aria-label={model.display_name} aria-pressed={preference?.model_profile_id===model.id} disabled={!preference||busy||pending||Boolean(reason)} onClick={()=>void choose(model.id)}>
              <span>{model.display_name}<small>{reason ?? (model.status==="available" ? "Text · tools" : "Not verified · no probe on selection")}</small></span>
              {preference?.model_profile_id===model.id && <span aria-hidden="true">✓</span>}
            </button>;
          })}
        </section>;
      })}
      {query && !models.some(model=>`${providers.find(provider=>provider.id===model.provider_id)?.display_name} ${model.display_name} ${model.remote_model_id}`.toLocaleLowerCase().includes(query)) && <p>No matching models.</p>}
      {preference?.model_profile_id && !selected && <p role="alert">Saved model is no longer in Registry.</p>}
      {(busy || !preference) && <span role="status">{busy ? "Saving selection…" : "Loading Registry…"}</span>}
      {error && <div role="alert"><p>{error}</p><div className="button-row">{pending && <button type="button" disabled={busy} onClick={() => void choose(null)}>Retry same selection</button>}<button type="button" disabled={busy} onClick={() => void reload()}>Reload saved selection</button></div></div>}
      <p className="agent-model-scope">Next Agent request only. Existing requests and Workflow image models stay unchanged; data and cost approval still applies.</p>
      <details className="agent-model-details"><summary>Selection details</summary><small>Selection revision {preference?.revision ?? "unknown"} · {selected?.remote_model_id ?? "Project default"} · {providers.find(provider=>provider.id===selected?.provider_id)?.endpoint_summary}</small></details>
      <div className="button-row"><button type="button" onClick={onSettings}>Manage models</button><button type="button" onClick={close}>Close</button></div>
    </section>}
  </details>;
}
