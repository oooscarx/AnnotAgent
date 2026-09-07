import { useEffect, useRef, useState } from "react";
import { api } from "../api";
import { t } from "../i18n";
import { projectJourneyPath } from "../navigation";
import { journeyConnectionReady, journeyModelMatches } from "../journeyConnections";
import { journeyPlanningPhase } from "./JourneyGoal";
import type { AgentSession, RegistryModelProfile } from "../types";

/** A task view over the existing Builder repair mode. Entry is read-only. */
export function JourneyRevision({ projectId, draftId, testId, imageId, onNavigate }: {
  projectId: string; draftId?: string; testId?: string; imageId?: string;
  onNavigate: (path: string, replace?: boolean) => void;
}) {
  const [evidence, setEvidence] = useState<Awaited<ReturnType<typeof api.samplePlanEvidence>>>();
  const [model, setModel] = useState<RegistryModelProfile>();
  const [goalRevision, setGoalRevision] = useState("");
  const [destination, setDestination] = useState("");
  const [session, setSession] = useState<AgentSession>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [consent, setConsent] = useState(false);
  const active = useRef(true);
  const pending = useRef(false);
  useEffect(() => {
    active.current = true;
    let current = true;
    if (!draftId || !testId) { setError(t("Choose a saved sample before requesting changes.")); return; }
    void Promise.all([api.samplePlanEvidence(projectId, draftId), api.projectGoal(projectId), api.agentModelBindings(), api.projectModelBindings(projectId), api.modelProfiles(), api.providers()]).then(([saved, goal, defaults, bindings, models, providers]) => {
      if (!current) return;
      if (saved.project_id !== projectId || saved.sample_test_id !== testId) throw new Error(t("This revision does not belong to the selected sample."));
      setEvidence(saved); setGoalRevision(goal.revision);
      const id = bindings.bindings.find((binding) => binding.role === "pipeline_builder")?.model_profile_id ?? defaults.pipeline_builder;
      const chosen = models.models.find((profile) => profile.id === id && journeyModelMatches(profile, "planning", goal.kind ?? "bounding_box") && journeyConnectionReady(profile, providers.providers));
      setModel(chosen); setDestination(providers.providers.find((provider) => provider.id === chosen?.provider_id)?.base_url ?? "");
    }).catch((error: Error) => { if (current) setError(error.message); });
    return () => { current = false; active.current = false; };
  }, [projectId, draftId, testId]);
  useEffect(() => {
    if (!draftId) return;
    let current = true;
    const poll = () => void api.agentSessions(projectId).then(({ sessions }) => {
      if (current) setSession(sessions.find((item) => item.kind === "pipeline_builder" && item.draft_id === draftId));
    }).catch((error: Error) => { if (current) setError(error.message); });
    poll(); const timer = window.setInterval(poll, 1200);
    return () => { current = false; window.clearInterval(timer); };
  }, [projectId, draftId]);
  async function revise() {
    if (!consent || !model || !draftId || !evidence || pending.current || session?.status === "running") return;
    pending.current = true; setBusy(true); setError(""); setConsent(false);
    try {
      const result = await api.suggestWorkflow(projectId, "llm", undefined, { require_review_gate: true }, {
        priority: "balanced", max_model_calls_per_image: 4, allow_external_models: true,
        allow_human_review: true, maximum_agent_turns: 16, maximum_tool_calls: 48,
        maximum_dry_runs: 0, maximum_agent_cost: "1",
      }, model.id, undefined, { kind: "repair_draft", draft_id: draftId }, {
        model_revision: model.revision, provider_id: model.provider_id, base_url: destination, goal_revision: goalRevision,
      });
      if (!active.current) return;
      setSession(result.agent_session);
      if (result.draft.id !== draftId) throw new Error(t("The service returned another plan. The original sample is unchanged; inspect the saved task before retrying."));
    } catch (error) { if (active.current) setError((error as Error).message); }
    finally { pending.current = false; if (active.current) setBusy(false); }
  }
  const running = busy || session?.status === "running";
  const ready = session?.outcome === "draft_ready_for_human_review" && session.status !== "running";
  const originalPath = evidence ? projectJourneyPath(projectId, "samples", { draftId: evidence.baseline_draft_id, sampleTestId: evidence.sample_test_id, imageId }) : `/projects/${encodeURIComponent(projectId)}`;
  return <section className="journey-scene" aria-label={t("Sample plan adjustment")}>
    <div className="journey-intro"><h2>{t(running ? "Adjusting the plan using your feedback." : ready ? "Check whether the revised plan helps." : "What should the next sample improve?")}</h2></div>
    {evidence && <p>{t("Your original plan and sample remain saved. Only a separate working copy can change.")} {t("{count} saved feedback entries", { count: evidence.feedback.length })}</p>}
    {running && <p role="status">{t(journeyPlanningPhase(session?.phase))}</p>}
    {!running && !ready && evidence && model && <section className="journey-consent" aria-label={t("Revision authorization")}>
      <p>{model.display_name} · {destination}</p>
      <p>{t("Your goal, saved feedback, sample outcome summaries and model catalog will be sent to this planning service. Up to 16 turns and a $1 reported-usage budget. Actual cost may be unknown. No image calls, publication or dataset processing are authorized here.")}</p>
      <label className="checkbox-row"><input type="checkbox" checked={consent} onChange={(event) => setConsent(event.target.checked)} />{t("I authorize this feedback and planning scope")}</label>
    </section>}
    {!running && !ready && evidence && !model && <p role="alert">{t("The planning connection is unavailable. Your feedback and original sample remain saved. Connect a model and return to this same adjustment; no planning starts automatically.")}</p>}
    {ready && <p>{t("This is a proposed change, not verified improvement. Review the image-test scope next; no extra image test has started.")}</p>}
    <footer className="journey-actions">
      <button onClick={() => onNavigate(originalPath)}>{t(running ? "Return to original sample; revision continues" : "Keep original plan")}</button>
      {session?.status === "running" && <button onClick={() => void api.cancelAgentSession(session.id).then((value) => { if (active.current) setSession(value.session); }).catch((error: Error) => setError(error.message))}>{t("Stop planning")}</button>}
      {ready ? <button className="primary" onClick={() => onNavigate(projectJourneyPath(projectId, "samples", { draftId, sampleView: "authorize" }))}>{t("Review revised sample scope")}</button> : !running && (evidence && !model ? <button className="primary" onClick={() => onNavigate(projectJourneyPath(projectId, "model", { returnScene: "revise", draftId, sampleTestId: testId, imageId }))}>{t("Connect planning model")}</button> : <button className="primary" disabled={!evidence || !model || !consent} onClick={() => void revise()}>{t("Authorize plan adjustment")}</button>)}
    </footer>
    {error && <p role="alert">{error}</p>}
  </section>;
}
