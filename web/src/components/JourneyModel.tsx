import { useEffect, useRef, useState } from "react";
import { api } from "../api";
import { t } from "../i18n";
import { projectJourneyPath } from "../navigation";
import type { ModelBindingRole, ModelCapability, ProjectSummary, ProviderProfile, RegistryModelProfile } from "../types";
import { journeyConnectionReady, journeyLocalModelMatches, journeyModelMatches, journeyReadyLocalModels, journeyVisionCapabilities, type JourneyConnectionPurpose } from "../journeyConnections";
import { JourneyLocalModel } from "./JourneyLocalModel";

// Presentation over the existing Registry. Credentials never enter URL/storage here.
export function JourneyModel({ project, purpose = "planning", revisionReturn, onNavigate }: {
  project: ProjectSummary; purpose?: JourneyConnectionPurpose;
  revisionReturn?: { draftId: string; sampleTestId: string; imageId?: string };
  onNavigate: (path: string) => void;
}) {
  const [models, setModels] = useState<RegistryModelProfile[]>([]);
  const [providers, setProviders] = useState<ProviderProfile[]>([]);
  const [selected, setSelected] = useState("");
  const [connecting, setConnecting] = useState(false);
  const localSetupKey = `annotagent.local-setup-open:${project.id}:${purpose}`;
  const [localSetup, setLocalSetup] = useState(() => { try { return purpose === "vision" && localStorage.getItem(localSetupKey) === "true"; } catch { return false; } });
  const openLocalSetup = (open: boolean) => { setLocalSetup(open); try { if (open) localStorage.setItem(localSetupKey, "true"); else localStorage.removeItem(localSetupKey); } catch { /* Presentation preference only. */ } };
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
  const [kind, setKind] = useState("bounding_box");
  const [returnReady, setReturnReady] = useState(!revisionReturn);
  const pending = useRef(false);
  const active = useRef(true);
  const savedSetupKey = `annotagent.connection-setup:${project.id}:${purpose}`;
  const saveSetup = (providerId: string, modelId = "") => { try { localStorage.setItem(savedSetupKey, JSON.stringify({ providerId, modelId })); } catch { /* Registry records still persist server-side. */ } };
  async function reload() {
    if (revisionReturn) {
      const evidence = await api.samplePlanEvidence(project.id, revisionReturn.draftId);
      if (evidence.project_id !== project.id || evidence.sample_test_id !== revisionReturn.sampleTestId) throw new Error(t("This revision does not belong to the selected sample."));
      if (active.current) setReturnReady(true);
    }
    const [registry, services, goal] = await Promise.all([api.modelProfiles(), api.providers(), api.projectGoal(project.id)]);
    if (!active.current) return;
    setKind(goal.kind ?? "bounding_box");
    setModels(registry.models.filter((model) => journeyModelMatches(model, purpose, goal.kind ?? "bounding_box")));
    setProviders(services.providers); setLoaded(true);
    if (!providerId) {
      try {
        const saved = JSON.parse(localStorage.getItem(savedSetupKey) ?? "null") as { providerId?: string; modelId?: string } | null;
        const provider = services.providers.find((item) => item.id === saved?.providerId);
        const model = registry.models.find((item) => item.id === saved?.modelId && item.provider_id === provider?.id);
        if (provider) {
          setProviderId(provider.id); setEndpoint(provider.base_url);
          setCreatedModel(model?.id ?? ""); setRemoteModel(model?.remote_model_id ?? "");
          if (model) setSelected(model.id); else setConnecting(true);
        }
      } catch { /* Invalid local preference cannot select or modify a Registry record. */ }
    }
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
  const ready = !!chosen && journeyConnectionReady(chosen, providers);
  const leave = () => onNavigate(revisionReturn ? projectJourneyPath(project.id, "revise", revisionReturn) : projectJourneyPath(project.id, "goal"));
  async function perform(action: () => Promise<void>) {
    if (pending.current) return;
    pending.current = true; setBusy(true); setError("");
    try { await action(); } catch (error) { if (active.current) setError((error as Error).message); }
    finally { pending.current = false; if (active.current) setBusy(false); }
  }
  async function connect() {
    if (!returnReady || !declared || !endpoint.trim() || !remoteModel.trim() || (!providerId && !secret)) return;
    await perform(async () => {
      if (revisionReturn) {
        const evidence = await api.samplePlanEvidence(project.id, revisionReturn.draftId);
        if (evidence.project_id !== project.id || evidence.sample_test_id !== revisionReturn.sampleTestId) throw new Error(t("This revision does not belong to the selected sample."));
      }
      // Retain successful substeps on error. Existing Registry entries are also recoverable
      // from the configured-model list after refresh; never silently remove partial setup.
      let id = providerId;
      if (!id) {
        const provider = await api.createProvider({ display_name: remoteModel.trim(), adapter: "open_ai_compatible", base_url: endpoint.trim() });
        id = provider.id; setProviderId(id); saveSetup(id);
      }
      if (!active.current) return;
      if (secret) {
        await api.saveProviderCredential(id, { source: "workspace_file", secret });
        setSecret("");
      }
      if (!active.current) return;
      let modelId = createdModel;
      if (!modelId) {
        const model = await api.createModelProfile({
          provider_id: id, display_name: remoteModel.trim(), remote_model_id: remoteModel.trim(),
          input_modalities: purpose === "planning" ? ["text"] : ["text", "image"], task_capabilities: purpose === "planning" ? ["text_generation"] : [journeyVisionCapabilities(kind)[0]],
          protocol_features: { tool_calls: purpose === "planning", structured_output: true, parallel_tool_calls: false, json_schema: false, usage_reporting: false, streaming: false, reasoning_controls: false },
        });
        modelId = model.id; setCreatedModel(modelId); saveSetup(id, modelId);
      }
      if (!active.current) return;
      await reload();
      if (active.current) { setSelected(modelId); setConnecting(false); }
    });
  }
  async function useModel() {
    if (!returnReady || !chosen || !ready) return;
    await perform(async () => {
      if (revisionReturn) {
        const evidence = await api.samplePlanEvidence(project.id, revisionReturn.draftId);
        if (evidence.project_id !== project.id || evidence.sample_test_id !== revisionReturn.sampleTestId) throw new Error(t("This revision does not belong to the selected sample."));
      }
      // Revalidate immediately before saving and preserve unrelated/locked bindings.
      const [registry, services, existing, currentGoal] = await Promise.all([api.modelProfiles(), api.providers(), api.projectModelBindings(project.id), api.projectGoal(project.id)]);
      if ((currentGoal.kind ?? "bounding_box") !== kind) throw new Error(t("The annotation goal changed during setup. Return to the saved task and check its required connection again."));
      const current = registry.models.find((model) => model.id === chosen.id && journeyModelMatches(model, purpose, kind) && journeyConnectionReady(model, services.providers));
      if (!current) throw new Error(t("This model is no longer available. Choose another connection."));
      const role: ModelBindingRole = purpose === "planning" ? "pipeline_builder" : kind === "classification" ? "classification" : kind === "semantic_mask" ? "segmentation" : "detection";
      const capability: ModelCapability = purpose === "planning" ? "text_generation" : journeyVisionCapabilities(kind).find((capability) => current.task_capabilities.includes(capability))!;
      const previous = existing.bindings.find((binding) => binding.match_kind === "role" && binding.role === role);
      const locked = previous?.locked && previous.model_profile_id !== chosen.id;
      if (locked) throw new Error(t("This task has a locked model connection. It was not changed."));
      if (!previous?.locked) await api.selectProjectModelBinding(project.id, { capability, role, match_kind: "role", model_profile_id: chosen.id, locked: false }, previous?.id);
      try { localStorage.removeItem(savedSetupKey); } catch { /* Preference cleanup only. */ }
      if (active.current) leave();
    });
  }
  if (localSetup && loaded && returnReady && purpose === "vision") return <JourneyLocalModel projectId={project.id} kind={kind} onBack={() => openLocalSetup(false)} onCancel={() => { openLocalSetup(false); leave(); }} onConnectService={() => { openLocalSetup(false); setConnecting(true); }} onReady={async () => {
    const [goal, native, plugins, bundles] = await Promise.all([api.projectGoal(project.id), api.modelInstances(), api.expertPlugins(), api.modelBundles()]);
    if ((goal.kind ?? "bounding_box") !== kind) throw new Error(t("The annotation goal changed during setup. Return to the saved task and check its required connection again."));
    if (!journeyReadyLocalModels(native.model_profiles, native.instances, plugins.installations, bundles.bundles).some((model) => journeyLocalModelMatches(model, kind))) throw new Error(t("This model is no longer available. Choose another connection."));
    if (revisionReturn) {
      const evidence = await api.samplePlanEvidence(project.id, revisionReturn.draftId);
      if (evidence.project_id !== project.id || evidence.sample_test_id !== revisionReturn.sampleTestId) throw new Error(t("This revision does not belong to the selected sample."));
    }
    openLocalSetup(false); leave();
  }} />;
  return <section className="journey-scene journey-model" aria-label={t(purpose === "planning" ? "Connect a planning model" : "Connect an image model")}>
    <div className="journey-intro"><h2>{t("One connection before we prepare your samples.")}</h2><p>{t(purpose === "planning" ? "Your images and goal are saved. This connection prepares a plan; image processing is authorized separately." : "Your goal is saved. Choose a model that can process images for this task; connecting it does not run your images.")}</p></div>
    {!loaded && !error && <p role="status">{t("Checking configured connections…")}</p>}
    {!connecting ? <>
      {options.length > 0 ? <fieldset className="journey-output-types" disabled={busy}><legend>{t(purpose === "planning" ? "Choose a configured planning model" : "Choose a compatible image model")}</legend>{options.map((model) => <label key={model.id}><input type="radio" name="planning-connection" checked={selected === model.id} onChange={() => { setSelected(model.id); setProbeConsent(false); }} /><span><strong>{model.display_name}</strong><small>{providerFor(model)?.base_url} · {t(model.status === "available" ? "Available" : "Connection needs verification")}</small></span></label>)}</fieldset> : loaded && <p>{t(purpose === "planning" ? "No configured planning connection is available yet." : "No compatible image connection is available yet.")}</p>}
      <button disabled={busy || !returnReady} onClick={() => setConnecting(true)}>{t("Connect a model service")}</button>
      {purpose === "vision" && <button disabled={busy || !returnReady || !loaded} onClick={() => openLocalSetup(true)}>{t("Prepare a local image model")}</button>}
      {providerId && <button disabled={busy} onClick={() => {
        setProviderId(""); setCreatedModel(""); setSelected(""); setEndpoint(""); setRemoteModel(""); setSecret(""); setDeclared(false); setProbeConsent(false); setConnecting(true);
        try { localStorage.removeItem(savedSetupKey); } catch { /* Existing Registry records are deliberately retained. */ }
      }}>{t("Connect another service without deleting the saved connection")}</button>}
      <p className="journey-notice">{t(purpose === "planning" ? "Local vision models do not replace a planning language model. This step does not install a model or download weights." : "A prompted boundary refiner alone cannot find objects. This connection step does not install weights; local plugin provisioning remains in Project management.")}</p>
      {chosen && !ready && <section className="journey-consent" aria-label={t("Connection verification")}><h3>{t("Verify this connection")}</h3><p>{chosen.remote_model_id} · {providerFor(chosen)?.base_url}</p><p>{t("Sends a short text probe, not your images. The service may charge; cost is unknown. Saving the connection alone does not verify it.")}</p><label><input type="checkbox" checked={probeConsent} onChange={(event) => setProbeConsent(event.target.checked)} disabled={busy} />{t("Allow the text verification request")}</label><button disabled={busy || !probeConsent} onClick={() => void perform(async () => { await api.activeProbe(chosen.provider_id, chosen.id); await reload(); setProbeConsent(false); })}>{t("Verify connection")}</button></section>}
      <footer className="journey-actions"><button onClick={leave}>{t(revisionReturn ? "Back to sample adjustment" : "Back to saved goal")}</button><button className="primary" disabled={busy || !ready || !returnReady} onClick={() => void useModel()}>{t("Use connection and return")}</button></footer>
    </> : <>
      <p>{t(purpose === "planning" ? "Connect an OpenAI-compatible text service that supports tool calls and structured responses. Capabilities remain user-declared until checked." : "Connect an OpenAI-compatible image service for the selected output type. A text-only model cannot process your images. Capabilities remain user-declared.")}</p>
      <p>{t("The text probe checks connectivity, not tool accuracy. Connection details are not saved until Save connection; reloading clears an unsaved key.")}</p>
      <label>{t("Service URL")}<input aria-label={t("Service URL")} type="url" value={endpoint} disabled={busy || Boolean(providerId)} onChange={(event) => setEndpoint(event.target.value)} placeholder="https://your-service.example/v1" /></label>
      <label>{t("Model name from your service")}<input aria-label={t("Model name from your service")} value={remoteModel} disabled={busy || Boolean(createdModel)} onChange={(event) => setRemoteModel(event.target.value)} /></label>
      <label>{t("API key")}<input aria-label={t("API key")} type="password" autoComplete="off" value={secret} disabled={busy} onChange={(event) => setSecret(event.target.value)} /><small>{t("Saved in the private workspace file, not the system keychain. The secret is never placed in a URL.")}</small></label>
      <label><input type="checkbox" checked={declared} disabled={busy} onChange={(event) => setDeclared(event.target.checked)} />{t(purpose === "planning" ? "This text model supports tool calls and structured responses" : "This model accepts images and supports my selected output type with structured responses")}</label>
      <p>{t("No model request is made by Save connection. A separate confirmation is required to verify it.")}</p>
      <footer className="journey-actions"><button disabled={busy} onClick={() => { setSecret(""); setConnecting(false); void reload(); }}>{t("Cancel connection")}</button><button className="primary" disabled={busy || !declared || !endpoint.trim() || !remoteModel.trim() || (!providerId && !secret)} onClick={() => void connect()}>{t("Save connection")}</button></footer>
    </>}
    {busy && <p role="status">{t("Saving or checking the selected connection…")}</p>}
    {error && <p role="alert">{error} {t("Saved images, goal and completed connection steps remain. No sample inference has started.")}</p>}
  </section>;
}
