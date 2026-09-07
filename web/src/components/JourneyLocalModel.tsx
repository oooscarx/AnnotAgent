import { useEffect, useRef, useState } from "react";
import { api } from "../api";
import { t } from "../i18n";
import { journeyLocalModelMatches, journeyReadyLocalModels, journeyVisionCapabilities } from "../journeyConnections";
import type { ModelCatalogEntry, ModelInstallOperation, ModelInstanceProfile } from "../types";

type Choice = { pluginId: string; pluginVersion: string; entry: ModelCatalogEntry };
const identity = (choice: Choice) => `${choice.pluginId}@${choice.pluginVersion}/${choice.entry.catalog_id}/${choice.entry.bundle_id}@${choice.entry.bundle_version}`;
const operationIdentity = (operation: ModelInstallOperation) => `${operation.plugin_id}@${operation.plugin_version}/${operation.catalog_id}/${operation.bundle_id}@${operation.bundle_version}`;

// A task presentation over the existing signed-catalog installation endpoints.
// It never installs plugin code, invents a model binding or executes Project images.
export function JourneyLocalModel({ projectId, kind, onBack, onCancel, onConnectService, onReady }: {
  projectId: string; kind: string; onBack: () => void; onCancel: () => void; onConnectService: () => void; onReady: () => Promise<void>;
}) {
  const [choices, setChoices] = useState<Choice[]>([]);
  const [ready, setReady] = useState<ModelInstanceProfile[]>([]);
  const [selected, setSelected] = useState("");
  const [operation, setOperation] = useState<ModelInstallOperation>();
  const [consent, setConsent] = useState(false);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [pollAttempt, setPollAttempt] = useState(0);
  const mounted = useRef(true);
  const pending = useRef(false);
  const preferenceKey = `annotagent.local-setup:${projectId}:${kind}`;
  const choice = choices.find((item) => identity(item) === selected);
  const running = operation?.status === "running";
  async function refresh() {
    const [plugins, models, bundles, operations, goal] = await Promise.all([api.expertPlugins(), api.modelInstances(), api.modelBundles(), api.modelInstallOperations(), api.projectGoal(projectId)]);
    if ((goal.kind ?? "bounding_box") !== kind) throw new Error(t("The annotation goal changed during setup. Return to the saved task and check its required connection again."));
    const inventories = await Promise.all(plugins.installations.filter((plugin) => plugin.enabled).map(async (plugin) => ({ plugin, inventory: await api.compatibleModelBundles(plugin.manifest.id, plugin.manifest.version) })));
    if (!mounted.current) return;
    const available = inventories.flatMap(({ plugin, inventory }) => inventory.available.filter((entry) => !entry.fixture && entry.publishable && entry.publisher.verified && !!entry.catalog_id && journeyVisionCapabilities(kind).some((capability) => entry.capabilities.includes(capability))).map((entry) => ({ entry, pluginId: plugin.manifest.id, pluginVersion: plugin.manifest.version })));
    setChoices(available);
    setReady(journeyReadyLocalModels(models.model_profiles, models.instances, plugins.installations, bundles.bundles).filter((model) => journeyLocalModelMatches(model, kind)));
    let saved = selected;
    try { saved ||= localStorage.getItem(preferenceKey) ?? ""; } catch { /* Server remains authoritative. */ }
    const current = available.find((item) => identity(item) === saved);
    // A catalog or plugin may change while a previously started job is running.
    // Preserve that job's progress even when it is no longer installable again.
    setOperation(operations.operations.filter((item) => operationIdentity(item) === saved).sort((a, b) => b.created_at.localeCompare(a.created_at))[0]);
    if (current) {
      setSelected(identity(current));
    } else setSelected("");
    setLoading(false);
  }
  useEffect(() => {
    mounted.current = true;
    void refresh().catch((value: Error) => { if (mounted.current) { setError(value.message); setLoading(false); } });
    return () => { mounted.current = false; };
  }, [projectId, kind]);
  useEffect(() => {
    if (!running || !operation) return;
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try {
        const next = await api.modelInstallOperation(operation.id);
        if (!active) return;
        setOperation(next);
        if (next.status === "running") timer = setTimeout(() => void poll(), 1000);
        else await refresh();
      } catch (value) { if (active) setError((value as Error).message); }
    };
    void poll();
    return () => { active = false; clearTimeout(timer); };
  }, [operation?.id, running, pollAttempt]);
  async function install() {
    if (!choice || !consent || pending.current || running) return;
    pending.current = true; setBusy(true); setError("");
    try {
      const goal = await api.projectGoal(projectId);
      if ((goal.kind ?? "bounding_box") !== kind) throw new Error(t("The annotation goal changed during setup. Return to the saved task and check its required connection again."));
      // Recheck the exact licensed entry before any acceptance or download.
      const current = await api.compatibleModelBundles(choice.pluginId, choice.pluginVersion);
      if (!current.available.some((entry) => JSON.stringify(entry) === JSON.stringify(choice.entry))) throw new Error(t("The available model changed. Check availability and review its source again."));
      if (choice.entry.license_summary.requires_acceptance) await api.acceptModelBundleLicense(choice.entry.bundle_id, choice.entry.bundle_version, choice.entry.license_summary.license_digest);
      const started = await api.startModelInstallOperation({ catalog_id: choice.entry.catalog_id!, bundle_id: choice.entry.bundle_id, bundle_version: choice.entry.bundle_version, plugin_id: choice.pluginId, plugin_version: choice.pluginVersion });
      if (mounted.current) { setOperation(started); setConsent(false); }
    } catch (value) {
      if (mounted.current) { setError((value as Error).message); setConsent(false); await refresh().catch(() => undefined); }
    } finally { pending.current = false; if (mounted.current) setBusy(false); }
  }
  return <section className="journey-scene journey-model" aria-label={t("Prepare a local image model")}>
    <div className="journey-intro"><h2>{t("Prepare a model on this computer")}</h2><p>{t("Only compatible verified models are listed. Installation does not run your Project images or replace your planning connection.")}</p></div>
    {loading && <p role="status">{t("Checking local model availability…")}</p>}
    {ready.length > 0 && <p role="status">{t("Available locally")}: {ready.map((model) => model.display_name).join(", ")}</p>}
    {!loading && !choices.length && !ready.length && <p>{t("No verified local model for this output is installable with the current plugins on this platform. Connect an image service, or return to Project management to prepare a compatible plugin. No ONNX files are required by this screen.")}</p>}
    {!running && choices.length > 0 && <fieldset className="journey-output-types" disabled={busy}><legend>{t("Compatible local models")}</legend>{choices.map((item) => <label key={identity(item)}><input type="radio" name="local-model" checked={selected === identity(item)} onChange={() => { setSelected(identity(item)); setOperation(undefined); setConsent(false); try { localStorage.setItem(preferenceKey, identity(item)); } catch { /* Optional preference only. */ } }} /><span><strong>{item.entry.display_name}</strong><small>{item.entry.description}</small></span></label>)}</fieldset>}
    {choice && !running && !ready.length && <section className="journey-consent" aria-label={t("Local model installation scope")}>
      <p>{choice.entry.publisher.display_name} · {choice.entry.license_summary.name}</p>
      <p>{t("Download source")}: {choice.entry.bundle_url}</p>
      <p>{t("Download size")}: {choice.entry.bundle_size_bytes.toLocaleString()} bytes</p>
      {choice.entry.license_summary.license_url?.startsWith("https://") && <a href={choice.entry.license_summary.license_url} target="_blank" rel="noreferrer">{t("Read model license")}</a>}
      <p>{t("Downloads model files, verifies them and runs the existing local smoke test. No Project images or credentials are sent. Leaving does not cancel installation; restarting this server may interrupt it.")}</p>
      <label><input type="checkbox" checked={consent} disabled={busy} onChange={(event) => setConsent(event.target.checked)} />{t("I accept this model license and authorize this download and local test")}</label>
    </section>}
    {operation && <div role="status" aria-live="polite"><p>{operation.detail}</p>{operation.bytes_total != null && <p>{operation.bytes_completed.toLocaleString()} / {operation.bytes_total.toLocaleString()} bytes</p>}{operation.error && <p>{operation.error}</p>}</div>}
    {error && <p role="alert">{error}</p>}
    <footer className="journey-actions">{!loading && !running && !choices.length && !ready.length ? <button className="primary" onClick={onConnectService}>{t("Connect a model service")}</button> : <button onClick={onBack}>{t(running ? "Back to connections; installation continues" : "Back to connections")}</button>}
      {!ready.length && <button onClick={onCancel}>{t("Back to saved task")}</button>}
      {ready.length ? <button className="primary" disabled={busy} onClick={() => { if (pending.current) return; pending.current = true; setBusy(true); void onReady().catch((value: Error) => { if (mounted.current) setError(value.message); }).finally(() => { pending.current = false; if (mounted.current) setBusy(false); }); }}>{t("Return to saved task")}</button> : choice && !running ? <button className="primary" disabled={busy || !consent} onClick={() => void install()}>{t("Install this local model")}</button> : null}
      {!busy && (!running || error) && <button onClick={() => { setConsent(false); setError(""); setPollAttempt((value) => value + 1); void refresh().catch((value: Error) => { if (mounted.current) setError(value.message); }); }}>{t("Check availability again")}</button>}
    </footer>
  </section>;
}
