import { useEffect, useRef, useState } from "react";
import { api } from "../api";
import { t } from "../i18n";
import { projectJourneyPath } from "../navigation";
import { journeyConnectionReady, journeyModelMatches } from "../journeyConnections";
import type { AgentSession, ImageItem, ProjectSummary, RegistryModelProfile } from "../types";

export function splitGoalLabels(value: string): string[] {
  return [...new Set(value.split(/[,，\n]/).map((label) => label.trim()).filter(Boolean))];
}

export function journeyPlanningPhase(phase: AgentSession["phase"]): string {
  switch (phase) {
    case "context_loading": return "Checking your project";
    case "feasibility_analysis": case "candidate_selection": return "Choosing a compatible method";
    case "drafting": case "draft_salvage": case "revising": return "Preparing the annotation plan";
    case "validating": return "Checking the plan";
    case "dry_running": return "Testing the plan";
    case "finalizing": return "Saving the prepared plan";
    case "waiting_for_human": case "completed": return "The plan is ready for your review";
    case "cancelled": return "Planning cancelled";
    case "failed": return "Planning needs attention";
    default: return "Waiting for the planning service…";
  }
}

export function JourneyGoal({ project, sessionId, onNavigate, onRefresh, onNavigationGuardChange }: {
  project: ProjectSummary; sessionId?: string; onNavigate: (path: string, replace?: boolean) => void; onRefresh: () => Promise<void>;
  onNavigationGuardChange: (guard?: () => boolean) => void;
}) {
  const [saved, setSaved] = useState<Awaited<ReturnType<typeof api.projectGoal>>>();
  const [goal, setGoal] = useState("");
  const [kind, setKind] = useState("bounding_box");
  const [labels, setLabels] = useState("");
  const [images, setImages] = useState<ImageItem[]>([]);
  const [model, setModel] = useState<RegistryModelProfile>();
  const [destination, setDestination] = useState("");
  const [visionModels, setVisionModels] = useState<RegistryModelProfile[]>([]);
  const [busy, setBusy] = useState(false);
  const [planning, setPlanning] = useState(false);
  const [session, setSession] = useState<AgentSession>();
  const [consent, setConsent] = useState(false);
  const [error, setError] = useState("");
  const pending = useRef(false);
  const mounted = useRef(true);
  const leaving = useRef(false);
  const started = useRef("");
  const visionReady = visionModels.some((profile) => journeyModelMatches(profile, "vision", kind));
  const dirty = Boolean(saved && (goal !== saved.goal || kind !== (saved.kind ?? "bounding_box") || JSON.stringify(splitGoalLabels(labels)) !== JSON.stringify(saved.labels ?? [])));
  useEffect(() => {
    mounted.current = true;
    let current = true;
    const controller = new AbortController();
    void Promise.all([api.projectGoal(project.id, controller.signal), api.images(project.id, controller.signal), api.agentModelBindings(), api.modelProfiles(), api.providers(), api.projectModelBindings(project.id)]).then(([value, images, defaults, profiles, providers, bindings]) => {
      if (!current) return;
      setSaved(value); setGoal(value.goal); setKind(value.kind ?? "bounding_box"); setLabels((value.labels ?? []).join(", ")); setImages(images.images);
      const plannerId = bindings.bindings.find((item) => item.role === "pipeline_builder")?.model_profile_id ?? defaults.pipeline_builder;
      const planner = profiles.models.find((item) => item.id === plannerId && item.enabled && item.status === "available" && item.input_modalities.includes("text") && item.task_capabilities.includes("text_generation") && item.protocol_features.tool_calls && item.protocol_features.structured_output && providers.providers.some((provider) => provider.id === item.provider_id && provider.enabled && provider.credential_configured && ["available", "configured"].includes(provider.health.status)));
      setModel(planner);
      setVisionModels(profiles.models.filter((profile) => journeyConnectionReady(profile, providers.providers)));
      setDestination(providers.providers.find((provider) => provider.id === planner?.provider_id)?.base_url ?? "");
    }).catch((error: Error) => { if (current) setError(error.message); });
    return () => { mounted.current = false; current = false; controller.abort(); };
  }, [project.id]);
  useEffect(() => {
    onNavigationGuardChange(() => leaving.current || (!dirty || window.confirm(t("Discard unsaved goal changes? Saved images will remain."))));
    const unload = (event: BeforeUnloadEvent) => { if (dirty && !leaving.current) event.preventDefault(); };
    window.addEventListener("beforeunload", unload);
    return () => { onNavigationGuardChange(undefined); window.removeEventListener("beforeunload", unload); };
  }, [dirty, onNavigationGuardChange]);
  useEffect(() => {
    if (!planning && !sessionId) return;
    let active = true;
    const poll = () => void api.agentSessions(project.id).then(({ sessions }) => {
      if (!active) return;
      const current = sessions.find((item) => item.kind === "pipeline_builder" && (sessionId ? item.id === sessionId : item.created_at >= started.current));
      setSession(current);
      if (!current) { if (sessionId) setError(t("This planning task does not exist in this Project. Your saved goal is unchanged.")); return; }
      if (!sessionId) onNavigate(projectJourneyPath(project.id, "goal", { agentSessionId: current.id }), true);
      if (!pending.current && current.status !== "running") {
        const draftId = current.builder_proposal?.draft.id ?? current.draft_id;
        if (draftId && current.outcome === "draft_ready_for_human_review") {
          onNavigate(projectJourneyPath(project.id, "samples", { draftId }), true);
        }
      }
    }).catch((error: Error) => { if (active) setError(error.message); });
    poll(); const timer = window.setInterval(poll, 1200);
    return () => { active = false; window.clearInterval(timer); };
  }, [planning, project.id, sessionId, onNavigate]);
  async function save() {
    if (!saved || !saved.editable || pending.current || !splitGoalLabels(labels).length) return undefined;
    pending.current = true; setBusy(true); setError("");
    try {
      const next = dirty ? await api.saveProjectGoal(project.id, { expected_revision: saved.revision, goal, kind, labels: splitGoalLabels(labels) }) : saved;
      setSaved(next); setGoal(next.goal); setLabels((next.labels ?? []).join(", "));
      await onRefresh();
      return next;
    } catch (error) { setError((error as Error).message); return undefined; }
    finally { pending.current = false; setBusy(false); }
  }
  async function prepare() {
    const next = await save();
    if (next && model && visionReady) setConsent(true);
    else if (next && model) {
      leaving.current = true; onNavigationGuardChange(undefined);
      onNavigate(projectJourneyPath(project.id, "model", { modelPurpose: "vision" }));
    }
    else if (next) {
      leaving.current = true; onNavigationGuardChange(undefined);
      onNavigate(projectJourneyPath(project.id, "model"));
    }
  }
  async function build() {
    if (pending.current || !model || !consent) return;
    pending.current = true; setPlanning(true); setConsent(false); setError(""); started.current = new Date().toISOString();
    try {
      const targetLabels = splitGoalLabels(labels);
      const target = targetLabels.length === 1 ? { task_id: "journey_labels", label: targetLabels[0] } : undefined;
      const proposal = await api.suggestWorkflow(project.id, "llm", target, { require_review_gate: true }, {
        priority: "balanced", max_model_calls_per_image: 4, allow_external_models: true,
        allow_human_review: true, maximum_agent_turns: 16, maximum_tool_calls: 48,
        maximum_dry_runs: 0, maximum_agent_cost: "1",
      }, model.id);
      if (!mounted.current) return;
      leaving.current = true; onNavigationGuardChange(undefined);
      if (proposal.agent_session && proposal.agent_session.outcome !== "draft_ready_for_human_review") {
        setSession(proposal.agent_session);
        setError(proposal.agent_session.next_action ?? t("Planning needs attention"));
        onNavigate(projectJourneyPath(project.id, "goal", { agentSessionId: proposal.agent_session.id }), true);
        return;
      }
      onNavigate(projectJourneyPath(project.id, "samples", { draftId: proposal.draft.id }));
    } catch (error) { setError((error as Error).message); }
    finally { pending.current = false; setPlanning(false); }
  }
  if (planning || sessionId) return <section className="journey-scene" aria-label={t("Preparing samples")}>
    <div className="journey-intro"><h2>{t("Preparing your annotation plan.")}</h2><p role="status">{t(journeyPlanningPhase(session?.phase))}</p></div>
    <p>{t(session && session.builder_constraints?.maximum_dry_runs !== 0 ? "Your saved planning task is restored. No additional requests are started by opening this page." : "Your images and goal are saved. Image inference has not been authorized yet.")}</p>
    <footer className="journey-actions"><button onClick={() => onNavigate(`/projects/${encodeURIComponent(project.id)}`)}>{t(session?.status === "running" ? "Return to project; planning continues" : "Back to project")}</button>{session?.status === "running" && <button onClick={() => void api.cancelAgentSession(session.id).then((value) => setSession(value.session)).catch((error: Error) => setError(error.message))}>{t("Stop planning")}</button>}</footer>
    {session && session.status !== "running" && <p role="status">{t("Planning has stopped. No dataset processing was started.")} {session.next_action}</p>}
    {(!session || session.status !== "running") && !planning && <button onClick={() => { setSession(undefined); setError(""); onNavigate(projectJourneyPath(project.id, "goal"), true); }}>{t("Back to saved goal")}</button>}
    {error && <p role="alert">{error}</p>}
  </section>;
  return <section className="journey-scene journey-goal" aria-label={t("Annotation goal")}>
    <div className="journey-intro"><h2>{t("What do you want from these images?")}</h2><p>{t("Describe the result, then name the categories you want to keep.")}</p></div>
    <div className="journey-goal-preview">{images.slice(0, 3).map((image) => <img src={image.url} alt={image.name} key={image.image_id} />)}<span>{t("{count} saved images", { count: images.length })}</span></div>
    {!saved ? <p role="status">{t("Loading your saved goal…")}</p> : !saved.editable ? <p role="alert">{t("This Project has several task definitions. They remain unchanged in Project management.")}</p> : <>
      <label>{t("Describe your goal")}<textarea aria-label={t("Describe your goal")} value={goal} maxLength={4000} disabled={busy || consent} onChange={(event) => setGoal(event.target.value)} placeholder={t("Find cups, but not bottles.")} /></label>
      <fieldset className="journey-output-types" disabled={busy || consent}><legend>{t("What you will get")}</legend>{[
        ["bounding_box", "Boxes around objects", "One box for each object you name."],
        ["semantic_mask", "Outlined regions", "A pixel region for each target; requires a segmentation model."],
        ["classification", "Image categories", "A category for the whole image."],
      ].map(([value, title, explanation]) => <label key={value}><input type="radio" name="journey-output" checked={kind === value} onChange={() => setKind(value)} /><span><strong>{t(title)}</strong><small>{t(explanation)}</small></span></label>)}</fieldset>
      <label>{t("Categories to keep")}<input aria-label={t("Categories to keep")} value={labels} disabled={busy || consent} onChange={(event) => setLabels(event.target.value)} placeholder={t("cup, bottle, plate")} /><small>{t("Separate categories with commas. These names are used as written; no language model is needed to save them.")}</small></label>
      <p role="status">{t(busy ? "Saving your goal…" : dirty ? "Unsaved goal changes" : "Goal saved")}</p>
      {!model && <p className="journey-notice">{t("Your goal can be saved without a language model. No available planning model is connected yet; no inference will start.")}</p>}
      {model && !visionReady && <p className="journey-notice">{t("Planning is connected, but this output still needs a compatible image model. No sample calls will start.")}</p>}
      {consent && <section className="journey-consent" aria-label={t("Planning authorization")}><h3>{t("First, prepare the plan")}</h3><p>{model?.display_name} · {destination}</p><p>{t("Only your goal, labels and model catalog are sent in this step. Up to 16 planning turns and a $1 reported-usage budget. Actual cost is unknown when the service does not report prices. No image testing is permitted in this planning step.")}</p><p>{t("Before testing images, you will confirm the actual services and sample scope separately. This does not publish or process your dataset.")}</p><div className="button-row"><button onClick={() => setConsent(false)}>{t("Back to goal")}</button><button className="primary" onClick={() => void build()}>{t("Authorize planning")}</button></div></section>}
      {!consent && <footer className="journey-actions"><button disabled={busy} onClick={() => onNavigate(projectJourneyPath(project.id, "images"))}>{t("Back to images")}</button><button className="primary" disabled={busy || !splitGoalLabels(labels).length || !images.length} onClick={() => void prepare()}>{t(model ? visionReady ? "Prepare sample results" : "Connect an image model" : "Save goal and connect model")}</button></footer>}
    </>}
    {error && <p role="alert">{error}</p>}
  </section>;
}
