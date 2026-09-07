import { useEffect, useRef, useState } from "react";
import { api } from "../api";
import { t } from "../i18n";
import { projectJourneyPath } from "../navigation";
import type { ProjectSummary, ProviderProfile, RegistryModelProfile } from "../types";

// Presentation over the existing Registry. Credentials never enter URL/storage here.
export function JourneyModel({ project, onNavigate }: {
  project: ProjectSummary; onNavigate: (path: string) => void;
}) {
  const [models, setModels] = useState<RegistryModelProfile[]>([]);
  const [providers, setProviders] = useState<ProviderProfile[]>([]);
  const [selected, setSelected] = useState("");
  const [connecting, setConnecting] = useState(false);
  const [endpoint, setEndpoint] = useState("");
  const [remoteModel, setRemoteModel] = useState("");
  const [secret, setSecret] = useState("");
  const [declared, setDeclared] = useState(false);
  const [probeConsent, setProbeConsent] = useState(false);
  const [providerId, setProviderId] = useState("");
  const [createdModel, setCreatedModel] = useState("");
  const [busy, setBusy] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState("");
  const pending = useRef(false);
  const active = useRef(true);
  async function reload() {
    const [registry, services] = await Promise.all([api.modelProfiles(), api.providers()]);
    if (!active.current) return;
    setModels(registry.models.filter((model) => model.enabled && model.input_modalities.includes("text") && model.task_capabilities.includes("text_generation") && model.protocol_features.tool_calls && model.protocol_features.structured_output));
    setProviders(services.providers); setLoaded(true);
  }
  useEffect(() => {
    active.current = true;
    void reload().catch((error: Error) => { if (active.current) setError(error.message); });
    return () => { active.current = false; };
  }, [project.id]);
  const providerFor = (model: RegistryModelProfile) => providers.find((provider) => provider.id === model.provider_id);
  const options = models.filter((model) => {
    const provider = providerFor(model);
    return provider?.enabled && provider.credential_configured;
  });
  const chosen = options.find((model) => model.id === selected);
  const ready = chosen?.status === "available" && ["available", "configured"].includes(providerFor(chosen)?.health.status ?? "");
  const leave = () => onNavigate(projectJourneyPath(project.id, "goal"));
  async function perform(action: () => Promise<void>) {
    if (pending.current) return;
    pending.current = true; setBusy(true); setError("");
    try { await action(); } catch (error) { if (active.current) setError((error as Error).message); }
    finally { pending.current = false; if (active.current) setBusy(false); }
  }
  async function connect() {
    if (!declared || !endpoint.trim() || !remoteModel.trim() || (!providerId && !secret)) return;
    await perform(async () => {
      // Retain successful substeps on error. Existing Registry entries are also recoverable
      // from the configured-model list after refresh; never silently remove partial setup.
      let id = providerId;
      if (!id) {
        const provider = await api.createProvider({ display_name: remoteModel.trim(), adapter: "open_ai_compatible", base_url: endpoint.trim() });
        id = provider.id; setProviderId(id);
      }
      if (secret) {
        await api.saveProviderCredential(id, { source: "workspace_file", secret });
        setSecret("");
      }
      let modelId = createdModel;
      if (!modelId) {
        const model = await api.createModelProfile({
          provider_id: id, display_name: remoteModel.trim(), remote_model_id: remoteModel.trim(),
          input_modalities: ["text"], task_capabilities: ["text_generation"],
          protocol_features: { tool_calls: true, structured_output: true, parallel_tool_calls: false, json_schema: false, usage_reporting: false, streaming: false, reasoning_controls: false },
        });
        modelId = model.id; setCreatedModel(modelId);
      }
      await reload();
      if (active.current) { setSelected(modelId); setConnecting(false); }
    });
  }
  async function useModel() {
    if (!chosen || !ready) return;
    await perform(async () => {
      // Revalidate immediately before saving and preserve unrelated/locked bindings.
      const [registry, existing] = await Promise.all([api.compatibleModelProfiles({ input_modalities: ["text"], capabilities: ["text_generation"], tool_calls: true, structured_output: true }), api.projectModelBindings(project.id)]);
      if (!registry.models.some((model) => model.id === chosen.id && model.enabled && model.status === "available")) throw new Error(t("This model is no longer available. Choose another connection."));
      const locked = existing.bindings.find((binding) => binding.role === "pipeline_builder" && binding.locked && binding.model_profile_id !== chosen.id);
      if (locked) throw new Error(t("This Project has a locked planning connection. It was not changed."));
      await api.saveProjectModelBindings(project.id, [...existing.bindings.filter((binding) => binding.role !== "pipeline_builder"), { capability: "text_generation", role: "pipeline_builder", match_kind: "role", model_profile_id: chosen.id, locked: existing.bindings.some((binding) => binding.role === "pipeline_builder" && binding.locked) }]);
      if (active.current) leave();
    });
  }
  return <section className="journey-scene journey-model" aria-label={t("Connect a planning model")}>
    <div className="journey-intro"><h2>{t("One connection before we prepare your samples.")}</h2><p>{t("Your images and goal are saved. This connection prepares a plan; image processing is authorized separately.")}</p></div>
    {!loaded && !error && <p role="status">{t("Checking configured connections…")}</p>}
    {!connecting ? <>
      {options.length > 0 ? <fieldset className="journey-output-types" disabled={busy}><legend>{t("Choose a configured planning model")}</legend>{options.map((model) => <label key={model.id}><input type="radio" name="planning-connection" checked={selected === model.id} onChange={() => { setSelected(model.id); setProbeConsent(false); }} /><span><strong>{model.display_name}</strong><small>{providerFor(model)?.base_url} · {t(model.status === "available" ? "Available" : "Connection needs verification")}</small></span></label>)}</fieldset> : loaded && <p>{t("No configured planning connection is available yet.")}</p>}
      <button disabled={busy} onClick={() => setConnecting(true)}>{t("Connect a model service")}</button>
      <p className="journey-notice">{t("Local vision models do not replace a planning language model. This step does not install a model or download weights.")}</p>
      {chosen && !ready && <section className="journey-consent" aria-label={t("Connection verification")}><h3>{t("Verify this connection")}</h3><p>{chosen.remote_model_id} · {providerFor(chosen)?.base_url}</p><p>{t("Sends a short text probe, not your images. The service may charge; cost is unknown. Saving the connection alone does not verify it.")}</p><label><input type="checkbox" checked={probeConsent} onChange={(event) => setProbeConsent(event.target.checked)} disabled={busy} />{t("Allow the text verification request")}</label><button disabled={busy || !probeConsent} onClick={() => void perform(async () => { await api.activeProbe(chosen.provider_id, chosen.id); await reload(); setProbeConsent(false); })}>{t("Verify connection")}</button></section>}
      <footer className="journey-actions"><button onClick={leave}>{t("Back to saved goal")}</button><button className="primary" disabled={busy || !ready} onClick={() => void useModel()}>{t("Use connection and return")}</button></footer>
    </> : <>
      <p>{t("Connect an OpenAI-compatible text service that supports tool calls and structured responses. Capabilities remain user-declared until checked.")}</p>
      <p>{t("The text probe checks connectivity, not tool accuracy. Connection details are not saved until Save connection; reloading clears an unsaved key.")}</p>
      <label>{t("Service URL")}<input aria-label={t("Service URL")} type="url" value={endpoint} disabled={busy || Boolean(providerId)} onChange={(event) => setEndpoint(event.target.value)} placeholder="https://your-service.example/v1" /></label>
      <label>{t("Model name from your service")}<input aria-label={t("Model name from your service")} value={remoteModel} disabled={busy || Boolean(createdModel)} onChange={(event) => setRemoteModel(event.target.value)} /></label>
      <label>{t("API key")}<input aria-label={t("API key")} type="password" autoComplete="off" value={secret} disabled={busy} onChange={(event) => setSecret(event.target.value)} /><small>{t("Saved in the private workspace file, not the system keychain. The secret is never placed in a URL.")}</small></label>
      <label><input type="checkbox" checked={declared} disabled={busy} onChange={(event) => setDeclared(event.target.checked)} />{t("This text model supports tool calls and structured responses")}</label>
      <p>{t("No model request is made by Save connection. A separate confirmation is required to verify it.")}</p>
      <footer className="journey-actions"><button disabled={busy} onClick={() => { setSecret(""); setConnecting(false); void reload(); }}>{t("Cancel connection")}</button><button className="primary" disabled={busy || !declared || !endpoint.trim() || !remoteModel.trim() || (!providerId && !secret)} onClick={() => void connect()}>{t("Save connection")}</button></footer>
    </>}
    {busy && <p role="status">{t("Saving or checking the selected connection…")}</p>}
    {error && <p role="alert">{error} {t("Saved images, goal and completed connection steps remain. No sample inference has started.")}</p>}
  </section>;
}
