import { useEffect, useRef, useState } from "react";
import { api } from "../api";
import type { ProviderProfile, RegistryModelProfile } from "../types";
import "./agent-model-picker.css";

type Preference = { revision: number; model_profile_id: string | null };
type Command = { conversation: string; input: { request_id: string; expected_revision: number; model_profile_id: string | null } };

/** Registry-backed next-authorization choice. No probes, credentials or model calls. */
export function AgentModelPicker({ project, conversation, onConversation, onSettings }: {
  project: string; conversation?: string; onConversation: (id: string) => void; onSettings: () => void;
}) {
  const [models, setModels] = useState<RegistryModelProfile[]>([]);
  const [providers, setProviders] = useState<ProviderProfile[]>([]);
  const [preference, setPreference] = useState<Preference>();
  const [search, setSearch] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [pending, setPending] = useState(false);
  const command = useRef<Command | undefined>(undefined);
  const alive = useRef(true);
  const details = useRef<HTMLDetailsElement>(null);
  const summary = useRef<HTMLElement>(null);
  useEffect(() => {
    alive.current = true;
    const controller = new AbortController();
    void Promise.all([api.modelProfiles(), api.providers(), conversation ? api.conversationAgentModel(project, conversation, controller.signal) : Promise.resolve({revision:0,model_profile_id:null})]).then(([registry, accounts, choice]) => {
      if (controller.signal.aborted) return;
      setModels(registry.models); setProviders(accounts.providers); setPreference(choice);
    }).catch(reason => { if (!controller.signal.aborted) setError((reason as Error).message); });
    return () => { alive.current = false; controller.abort(); };
  }, [project, conversation]);
  const compatible = (model: RegistryModelProfile) => model.input_modalities.includes("text") && model.task_capabilities.includes("text_generation") && model.protocol_features.tool_calls && model.protocol_features.structured_output;
  const selected = models.find(model => model.id === preference?.model_profile_id);
  const close = () => { if (details.current) details.current.open = false; summary.current?.focus(); };
  async function choose(id: string | null) {
    if (busy || !preference) return;
    setBusy(true); setError("");
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
      setPreference(result); command.current = undefined; setPending(false);
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
      setPreference(current); command.current = undefined; setPending(false); setError(""); onConversation(owner);
    } catch (reason) { if (alive.current) setError((reason as Error).message); }
    finally { if (alive.current) setBusy(false); }
  }
  const query = search.trim().toLocaleLowerCase();
  return <details ref={details} className="agent-model-picker" onKeyDown={event => {
    if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); close(); }
    if (event.key === "Enter" && event.target instanceof HTMLInputElement && !event.nativeEvent.isComposing) { event.preventDefault(); event.stopPropagation(); }
  }}>
    <summary ref={summary} aria-label="Choose Agent model">{selected ? `${selected.display_name} · ${providers.find(provider => provider.id === selected.provider_id)?.display_name ?? "Missing Provider"}` : (preference?.model_profile_id ? "Unavailable model reference" : "Project Agent default")}</summary>
    <section aria-label="Agent model selection" className="agent-model-panel">
      <strong>Agent model · next authorization</strong>
      <p>Existing requests and image-model bindings stay unchanged. A new request still requires its own data and cost authorization.</p>
      <label>Search models<input type="search" value={search} onChange={event => setSearch(event.target.value)} /></label>
      <label>Provider / account and model<select aria-label="Agent model" disabled={!preference || busy || pending} value={preference?.model_profile_id ?? ""} onChange={event => void choose(event.target.value || null)}>
        <option value="">Use Project Agent default</option>
        {preference?.model_profile_id && !selected && <option value={preference.model_profile_id} disabled>Saved model is no longer in Registry</option>}
        {providers.map(provider => <optgroup key={provider.id} label={`${provider.display_name} · ${provider.id.slice(0,8)}`}>
          {models.filter(model => model.provider_id === provider.id && (model.id === preference?.model_profile_id || `${provider.display_name} ${model.display_name} ${model.remote_model_id}`.toLocaleLowerCase().includes(query))).map(model => <option key={model.id} value={model.id} disabled={!compatible(model) || !model.enabled || !provider.enabled || provider.adapter === "mock"}>{model.display_name} · {model.status}{compatible(model) ? " · text/tools" : " · incompatible Agent capabilities"}</option>)}
        </optgroup>)}
      </select></label>
      {selected && <small>{providers.find(provider => provider.id === selected.provider_id)?.endpoint_summary} · {selected.remote_model_id} · {selected.status}</small>}
      <span role="status">{busy ? "Saving selection…" : preference ? `Selection revision ${preference.revision}` : "Loading Registry…"}</span>
      {error && <div role="alert"><p>{error}</p><div className="button-row">{pending && <button type="button" disabled={busy} onClick={() => void choose(null)}>Retry same selection</button>}<button type="button" disabled={busy} onClick={() => void reload()}>Reload saved selection</button></div></div>}
      <div className="button-row"><button type="button" onClick={onSettings}>Manage models</button><button type="button" onClick={close}>Close</button></div>
    </section>
  </details>;
}
