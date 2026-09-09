import { t, localeTag, useLocale } from "./i18n";
import { reviewLabelText, withReviewLabel } from "./review-label";
import { canRefreshReviewDraft, mergeReviewQueue } from "./reviewQueue";
import { isTextEditingTarget, workspaceShortcutAllowed } from "./workspaceKeyboard";
import { recoveryNodeIds, builderStopLabel, builderPlanSource } from "./pipelinePresentation";
import { Fragment, useCallback, useEffect, useRef, useState } from "react";
import { LanguageSelector } from "./components/LanguageSelector";
import { ApiRequestError, api, subscribeEvents } from "./api";
import { AnnotationCanvas } from "./components/AnnotationCanvas";
import { FirstResultEntry } from "./components/FirstResultEntry";
import { SampleFeedbackEditor } from "./components/SampleFeedbackEditor";
import { SampleGeometryComparison } from "./components/SampleGeometryComparison";
import { FocusHeader, usesFocusLayout } from "./components/FocusHeader";
import { JourneyBatch } from "./components/JourneyBatch";
import { JourneyImages } from "./components/JourneyImages";
import { ConversationWorkspace } from "./components/ConversationWorkspace";
import { JourneyGoal } from "./components/JourneyGoal";
import { JourneyModel } from "./components/JourneyModel";
import { JourneySampleTask } from "./components/JourneySampleTask";
import { JourneyConfirm } from "./components/JourneyConfirm";
import { JourneyRevision } from "./components/JourneyRevision";
import { JourneyRun } from "./components/JourneyRun";
import { ImproveAutomationPanel } from "./components/GeometrySafetyPanel";
import { NotFoundPage } from "./features/notFound/NotFoundPage";
import {
  PROVIDER_PRESETS,
  isEnvironmentVariableName,
} from "./providerCatalog";
import { visualProfilesForSkills } from "./skills/visualProfiles";
import { annotationColor, annotationVisual, type LabelVisualMapping } from "./annotationVisuals";
import { deriveProjectRunView } from "./runState";
import { projectForReview, projectForRun, resolvedRunProjectId, runsForContext } from "./workspaceContext";
import {
  parseWorkspaceRoute,
  projectBuildPath,
  projectWorkPath,
  projectJourneyPath,
  projectBatchPath,
  projectReviewPath,
  projectRunPath,
  projectRunsPath,
  projectTrashPath,
  routeFocusKey,
  withConversationReturn,
  conversationSettingsPath,
  type SettingsSection,
  type WorkspaceRoute,
} from "./navigation";
import { isAbortError, queryKeys, workspaceQueries } from "./queryCache";
import { useRouteQuery } from "./useRouteQuery";
import {
  NO_PROJECT_MESSAGE,
  PRIMARY_NAVIGATION,
  PRODUCT_NAME,
  PRODUCT_TAGLINE,
  activeSkills,
  type ProductPage,
} from "./productIdentity";
import type {
  AgentSession,
  Annotation,
  AnnotationRevision,
  CorrectionMemoryRecord,
  DatasetBatchSummary,
  DetectionWorkerTestResult,
  DetectionWorkerSampleTestResult,
  DetectionEvidenceDto,
  EvidenceGateReportDto,
  HistoryRun,
  ImageItem,
  ModelBinding,
  NodeReplayReport,
  OptimizationPriority,
  PipelineBuildMode,
  PipelineBuilderConstraints,
  PipelineDraftDiff,
  PipelineArtifact,
  PipelineArtifactType,
  PipelineSource,
  PipelineStep,
  ProviderPresetProfile,
  ProviderProfile,
  RegistryModelProfile,
  ModelCapability,
  InputModality,
  ProviderProbeUsage,
  ProjectModelBinding,
  ModelBindingRole,
  ModelCapabilityQualityContract,
  GlobalModelDefaults,
  LegacyRegistryImportPreview,
  ExportReadiness,
  ProjectExportResult,
  GuidedAction,
  ProjectSummary,
  ProjectGuidance,
  ProjectWorkspaceSummary,
  ReviewItem,
  ReviewNavigation,
  ReviewQueueProgress,
  ResultLineageStage,
  RunEvent,
  RunAnnotationInspection,
  RunDebugSummary,
  RunNodeArtifactInspection,
  RunResultSummary,
  RunProvenanceSummary,
  SkillDetail,
  WorkflowCatalog,
  WorkflowDraft,
  WorkflowDraftNode,
  WorkflowDryRunReport,
  WorkflowVersion,
  WorkflowVersionComparison,
  WorkflowSuggestion,
  ExpertPluginInstallation,
  ExpertPluginRegistry,
  InstalledModelBundle,
  InstalledModelInstance,
  ModelCatalogEntry,
  ModelInstallOperation,
  ModelInstanceProfile,
  ManagementAction,
  ManagementObjectKind,
  ManagementObjectRef,
  ManagementPreview,
  ManagementReceipt,
  ManagementRequest,
  ManagementUsageSummary,
  PipelineLifecycleSummary,
  TrashEntry,
  VerifiedExpertPluginPackage,
  VerifiedModelBundlePackage,
} from "./types";

type WorkspacePage = ProductPage | "project" | "build" | "export" | "notFound";

const PAGE_TITLES: Record<WorkspacePage, string> = {
  home: "Home",
  projects: "Projects",
  project: "Project",
  build: "Build",
  export: "Export",
  runs: "Runs",
  review: "Review",
  settings: "Settings",
  notFound: "Not Found",
};

const DEFAULT_PIPELINE_BUILDER_CONSTRAINTS: PipelineBuilderConstraints = {
  priority: "balanced",
  max_model_calls_per_image: 4,
  target_review_rate: 0.25,
  allow_external_models: true,
  allow_human_review: true,
  maximum_agent_turns: 16,
  maximum_tool_calls: 48,
  maximum_dry_runs: 3,
  maximum_agent_cost: "1",
};

function readableErrorMessage(value: string): string {
  const decoded = value
    .replace(/&#x([0-9a-f]+);/gi, (_match, code: string) =>
      String.fromCodePoint(Number.parseInt(code, 16)),
    )
    .replace(/&#([0-9]+);/g, (_match, code: string) =>
      String.fromCodePoint(Number.parseInt(code, 10)),
    )
    .replaceAll("&nbsp;", " ")
    .replace(/\\+\s*$/g, "")
    .trim();
  if (/^step or tool-call budget exhausted$/i.test(decoded)) {
    return "The Pipeline Builder used its available work budget before it saved an outcome. Reload the latest state, then retry with fresh counters from the current persisted Draft.";
  }
  return decoded;
}

const PROJECT_MODEL_CHOICES: {
  role: ModelBindingRole;
  capability: ModelCapability;
  label: string;
  modality: InputModality;
}[] = [
  {
    role: "pipeline_builder",
    capability: "text_generation",
    label: "Pipeline Builder",
    modality: "text",
  },
  {
    role: "detection",
    capability: "object_detection",
    label: "Specialist detection",
    modality: "image",
  },
  {
    role: "classification",
    capability: "image_classification",
    label: "Classification",
    modality: "image",
  },
  {
    role: "verification",
    capability: "vision_language",
    label: "VLM detection / verification",
    modality: "image",
  },
];

function pipelineDiffChangeIds(diff: PipelineDraftDiff): string[] {
  return [
    ...diff.added_nodes,
    ...diff.removed_nodes,
    ...diff.modified_nodes,
    ...diff.added_edges,
    ...diff.removed_edges,
    ...diff.model_binding_changes,
    ...diff.policy_changes,
  ].map((change) => change.change_id);
}

function pipelineDiffRows(diff: PipelineDraftDiff, proposal: WorkflowDraft) {
  const titleFor = (nodeId: string) =>
    workflowNodeTitle(
      proposal.nodes.find((node) => node.id === nodeId)?.node_type ?? "automation step",
    );
  return [
    ...diff.added_nodes.map((change) => ({
      id: change.change_id,
      tone: "added",
      label: `Add ${workflowNodeTitle(change.node_type)}`,
    })),
    ...diff.removed_nodes.map((change) => ({
      id: change.change_id,
      tone: "removed",
      label: `Remove ${workflowNodeTitle(change.node_type)}`,
    })),
    ...diff.modified_nodes.map((change) => ({
      id: change.change_id,
      tone: "changed",
      label: `Update ${titleFor(change.node_id)}`,
    })),
    ...diff.model_binding_changes.map((change) => ({
      id: change.change_id,
      tone: "changed",
      label: `Change the model for ${titleFor(change.node_id)}`,
    })),
    ...diff.policy_changes.map((change) => ({
      id: change.change_id,
      tone: "changed",
      label: `Update the decision policy for ${titleFor(change.node_id)}`,
    })),
    ...diff.added_edges.map((change) => ({
      id: change.change_id,
      tone: "added",
      label: `Connect ${titleFor(change.edge.from_node)} to ${titleFor(change.edge.to_node)}`,
    })),
    ...diff.removed_edges.map((change) => ({
      id: change.change_id,
      tone: "removed",
      label: `Disconnect ${titleFor(change.edge.from_node)} from ${titleFor(change.edge.to_node)}`,
    })),
  ];
}

export function App() {
  useLocale();
  const [route, setRoute] = useState(() =>
    parseWorkspaceRoute(
      window.location.pathname,
      window.location.search,
      window.location.hash,
    ),
  );
  const [projects, setProjects] = useState<ProjectSummary[]>([]);
  const [runs, setRuns] = useState<HistoryRun[]>([]);
  const [models, setModels] = useState<ModelBinding[]>([]);
  const [events, setEvents] = useState<RunEvent[]>([]);
  const [error, setErrorState] = useState<{ message: string; scope: string }>();
  const [dashboardLoaded, setLoaded] = useState(false);
  const [connection, setConnection] = useState<"connecting" | "connected" | "reconnecting">("connecting");
  const [routeRetryVersion, setRouteRetryVersion] = useState(0);
  const hasConnectedRef = useRef(false);
  const needsReconnectSyncRef = useRef(false);
  const pageTitleRef = useRef<HTMLHeadingElement>(null);
  const navigationGuardRef = useRef<(() => boolean) | undefined>(undefined);
  const acceptedLocationRef = useRef(
    `${window.location.pathname}${window.location.search}${window.location.hash}`,
  );

  const setNavigationGuard = useCallback((guard?: () => boolean) => {
    navigationGuardRef.current = guard;
  }, []);
  const routeScope = routeFocusKey(route);
  const setError = (message: string) =>
    setErrorState(message ? { message, scope: routeScope } : undefined);
  const visibleError = error?.scope === routeScope ? error.message : undefined;
  const retryCurrentView = () => {
    if (navigationGuardRef.current && !navigationGuardRef.current()) return;
    setErrorState(undefined);
    setRouteRetryVersion((version) => version + 1);
  };

  const navigate = (path: string, replace = false) => {
    if (navigationGuardRef.current && !navigationGuardRef.current()) return false;
    if (replace) window.history.replaceState({}, "", path);
    else window.history.pushState({}, "", path);
    acceptedLocationRef.current = `${window.location.pathname}${window.location.search}${window.location.hash}`;
    setRoute(
      parseWorkspaceRoute(
        window.location.pathname,
        window.location.search,
        window.location.hash,
      ),
    );
    return true;
  };

  useEffect(() => {
    const restore = () => {
      if (navigationGuardRef.current && !navigationGuardRef.current()) {
        window.history.pushState({}, "", acceptedLocationRef.current);
        return;
      }
      acceptedLocationRef.current = `${window.location.pathname}${window.location.search}${window.location.hash}`;
      setRoute(
        parseWorkspaceRoute(
          window.location.pathname,
          window.location.search,
          window.location.hash,
        ),
      );
    };
    window.addEventListener("popstate", restore);
    return () => window.removeEventListener("popstate", restore);
  }, []);
  useEffect(() => {
    const current = `${window.location.pathname}${window.location.search}`;
    if (route.canonicalPath !== current || window.location.hash)
      navigate(route.canonicalPath, true);
  }, [route.canonicalPath]);

  const applyDashboard = (data: Awaited<ReturnType<typeof api.dashboard>>) => {
    setProjects(data.projects);
    setRuns(data.runs);
    setModels(data.models);
    setLoaded(true);
  };
  const refresh = (force = true) =>
    workspaceQueries
      .load(queryKeys.dashboard, (signal) => api.dashboard(signal), {
        force,
        staleTime: 15_000,
      })
      .then(applyDashboard)
      .catch((reason: Error) => {
        if (isAbortError(reason)) return;
        setLoaded(true);
        setError(reason.message);
      });

  const refreshExecutionState = async (runId: string) => {
    workspaceQueries.invalidate(queryKeys.run(runId));
    const nextRuns = await workspaceQueries.load(
      "runs",
      (signal) => api.runs(signal),
      { force: true },
    );
    setRuns(nextRuns.runs);
    const owner = nextRuns.runs.find((run) => run.id === runId)?.project_id;
    if (!owner) return;
    workspaceQueries.invalidate(queryKeys.project(owner));
    const summary = await workspaceQueries.load(
      queryKeys.projectSummary(owner),
      (signal) => api.projectSummary(owner, signal),
      { force: true },
    );
    setProjects((current) =>
      current.map((project) => project.id === owner ? summary.project : project),
    );
  };

  useEffect(() => {
    void refresh(false);
  }, []);
  useEffect(() => {
    const applyManagementReceipt = (receipt: ManagementReceipt) => {
      workspaceQueries.invalidate(queryKeys.dashboard);
      workspaceQueries.invalidate("runs");
      workspaceQueries.invalidate(queryKeys.project(receipt.project_id));
      workspaceQueries.invalidate(queryKeys.projectSummary(receipt.project_id));
      workspaceQueries.invalidate(queryKeys.projectRuns(receipt.project_id));
      workspaceQueries.invalidate(queryKeys.workflowDrafts(receipt.project_id));
      workspaceQueries.invalidate(queryKeys.reviewQueue(receipt.project_id));
      workspaceQueries.invalidate(queryKeys.reviewQueue());
      for (const object of receipt.affected_objects) {
        if (object.kind === "run") {
          workspaceQueries.invalidate(queryKeys.run(object.id));
          workspaceQueries.invalidate(queryKeys.runResults(object.id));
          workspaceQueries.invalidate(queryKeys.runDebug(object.id));
          workspaceQueries.invalidate(queryKeys.runAnnotations(object.id));
        }
        if (object.kind === "workflow_draft")
          workspaceQueries.invalidate(queryKeys.workflowDraft(object.id));
      }
      void refresh(true);
    };
    const local = (event: Event) => {
      const receipt = (event as CustomEvent<ManagementReceipt>).detail;
      if (receipt) applyManagementReceipt(receipt);
    };
    const remote = (event: StorageEvent) => {
      if (event.key !== "annotagent.management.receipt" || !event.newValue) return;
      try {
        applyManagementReceipt(JSON.parse(event.newValue) as ManagementReceipt);
      } catch {
        // A malformed or legacy localStorage value is ignored; the next server refresh wins.
      }
    };
    window.addEventListener("annotagent:management", local);
    window.addEventListener("storage", remote);
    return () => {
      window.removeEventListener("annotagent:management", local);
      window.removeEventListener("storage", remote);
    };
  }, []);
  useEffect(
    () =>
      subscribeEvents(
        (event) => {
          setEvents((previous) => [...previous.slice(-149), event]);
          workspaceQueries.invalidate(queryKeys.run(event.run_id));
          if (event.kind === "review_requested") {
            workspaceQueries.invalidate("review-queue");
            void workspaceQueries
              .load(
                queryKeys.reviewQueue(),
                (signal) => api.reviews(undefined, signal),
                { force: true },
              )
              .catch(() => undefined);
          }
          if (
            [
              "run_created",
              "run_started",
              "run_paused",
              "run_resumed",
              "run_cancelled",
              "run_budget_exceeded",
              "run_completed",
              "review_requested",
              "run_failed",
              "run_interrupted",
            ].includes(event.kind)
          )
            void refreshExecutionState(event.run_id).catch(() => undefined);
        },
        () => {
          needsReconnectSyncRef.current = true;
          setConnection("reconnecting");
        },
        () => {
          setConnection("connected");
          if (hasConnectedRef.current || needsReconnectSyncRef.current) void refresh(true);
          hasConnectedRef.current = true;
          needsReconnectSyncRef.current = false;
        },
      ),
    [],
  );
  useEffect(() => {
    pageTitleRef.current?.focus();
  }, [routeFocusKey(route)]);

  const routeProjectId = (() => {
    switch (route.kind) {
      case "project":
      case "journey":
      case "conversation":
      case "build":
      case "export":
      case "projectRuns":
      case "projectRun":
      case "projectBatch":
      case "projectReview":
      case "projectTrash":
        return route.projectId;
      default:
        return "";
    }
  })();
  const routeRun =
    (route.kind === "runs" || route.kind === "projectRun") && route.runId
    ? runs.find((run) => run.id === route.runId)
    : undefined;
  const routeRunProject = projectForRun(projects, routeRun);
  const projectId = routeProjectId || routeRunProject?.id || "";
  const listedProject = projects.find((project) => project.id === projectId);
  const [routeProject, setRouteProject] = useState<{id:string;project?:ProjectSummary;error?:Error}>();
  const routeProjectIdRef=useRef(routeProjectId);routeProjectIdRef.current=routeProjectId;
  useEffect(() => {
    if (!routeProjectId || listedProject) return;
    let current = true;
    void workspaceQueries.load(queryKeys.projectSummary(routeProjectId), signal => api.projectSummary(routeProjectId, signal))
      .then(value => { if(value.project.id!==routeProjectId)throw new Error("Project summary does not match the requested Project.");if(current)setRouteProject({id:routeProjectId,project:value.project}); })
      .catch((error:Error) => { if(current&&!isAbortError(error))setRouteProject({id:routeProjectId,error}); });
    return () => { current = false; };
  }, [routeProjectId, Boolean(listedProject)]);
  const resolvedRouteProject = routeProject?.id===routeProjectId ? routeProject : undefined;
  const selectedProject = listedProject ?? resolvedRouteProject?.project;
  // The dashboard is a page, not the complete set of owners of routed objects.
  const contextProjects = selectedProject && !listedProject ? [...projects, selectedProject] : projects;
  const projectLookupError = !selectedProject ? resolvedRouteProject?.error : undefined;
  const projectNotFound = projectLookupError instanceof ApiRequestError && projectLookupError.status===404;
  const loaded = dashboardLoaded && (!routeProjectId || Boolean(selectedProject) || projectNotFound);
  const isProjectWorkspace = Boolean(routeProjectId);
  const setProjectContext = (id: string) => {
    if (id) window.localStorage.setItem("annotagent.preferredProjectId", id);
    else window.localStorage.removeItem("annotagent.preferredProjectId");
  };
  const openProject = (id: string) => {
    if (navigate(id ? `/projects/${encodeURIComponent(id)}` : "/projects"))
      setProjectContext(id);
  };
  const switchProject = (id: string) => {
    let destination = "/projects";
    if (id && route.kind === "export")
      destination = `/projects/${encodeURIComponent(id)}/export`;
    else if (id && route.kind === "build")
      destination = `/projects/${encodeURIComponent(id)}/build/${route.step}`;
    else if (id && route.kind === "projectTrash")
      destination = projectTrashPath(id, route.objectKind);
    else if (id && (
      route.kind === "projectRuns" ||
      route.kind === "projectRun" ||
      route.kind === "projectBatch"
    ))
      destination = projectRunsPath(id);
    else if (id && route.kind === "projectReview")
      destination = projectReviewPath(id);
    else if (id && route.kind === "project")
      destination = `/projects/${encodeURIComponent(id)}`;
    if (navigate(destination)) setProjectContext(id);
  };
  useEffect(() => {
    const resolved = routeProjectId || routeRunProject?.id;
    if (resolved) setProjectContext(resolved);
  }, [routeProjectId, routeRunProject?.id]);
  useEffect(() => {
    if (
      route.kind === "runs" &&
      route.runId &&
      resolvedRunProjectId(routeRun)
    ) {
      navigate(
        projectRunPath(resolvedRunProjectId(routeRun)!, route.runId, {
          annotationId: route.annotationId, canvasView: route.canvasView,
          imageId: route.imageId,
          nodeId: route.nodeId,
          artifactId: route.artifactId,
          view: route.view,
        }),
        true,
      );
    }
  }, [route.kind, routeRun?.id, routeRun?.project_id, routeRun?.ownership_status]);
  const page: WorkspacePage =
    route.kind === "projectRuns" ||
    route.kind === "projectRun" ||
    route.kind === "projectBatch"
    || route.kind === "projectTrash"
      ? "runs"
      : route.kind === "projectReview"
        ? "review"
        : route.kind === "journey" || route.kind === "conversation" ? "project" : route.kind;

  const focusLayout = usesFocusLayout(route);
  return (
    <div className={`app-shell ${focusLayout ? "focus-layout" : ""}`} data-layout={focusLayout ? "focus" : "management"}>
      <a className="skip-link" href="#main-content">{t("Skip to workspace")}</a>
      {!focusLayout && <aside className="sidebar aa-dark">
        <a
          className="brand"
          href="/"
          aria-label={`${PRODUCT_NAME} home`}
          onClick={(event) => {
            event.preventDefault();
            navigate("/");
          }}
        >
          <img
            className="brand-lockup"
            src="/brand/core/annotagent-lockup-dark.svg"
            alt={PRODUCT_NAME}
          />
          <img
            className="brand-mark-compact"
            src="/brand/core/annotagent-mark-dark-surface.svg"
            alt=""
            aria-hidden="true"
          />
        </a>
        <nav aria-label={t("Primary navigation")}>
          {PRIMARY_NAVIGATION.map((item) => (
            <Nav
              key={item.page}
              icon={item.icon}
              active={
                item.page === "projects"
                  ? route.kind === "projects" ||
                    route.kind === "project" ||
                    route.kind === "build" ||
                    route.kind === "export" ||
                    route.kind === "projectRuns" ||
                    route.kind === "projectRun" ||
                    route.kind === "projectBatch" ||
                    route.kind === "projectReview"
                    || route.kind === "projectTrash"
                  : !isProjectWorkspace && page === item.page
              }
              href={item.href}
              onClick={() => navigate(item.href)}
            >
              {t(item.label)}
            </Nav>
          ))}
        </nav>
        <div className="sidebar-foot">
          <span className={`live-dot ${connection}`} aria-hidden="true" /> SSE {t(connection)}
          <small>
            {events.at(-1)?.kind.replaceAll("_", " ") ?? t("waiting for events")}
          </small>
        </div>
      </aside>}
      <main
        key={`${routeScope}:${routeRetryVersion}`}
        id="main-content"
        aria-busy={!loaded}
        className={page === "review" ? "review-main" : undefined}
      >
        {focusLayout ? <FocusHeader project={selectedProject} route={route} titleRef={pageTitleRef} loaded={loaded} connection={connection} onNavigate={navigate} /> : <header className="topbar">
          <div>
            <span className="product-tagline">{t(PRODUCT_TAGLINE)}</span>
            <h1 ref={pageTitleRef} tabIndex={-1}>{t(PAGE_TITLES[page])}</h1>
          </div>
          <LanguageSelector />
          {routeProjectId && <div className="project-switch">
            {activeSkills(selectedProject).map((skill) => {
              const profile = visualProfilesForSkills([skill.id])[0];
              return (
                <span className="skill-badge" key={skill.id}>
                  {profile?.icon && (
                    <img src={profile.icon} alt="" aria-hidden="true" />
                  )}
                  {skill.display_name}
                </span>
              );
            })}
            <span aria-hidden="true">{t("Project context")}</span>
            <label className="sr-only" htmlFor="active-project">{t("Active project")}</label>
            <select
              id="active-project"
              value={projectId}
              onChange={(event) => switchProject(event.target.value)}
            >
              <option value="">{t(NO_PROJECT_MESSAGE)}</option>
              {projects.map((project) => (
                <option key={project.id} value={project.id}>
                  {project.name}
                </option>
              ))}
            </select>
          </div>}
        </header>}
        {visibleError && (
          <div className="error-banner" role="alert">
            <span className="error-message">
              <strong>{t("AnnotAgent couldn’t complete that action.")}</strong>
              <span>{readableErrorMessage(visibleError)}</span>
              <small>Saved workspace data remains on the server. Dismiss this message, correct the indicated input if needed, then retry the same action on this page.</small>
            </span>
            <span className="error-actions">
              <button onClick={retryCurrentView}>{t("Retry this view")}</button>
              <button aria-label={t("Dismiss error")} onClick={() => setError("")}>{t("Dismiss")}</button>
            </span>
          </div>
        )}
        {!loaded && !projectLookupError && <div className="loading-banner" role="status">{t("Loading workspace state…")}</div>}
        {projectLookupError && !projectNotFound && <div className="error-banner" role="alert"><span>{projectLookupError.message}</span><button onClick={()=>{
          const id=routeProjectId;setRouteProject({id});
          void workspaceQueries.load(queryKeys.projectSummary(id),signal=>api.projectSummary(id,signal),{force:true})
            .then(value=>{if(value.project.id!==id)throw new Error("Project summary does not match the requested Project.");if(routeProjectIdRef.current===id)setRouteProject({id,project:value.project});}).catch((error:Error)=>{if(routeProjectIdRef.current===id)setRouteProject({id,error});});
        }}>{t("Retry this view")}</button></div>}
        {loaded && route.kind === "projects" && (
          <ProjectsPage
            projects={projects}
            createOnOpen={route.create}
            onNavigate={navigate}
            onNavigationGuardChange={setNavigationGuard}
            onSelect={(id)=>{if(navigate(projectWorkPath(id)))setProjectContext(id);}}
            onRefresh={refresh}
            onError={setError}
          />
        )}
        {loaded && route.kind === "conversation" && (
          selectedProject ? <ConversationWorkspace key={route.projectId} project={selectedProject} classReviewId={route.classReviewId} referenceMessageId={route.referenceMessageId} conversationId={route.conversationId} imageId={route.imageId} draftId={route.draftId} sampleTestId={route.sampleTestId} taskId={route.taskId} humanRequestId={route.humanRequestId} processingOperationId={route.processingOperationId} exportBefore={route.exportBefore} results={route.results} onNavigate={navigate} onNavigationGuardChange={setNavigationGuard} />
            : <NotFoundPage invalidPath={route.canonicalPath} onNavigate={navigate} />
        )}
        {loaded && route.kind === "journey" && (
          !selectedProject ? <NotFoundPage invalidPath={route.canonicalPath} onNavigate={navigate} />
          : route.scene === "images" ? <JourneyImages key={route.projectId} project={selectedProject} onNavigationGuardChange={setNavigationGuard} onContinue={async (id) => {
            setNavigationGuard(undefined); await refresh(); navigate(projectJourneyPath(id, "goal"));
          }} />
          : route.scene === "goal" ? <JourneyGoal key={route.projectId} project={selectedProject} sessionId={route.agentSessionId} onNavigate={navigate} onRefresh={refresh} onNavigationGuardChange={setNavigationGuard} />
          : route.scene === "model" ? <JourneyModel key={`${route.projectId}:${route.modelPurpose ?? "planning"}:${route.returnScene ?? "goal"}:${route.draftId ?? ""}:${route.sampleTestId ?? ""}`} project={selectedProject} purpose={route.modelPurpose ?? "planning"} revisionReturn={route.returnScene === "revise" && route.draftId && route.sampleTestId ? { draftId: route.draftId, sampleTestId: route.sampleTestId, imageId: route.imageId } : undefined} onNavigate={navigate} />
          : route.scene === "confirm" ? <JourneyConfirm key={route.projectId} projectId={route.projectId} draftId={route.draftId} testId={route.sampleTestId} imageId={route.imageId} operationId={route.processingOperationId} onNavigate={navigate} />
          : route.scene === "revise" ? <JourneyRevision key={`${route.projectId}:${route.draftId}`} projectId={route.projectId} draftId={route.draftId} testId={route.sampleTestId} imageId={route.imageId} onNavigate={navigate} />
          : <BuildTestPublish key={route.projectId} project={selectedProject} guided selectedDraftId={route.draftId} selectedSampleTestId={route.sampleTestId} selectedSampleImageId={route.imageId}
            sampleOperationId={route.sampleOperationId}
            requestNewSample={route.sampleView === "authorize"}
            onAdopt={(draftId, sampleTestId, imageId) => navigate(projectJourneyPath(route.projectId, "confirm", { draftId, sampleTestId, imageId }))}
            onImprove={(draftId, sampleTestId, imageId) => navigate(projectJourneyPath(route.projectId, "revise", { draftId, sampleTestId, imageId }))}
            onSampleOperation={(sampleOperationId) => navigate(projectJourneyPath(route.projectId, "samples", { draftId: route.draftId, sampleOperationId, sampleView: sampleOperationId ? undefined : "authorize" }), true)}
            onNavigationGuardChange={setNavigationGuard}
            onSelectTestContext={(draftId, sampleTestId, replace, imageId) => navigate(projectJourneyPath(route.projectId, "samples", { draftId, sampleTestId, imageId }), replace)}
            onNavigate={() => navigate(projectJourneyPath(route.projectId, "goal"))}
            onOpenRuns={(batchId) => navigate(batchId ? projectBatchPath(route.projectId, batchId) : projectRunsPath(route.projectId))}
            onRefresh={refresh} onError={setError} />
        )}
        {loaded && route.kind === "project" && (
          <ProjectPage
            project={selectedProject}
            runs={runs}
            events={events}
            onRefresh={refresh}
            onOpenWorkflows={() =>
              navigate(`/projects/${encodeURIComponent(route.projectId)}/build/pipeline`)
            }
            onOpenBuild={(step) =>
              navigate(`/projects/${encodeURIComponent(route.projectId)}/build/${step}`)
            }
            onOpenRun={(runId) =>
              navigate(projectRunPath(route.projectId, runId))
            }
            onOpenReview={() =>
              navigate(projectReviewPath(route.projectId))
            }
            onNavigate={navigate}
            onError={setError}
          />
        )}
        {loaded && route.kind === "build" && route.step === "pipeline" && route.workspaceReturn && <section className="conversation-consent conversation-setup-return" aria-label={t(route.workspaceReturn.includes("class_review=") ? "Return to image-class review" : "Back to annotation workspace")}><p>{t("This revision Draft is separate from the saved sample. Returning does not run a model or apply the Draft.")}</p><button onClick={() => navigate(route.workspaceReturn!)}>{t(route.workspaceReturn.includes("class_review=") ? "Return to image-class review" : "Back to annotation workspace")}</button></section>}
        {loaded && route.kind === "build" && route.step === "pipeline" && (
          <WorkflowsPage
            projects={contextProjects}
            runs={runs}
            activeProjectId={projectId}
            selectedDraftId={route.draftId}
            selectedWorkflow={route.workflowId && route.workflowVersion
              ? { id: route.workflowId, version: route.workflowVersion }
              : undefined}
            selectedAgentSessionId={route.agentSessionId}
            selectedImprovementSessionId={route.improvementSessionId}
            onActivate={(id) =>
              navigate(projectBuildPath(id, "pipeline"))
            }
            onRefresh={refresh}
            onNavigate={(step, draftId) =>
              navigate(projectBuildPath(route.projectId, step, { draftId, workspaceReturn: route.workspaceReturn }))
            }
            onSelectContext={(context, replace) =>
              navigate(projectBuildPath(route.projectId, "pipeline", { ...context, workspaceReturn: route.workspaceReturn }), replace)
            }
            onOpenProjects={() => navigate("/projects")}
            onOpenProject={() => openProject(route.projectId)}
            onOpenTrash={() => navigate(projectTrashPath(route.projectId, "pipeline"))}
            onNavigationGuardChange={setNavigationGuard}
            onError={setError}
          />
        )}
        {loaded && route.kind === "build" && route.step !== "pipeline" && (
          <BuildWorkspace
            project={selectedProject}
            step={route.step}
            selectedDraftId={route.draftId}
            selectedSampleTestId={route.sampleTestId}
            selectedSampleImageId={route.imageId}
            onNavigationGuardChange={setNavigationGuard}
            onNavigate={(step, draftId, replace) =>
              navigate(projectBuildPath(route.projectId, step, { draftId }), replace)
            }
            onSelectTestContext={(draftId, sampleTestId, replace, imageId) =>
              navigate(projectBuildPath(route.projectId, "test", {
                draftId,
                sampleTestId,
                imageId,
              }), replace)
            }
            onOpenRuns={(batchId) =>
              navigate(batchId
                ? projectBatchPath(route.projectId, batchId)
                : projectRunsPath(route.projectId))
            }
            onOpenProjects={() => navigate("/projects")}
            onOpenProject={() => openProject(route.projectId)}
            onRefresh={refresh}
            onError={setError}
          />
        )}
        {loaded && route.kind === "export" && (
          <ProjectExportPage
            project={selectedProject}
            workspaceReturn={route.workspaceReturn}
            onNavigate={navigate}
            onError={setError}
          />
        )}
        {loaded && (route.kind === "runs" || route.kind === "projectRuns" || route.kind === "projectRun") && (
          <RunsPage
            onNavigationGuardChange={setNavigationGuard}
            runs={runs}
            projects={contextProjects}
            activeProject={selectedProject}
            route={route}
            onNavigate={navigate}
            onRefresh={refresh}
            onError={setError}
          />
        )}
        {loaded && route.kind === "projectBatch" && (
          <BatchDetailWorkspace
            onNavigationGuardChange={setNavigationGuard}
            route={route}
            runs={runs}
            projects={contextProjects}
            onNavigate={navigate}
            onRefresh={refresh}
            onError={setError}
          />
        )}
        {loaded && route.kind === "projectTrash" && (
          <TrashWorkspace
            project={selectedProject}
            route={route}
            onNavigate={navigate}
            onRefresh={refresh}
            onError={setError}
          />
        )}
        {loaded && (route.kind === "review" || route.kind === "projectReview") && (
          <ReviewPage
            project={selectedProject}
            projects={contextProjects}
            models={models}
            events={events}
            route={route}
            onNavigate={(path,replace)=>navigate(withConversationReturn(path,route.kind==="projectReview" ? route.workspaceReturn : undefined),replace)}
            onNavigationGuardChange={setNavigationGuard}
            onError={setError}
          />
        )}
        {loaded && route.kind === "settings" && (
          <>
          {route.workspaceReturn && <section className="conversation-consent conversation-setup-return" aria-label="Return to annotation task"><h2>{t("Model setup for your annotation task")}</h2><p>{projects.find(project=>project.id===route.returnProjectId)?.name ?? route.returnProjectId}</p><p>{t("Changes are saved through the existing settings services. Return when ready or cancel setup; returning does not authorize model calls.")}</p><button onClick={()=>navigate(route.workspaceReturn!)}>{t("Return to annotation task")}</button></section>}
          <SettingsWorkspace
            section={route.section}
            models={models}
            onNavigate={(section) =>
              navigate(conversationSettingsPath(route.returnProjectId ?? "",section,route.workspaceReturn))
            }
            onError={setError}
          />
          </>
        )}
        {loaded && route.kind === "notFound" && (
          <NotFoundPage invalidPath={route.invalidPath} onNavigate={navigate} />
        )}
      </main>
    </div>
  );
}

function Nav({
  icon,
  active,
  href,
  onClick,
  children,
}: {
  icon: string;
  active: boolean;
  href: string;
  onClick: () => void;
  children: string;
}) {
  return (
    <a
      href={href}
      title={children}
      className={active ? "active" : ""}
      aria-current={active ? "page" : undefined}
      onClick={(event) => {
        event.preventDefault();
        onClick();
      }}
    >
      <img src={`/brand/core/icons/${icon}.svg`} alt="" aria-hidden="true" />
      {children}
    </a>
  );
}

function ProjectBreadcrumb({
  project,
  current,
  onOpenProjects,
  onOpenProject,
}: {
  project?: ProjectSummary;
  current: string;
  onOpenProjects: () => void;
  onOpenProject?: () => void;
}) {
  return (
    <nav className="breadcrumb" aria-label={t("Breadcrumb")}>
      <button className="text-button" onClick={onOpenProjects}>{t("Projects")}</button>
      {project && (
        <>
          <span aria-hidden="true">/</span>
          <button className="text-button" onClick={onOpenProject}>{project.name}</button>
        </>
      )}
      <span aria-hidden="true">/</span>
      <strong>{t(current)}</strong>
    </nav>
  );
}

const BUILD_SEQUENCE = ["data", "labels", "pipeline", "test"] as const;
type BuildStep = (typeof BUILD_SEQUENCE)[number];

function useBuildSummary(
  project: ProjectSummary | undefined,
  onError: (value: string) => void,
): ProjectWorkspaceSummary | undefined {
  const query = useRouteQuery(
    project ? queryKeys.projectSummary(project.id) : undefined,
    (signal) => api.projectSummary(project!.id, signal),
    { staleTime: 5_000 },
  );
  useEffect(() => {
    if (query.error) onError(query.error.message);
  }, [query.error]);
  return query.data?.project.id === project?.id ? query.data : undefined;
}

function journeyForBuildStep(guidance: ProjectGuidance, step: BuildStep) {
  const id = step === "pipeline" ? "automation" : step === "test" ? "sample_test" : step;
  return guidance.journey.find((item) => item.id === id);
}

function buildStepAllowed(guidance: ProjectGuidance, step: BuildStep): boolean {
  const complete = (id: string) =>
    guidance.journey.find((item) => item.id === id)?.state === "complete";
  if (step === "data") return true;
  if (step === "labels") return complete("data");
  if (step === "pipeline") return complete("data") && complete("labels");
  return complete("data") && complete("labels") && complete("automation");
}

function BuildWorkspace({
  project,
  step,
  selectedDraftId,
  selectedSampleTestId,
  selectedSampleImageId,
  onNavigationGuardChange,
  onNavigate,
  onSelectTestContext,
  onOpenRuns,
  onOpenProjects,
  onOpenProject,
  onRefresh,
  onError,
}: {
  project?: ProjectSummary;
  step: "data" | "labels" | "test";
  selectedDraftId?: string;
  selectedSampleTestId?: string;
  selectedSampleImageId?: string;
  onNavigationGuardChange: (guard?: () => boolean) => void;
  onNavigate: (step: BuildStep, draftId?: string, replace?: boolean) => void;
  onSelectTestContext: (draftId: string, sampleTestId?: string, replace?: boolean, imageId?: string) => void;
  onOpenRuns: (batchId?: string) => void;
  onOpenProjects: () => void;
  onOpenProject: () => void;
  onRefresh: () => Promise<void>;
  onError: (value: string) => void;
}) {
  const summary = useBuildSummary(project, onError);
  const allowed = summary ? buildStepAllowed(summary.guidance, step) : false;
  const currentIndex = BUILD_SEQUENCE.indexOf(step);
  const nextStep = BUILD_SEQUENCE[currentIndex + 1];
  const previousStep = BUILD_SEQUENCE[currentIndex - 1];
  return (
    <section className="page-stack">
      <ProjectBreadcrumb project={project} current="Build" onOpenProjects={onOpenProjects} onOpenProject={onOpenProject} />
      <BuildNavigation step={step} guidance={summary?.guidance} onNavigate={onNavigate} />
      {!project ? (
        <Empty title={t("Project unavailable")} detail={t("Return to Projects and choose a valid Project.")} />
      ) : !summary ? (
        <div className="loading-banner" role="status">{t("Loading Build readiness…")}</div>
      ) : !allowed ? (
        <BuildBlocker guidance={summary.guidance} onNavigate={onNavigate} />
      ) : step === "data" ? (
        <BuildData project={project} onRefresh={onRefresh} onError={onError} />
      ) : step === "labels" ? (
        <BuildLabels project={project} onRefresh={onRefresh} onError={onError} />
      ) : (
        <BuildTestPublish project={project} selectedDraftId={selectedDraftId} selectedSampleTestId={selectedSampleTestId} selectedSampleImageId={selectedSampleImageId} onNavigationGuardChange={onNavigationGuardChange} onSelectTestContext={onSelectTestContext} onNavigate={onNavigate} onOpenRuns={onOpenRuns} onRefresh={onRefresh} onError={onError} />
      )}
      {project && summary && allowed && step !== "test" && <BuildFooter
        previous={previousStep}
        next={nextStep}
        nextEnabled={nextStep ? buildStepAllowed(summary.guidance, nextStep) : false}
        onNavigate={onNavigate}
      />}
    </section>
  );
}

function BuildBlocker({
  guidance,
  onNavigate,
}: {
  guidance: ProjectGuidance;
  onNavigate: (step: BuildStep) => void;
}) {
  const destination = guidance.primary_action.destination?.match(/\/build\/(data|labels|pipeline|test)$/)?.[1] as BuildStep | undefined;
  return <section className="build-blocker" aria-label={t("Build step blocked")}>
    <span aria-hidden="true">!</span>
    <div><span className="eyebrow">{t("Complete the current step first")}</span><h2>{t(guidance.headline)}</h2><p>{t(guidance.explanation)}</p></div>
    {destination && <button className="primary" onClick={() => onNavigate(destination)}>{t(guidance.primary_action.label)}</button>}
  </section>;
}

function BuildFooter({
  previous,
  next,
  nextEnabled,
  nextPrimary = true,
  onNavigate,
}: {
  previous?: BuildStep;
  next?: BuildStep;
  nextEnabled: boolean;
  nextPrimary?: boolean;
  onNavigate: (step: BuildStep) => void;
}) {
  const name = (step: BuildStep) => step === "pipeline" ? "Automation" : step === "test" ? "Test & Activate" : step[0].toUpperCase() + step.slice(1);
  return <footer className="build-footer">
    <span>{t("Changes in this step are saved to the Project as you complete them.")}</span>
    <div className="button-row">
      {previous && <button onClick={() => onNavigate(previous)}>← {t(name(previous))}</button>}
      {next && <button
        className={nextEnabled && nextPrimary ? "primary" : ""}
        aria-disabled={!nextEnabled}
        aria-label={!nextEnabled ? `Continue to ${t(name(next))} unavailable: complete this step first` : undefined}
        title={!nextEnabled ? t("Complete this step before continuing") : undefined}
        onClick={() => nextEnabled && onNavigate(next)}
      >{t("Continue to")}{" "}{t(name(next))} →</button>}
    </div>
  </footer>;
}

function BuildNavigation({
  step,
  guidance,
  onNavigate,
}: {
  step: BuildStep;
  guidance?: ProjectGuidance;
  onNavigate: (step: BuildStep) => void;
}) {
  return (
    <nav className="section-tabs build-steps" aria-label={t("Build steps")}>
      {BUILD_SEQUENCE.map((item, index) => {
        const journey = guidance && journeyForBuildStep(guidance, item);
        const complete = journey?.state === "complete" || (item === "test" && guidance?.journey.find((entry) => entry.id === "activation")?.state === "complete");
        const allowed = guidance ? buildStepAllowed(guidance, item) : item === step;
        return (
        <button
          key={item}
          className={`${step === item ? "active" : ""} ${complete ? "complete" : ""}`.trim()}
          aria-current={step === item ? "step" : undefined}
          aria-disabled={!allowed}
          aria-label={!allowed ? `${item === "pipeline" ? "Automation" : item === "test" ? "Test & Activate" : t(item[0].toUpperCase() + item.slice(1))} unavailable until the earlier Build step is complete` : undefined}
          title={!allowed ? t("Complete the earlier Build step first") : journey?.detail}
          onClick={() => allowed && onNavigate(item)}
        >
          <span>{complete ? "✓" : index + 1}</span>
          {item === "pipeline" ? t("Automation") : item === "test" ? t("Test & Activate") : t(item[0].toUpperCase() + item.slice(1))}
        </button>
      )})}
    </nav>
  );
}

function BuildData({
  project,
  onRefresh,
  onError,
}: {
  project: ProjectSummary;
  onRefresh: () => Promise<void>;
  onError: (value: string) => void;
}) {
  const [images, setImages] = useState<ImageItem[]>([]);
  const [imagesLoaded, setImagesLoaded] = useState(false);
  const [source, setSource] = useState("");
  const [result, setResult] = useState<Awaited<ReturnType<typeof api.importImages>>>();
  const [busy, setBusy] = useState(false);
  const load = () =>
    api.images(project.id).then((value) => {
      setImages(value.images);
      setImagesLoaded(true);
    });
  useEffect(() => {
    setImagesLoaded(false);
    void load().catch((error: Error) => onError(error.message));
  }, [project.id]);
  const importImages = () => {
    if (!source.trim()) return onError("Choose a workspace-local image file or directory.");
    setBusy(true);
    void api
      .importImages(project.id, source.trim())
      .then((report) => {
        setResult(report);
        return Promise.all([load(), onRefresh()]);
      })
      .catch((error: Error) => onError(error.message))
      .finally(() => setBusy(false));
  };
  const removeImage = (image: ImageItem) => {
    setBusy(true);
    void api
      .removeImage(project.id, image.image_id, image.content_hash)
      .then(() => Promise.all([load(), onRefresh()]))
      .catch((error: Error) => onError(error.message))
      .finally(() => setBusy(false));
  };
  return (
    <>
      <div className="build-step-heading"><span className="eyebrow">{t("Step 1 · Data")}</span><h2>{t("Add images to your Project")}</h2><p>Import PNG or JPEG files already reachable by this local AnnotAgent server. It validates every image, skips matching content, and keeps the Project copy under its dataset root.</p></div>
      <div className="metrics-grid build-metrics">
        <Metric label={t("Project images")} value={images.length} detail={t("Ready for sample testing")} />
        <Metric label={t("Latest import")} value={result?.imported ?? 0} detail={`${result?.discovered ?? 0} supported files found`} />
        <Metric label={t("Needs attention")} value={(result?.duplicates ?? 0) + (result?.corrupt.length ?? 0)} detail={`${result?.duplicates ?? 0} duplicate · ${result?.corrupt.length ?? 0} corrupt`} />
      </div>
      <Panel title={t("Import from a local path")} eyebrow={t("Advanced server-local source")}>
        <label>{t("Choose images")}<input type="file" accept="image/png,image/jpeg" multiple disabled={busy} onChange={(event) => {
          const files = Array.from(event.target.files ?? []);
          if (!files.length) return;
          setBusy(true);
          void (async () => {
            for (const file of files) {
              const report = await api.uploadImage(project.id, file);
              if (report.corrupt.length) throw new Error(report.corrupt.map((issue) => issue.message).join("; "));
            }
            await load(); await onRefresh();
          })().catch((error: Error) => onError(error.message)).finally(() => setBusy(false));
        }} /></label>
        <p>{t("PNG or JPEG · up to 25 MB per image · uploaded to this AnnotAgent server")}</p>
        <label>{t("Server-local image file or folder path")}<input value={source} onChange={(event) => setSource(event.target.value)} placeholder="/workspace/dataset/images" />
        </label>
        <div className="button-row">
          <button className={imagesLoaded && images.length === 0 ? "primary" : ""} disabled={busy || !source.trim()} onClick={importImages}>
            {busy ? t("Importing…") : t("Add images")}
          </button>
          <small>This is not a browser file picker. The path is read by the local AnnotAgent process. Supported: PNG and JPEG · recursive discovery · 100 MP decode safety limit.</small>
        </div>
        {result && <div className="import-outcome" aria-live="polite">
          <strong>{result.imported}{" "}{t("images added")}</strong>
          <span>{result.discovered}{" "}{t("discovered ·")}{" "}{result.duplicates}{" "}{t("duplicates skipped ·")}{" "}{result.unsupported_files}{" "}{t("unsupported files ignored")}</span>
          <small>{t("Source:")}{" "}{result.source}</small>
          {result.corrupt.length > 0 && <details><summary>{result.corrupt.length}{" "}{t("corrupt images were not imported")}</summary><ul>{result.corrupt.map((issue) => <li key={`${issue.name}:${issue.message}`}><strong>{issue.name}</strong> — {issue.message}</li>)}</ul></details>}
        </div>}
      </Panel>
      <Panel title={t("Project images")} eyebrow={`${images.length} registered · ${project.dataset.root}`}>
        {images.length ? <div className="build-image-list">
          {images.map((image) => <article key={image.image_id}>
            <img src={image.url} alt="" />
            <span><strong>{image.name}</strong><small>{image.path} · {(image.size_bytes / 1024).toFixed(1)} KB</small></span>
            <button className="danger-text" disabled={busy} onClick={() => removeImage(image)} aria-label={`Remove ${image.name} from Project`}>{t("Remove")}</button>
          </article>)}
        </div> : <Empty title={t("No images yet")} detail={t("Add a supported image or folder to complete the Data step.")} />}
        <details className="advanced-settings"><summary>{t("Dataset discovery settings")}</summary><Fact label={t("Discovery")} value={project.dataset.recursive ? "Recursive" : "Top level"} /><TagGroup title={t("Include patterns")} values={project.dataset.include} /></details>
      </Panel>
    </>
  );
}

function BuildLabels({
  project,
  onRefresh,
  onError,
}: {
  project: ProjectSummary;
  onRefresh: () => Promise<void>;
  onError: (value: string) => void;
}) {
  const [displayName, setDisplayName] = useState("");
  const [kind, setKind] = useState("bounding_box");
  const [labels, setLabels] = useState("");
  const [attributeName, setAttributeName] = useState("");
  const [attributeKind, setAttributeKind] = useState<"enum" | "string" | "number" | "boolean">("string");
  const [busy, setBusy] = useState(false);
  const kindName = (value: string) => ({
    classification: "Image classification",
    bounding_box: "Object detection",
    keypoints: "Keypoints",
    polygon: "Polygon regions",
    semantic_mask: "Semantic segmentation",
  }[value] ?? value.replaceAll("_", " "));
  const outputName = (value: string) => ({
    classification: "Class labels",
    bounding_box: "Bounding boxes",
    keypoints: "Named points",
    polygon: "Polygons",
    semantic_mask: "Semantic masks",
  }[value] ?? value.replaceAll("_", " "));
  const create = () => {
    const parsedLabels = labels.split(",").map((value) => value.trim()).filter(Boolean);
    if (!displayName.trim() || parsedLabels.length === 0)
      return onError("Enter a display name and at least one Label.");
    setBusy(true);
    const attributes = attributeName.trim()
      ? { [attributeName.trim()]: { type: attributeKind, required: false, values: [] } }
      : {};
    void api
      .addProjectTask(project.id, {
        display_name: displayName.trim(),
        kind,
        labels: parsedLabels,
        attributes,
      })
      .then(() => {
        setDisplayName("");
        setLabels("");
        setAttributeName("");
        return onRefresh();
      })
      .catch((error: Error) => onError(error.message))
      .finally(() => setBusy(false));
  };
  return (
    <>
      <div className="build-step-heading"><span className="eyebrow">{t("Step 2 · Labels")}</span><h2>{t("What do you want to annotate?")}</h2><p>Labels describe the meaning and output you want. Models and execution order belong to the next Automation step.</p></div>
      <div className="build-label-layout">
      <Panel title={t("Add a Label group")} eyebrow={t("Annotation meaning")}>
        <div className="label-group-form">
        <label>{t("What should this group be called?")}<input value={displayName} onChange={(event) => setDisplayName(event.target.value)} placeholder="Football" /></label>
        <label>{t("What kind of annotation?")}<select value={kind} onChange={(event) => setKind(event.target.value)}>
          <option value="classification">{t("Classification")}</option>
          <option value="bounding_box">{t("Bounding box")}</option>
          <option value="keypoints">{t("Keypoints")}</option>
          <option value="polygon">{t("Polygon")}</option>
          <option value="semantic_mask">{t("Semantic mask")}</option>
        </select></label>
        <label>{t("Labels to use")}<input value={labels} onChange={(event) => setLabels(event.target.value)} placeholder="football, training ball" /></label>
        <div className="label-output-preview"><span>{t("Output")}</span><strong>{outputName(kind)}</strong></div>
        <details className="advanced-settings"><summary>{t("Attributes and internal settings")}</summary>
          <div className="form-grid">
            <label>{t("Optional attribute")}<input value={attributeName} onChange={(event) => setAttributeName(event.target.value)} placeholder="occluded" /></label>
            <label>{t("Attribute type")}<select value={attributeKind} onChange={(event) => setAttributeKind(event.target.value as typeof attributeKind)}>
              <option value="string">{t("Text")}</option><option value="boolean">{t("Boolean")}</option><option value="number">{t("Number")}</option><option value="enum">{t("Choice")}</option>
            </select></label>
          </div>
          <small>{t("The internal task ID is generated from the display name and validated by Core. Raw Schema fields remain an Advanced concern.")}</small>
        </details>
        <button className={project.annotation_schema.length === 0 ? "primary form-submit-action" : "form-submit-action"} disabled={busy || !displayName.trim() || !labels.trim()} onClick={create}>{busy ? t("Adding…") : t("Add Label group")}</button>
        </div>
      </Panel>
      <Panel title={t("Current Labels")} eyebrow={`${project.task_count} groups`}>
        <div className="label-definition-list">
          {project.annotation_schema.map((task) => (
            <article key={task.id}><span><strong>{kindName(task.kind)}</strong><small>{task.display_name}</small></span><dl><div><dt>{t("Labels")}</dt><dd>{task.labels.join(", ") || t("None")}</dd></div><div><dt>{t("Output")}</dt><dd>{outputName(task.kind)}</dd></div></dl><details><summary>{t("Advanced internal ID")}</summary><code>{task.id}</code></details></article>
          ))}
        </div>
        {project.annotation_schema.length === 0 && <Empty title={t("No Labels defined")} detail="Create the first semantic Label group. Models and execution order belong in Pipeline." />}
      </Panel>
      </div>
    </>
  );
}

function BuildTestPublish({
  project,
  guided = false,
  sampleOperationId,
  requestNewSample = false,
  onAdopt,
  onImprove,
  onSampleOperation,
  selectedDraftId,
  selectedSampleTestId,
  selectedSampleImageId,
  onNavigationGuardChange,
  onSelectTestContext,
  onNavigate,
  onOpenRuns,
  onRefresh,
  onError,
}: {
  project: ProjectSummary;
  guided?: boolean;
  sampleOperationId?: string;
  requestNewSample?: boolean;
  onAdopt?: (draftId: string, testId: string, imageId?: string) => void;
  onImprove?: (draftId: string, testId: string, imageId?: string) => void;
  onSampleOperation?: (id?: string) => void;
  selectedDraftId?: string;
  selectedSampleTestId?: string;
  selectedSampleImageId?: string;
  onNavigationGuardChange: (guard?: () => boolean) => void;
  onSelectTestContext: (draftId: string, sampleTestId?: string, replace?: boolean, imageId?: string) => void;
  onNavigate: (step: BuildStep, draftId?: string, replace?: boolean) => void;
  onOpenRuns: (batchId?: string) => void;
  onRefresh: () => Promise<void>;
  onError: (value: string) => void;
}) {
  const [drafts, setDrafts] = useState<WorkflowDraft[]>([]);
  const [draftId, setDraftId] = useState("");
  const [activeSampleTest, setActiveSampleTest] = useState<{
    draftId: string;
    id: string;
  }>();
  const [sampleCount, setSampleCount] = useState(3);
  const [report, setReport] = useState<WorkflowDryRunReport>();
  const [reportLoading, setReportLoading] = useState(false);
  const [staleReport, setStaleReport] = useState(false);
  const [restoredAt, setRestoredAt] = useState<string>();
  const [images, setImages] = useState<ImageItem[]>([]);
  const [activated, setActivated] = useState<{ workflow_id: string; version: number }>();
  const [busy, setBusy] = useState(false);
  const [startingRun, setStartingRun] = useState(false);
  const inspectedPosition = report?.sample_inputs?.findIndex((input) => input.image_id === selectedSampleImageId) ?? -1;
  const inspectedSampleIndex = inspectedPosition >= 0 ? report?.samples[inspectedPosition]?.image_index : undefined;
  const setInspectedSampleIndex = (index: number | undefined) => {
    const position = report?.samples.findIndex((sample) => sample.image_index === index) ?? -1;
    const imageId = position >= 0 ? report?.sample_inputs?.[position]?.image_id : undefined;
    onSelectTestContext(draftId, activeSampleTest?.id, false, imageId);
  };
  const sampleLoadGeneration = useRef(0);
  const testPending = useRef(false);
  const testPageMounted = useRef(true);
  useEffect(() => { testPageMounted.current = true; return () => { testPageMounted.current = false; }; }, [project.id]);
  const load = (selectFallback = true) => workspaceQueries.load(
    queryKeys.workflowDrafts(project.id),
    (signal) => api.workflowDrafts(project.id, signal),
    { force: true },
  ).then((value) => {
    setDrafts(value.drafts);
    const available = value.drafts.filter((draft) => draft.status !== "archived");
    setDraftId((current) => {
      const requested = available.find((draft) => draft.id === selectedDraftId)?.id;
      const retained = available.find((draft) => draft.id === current)?.id;
      const restored = available.find(
        (draft) => draft.id === value.latest_current_sample_test_draft_id,
      )?.id;
      return requested ?? (guided ? "" : retained ?? (selectFallback ? restored ?? available[0]?.id ?? "" : ""));
    });
  });
  useEffect(() => {
    const imageKey = queryKeys.projectImages(project.id);
    void Promise.all([
      load(),
      workspaceQueries.load(
        imageKey,
        (signal) => api.images(project.id, signal),
        { staleTime: 30_000 },
      ).then((value) => setImages(value.images)),
    ])
      .catch((error: Error) => onError(error.message));
    return () => {
      workspaceQueries.abort(queryKeys.workflowDrafts(project.id));
      workspaceQueries.abort(imageKey);
    };
  }, [project.id]);
  useEffect(() => {
    if (draftId && draftId !== selectedDraftId) onSelectTestContext(draftId, undefined, true);
  }, [draftId, selectedDraftId]);
  useEffect(() => {
    if (draftId && selectedSampleTestId)
      setActiveSampleTest({ draftId, id: selectedSampleTestId });
  }, [draftId, selectedSampleTestId]);
  useEffect(() => {
    const generation = ++sampleLoadGeneration.current;
    setReport(undefined);
    setRestoredAt(undefined);
    setStaleReport(false);
    if (!draftId || (guided && (sampleOperationId || requestNewSample))) {
      setReportLoading(false);
      return;
    }
    const key = queryKeys.sampleTest(draftId);
    setReportLoading(true);
    void workspaceQueries.load(
      key,
      (signal) => api.workflowSampleTest(draftId, signal, selectedSampleTestId),
      { force: true },
    )
      .then(({ sample_test: sampleTest, current }) => {
        if (generation !== sampleLoadGeneration.current) return;
        if (sampleTest) {
          setReport({ ...sampleTest.report, sample_inputs: sampleTest.inputs });
          setStaleReport(!current);
          setRestoredAt(sampleTest.completed_at);
          setActiveSampleTest({ draftId, id: sampleTest.id });
          if (sampleTest.id !== selectedSampleTestId)
            onSelectTestContext(draftId, sampleTest.id, true);
        } else {
          setStaleReport(Boolean(sampleTest));
        }
      })
      .catch((error: Error) => {
        if (generation === sampleLoadGeneration.current && !isAbortError(error)) onError(error.message);
      })
      .finally(() => {
        if (generation === sampleLoadGeneration.current) setReportLoading(false);
      });
    return () => workspaceQueries.abort(key);
  }, [draftId, selectedSampleTestId, guided, sampleOperationId, requestNewSample]);
  useEffect(() => {
    if (guided && !sampleOperationId && !requestNewSample && report && !selectedSampleImageId && report.sample_inputs?.[0])
      onSelectTestContext(draftId, activeSampleTest?.id, true, report.sample_inputs[0].image_id);
  }, [guided, report, selectedSampleImageId, draftId, activeSampleTest?.id, sampleOperationId, requestNewSample]);
  const test = (expectedRevision?: number, count = sampleCount, authorizationFingerprint?: string) => {
    if (!draftId || testPending.current) return;
    testPending.current = true;
    setBusy(true);
    void api.dryRunWorkflow(draftId, Array.from({ length: count }, (_, index) => index), expectedRevision, authorizationFingerprint)
      .then((value) => {
        if (!testPageMounted.current) return;
        setActivated(undefined);
        setReport(value);
        setRestoredAt(undefined);
        setStaleReport(false);
        if (value.sample_test_id) {
          setActiveSampleTest({ draftId, id: value.sample_test_id });
          onSelectTestContext(draftId, value.sample_test_id, true);
        }
      })
      .then(() => testPageMounted.current ? load() : undefined)
      .catch((error: Error) => { if (testPageMounted.current) onError(error.message); })
      .finally(() => { testPending.current = false; setBusy(false); });
  };
  const publish = () => {
    if (!draftId || staleReport || !report?.validation.valid || drafts.find((draft) => draft.id === draftId)?.status === "published") return;
    setBusy(true);
    void api.publishWorkflow(draftId)
      .then((version) => {
        setActivated(version);
        return Promise.all([onRefresh(), load(false)]).then(() => {
          if (activeSampleTest?.draftId === draftId)
            onSelectTestContext(draftId, activeSampleTest.id, true);
        });
      })
      .catch((error: Error) => onError(error.message))
      .finally(() => setBusy(false));
  };
  const discard = () => {
    if (!draftId) return;
    setBusy(true);
    void api.archiveWorkflowDraft(draftId).then(() => { setReport(undefined); return load(); }).catch((error: Error) => onError(error.message)).finally(() => setBusy(false));
  };
  const summary = report?.summary;
  const resultCount = report?.samples.reduce((total, sample) => total + sample.result_count, 0) ?? 0;
  const uncertainSamples = report?.samples.filter((sample) => sample.review_count > 0 || sample.failed) ?? [];
  const needsAttention = (summary?.needs_review_count ?? 0) + (summary?.failed_count ?? 0);
  const fullRun = summary?.estimated_full_run;
  const hasUnknownRemoteModelCost = report?.samples.some((sample) => sample.nodes.some((node) => (
    asModelInputTrace(node)
    && node.metadata?.provider !== "rust_plugin"
    && (!node.estimated_cost || Number(node.estimated_cost) === 0)
  ))) ?? false;
  const sampleLimit = Math.min(10, images.length);
  const currentDraft = drafts.find((draft) => draft.id === draftId);
  const inspectedSample = report?.samples.find(
    (sample) => sample.image_index === inspectedSampleIndex,
  );
  const sampleImage = (sample: WorkflowDryRunReport["samples"][number]) => {
    const position = report?.samples.indexOf(sample) ?? -1;
    const input = report?.sample_inputs?.[position];
    return input ? images.find((item) => item.image_id === input.image_id && item.content_hash === input.content_hash) : undefined;
  };
  const configuredRefiners = currentDraft?.nodes.filter(
    (node) =>
      node.kind === "refiner" ||
      node.node_type.toLowerCase().includes("segment"),
  ) ?? [];
  const isActivated = currentDraft?.status === "published";
  const publishedWorkflowVersion = project.available_workflow_versions.find(
    (workflow) => workflow.source === `published draft ${draftId}` && workflow.status === "published",
  );
  const publishedWorkflow = activated ?? (publishedWorkflowVersion
    ? {
        workflow_id: publishedWorkflowVersion.workflow_id,
        version: Number(publishedWorkflowVersion.version),
      }
    : undefined);
  const startFullRun = () => {
    if (!publishedWorkflow || project.active_batch || project.active_run) return;
    setStartingRun(true);
    void api
      .startBatch(project.id, undefined, publishedWorkflow)
      .then(async ({ batch }) => {
        await onRefresh();
        onOpenRuns(batch.id);
      })
      .catch((error: Error) => onError(error.message))
      .finally(() => setStartingRun(false));
  };
  const chooseDraft = (nextDraftId: string) => {
    setDraftId(nextDraftId);
    setReport(undefined);
    setRestoredAt(undefined);
    setStaleReport(false);
    setActivated(undefined);
    setActiveSampleTest(undefined);
    if (nextDraftId) onSelectTestContext(nextDraftId, undefined, true);
  };
  useEffect(() => {
    setSampleCount((current) => Math.min(current, Math.max(1, sampleLimit)));
  }, [sampleLimit]);
  const draftControls = <div className="sample-test-controls" aria-label={t("Sample Test controls")}>
    <label className="sample-test-field"><span>{t("Automation Draft")}</span><select aria-label={t("Current Draft")} value={draftId} onChange={(event) => chooseDraft(event.target.value)}><option value="">{t("Choose Current Draft…")}</option>{drafts.filter((draft) => draft.status !== "archived").map((draft) => <option key={draft.id} value={draft.id}>{draft.name} · {draft.status === "published" ? t("Activated") : draft.status.replaceAll("_", " ")}</option>)}</select></label>
    <label className="sample-test-field"><span>{t("Sample images")}</span><input type="number" min="1" max={Math.max(1, sampleLimit)} value={sampleCount} disabled={sampleLimit === 0} aria-describedby="sample-image-limit" onChange={(event) => setSampleCount(Math.max(1, Math.min(Math.max(1, sampleLimit), Number(event.target.value))))} /><small id="sample-image-limit">{sampleLimit ? t("Up to {count} available Project images", { count: sampleLimit }) : t("No Project images are available")}</small></label>
    <button className={!report && sampleLimit > 0 ? "primary" : ""} onClick={() => test()} disabled={busy || reportLoading || !draftId || isActivated || sampleLimit === 0}>{busy ? t("Testing…") : isActivated ? t("Already activated") : sampleLimit === 0 ? t("Add images first") : report ? t("Test again") : t("Test samples")}</button>
    {sampleLimit === 0 && <button onClick={() => onNavigate("data")}>{t("Open Data step")}</button>}
  </div>;
  if (selectedSampleImageId && reportLoading) return <p role="status">{t("Restoring the saved Sample Test…")}</p>;
  if (selectedSampleImageId && !reportLoading && !inspectedSample) return <section role="alert"><h2>{t("Sample image unavailable")}</h2><p>{t("This image is not part of the selected saved Sample Test. No other result was substituted.")}</p><button onClick={() => setInspectedSampleIndex(undefined)}>{t("View all sample images")}</button></section>;
  if (inspectedSample && staleReport) {
    const image = sampleImage(inspectedSample);
    const savedAnnotations: Annotation[] = image && inspectedSample.projection ? inspectedSample.outcomes.flatMap((outcome) => outcome.value ? [{ id: outcome.id, image_id: image.image_id, task_id: "sample", label: outcome.label, value: outcome.value, attributes: {}, source: "saved sample", review_status: "needs_review" as const, provenance: {}, created_at: "" }] : []) : [];
    return <section className="journey-scene journey-results">
      <div className="journey-intro"><h2>{t("Sample Test is out of date")}</h2><p role="alert">{t("Sample scope cannot be verified. The plan, images or models may have changed, or this older test lacks a model snapshot. Saved results are read-only and cannot authorize activation.")}</p></div>
      <p>{inspectedSample.image_name} · {t("Sandbox sample")} · {inspectedPosition + 1}/{report?.samples.length}</p>
      <button onClick={() => setInspectedSampleIndex(undefined)}>{t("View all sample images")}</button>
      {image ? <AnnotationCanvas key={image.image_id} compactList imageUrl={image.url} annotations={savedAnnotations} readOnly onSelect={() => undefined} onChange={() => undefined} /> : <p>{t("The original image is unavailable. No substitute result is shown.")}</p>}
      <footer className="journey-actions"><button disabled={inspectedPosition <= 0} onClick={() => setInspectedSampleIndex(report!.samples[inspectedPosition - 1].image_index)}>{t("Previous image")}</button><button disabled={inspectedPosition + 1 >= (report?.samples.length ?? 0)} onClick={() => setInspectedSampleIndex(report!.samples[inspectedPosition + 1].image_index)}>{t("Next image")}</button>{guided && <button className="primary" onClick={() => onSampleOperation?.(undefined)}>{t("Review new sample scope")}</button>}</footer>
    </section>;
  }
  if (inspectedSample) return <SampleAnnotationDialog key={`${activeSampleTest?.id}:${selectedSampleImageId}`}
    projectId={project.id} draftId={draftId} onImprove={onImprove} onKeepOriginal={(id, test, image) => onSelectTestContext(id, test, false, image)}
    onAdopt={guided && activeSampleTest ? () => onAdopt?.(draftId, activeSampleTest.id, selectedSampleImageId) : undefined}
    guided={guided}
    sample={inspectedSample} image={sampleImage(inspectedSample)} configuredRefiners={configuredRefiners}
    sampleTestId={activeSampleTest?.draftId === draftId ? activeSampleTest.id : undefined}
    onClose={() => setInspectedSampleIndex(undefined)} onNavigationGuardChange={onNavigationGuardChange}
    position={inspectedPosition + 1} count={report?.samples.length ?? 0}
    onPrevious={inspectedPosition > 0 ? () => setInspectedSampleIndex(report!.samples[inspectedPosition - 1].image_index) : undefined}
    onNext={inspectedPosition + 1 < (report?.samples.length ?? 0) ? () => setInspectedSampleIndex(report!.samples[inspectedPosition + 1].image_index) : undefined}
  />;
  if (guided) return reportLoading ? <p role="status">{t("Restoring the saved Sample Test…")}</p> : <JourneySampleTask projectId={project.id} draftId={draftId} operationId={sampleOperationId} images={images} stale={staleReport} onOperation={(id) => onSampleOperation?.(id)} onComplete={(id) => onSelectTestContext(draftId, id, true)} onBack={() => onNavigate("labels", draftId)} />;
  return (
    <>
      <div className="toolbar-panel sample-test-toolbar">
        <div className="sample-test-toolbar-copy"><span className="eyebrow">{t("Step 4 · Test & Activate")}</span><h2>{t("Test samples, then activate automation")}</h2><p>A Sample Test executes up to 10 available Project images in a sandbox and never writes formal annotations. Activation publishes the tested Draft as an immutable Version.</p></div>
        <button className="sample-test-back" onClick={() => onNavigate("pipeline", draftId)}>{t("← Edit Automation")}</button>
      {draftControls}
      {staleReport && <p role="alert">{t("Sample scope cannot be verified. The plan, images or models may have changed, or this older test lacks a model snapshot. Saved results are read-only and cannot authorize activation.")}</p>}
      </div>
      {!report && <ol className="activation-lifecycle" aria-label={t("Automation activation lifecycle")}>
        <li className={draftId ? "complete" : "current"}><span>1</span><strong>{draftId ? t("Unpublished changes") : t("Choose a Draft")}</strong></li>
        <li className={draftId ? "current" : ""}><span>2</span><strong>{t("Check setup")}</strong></li>
        <li><span>3</span><strong>{t("Test samples")}</strong></li>
        <li><span>4</span><strong>{t("Activate automation")}</strong></li>
      </ol>}
      {summary ? (
        <>
          <section className={`sample-test-hero ${report.validation.valid ? "ready" : "blocked"}`} aria-label={t("Dry Run result summary")}>
            <div className="sample-test-hero-copy">
              <span className="eyebrow">{staleReport ? t("Sample Test is out of date") : isActivated ? t("Activated evidence") : report.validation.valid ? t("Ready to activate") : t("Automation needs changes")}</span>
              <h2>{t("Sample test complete")}</h2>
              <p>AnnotAgent tested real Project images in a sandbox. No formal Annotations were written.</p>
            </div>
            <dl className="sample-outcome-metrics">
              <div><dt>{t("Images")}</dt><dd>{summary.image_count}</dd><small>{t("tested")}</small></div>
              <div><dt>{t("Results found")}</dt><dd>{resultCount}</dd><small>{summary.auto_accepted_count}{" "}{t("ready to accept")}</small></div>
              <div><dt>{t("Needs attention")}</dt><dd>{needsAttention}</dd><small>{summary.needs_review_count}{" "}{t("review ·")}{" "}{summary.failed_count}{" "}{t("failed")}</small></div>
            </dl>
            <div className="sample-test-context">
              <span>{summary.empty_count}{" "}{t("no-target result")}{summary.empty_count === 1 ? "" : t("s")}</span>
              <span>{summary.fallback_count}{" "}{t("fallback")}{summary.fallback_count === 1 ? "" : t("s")}</span>
              <span>{summary.cache_hit_count}{" "}{t("cache hit")}{summary.cache_hit_count === 1 ? "" : t("s")}</span>
              <span>{formatSampleDuration(summary.duration_ms)}</span>
              <span>{hasUnknownRemoteModelCost ? t("Sample cost unknown") : `$${summary.usage.estimated_cost} sample cost`}</span>
              {restoredAt && <span title={new Date(restoredAt).toLocaleString(localeTag())}>{t("Restored saved Sample Test")}</span>}
            </div>
            {isActivated ? <div className="activation-success" role="status"><span><strong>{t("Automation activated")}</strong><small>{t("This saved Sample Test belongs to the immutable active Version.")}</small></span><button className="primary" disabled={!publishedWorkflow || startingRun || Boolean(project.active_batch || project.active_run)} onClick={startFullRun}>{startingRun ? t("Starting…") : project.active_batch || project.active_run ? t("Run already active") : t("Start full Run")}</button></div> : <>
              <div className="button-row">
                {!report.validation.valid || summary.failed_count > 0 ? <button className="primary" onClick={() => onNavigate("pipeline", draftId)}>{t("Fix automation")}</button> : summary.needs_review_count > 0 ? <button className="primary" onClick={() => document.getElementById("uncertain-results")?.scrollIntoView({ behavior: "smooth", block: "start" })}>{t("Inspect uncertain samples")}</button> : <button className="primary" onClick={publish} disabled={busy || staleReport || Boolean(activated)}>{busy ? t("Activating…") : t("Activate automation")}</button>}
                {report.validation.valid && summary.needs_review_count > 0 && <button onClick={publish} disabled={busy || staleReport || Boolean(activated)}>{busy ? t("Activating…") : t("Activate with Review gate")}</button>}
              </div>
              {activated && <div className="activation-success" role="status"><span><strong>{t("Automation activated")}</strong><small>{t("Immutable Version v")}{activated.version}{" "}{t("is ready for the full Dataset Run.")}</small></span><button className="primary" disabled={startingRun || Boolean(project.active_batch || project.active_run)} onClick={startFullRun}>{startingRun ? t("Starting…") : project.active_batch || project.active_run ? t("Run already active") : t("Start full Run")}</button></div>}
            </>}
          </section>
          {fullRun && <section className="full-run-estimate" aria-label={t("Full Run Estimate")}>
            <div><span className="eyebrow">{t("Full Run Estimate")}</span><h2>{fullRun.image_count}{" "}{t("Project images")}</h2><p>{t("Projected from this Sample Test; actual usage can vary with image content and Provider behavior.")}</p></div>
            <dl><div><dt>{t("Estimated cost")}</dt><dd>{hasUnknownRemoteModelCost ? t("Unknown") : `$${fullRun.estimated_cost}`}</dd></div><div><dt>{t("Estimated duration")}</dt><dd>{formatSampleDuration(fullRun.duration_ms)}</dd></div><div><dt>{t("Review workload")}</dt><dd>{fullRun.review_count_min === fullRun.review_count_max ? fullRun.review_count_min : `${fullRun.review_count_min}–${fullRun.review_count_max}`}{" "}{t("results")}</dd></div></dl>
          </section>}
          <SampleExecutionSummary
            report={report}
            sampleTestId={activeSampleTest?.draftId === draftId ? activeSampleTest.id : undefined}
          />
          <section className="sample-results-section" aria-labelledby="sample-results-title">
            <div className="section-heading"><div><span className="eyebrow">{t("Results Gallery")}</span><h2 id="sample-results-title">{t("What the automation found")}</h2></div><small>{summary.image_count} sandbox image{summary.image_count === 1 ? "" : t("s")}</small></div>
            <div className="sample-results-gallery">{report.samples.map((sample) => <SampleResultCard key={`${sample.image_index}-${sample.image_name}`} sample={sample} image={sampleImage(sample)} onInspect={() => setInspectedSampleIndex(sample.image_index)} />)}</div>
          </section>
          <section className="sample-results-section uncertain-results" id="uncertain-results" aria-labelledby="uncertain-results-title">
            <div className="section-heading"><div><span className="eyebrow">{t("Uncertain Results")}</span><h2 id="uncertain-results-title">{t("What needs a human decision")}</h2></div><small>{uncertainSamples.length}{" "}{t("image")}{uncertainSamples.length === 1 ? "" : t("s")}</small></div>
            {uncertainSamples.length ? <div className="sample-results-gallery">{uncertainSamples.map((sample) => <SampleResultCard key={`uncertain-${sample.image_index}-${sample.image_name}`} sample={sample} image={sampleImage(sample)} compact onInspect={() => setInspectedSampleIndex(sample.image_index)} />)}</div> : <div className="positive-empty"><strong>{t("No uncertain results in this sample")}</strong><span>{t("The configured confidence and Review gates accepted every result.")}</span></div>}
          </section>
          <section className="sample-diagnostics" aria-label={t("Sample Test diagnostics")}>
            <div className="section-heading"><div><span className="eyebrow">{t("Diagnostics")}</span><h2>{t("Inspect only when you need to troubleshoot")}</h2></div></div>
            <details><summary>{t("Pipeline Diagnostics")}</summary><div>{report.validation.issues.map((issue) => <div className="error-banner" key={`${issue.path}-${issue.code}`}><span>{issue.code}: {issue.message}</span></div>)}{!report.validation.issues.length && <p>{t("No blocking static or execution issues.")}</p>}</div></details>
            <details><summary>{t("Model Usage")}</summary><dl className="diagnostic-facts"><div><dt>{t("Input tokens")}</dt><dd>{summary.usage.input_tokens.toLocaleString(localeTag())}</dd></div><div><dt>{t("Output tokens")}</dt><dd>{summary.usage.output_tokens.toLocaleString(localeTag())}</dd></div><div><dt>{t("Estimated cost")}</dt><dd>{hasUnknownRemoteModelCost ? t("Unknown") : `$${summary.usage.estimated_cost}`}</dd></div></dl></details>
            <details><summary>{t("Node Timings")}</summary>{report.samples.map((sample) => <div className="diagnostic-sample" key={`timing-${sample.image_index}`}><strong>{sample.image_name}</strong>{sample.nodes.map((node) => <span key={node.node_id}>{node.node_id}<small>{node.latency_ms} ms · {node.status}</small></span>)}</div>)}</details>
            <details><summary>{t("Technical Artifacts")}</summary>{report.samples.map((sample) => <div className="diagnostic-sample" key={`artifacts-${sample.image_index}`}><strong>{sample.image_name}</strong>{sample.nodes.filter((node) => node.output_types.length).map((node) => <span key={node.node_id}>{node.node_id}<small>{node.output_types.join(", ")}</small></span>)}</div>)}</details>
          </section>
        </>
      ) : reportLoading ? <div className="loading-banner" role="status">{t("Restoring the saved Sample Test…")}</div> : staleReport ? <Empty title={t("Sample Test is out of date")} detail={t("This Draft changed after its saved Sample Test. Test the current Draft again before activation.")} /> : <Empty title={t("No Sample Test result")} detail={t("Choose a Current Draft and test 1–10 images to see result counts, diagnostics, and trace.")} />}
      {!isActivated && <details className="advanced-settings"><summary>{t("Discard this Draft")}</summary><p>Archiving removes this unpublished Draft from the active Build flow. Published Versions are never changed.</p><button onClick={discard} disabled={busy || !draftId}>{t("Discard unpublished changes")}</button></details>}
    </>
  );
}

function formatSampleDuration(durationMs: number) {
  if (durationMs < 1_000) return `${durationMs} ms`;
  if (durationMs < 60_000) return `${(durationMs / 1_000).toFixed(durationMs < 10_000 ? 1 : 0)} sec`;
  return `${Math.ceil(durationMs / 60_000)} min`;
}

type SampleNodeResult = WorkflowDryRunReport["samples"][number]["nodes"][number];

type ModelInputTraceView = {
  source_region_pixels: number[];
  crop_dimensions: number[];
  submitted_dimensions: number[];
  submitted_image_sha256: string;
  normalized_pixel_digest: string;
  interpolation: string;
  color_format: string;
  letterbox_padding: number[];
  provider_effective_dimensions?: unknown;
  transform_to_original?: {
    source_region: number[];
    content_region: number[];
  };
};

type ModelInputOverlay = {
  id: string;
  label: string;
  rect: [number, number, number, number];
  tone: "prediction" | "final";
};

type ModelInputInspection = {
  imageName: string;
  node: SampleNodeResult;
  trace: ModelInputTraceView;
  url: string;
  overlays: ModelInputOverlay[];
};

function asModelInputTrace(node: SampleNodeResult): ModelInputTraceView | undefined {
  const value = node.metadata?.model_input_trace;
  if (!value || typeof value !== "object" || Array.isArray(value)) return undefined;
  const trace = value as Partial<ModelInputTraceView>;
  if (!Array.isArray(trace.source_region_pixels)
    || !Array.isArray(trace.crop_dimensions)
    || !Array.isArray(trace.submitted_dimensions)
    || typeof trace.submitted_image_sha256 !== "string"
    || typeof trace.normalized_pixel_digest !== "string") return undefined;
  return trace as ModelInputTraceView;
}

function metadataText(value: unknown, fallback = "Not reported") {
  if (value == null || value === "") return fallback;
  if (typeof value === "string" || typeof value === "number" || typeof value === "boolean") return String(value);
  if (typeof value === "object" && !Array.isArray(value)) {
    const record = value as Record<string, unknown>;
    if (record.status === "observed" || record.status === "estimated") return `${record.status} · ${record.width}×${record.height}`;
    if (record.status === "unknown") return "Unknown";
  }
  return JSON.stringify(value);
}

function sampleModelInputUrl(sampleTestId: string, imageIndex: number, nodeId: string) {
  return `/api/workflow-sample-tests/${encodeURIComponent(sampleTestId)}/samples/${imageIndex}/nodes/${encodeURIComponent(nodeId)}/model-input`;
}

function modelCallCost(node: SampleNodeResult) {
  if (node.metadata?.provider === "rust_plugin") return "$0 local";
  if (!node.estimated_cost || Number(node.estimated_cost) === 0) return "Unknown";
  return `$${node.estimated_cost}`;
}

function clampUnit(value: number) {
  return Math.max(0, Math.min(1, value));
}

export function projectOriginalRectToSubmitted(
  rect: [number, number, number, number],
  trace: ModelInputTraceView,
): [number, number, number, number] | undefined {
  const source = trace.transform_to_original?.source_region;
  const content = trace.transform_to_original?.content_region;
  if (!source || source.length !== 4 || !content || content.length !== 4
    || source[2] <= 0 || source[3] <= 0) return undefined;
  const left = Math.max(rect[0], source[0]);
  const top = Math.max(rect[1], source[1]);
  const right = Math.min(rect[0] + rect[2], source[0] + source[2]);
  const bottom = Math.min(rect[1] + rect[3], source[1] + source[3]);
  if (right <= left || bottom <= top) return undefined;
  const x = content[0] + ((left - source[0]) / source[2]) * content[2];
  const y = content[1] + ((top - source[1]) / source[3]) * content[3];
  const width = ((right - left) / source[2]) * content[2];
  const height = ((bottom - top) / source[3]) * content[3];
  return [clampUnit(x), clampUnit(y), clampUnit(width), clampUnit(height)];
}

function modelInputOverlays(
  sample: WorkflowDryRunReport["samples"][number],
  node: SampleNodeResult,
  trace: ModelInputTraceView,
): ModelInputOverlay[] {
  const direct = (sample.projection?.debug_stages ?? []).flatMap((stage) => {
    if (stage.node_id !== node.node_id || stage.value?.kind !== "bounding_box") return [];
    return [{
      id: `prediction-${stage.artifact_id}-${stage.lineage_id}`,
      label: `${stage.label ?? "Object"} · node output`,
      rect: stage.value.rect,
      tone: "prediction" as const,
    }];
  });
  const final = sample.outcomes.flatMap((outcome) => {
    if (outcome.value?.kind !== "bounding_box") return [];
    const rect = projectOriginalRectToSubmitted(outcome.value.rect, trace);
    return rect ? [{
      id: `final-${outcome.id}`,
      label: `${outcome.label} · final`,
      rect,
      tone: "final" as const,
    }] : [];
  });
  return [...direct, ...final];
}

function SampleExecutionSummary({
  report,
  sampleTestId,
}: {
  report: WorkflowDryRunReport;
  sampleTestId?: string;
}) {
  const [inspectedInput, setInspectedInput] = useState<ModelInputInspection>();
  const observed = report.samples.some((sample) => sample.nodes.some((node) => (
    asModelInputTrace(node)
    || node.metadata?.selected_route != null
    || node.metadata?.requested_route != null
  )));
  return <>
    <section className="sample-execution-summary" aria-labelledby="sample-execution-title">
    <div className="section-heading">
      <div><span className="eyebrow">{t("Execution Evidence")}</span><h2 id="sample-execution-title">{t("What actually ran")}</h2></div>
      <small>{observed ? t("Persisted node facts") : t("No execution trace available")}</small>
    </div>
    {!observed ? <div className="sample-execution-empty"><strong>{t("This result has no model-input trace.")}</strong><span>Run this Draft again to record exact submitted pixels, recovery routes, and refiner execution.</span></div> : <div className="sample-execution-grid">
      {report.samples.map((sample) => {
        const calls = sample.nodes.flatMap((node) => {
          const trace = asModelInputTrace(node);
          return trace ? [{ node, trace }] : [];
        });
        const routes = sample.nodes.filter((node) => node.metadata?.selected_route != null || node.metadata?.requested_route != null);
        return <article className="sample-execution-card" key={`execution-${sample.image_index}`}>
          <header><div><strong>{sample.image_name}</strong><small>{calls.length} model call{calls.length === 1 ? "" : t("s")} with input evidence</small></div><Status status={sample.failed ? "Failed" : sample.review_count ? "Needs review" : "Completed"} /></header>
          {calls.length ? <div className="model-input-evidence-list">{calls.map(({ node, trace }) => <article key={node.node_id} className="model-input-evidence">
            {sampleTestId ? <button
              type="button"
              className="model-input-evidence-preview"
              aria-label={`Open actual image submitted to ${node.node_id}`}
              onClick={() => setInspectedInput({
                imageName: sample.image_name,
                node,
                trace,
                url: sampleModelInputUrl(sampleTestId, sample.image_index, node.node_id),
                overlays: modelInputOverlays(sample, node, trace),
              })}
            ><img src={sampleModelInputUrl(sampleTestId, sample.image_index, node.node_id)} alt="" /><span>{t("Actual submitted image")}<small>{t("Open full size")}</small></span></button> : <div className="model-input-placeholder">{t("Saved preview unavailable")}</div>}
            <div className="model-input-evidence-body">
              <header><strong>{metadataText(node.metadata?.model, node.node_id)}</strong><small>{metadataText(node.metadata?.provider, "Model backend")}</small></header>
              <dl>
                <div><dt>{t("Node")}</dt><dd>{node.node_id}</dd></div>
                <div><dt>{t("Source pixels")}</dt><dd>{trace.source_region_pixels.join(" × ")}</dd></div>
                <div><dt>{t("Crop → submitted")}</dt><dd>{trace.crop_dimensions.join("×")} → {trace.submitted_dimensions.join("×")}</dd></div>
                <div><dt>{t("Preprocessing")}</dt><dd>{trace.interpolation} · {trace.color_format}</dd></div>
                <div><dt>{t("Provider effective")}</dt><dd>{metadataText(trace.provider_effective_dimensions, "Unknown")}</dd></div>
                <div><dt>{t("Latency · cost")}</dt><dd>{node.latency_ms} ms · {modelCallCost(node)}</dd></div>
              </dl>
              <code title={trace.submitted_image_sha256}>Input SHA · {trace.submitted_image_sha256.slice(0, 16)}…</code>
            </div>
          </article>)}</div> : <p className="sample-execution-note">No model call with a persisted input trace executed for this image.</p>}
          <div className="recovery-route-summary">
            <strong>{t("Recovery and refinement")}</strong>
            {routes.length ? routes.map((node) => <div key={`route-${node.node_id}`}><span><b>{node.node_id}</b><small>attempt {metadataText(node.metadata?.recovery_attempt, "—")} / {metadataText(node.metadata?.maximum_recovery_attempts, "—")}</small></span><span><b>{metadataText(node.metadata?.selected_route, "No route")}</b><small>{metadataText(node.metadata?.failure_code, "No failure reason")}</small></span></div>) : <span className="sample-execution-note">{t("No recovery Gate executed.")}</span>}
            <small>{sample.nodes.some((node) => node.output_types.some((type) => String(type).toLowerCase().includes("mask"))) ? t("Prompted segmentation produced a Mask Artifact.") : t("No Mask Artifact was produced; a configured refiner may not have been reached.")}</small>
          </div>
        </article>;
      })}
    </div>}
    </section>
    {inspectedInput && <ModelInputPreviewDialog
      inspection={inspectedInput}
      onClose={() => setInspectedInput(undefined)}
    />}
  </>;
}

function ModelInputPreviewDialog({
  inspection,
  onClose,
}: {
  inspection: ModelInputInspection;
  onClose: () => void;
}) {
  useEffect(() => {
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [onClose]);
  return <div className="modal-backdrop model-input-preview-backdrop" onMouseDown={(event) => {
    if (event.target === event.currentTarget) onClose();
  }}>
    <section className="model-input-preview-dialog" role="dialog" aria-modal="true" aria-labelledby="model-input-preview-title">
      <header>
        <div><span className="eyebrow">{t("Submitted model input")}</span><h2 id="model-input-preview-title">{inspection.node.node_id}</h2><small>{inspection.imageName}</small></div>
        <button type="button" onClick={onClose} aria-label={t("Close model input preview")}>{t("Close")}</button>
      </header>
      <div className="model-input-preview-layout">
        <figure><div className="model-input-preview-canvas" style={{ aspectRatio: `${inspection.trace.submitted_dimensions[0]} / ${inspection.trace.submitted_dimensions[1]}` }}>
          <img src={inspection.url} alt={`Actual image submitted to ${inspection.node.node_id}`} />
          {inspection.overlays.map((overlay) => <span
            className={`model-input-overlay ${overlay.tone}`}
            key={overlay.id}
            style={{ left: `${overlay.rect[0] * 100}%`, top: `${overlay.rect[1] * 100}%`, width: `${overlay.rect[2] * 100}%`, height: `${overlay.rect[3] * 100}%` }}
          ><b>{overlay.label}</b></span>)}
        </div></figure>
        <aside>
          <div className="model-input-overlay-legend"><span>{t("Visible overlays")}</span>{inspection.overlays.length ? inspection.overlays.map((overlay) => <strong className={overlay.tone} key={`legend-${overlay.id}`}>{overlay.label}</strong>) : <strong>{t("No bbox is associated with this model call")}</strong>}<small>Overlays are drawn by AnnotAgent and were not part of the submitted image bytes.</small></div>
          <div><span>{t("Model")}</span><strong>{metadataText(inspection.node.metadata?.model, inspection.node.node_id)}</strong></div>
          <div><span>{t("Source region")}</span><strong>{inspection.trace.source_region_pixels.join(" × ")} px</strong></div>
          <div><span>{t("Crop → submitted")}</span><strong>{inspection.trace.crop_dimensions.join("×")} → {inspection.trace.submitted_dimensions.join("×")}</strong></div>
          <div><span>{t("Preprocessing")}</span><strong>{inspection.trace.interpolation} · {inspection.trace.color_format}</strong></div>
          <div><span>{t("Provider effective")}</span><strong>{metadataText(inspection.trace.provider_effective_dimensions, "Unknown")}</strong></div>
          <div><span>Submitted SHA-256</span><code>{inspection.trace.submitted_image_sha256}</code></div>
        </aside>
      </div>
    </section>
  </div>;
}

function SampleResultCard({
  sample,
  image,
  compact = false,
  onInspect,
}: {
  sample: WorkflowDryRunReport["samples"][number];
  image?: ImageItem;
  compact?: boolean;
  onInspect: () => void;
}) {
  const stages = sample.projection?.debug_stages ?? [];
  const hasTerminalProjection = Boolean(sample.projection && (
    sample.projection.final_candidates.length
    || sample.projection.review_candidates.length
    || sample.projection.committed_annotations.length
    || sample.projection.no_target
    || sample.projection.intermediate_artifact_ids.length
    || sample.projection.debug_stages.length
  ));
  const [selectedStage, setSelectedStage] = useState<ResultLineageStage>("final");
  const stageOrder: ResultLineageStage[] = ["coarse", "search_region", "relocalized", "prompt_coverage", "mask", "refined", "final"];
  const availableStages = stageOrder.filter((stage) => stage === "final" || stages.some((item) => item.stage === stage));
  const selectedStages = stages.filter((item) => item.stage === selectedStage);
  const visualResults = selectedStage === "final"
    ? sample.outcomes.map((outcome) => ({
        id: outcome.id,
        label: outcome.label,
        confidence: outcome.confidence,
        value: outcome.value,
        source: "Terminal projection",
        detail: outcome.status.replaceAll("_", " "),
      }))
    : selectedStages.map((stage, index) => ({
        id: `${stage.artifact_id}-${stage.lineage_id}-${index}`,
        label: stage.label ?? selectedStage.replaceAll("_", " "),
        confidence: stage.confidence,
        value: stage.value,
        source: stage.source,
        detail: stage.detail,
      }));
  const boxes = visualResults.filter((outcome) => outcome.value?.kind === "bounding_box");
  const terminalCandidates = [
    ...(sample.projection?.final_candidates ?? []),
    ...(sample.projection?.review_candidates ?? []).map((review) => review.candidate),
  ];
  const reviewExplanation = sample.projection?.review_candidates[0]?.explanation;
  const state = sample.failed ? "Failed" : sample.review_count ? "Needs review" : sample.empty ? "No target found" : "Ready";
  return <article className={`sample-result-card ${sample.failed ? "failed" : sample.review_count ? "review" : "ready"} ${compact ? "compact" : ""}`}>
    <figure className="sample-result-preview" style={{ aspectRatio: `${sample.width} / ${sample.height}` }}>
      {image ? <img src={image.url} alt={sample.image_name} /> : <div className="image-placeholder">{t("Preview unavailable")}</div>}
      {boxes.map((outcome) => {
        const rect = outcome.value?.kind === "bounding_box" ? outcome.value.rect : undefined;
        return rect ? <span className={`sample-result-box ${selectedStage === "final" ? "final" : "diagnostic"}`} key={outcome.id} style={{ left: `${rect[0] * 100}%`, top: `${rect[1] * 100}%`, width: `${rect[2] * 100}%`, height: `${rect[3] * 100}%` }}><b>{outcome.label}{outcome.confidence != null ? ` ${Math.round(outcome.confidence * 100)}%` : ""}</b></span> : null;
      })}
      <button className="sample-result-preview-trigger" type="button" onClick={onInspect} aria-label={`Open annotation preview for ${sample.image_name}`}>
        <span>{t("Open annotation preview")}</span>
      </button>
    </figure>
    <div className="sample-result-body">
      <header className="sample-result-heading"><strong>{sample.image_name}</strong><Status status={state} /></header>
      <p className="sample-result-summary">{sample.failed ? t("A Pipeline step failed on this image.") : sample.empty ? t("No target found. This is a valid empty result.") : `${sample.result_count} ${hasTerminalProjection ? "terminal" : "legacy"} result${sample.result_count === 1 ? "" : t("s")} · ${sample.auto_accepted_count} ready · ${sample.review_count} review`}</p>
      {!hasTerminalProjection && <aside className="sample-legacy-projection"><strong>{t("Legacy result aggregation")}</strong><span>This saved Sample Test predates terminal projection and may include intermediate detections. Test this Draft again to generate final-only Results and lineage Diagnostics.</span></aside>}
      {terminalCandidates.length > 0 && <div className="sample-terminal-facts">{terminalCandidates.map((candidate) => <article key={candidate.lineage_id}><header><strong>{candidate.outcome.label}</strong><small>{candidate.outcome.confidence != null ? `${Math.round(candidate.outcome.confidence * 100)}%` : t("Score not provided")}</small></header><dl><div><dt>{t("Localization")}</dt><dd>{candidate.localization}</dd></div><div><dt>{t("Geometry")}</dt><dd>{candidate.geometry}</dd></div><div><dt>{t("Final status")}</dt><dd>{candidate.final_status}</dd></div></dl></article>)}</div>}
      {!terminalCandidates.length && sample.outcomes.length > 0 && <ul className="sample-legacy-outcomes">{sample.outcomes.map((outcome) => <li key={`summary-${outcome.id}`}><span>{outcome.label}</span><small>{outcome.status.replaceAll("_", " ")}{outcome.confidence != null ? ` · ${Math.round(outcome.confidence * 100)}%` : ""}</small></li>)}</ul>}
      {reviewExplanation && <aside className="sample-result-explanation"><strong>{reviewExplanation.title}</strong><p>{reviewExplanation.summary}</p>{reviewExplanation.recommendation && <small>{reviewExplanation.recommendation}</small>}</aside>}
      {stages.length > 0 && <details className="sample-lineage-debug"><summary>Diagnostics · {stages.length} lineage stage{stages.length === 1 ? "" : t("s")}</summary><div className="sample-lineage-stage-tabs" role="tablist" aria-label={`Artifact lineage for ${sample.image_name}`}>{availableStages.map((stage) => <button key={stage} role="tab" aria-selected={selectedStage === stage} className={selectedStage === stage ? "active" : ""} onClick={() => setSelectedStage(stage)}>{stage === "search_region" ? t("Search region") : stage === "prompt_coverage" ? t("Prompt coverage") : stage[0].toUpperCase() + stage.slice(1)}</button>)}</div><div className="sample-lineage-stage-detail"><strong>{selectedStage === "final" ? t("Final terminal projection") : selectedStage.replaceAll("_", " ")}</strong>{selectedStage === "final" ? <span>{t("Only committed or current Review candidates appear here.")}</span> : selectedStages.length ? selectedStages.map((stage, index) => <span key={`${stage.artifact_id}-${index}`}><b>{stage.source}</b>{stage.detail ? ` · ${stage.detail}` : ""}<code>{stage.artifact_ref}</code></span>) : <span>{t("No Artifact was produced for this stage.")}</span>}</div></details>}
    </div>
  </article>;
}

function SampleAnnotationDialog({
  sample,
  projectId,
  draftId,
  onKeepOriginal,
  onImprove,
  guided = false,
  onAdopt,
  image,
  configuredRefiners,
  sampleTestId,
  onClose: closeDialog,
  onNavigationGuardChange, position, count, onPrevious, onNext,
}: {
  sample: WorkflowDryRunReport["samples"][number];
  projectId: string;
  draftId: string;
  onKeepOriginal: (draftId: string, testId: string, imageId?: string) => void;
  onImprove?: (draftId: string, testId: string, imageId?: string) => void;
  guided?: boolean;
  onAdopt?: () => void;
  image?: ImageItem;
  configuredRefiners: WorkflowDraftNode[];
  sampleTestId?: string;
  onClose: () => void;
  onNavigationGuardChange: (guard?: () => boolean) => void;
  position: number; count: number; onPrevious?: () => void; onNext?: () => void;
}) {
  const [feedbackDirty, setFeedbackDirty] = useState(false);
  const [showComparison, setShowComparison] = useState(false);
  const onClose = closeDialog;
  useEffect(() => {
    onNavigationGuardChange(() => !feedbackDirty || window.confirm(t("Discard unsaved sample feedback?")));
    return () => onNavigationGuardChange(undefined);
  }, [feedbackDirty, onNavigationGuardChange]);
  const stages = sample.projection?.debug_stages ?? [];
  const [selectedStage, setSelectedStage] = useState<ResultLineageStage>("final");
  const stageOrder: ResultLineageStage[] = ["coarse", "search_region", "relocalized", "prompt_coverage", "mask", "refined", "final"];
  const availableStages = stageOrder.filter(
    (stage) => stage === "final" || stages.some((item) => item.stage === stage),
  );
  const selectedStages = stages.filter((item) => item.stage === selectedStage);
  const visualResults = selectedStage === "final"
    ? sample.projection ? sample.outcomes : []
    : selectedStages.map((stage, index) => ({
        id: `${stage.artifact_id}-${stage.lineage_id}-${index}`,
        label: stage.label ?? selectedStage.replaceAll("_", " "),
        confidence: stage.confidence,
        value: stage.value,
      }));
  const boxes = visualResults.filter((result) => result.value?.kind === "bounding_box");
  const executedNodeIds = new Set(sample.nodes.map((node) => node.node_id));
  const reachedRefiners = configuredRefiners.filter((node) => executedNodeIds.has(node.id));
  const coverage = stages.find((stage) => stage.stage === "prompt_coverage");
  const modelInputs = sample.nodes.flatMap((node) => {
    const trace = asModelInputTrace(node);
    return trace ? [{ node, trace }] : [];
  });
  return <section className={`sample-preview-workspace${guided ? " journey-samples" : ""}`} aria-labelledby="sample-preview-title">
      <header className="sample-preview-header">
        <div><span>{t("Sandbox sample")} · {position}/{count} · {sample.image_name}</span><h2 id="sample-preview-title">{guided ? t("Does this result match what you need?") : sample.image_name}</h2></div>
        {!guided && <button type="button" onClick={onClose} aria-label={t("Close annotation preview")}>{t("View all sample images")}</button>}
      </header>
      <p className="sample-risk-notice">{t("Model confidence is not boundary accuracy. Sample decisions do not accept formal annotations.")}</p>
      {!sample.projection && <p role="alert">{t("This legacy test has no final-result projection. Test the Draft again before confirming its annotations.")}</p>}
      {!showComparison && selectedStage === "final" && image && sampleTestId && <SampleFeedbackEditor projectId={projectId} draftId={draftId} onKeepOriginal={onKeepOriginal} onImprove={onImprove} onAdopt={onAdopt} sample={sample} image={image} testId={sampleTestId} onDirtyChange={setFeedbackDirty} onConfirmed={onNext ? () => { onNavigationGuardChange(undefined); onNext(); } : undefined} navigation={guided && count > 1 ? <nav className="button-row" aria-label={t("Sample images")}><button disabled={!onPrevious} onClick={onPrevious}>{t("Previous image")}</button><span>{position}/{count}</span><button disabled={!onNext} onClick={onNext}>{t("Next image")}</button></nav> : undefined} />}
      {!guided && <details className="sample-technical-details"><summary>{t("View execution details")}</summary>
      <nav className="sample-preview-stage-tabs" aria-label={t("Annotation stages")}>
        <button type="button" aria-pressed={showComparison} onClick={() => { if (!feedbackDirty || window.confirm(t("Discard unsaved sample feedback?"))) setShowComparison(!showComparison); }}>{t("Compare all geometry")}</button>
        {availableStages.map((stage) => <button key={stage} type="button" className={!showComparison && selectedStage === stage ? "active" : ""} aria-pressed={!showComparison && selectedStage === stage} onClick={() => { if (stage === selectedStage || !feedbackDirty || window.confirm(t("Discard unsaved sample feedback?"))) { setSelectedStage(stage); setShowComparison(false); } }}>{stage === "search_region" ? t("Search region") : stage === "prompt_coverage" ? t("Prompt coverage") : stage[0].toUpperCase() + stage.slice(1)}</button>)}
      </nav>
      {showComparison ? <SampleGeometryComparison sample={sample} imageUrl={image?.url} /> : <details open={selectedStage !== "final" || !sampleTestId}><summary>{t("Technical evidence")}</summary><div className="sample-preview-layout">
        <figure className="sample-preview-canvas" style={{ aspectRatio: `${sample.width} / ${sample.height}` }}>
          {image ? <img src={image.url} alt={sample.image_name} /> : <div className="image-placeholder">{t("Preview unavailable")}</div>}
          {boxes.map((result) => {
            const rect = result.value?.kind === "bounding_box" ? result.value.rect : undefined;
            return rect ? <span className={`sample-result-box ${selectedStage === "final" ? "final" : "diagnostic"}`} key={result.id} style={{ left: `${rect[0] * 100}%`, top: `${rect[1] * 100}%`, width: `${rect[2] * 100}%`, height: `${rect[3] * 100}%` }}><b>{result.label}{result.confidence != null ? ` ${Math.round(result.confidence * 100)}%` : ""}</b></span> : null;
          })}
        </figure>
        <aside className="sample-preview-inspector">
          <div><span className="eyebrow">{t("Visible stage")}</span><h3>{selectedStage === "final" ? t("Final annotation") : selectedStage.replaceAll("_", " ")}</h3><p>{boxes.length}{" "}{t("bounding box")}{boxes.length === 1 ? "" : "es"}{" "}{t("shown")}</p></div>
          <section className={`sample-refiner-state ${configuredRefiners.length && !reachedRefiners.length ? "warning" : ""}`}>
            <strong>{t("Geometry refinement")}</strong>
            {!configuredRefiners.length ? <span>{t("No segmentation or refiner node is configured in this Draft.")}</span> : reachedRefiners.length ? <span>{reachedRefiners.length}{" "}{t("of")}{" "}{configuredRefiners.length} configured refiner node{configuredRefiners.length === 1 ? "" : t("s")} executed.</span> : <span>{t("A refiner is configured but was not reached in this Sample Test.")}{coverage?.detail ? ` ${coverage.detail}` : ""}</span>}
          </section>
          <section className="sample-preview-stage-detail">
            <strong>{t("Stage evidence")}</strong>
            {selectedStage === "final" ? <span>Only terminal results eligible for Review or Commit are displayed.</span> : selectedStages.length ? selectedStages.map((stage, index) => <span key={`${stage.artifact_id}-${index}`}><b>{stage.source}</b>{stage.detail ? ` · ${stage.detail}` : ""}</span>) : <span>{t("No Artifact was produced for this stage.")}</span>}
          </section>
          <section className="sample-preview-model-inputs">
            <strong>{t("Actual model inputs")}</strong>
            {modelInputs.length ? modelInputs.map(({ node, trace }) => <article key={`preview-input-${node.node_id}`}>
              {sampleTestId && <img src={sampleModelInputUrl(sampleTestId, sample.image_index, node.node_id)} alt={`Actual image submitted to ${node.node_id}`} />}
              <span><b>{metadataText(node.metadata?.model, node.node_id)}</b><small>{trace.submitted_dimensions.join("×")} · {trace.interpolation} · {node.latency_ms} ms · {modelCallCost(node)}</small></span>
            </article>) : <span>{t("No persisted model-input evidence exists for this result.")}</span>}
          </section>
        </aside>
      </div>
      </details>
      }
      </details>
      }
      {!guided && <footer className="task-action-bar"><button disabled={!onPrevious} onClick={onPrevious}>{t("Previous image")}</button><span>{position}/{count} · {t("Sandbox only")}</span><button disabled={!onNext} onClick={onNext}>{t("Next image")}</button></footer>}
    </section>
  ;
}

function SettingsWorkspace({
  section,
  models,
  onNavigate,
  onError,
}: {
  section: SettingsSection;
  models: ModelBinding[];
  onNavigate: (section: SettingsSection) => void;
  onError: (value: string) => void;
}) {
  return (
    <section className="page-stack">
      <nav className="section-tabs" aria-label={t("Settings sections")}>
        {(
          [
            ["providers", "Providers"],
            ["models", "Models"],
            ["plugins", "Expert Model Plugins"],
            ["vision-workers", "Legacy HTTP"],
            ["storage", "Storage"],
            ["usage", "Usage"],
          ] as const
        ).map(([value, label]) => (
          <button
            key={value}
            className={section === value ? "active" : ""}
            aria-current={section === value ? "page" : undefined}
            onClick={() => onNavigate(value)}
          >
            {t(label)}
          </button>
        ))}
      </nav>
      {section === "providers" && (
        <ProviderRegistryPage onOpenModels={() => onNavigate("models")} onError={onError} />
      )}
      {section === "models" && (
        <ModelRegistryPage onOpenProviders={() => onNavigate("providers")} onError={onError} />
      )}
      {section === "plugins" && <ExpertModelPluginsPage onError={onError} />}
      {section === "vision-workers" && (
        <SettingsPage view="workers" onError={onError} />
      )}
      {section === "storage" && <SettingsPage view="storage" onError={onError} />}
      {section === "usage" && <RegistryUsagePage onError={onError} />}
    </section>
  );
}


function ProjectsPage({
  projects,
  createOnOpen,
  onSelect,
  onRefresh,
  onError,
  onNavigate,
  onNavigationGuardChange,
}: {
  projects: ProjectSummary[];
  createOnOpen?: boolean;
  onSelect: (id: string) => void;
  onRefresh: () => Promise<void>;
  onError: (value: string) => void;
  onNavigate: (path: string) => void;
  onNavigationGuardChange: (guard?: () => boolean) => void;
}) {
  const [search, setSearch] = useState("");
  if (createOnOpen) return <JourneyImages onNavigationGuardChange={onNavigationGuardChange} onContinue={async (id) => {
    onNavigationGuardChange(undefined);
    await onRefresh(); onNavigate(projectWorkPath(id));
  }} />;
  return (
    <section className="page-stack">
      <div className="toolbar-panel">
        <div>
          <span className="eyebrow">{t("Project inventory")}</span>
          <h2>{t("My projects")}</h2>
          <p>
            {t("Choose a Project to continue, or bring images for a new annotation task.")}
          </p>
        </div>
        <button className="primary" onClick={() => onNavigate("/projects?new=1")}>{t("New annotation project")}</button>
      </div>
      <label className="project-search">{t("Search projects")}<input type="search" value={search} onChange={(event) => setSearch(event.target.value)} /></label>
      <ProjectList projects={projects.filter((project) => project.name.toLocaleLowerCase().includes(search.toLocaleLowerCase()))} onSelect={onSelect} />
      <details className="project-examples"><summary>{t("Input and output examples")}</summary><FirstResultEntry returning={false} onStart={() => onNavigate("/projects?new=1")} /></details>
    </section>
  );
}

function ProjectList({
  projects,
  onSelect,
}: {
  projects: ProjectSummary[];
  onSelect: (id: string) => void;
}) {
  return (
    <div className="table-list">
      {projects.length === 0 && (
        <Empty
          title={t("No projects yet")}
          detail="Create a Project from a validated schema and a registered Skill template."
        />
      )}
      {projects.map((project) => (
        <button
          className="project-row"
          key={project.id}
          onClick={() => onSelect(project.id)}
        >
          <span className="project-avatar">
            {project.name.slice(0, 2).toUpperCase()}
          </span>
          <span>
            <strong>{project.name}</strong>
            <small>
              {project.image_count} images · {project.active_workflow.name}@v
              {project.active_workflow.version}
            </small>
          </span>
          <Status
            status={project.readiness}
          />
          <b>→</b>
        </button>
      ))}
    </div>
  );
}

function ProjectPage({
  project: initialProject,
  runs,
  events,
  onRefresh,
  onOpenWorkflows,
  onOpenBuild,
  onOpenRun,
  onOpenReview,
  onNavigate,
  onError,
}: {
  project?: ProjectSummary;
  runs: HistoryRun[];
  events: RunEvent[];
  onRefresh: () => Promise<void>;
  onOpenWorkflows: () => void;
  onOpenBuild: (step: "data" | "labels" | "pipeline" | "test") => void;
  onOpenRun: (runId: string) => void;
  onOpenReview: () => void;
  onNavigate: (destination: string) => void;
  onError: (value: string) => void;
}) {
  const [workspace, setWorkspace] = useState<ProjectWorkspaceSummary>();
  const activeWorkspace =
    workspace?.project.id === initialProject?.id ? workspace : undefined;
  const project = activeWorkspace?.project ?? initialProject;
  const [images, setImages] = useState<ImageItem[]>([]);
  const [starting, setStarting] = useState(false);
  const [startingImageId, setStartingImageId] = useState("");
  const [workflowKey, setWorkflowKey] = useState("");
  const refreshWorkspace = async () => {
    await onRefresh();
    if (initialProject)
      setWorkspace(await api.projectSummary(initialProject.id));
  };
  useEffect(() => {
    if (!initialProject) {
      setWorkspace(undefined);
      return;
    }
    void api
      .projectSummary(initialProject.id)
      .then(setWorkspace)
      .catch((error: Error) => onError(error.message));
  }, [
    initialProject?.id,
    initialProject?.active_run?.updated_at,
    initialProject?.active_batch?.event_sequence,
    initialProject?.review_count,
    initialProject?.readiness,
    initialProject?.image_count,
    initialProject?.task_count,
    initialProject?.active_workflow.workflow_id,
    initialProject?.active_workflow.version,
    initialProject?.default_workflow_version?.workflow_id,
    initialProject?.default_workflow_version?.version,
  ]);
  useEffect(() => {
    if (project)
      void api
        .images(project.id)
        .then((value) => setImages(value.images))
        .catch((error: Error) => onError(error.message));
    else setImages([]);
  }, [project?.id]);
  useEffect(() => {
    setWorkflowKey(
      project
        ? `${project.active_workflow.workflow_id}:${project.active_workflow.version}`
        : "",
    );
  }, [
    project?.id,
    project?.active_workflow.workflow_id,
    project?.active_workflow.version,
  ]);
  if (!project)
    return (
      <section className="page-stack">
        <Empty
          title={t("No project opened")}
          detail={t("Choose a Project from Projects or the active Project switcher.")}
        />
      </section>
    );
  if (!activeWorkspace)
    return (
      <section className="page-stack">
        <div className="loading-banner" role="status">{t("Loading Project guidance…")}</div>
      </section>
    );
  const projectRuns = runs.filter((run) => run.project_id === project.project_id);
  const restoredRun = deriveProjectRunView(project);
  const activeRun = restoredRun.activeRunId;
  const runEvents = events.filter((event) => event.run_id === activeRun);
  const usage = [...runEvents]
    .reverse()
    .find((event) => event.kind === "usage_updated");
  const lastTask = [...runEvents]
    .reverse()
    .find((event) => event.task_id)?.task_id;
  const latestState = [...runEvents]
    .reverse()
    .find((event) => event.payload.type === "state");
  const activeSummary = projectRuns.find((run) => run.id === activeRun);
  const visibleStatus =
    (latestState?.payload.data.to as string | undefined) ??
    activeSummary?.status ??
    (activeRun ? "running" : (projectRuns[0]?.status ?? "pending"));
  const activeProgress = project.active_batch_progress && project.active_batch_progress.total_images > 0
    ? Math.round(
        (project.active_batch_progress.completed_images /
          project.active_batch_progress.total_images) * 100,
      )
    : undefined;
  const selectedWorkflow =
    project.available_workflow_versions.find(
      (workflow) =>
        `${workflow.workflow_id}:${workflow.version}` === workflowKey,
    ) ?? project.active_workflow;
  const selectedPublishedWorkflow =
    selectedWorkflow.status === "published" &&
    selectedWorkflow.source.startsWith("published draft")
      ? selectedWorkflow
      : undefined;
  const guidance = activeWorkspace.guidance;
  const startBatch = () => {
    if (!selectedPublishedWorkflow) {
      onError("Publish a Registry-backed Workflow Version before starting a Run.");
      return;
    }
    setStarting(true);
    void api
      .startBatch(
        project.id,
        undefined,
        {
          workflow_id: selectedPublishedWorkflow.workflow_id,
          version: Number(selectedPublishedWorkflow.version),
        },
      )
      .then(async ({ batch }) => {
        await refreshWorkspace();
        onNavigate(projectBatchPath(project.id, batch.id));
      })
      .catch((error: Error) => onError(error.message))
      .finally(() => setStarting(false));
  };
  const startImageRun = (image: ImageItem) => {
    if (!selectedPublishedWorkflow) {
      onError("Publish a Registry-backed Workflow Version before starting an image Run.");
      return;
    }
    setStartingImageId(image.image_id);
    void api
      .startRun(
        project.id,
        {
          workflow_id: selectedPublishedWorkflow.workflow_id,
          version: Number(selectedPublishedWorkflow.version),
        },
        crypto.randomUUID(),
        image.image_id,
      )
      .then(async (started) => {
        await refreshWorkspace();
        onNavigate(projectRunPath(project.id, started.run_id, { imageId: started.image_id }));
      })
      .catch((error: Error) => onError(error.message))
      .finally(() => setStartingImageId(""));
  };
  const control = (action: "pause" | "resume" | "cancel") => {
    const request = project.active_batch
      ? api.controlBatch(project.active_batch.id, action)
      : activeRun
        ? api.control(activeRun, action)
        : undefined;
    if (request)
      void request
        .then(refreshWorkspace)
        .catch((error: Error) => onError(error.message));
  };
  const runGuidedAction = (action: GuidedAction) => {
    if (!action.enabled) return;
    if (action.kind === "run_dataset") return startBatch();
    if (action.kind === "export_dataset")
      return onNavigate(`/projects/${encodeURIComponent(project.id)}/export`);
    if (action.kind === "open_active_run" && project.active_batch)
      return document.getElementById("project-active-run")?.scrollIntoView({ behavior: "smooth" });
    if (action.destination) return onNavigate(action.destination);
  };
  const openJourneyStep = (step: ProjectWorkspaceSummary["guidance"]["journey"][number]) => {
    if (!step.destination) return;
    onNavigate(step.destination);
  };
  const labelCount = project.annotation_schema.reduce(
    (count, task) => count + task.labels.length,
    0,
  );
  const projectUsage = projectRuns.reduce(
    (usage, run) => ({
      tokens: usage.tokens + run.input_tokens + run.output_tokens,
      cost: usage.cost + Number(run.cost || 0),
    }),
    { tokens: 0, cost: 0 },
  );
  return (
    <section className="page-stack">
      <nav className="section-tabs" aria-label={`${project.name} workspace`}>
        <button className="active" aria-current="page">{t("Overview")}</button>
        <button onClick={() => onOpenBuild("data")}>{t("Build")}</button>
        <button onClick={() => onNavigate(projectRunsPath(project.id))}>{t("Runs")}</button>
        <button onClick={onOpenReview}>{t("Review")}</button>
        <button onClick={() => onNavigate(`/projects/${encodeURIComponent(project.id)}/export`)}>{t("Export")}</button>
      </nav>
      <header className="project-context-header">
        <div>
          <span className="eyebrow">{t("Project workspace")}</span>
          <h2>{project.name}</h2>
          <p>{project.annotation_goal || project.description || t("No Project description provided.")}</p>
        </div>
        <div className="project-context-facts" aria-label={t("Project status")}>
          <span><b>{project.image_count}</b>{" "}{t("Images")}</span>
          <span><b>{labelCount}</b>{" "}{t("Labels")}</span>
          <span><b>{project.review_count}</b>{" "}{t("Needs review")}</span>
        </div>
        <div className="project-context-status" aria-label={t("Project operational status")}>
          <span>{t("Automation")}{" "}<b>{project.default_workflow_version?.name ?? t("Not active")}</b></span>
          <span>{t("Active run")}{" "}<b>{project.active_run?.status ?? project.active_batch?.status ?? t("None")}</b></span>
          <span>{t("Readiness")}{" "}<b>{guidance.stage.replaceAll("_", " ")}</b></span>
        </div>
      </header>

      <section className="guidance-hero" aria-labelledby="project-guidance-title">
        <div className="guidance-copy">
          <span className="eyebrow">{t("Next step ·")}{" "}{guidance.completed_steps}{" "}{t("of")}{" "}{guidance.total_steps}{" "}{t("complete")}</span>
          <h2 id="project-guidance-title">{t(guidance.headline)}</h2>
          <p>{t(guidance.explanation)}</p>
          <div className="guidance-progress" aria-label={`${guidance.completed_steps} of ${guidance.total_steps} journey steps complete`}>
            <i style={{ width: `${(guidance.completed_steps / guidance.total_steps) * 100}%` }} />
          </div>
        </div>
        <div className="guidance-actions">
          <button
            className="primary"
            aria-disabled={starting || !guidance.primary_action.enabled}
            aria-label={!guidance.primary_action.enabled && guidance.primary_action.disabled_reason ? `${t(guidance.primary_action.label)} unavailable: ${guidance.primary_action.disabled_reason}` : undefined}
            title={guidance.primary_action.disabled_reason}
            onClick={() => !starting && guidance.primary_action.enabled && runGuidedAction(guidance.primary_action)}
          >
            {starting ? t("Starting…") : t(guidance.primary_action.label)}
          </button>
          {guidance.secondary_actions.slice(0, 2).map((action) => (
            <button key={`${action.kind}:${action.destination}`} aria-disabled={!action.enabled} aria-label={!action.enabled && action.disabled_reason ? `${t(action.label)} unavailable: ${action.disabled_reason}` : undefined} title={action.disabled_reason} onClick={() => action.enabled && runGuidedAction(action)}>{t(action.label)}</button>
          ))}
          {project.available_workflow_versions.some((workflow) => workflow.status === "published") && <button onClick={onOpenWorkflows}>{t("Improve automation")}</button>}
        </div>
        {guidance.blockers.length > 0 && <div className="guidance-blockers" aria-label={t("Project blockers")}>
          {guidance.blockers.map((blocker) => <article key={blocker.code}>
            <span aria-hidden="true">!</span>
            <div><strong>{t(blocker.title)}</strong><small>{t(blocker.explanation)}</small></div>
            {blocker.repair_action && blocker.repair_action.kind !== guidance.primary_action.kind && <button onClick={() => runGuidedAction(blocker.repair_action!)}>{t(blocker.repair_action.label)}</button>}
          </article>)}
        </div>}
      </section>

      <section className="journey-panel" aria-labelledby="project-journey-title">
        <div className="section-heading"><div><span className="eyebrow">{t("Project journey")}</span><h2 id="project-journey-title">{t("From data to compatible export")}</h2></div><small>{t("Server-owned state · updated")}{" "}{new Date(guidance.updated_at).toLocaleString(localeTag())}</small></div>
        <ol className="journey-timeline">
          {guidance.journey.map((step, index) => <li key={step.id} className={step.state}>
            <button onClick={() => step.destination && openJourneyStep(step)} aria-disabled={!step.destination} aria-label={`${t(step.label)}: ${t(step.detail)}${step.destination ? "" : ". No action is available yet."}`}>
              <i aria-hidden="true">{step.state === "complete" ? "✓" : index + 1}</i>
              <span><strong>{t(step.label)}</strong><small>{t(step.detail)}</small></span>
              <b>{step.state.replaceAll("_", " ")}</b>
            </button>
          </li>)}
        </ol>
      </section>
      {(project.active_batch || project.active_run || project.last_run || selectedPublishedWorkflow) && <div className="run-state-grid" id="project-active-run">
        <Panel title={t("Active Run")} eyebrow={t("Server-owned state")}>
          {project.active_batch ? (
            <>
              <Fact label={t("Batch")} value={project.active_batch.id.slice(0, 8)} />
              <Status status={project.active_batch.status} />
              {project.active_batch_progress && (
                <Fact
                  label={t("Images")}
                  value={`${project.active_batch_progress.completed_images}/${project.active_batch_progress.total_images}`}
                />
              )}
              <div className="button-row" aria-label="Active Batch controls">
                {visibleStatus === "running" && <button onClick={() => control("pause")}><img src="/brand/core/icons/pause.svg" alt="" aria-hidden="true" />{" "}{t("Pause")}</button>}
                {visibleStatus === "paused" && <button onClick={() => control("resume")}><img src="/brand/core/icons/resume.svg" alt="" aria-hidden="true" />{" "}{t("Resume")}</button>}
                <button className="danger" onClick={() => control("cancel")}><img src="/brand/core/icons/cancel.svg" alt="" aria-hidden="true" />{" "}{t("Cancel")}</button>
              </div>
            </>
          ) : project.active_run ? (
            <>
              <Fact label={t("Run")} value={project.active_run.id.slice(0, 8)} />
              <Status status={project.active_run.status} />
              <div className="button-row" aria-label="Active Run controls">
                {visibleStatus === "running" && <button onClick={() => control("pause")}><img src="/brand/core/icons/pause.svg" alt="" aria-hidden="true" />{" "}{t("Pause")}</button>}
                {visibleStatus === "paused" && <button onClick={() => control("resume")}><img src="/brand/core/icons/resume.svg" alt="" aria-hidden="true" />{" "}{t("Resume")}</button>}
                <button className="danger" onClick={() => control("cancel")}><img src="/brand/core/icons/cancel.svg" alt="" aria-hidden="true" />{" "}{t("Cancel")}</button>
              </div>
            </>
          ) : (
            <div className="empty-run-state">
              <Empty
                title={t("No active Run")}
                detail={t("Run every image with the selected immutable Workflow Version.")}
              />
              {selectedPublishedWorkflow && <button disabled={starting} onClick={startBatch}>{starting ? t("Starting…") : t("Start full Run")}</button>}
            </div>
          )}
        </Panel>
        <Panel title={t("Last Run")} eyebrow={t("Terminal history")}>
          {project.last_run ? (
            <>
              <Fact label={t("Run")} value={project.last_run.id.slice(0, 8)} />
              <Status status={project.last_run.status} />
              {project.last_run.terminal_reason && (
                <small className="run-reason">
                  {project.last_run.terminal_reason}
                </small>
              )}
            </>
          ) : (
            <Empty
              title={t("No completed Run")}
              detail={t("Terminal history will appear here.")}
            />
          )}
        </Panel>
      </div>}
      {activeRun && (
        <div className="run-progress aa-dark" aria-live="polite">
          <div>
            <span className="live-dot" aria-hidden="true" />
            <strong>{t("Run")}{" "}{activeRun.slice(0, 8)}</strong>
            <small>
              {lastTask ?? "restored active Run"} ·{" "}
              {runEvents.at(-1)?.kind.replaceAll("_", " ") ??
                visibleStatus.replaceAll("_", " ")}
            </small>
          </div>
          <div
            className={`progress-track${activeProgress === undefined ? " indeterminate" : ""}`}
            role="progressbar"
            aria-label={t("Active Run progress")}
            aria-valuemin={activeProgress === undefined ? undefined : 0}
            aria-valuemax={activeProgress === undefined ? undefined : 100}
            aria-valuenow={activeProgress}
            aria-valuetext={activeProgress === undefined ? "Current stage is active; image progress is not available for this Run" : `${project.active_batch_progress?.completed_images} of ${project.active_batch_progress?.total_images} images complete`}
          >
            <i
              style={activeProgress === undefined ? undefined : { width: `${activeProgress}%` }}
            />
          </div>
          <pre>
            {usage
              ? JSON.stringify(usage.payload.data, null, 2)
              : activeSummary
                ? `${(activeSummary.input_tokens + activeSummary.output_tokens).toLocaleString(localeTag())} tokens\n$${activeSummary.cost}`
                : "usage pending"}
          </pre>
        </div>
      )}
      <div className="project-support-grid">
        <Panel title={t("Recent activity")} eyebrow={t("Latest dataset work")}>
          {projectRuns.length ? <div className="activity-list">
            {projectRuns.slice(0, 3).map((run) => <button key={run.id} onClick={() => onOpenRun(run.id)}><span><strong>{run.workflow_name}</strong><small>{new Date(run.updated_at).toLocaleString(localeTag())}</small></span><Status status={run.status} /></button>)}
          </div> : <Empty title={t("No Runs yet")} detail="Dataset activity will appear after the first active Automation Run." />}
        </Panel>
        <Panel title={t("Usage")} eyebrow={t("Persisted across Project Runs")}>
          <div className="usage-summary"><span><b>{projectRuns.length}</b>{" "}{t("Runs")}</span><span><b>{projectUsage.tokens.toLocaleString(localeTag())}</b>{" "}{t("Tokens")}</span><span><b>${projectUsage.cost.toFixed(4)}</b>{" "}{t("Cost")}</span></div>
        </Panel>
      </div>

      <details className="advanced-project-details" id="project-advanced-details">
        <summary><span><strong>{t("Current Project configuration")}</strong><small>{t("Read-only schema, Automation, model, Skill, and image records")}</small></span><b aria-hidden="true">⌄</b></summary>
        <div className="project-overview-grid">
        <Panel title={t("Dataset")} eyebrow={t("Project-owned")}>
          <Fact label={t("Root")} value={project.dataset.root} />
          <Fact label={t("Images")} value={project.dataset.image_count} />
          <Fact
            label={t("Discovery")}
            value={project.dataset.recursive ? "Recursive" : "Top level"}
          />
          <TagGroup title={t("Include patterns")} values={project.dataset.include} />
        </Panel>
        <Panel
          title={t("Active Workflow")}
          eyebrow={`${project.active_workflow.validation_status} · ${project.active_workflow.status}`}
        >
          <Fact label={t("Version")} value={`v${project.active_workflow.version}`} />
          <Fact label={t("Nodes")} value={project.active_workflow.nodes.length} />
          <Fact label={t("Source")} value={project.active_workflow.source} />
          <button onClick={onOpenWorkflows}>{t("Open Automation editor")}</button>
        </Panel>
        <Panel title={t("Enabled Skills")} eyebrow={t("Domain extensions")}>
          {project.enabled_skills.length ? (
            <div className="catalog-list">
              {project.enabled_skills.map((skill) => (
                <article key={skill.id}>
                  <span className="catalog-monogram">
                    {skill.display_name.slice(0, 2).toUpperCase()}
                  </span>
                  <span>
                    <strong>{skill.display_name}</strong>
                    <small>
                      {skill.id}@{skill.version}
                    </small>
                  </span>
                </article>
              ))}
            </div>
          ) : (
            <Empty
              title={t("No Skills enabled")}
              detail={t("Stable schema and hash visuals remain available.")}
            />
          )}
          <button onClick={() => onOpenBuild("pipeline")}>{t("Edit Skills in Automation")}</button>
        </Panel>
        <Panel title={t("Model Bindings")} eyebrow={t("Node execution")}>
          {project.model_bindings.map((binding) => (
            <Fact
              key={binding.id}
              label={`${binding.id} · ${binding.role}`}
              value={`${binding.provider} / ${binding.model}`}
            />
          ))}
        </Panel>
        <Panel
          title={t("Annotation Schema")}
          eyebrow={`${project.annotation_schema.length} typed tasks`}
        >
          <div className="schema-list">
            {project.annotation_schema.map((task) => (
              <article key={task.id}>
                <span>
                  <strong>{task.id}</strong>
                  <small>
                    {task.kind}
                    {task.required ? " · required" : ""}
                  </small>
                </span>
                <span>{task.labels.join(", ") || t("No labels")}</span>
              </article>
            ))}
          </div>
          <button onClick={() => onOpenBuild("labels")}>{t("Edit Labels in Build")}</button>
        </Panel>
        <Panel title={t("Versions, Runs & Reviews")} eyebrow={t("Project outputs")}>
          <Fact
            label={t("Workflow versions")}
            value={project.available_workflow_versions.length}
          />
          <Fact label={t("Runs")} value={projectRuns.length} />
          <Fact
            label={t("Runs awaiting review")}
            value={
              projectRuns.filter(
                (run) => run.status === "completed_with_review",
              ).length
            }
          />
          <button onClick={() => onNavigate(`/projects/${encodeURIComponent(project.id)}/export`)}>{t("Open Export workspace")}</button>
        </Panel>
        </div>
        <ProjectAgentActivity projectId={project.id} onError={onError} />
      <Panel title={t("Dataset images")} eyebrow={`${images.length} visible`}>
        <div className="image-grid">
          {images.map((image) => (
            <article key={image.image_id}>
              <img src={image.url} alt={image.name} />
              <div>
                <span>
                  <strong>{image.name}</strong>
                  <small>{t("Image")}{" "}{image.index + 1}</small>
                </span>
                <Status status={image.status} />
              </div>
              <button
                aria-disabled={Boolean(startingImageId || project.active_batch || project.active_run) || !selectedPublishedWorkflow}
                aria-label={!selectedPublishedWorkflow ? `Run ${image.name} unavailable: publish an Automation first` : project.active_batch || project.active_run ? `Run ${image.name} unavailable: another Run is active` : undefined}
                title={!selectedPublishedWorkflow ? t("Publish an Automation before running this image.") : undefined}
                onClick={() => !(startingImageId || project.active_batch || project.active_run) && selectedPublishedWorkflow && startImageRun(image)}
              >
                {startingImageId === image.image_id ? t("Starting…") : t("Run this image")}
              </button>
            </article>
          ))}
          {images.length === 0 && (
            <Empty
              title={t("No images")}
              detail="Import images with the CLI or controlled workspace import API."
            />
          )}
        </div>
      </Panel>
      </details>
    </section>
  );
}

function ProjectExportPage({
  project,
  workspaceReturn,
  onNavigate,
  onError,
}: {
  project?: ProjectSummary;
  workspaceReturn?: string;
  onNavigate: (destination: string) => void;
  onError: (value: string) => void;
}) {
  const [readiness, setReadiness] = useState<ExportReadiness>();
  const [format, setFormat] = useState("");
  const [exporting, setExporting] = useState(false);
  const [result, setResult] = useState<ProjectExportResult>();
  const [copyStatus, setCopyStatus] = useState("");
  const exportPending = useRef(false);
  const exportObservation = useRef<AbortController|undefined>(undefined);
  useEffect(()=>()=>exportObservation.current?.abort(),[project?.id]);
  const exportOwner = useRef(project?.id); exportOwner.current = project?.id;
  const readinessGeneration = useRef(0);
  const exportMounted = useRef(false);
  useEffect(() => { exportMounted.current = true; return () => { exportMounted.current = false; exportObservation.current?.abort(); readinessGeneration.current += 1; }; }, []);
  const activeReadiness = readiness?.project_id === project?.id ? readiness : undefined;
  const loadReadiness = (signal?: AbortSignal) => {
    if (!project) return Promise.resolve();
    const generation = ++readinessGeneration.current;
    return api.exportReadiness(project.id, signal).then((value) => {
      if (!exportMounted.current || signal?.aborted || exportOwner.current !== project.id || generation !== readinessGeneration.current) return;
      setReadiness(value);
      setFormat((current) =>
        value.formats.some((item) => item.format === current && item.supported)
          ? current
          : value.recommended_format ?? value.formats.find((item) => item.supported)?.format ?? "",
      );
      setResult(value.last_export);
    });
  };
  useEffect(() => {
    const controller = new AbortController();
    setReadiness(undefined);
    setResult(undefined);
    setCopyStatus("");
    void loadReadiness(controller.signal).catch((error: Error) => { if (!controller.signal.aborted) onError(error.message); });
    return () => controller.abort();
  }, [
    project?.id,
    project?.image_count,
    project?.review_count,
    project?.active_run?.updated_at,
    project?.active_batch?.event_sequence,
  ]);
  const executeExport = () => {
    if (!project || !format || !activeReadiness?.ready || exportPending.current) return;
    const destination=workspaceReturn ? new URL(workspaceReturn,window.location.origin) : undefined;
    const source=destination ? parseWorkspaceRoute(destination.pathname,destination.search) : undefined;
    const context=source?.kind==="conversation" && source.projectId===project.id && source.conversationId && source.taskId ? {conversation_id:source.conversationId,task_id:source.taskId} : undefined;
    const receiptKey=context ? `conversation-export:${project.id}:${context.conversation_id}:${context.task_id}:${format}` : undefined;
    let operation:({id:string;conversation_id:string;task_id:string})|undefined;
    try {
      if(context && receiptKey){const id=sessionStorage.getItem(receiptKey) ?? crypto.randomUUID();sessionStorage.setItem(receiptKey,id);operation={...context,id};}
    } catch {onError("Cannot preserve the export retry identity in this browser. No export was started.");return;}
    exportPending.current = true;
    const observation=new AbortController();exportObservation.current=observation;
    setExporting(true);
    setCopyStatus("");
    void api
      .export(project.id, format, operation,observation.signal)
      .then((value) => {
        if(receiptKey)try{sessionStorage.removeItem(receiptKey);}catch{/* Keeping the completed identity is safe on retry. */}
        if (!exportMounted.current || exportOwner.current !== project.id) return;
        setResult(value);
        return loadReadiness();
      })
      .catch(async (error: Error) => {
        if(observation.signal.aborted)return;
        let explanation=error.message;
        if(context && operation && receiptKey)try{
          const history=await api.conversationExports(project.id,context.conversation_id,context.task_id);
          if(history.some(record=>record.id===operation.id && record.error && !record.result)){
            sessionStorage.removeItem(receiptKey);
            explanation+=" The failed export is saved. After correcting the issue, clicking Export starts a new request.";
          }
        }catch{/* Unknown outcomes retain their retry identity; no new work is admitted. */}
        if (exportMounted.current && exportOwner.current === project.id) onError(explanation);
      })
      .finally(() => { if(context)workspaceQueries.invalidate(`conversation-exports:${project.id}:${context.conversation_id}:${context.task_id}`); if(exportObservation.current===observation){exportPending.current = false; if (exportMounted.current) setExporting(false);} });
  };
  const copyOutputPath = () => {
    if (!result) return;
    void navigator.clipboard
      .writeText(result.output_path)
      .then(() => setCopyStatus("Folder path copied"))
      .catch(() => setCopyStatus("Select the path above to copy it"));
  };
  if (!project)
    return <section className="page-stack"><Empty title={t("Project unavailable")} detail={t("Return to Projects and choose a valid Project.")} /></section>;
  return (
    <section className="journey-scene export-workspace">
      {!activeReadiness ? (
        <div className="loading-banner" role="status">{t("Checking dataset export readiness…")}</div>
      ) : (
        <>
          <header className={`export-hero ${activeReadiness.ready ? "ready" : "blocked"}`}>
            <div>
              <span className="eyebrow">{t("Dataset delivery")}</span>
              <h2>{activeReadiness.ready ? t("Your dataset is ready") : t("Export needs attention")}</h2>
              <p>{activeReadiness.ready ? "All images have completed runs and every review decision is resolved. Choose a compatible format to create the dataset." : t("Resolve the items below before creating a formal dataset export.")}</p>
            </div>
            <dl className="export-readiness-metrics">
              <div><dt>{t("Images")}</dt><dd>{activeReadiness.image_count}</dd><small>{activeReadiness.processed_image_count} processed</small></div>
              <div><dt>{t("Accepted annotations")}</dt><dd>{activeReadiness.accepted_annotations}</dd><small>{t("Included in export")}</small></div>
              <div><dt>{t("Unresolved reviews")}</dt><dd>{activeReadiness.unresolved_reviews}</dd><small>{activeReadiness.unresolved_reviews ? t("Blocking export") : t("Queue is clear")}</small></div>
            </dl>
          </header>

          {activeReadiness.blocking_issues.length > 0 && <section className="export-blockers" aria-labelledby="export-blockers-title">
            <div className="section-heading"><div><span className="eyebrow">{t("Blocking reviews and setup")}</span><h2 id="export-blockers-title">{t("Complete these items")}</h2></div></div>
            {activeReadiness.blocking_issues.map((issue) => <article key={issue.code}>
              <span aria-hidden="true">!</span>
              <div><strong>{issue.title}</strong><small>{issue.explanation}</small></div>
              <button onClick={() => onNavigate(issue.repair_destination)}>{t("Resolve")}</button>
            </article>)}
          </section>}

          <section className="export-format-section" aria-labelledby="export-format-title">
            <div className="section-heading"><div><span className="eyebrow">{t("Schema compatibility")}</span><h2 id="export-format-title">{t("Choose an export format")}</h2></div><small>{t("The recommendation is calculated from the active Project Schema.")}</small></div>
            <div className="export-format-grid">
              {activeReadiness.formats.filter((item) => item.supported).map((item) => <label className={`export-format-card ${format === item.format ? "selected" : ""}`} key={item.format}>
                <input type="radio" name="export-format" value={item.format} checked={format === item.format} disabled={!item.supported} onChange={() => setFormat(item.format)} />
                <span>
                  <span className="export-format-title"><strong>{item.display_name}</strong>{item.recommended && <b>{t("Recommended")}</b>}{!item.supported && <b>{t("Incompatible")}</b>}</span>
                  <small>{item.summary}</small>
                  {item.unsupported_task_kinds.length > 0 && <small>{t("Unsupported:")}{" "}{item.unsupported_task_kinds.join(", ")}</small>}
                  {item.warnings.map((warning) => <small key={warning}>{warning}</small>)}
                </span>
              </label>)}
            </div>
            <div className="export-actions">
              <button onClick={() => onNavigate(`/projects/${encodeURIComponent(project.id)}`)}>{t("Back to Project")}</button>
              <button className="primary" disabled={!activeReadiness.ready || !format || exporting} onClick={executeExport}>
                {exporting ? t("Exporting dataset…") : `Export ${activeReadiness.formats.find((item) => item.format === format)?.display_name ?? "dataset"} dataset`}
              </button>
            </div>
            {exporting && workspaceReturn && <p role="status">Export runs on the server. Leaving this page stops status observation, not the export. Its result remains linked to your conversation.</p>}
          </section>

          {result && <section className="export-success" aria-live="polite">
            <div className="export-success-heading"><span aria-hidden="true">✓</span><div><span className="eyebrow">{t("Export complete")}</span><h2>{t("Dataset exported successfully")}</h2><p>{result.report.exported_count} annotation{result.report.exported_count === 1 ? "" : t("s")} exported · {result.report.skipped_count} skipped · {new Date(result.completed_at).toLocaleString(localeTag())}</p></div></div>
            {result.delivery && project && (
              <div className="button-row">
                <a className="button primary"
                  href={`/api/projects/${encodeURIComponent(project.id)}/exports/${encodeURIComponent(result.delivery.id)}/download`}
                  download>{t("Download annotation archive")}</a>
                <small>{t("Generated annotation files and report. Original images are not bundled unless the exporter includes them.")} · {result.delivery.bytes.toLocaleString()} bytes</small>
              </div>
            )}
            <div className="export-result-path"><span>{t("Result folder")}</span><code>{result.output_path}</code><button onClick={copyOutputPath}>{t("Copy folder path")}</button>{copyStatus && <small role="status">{copyStatus}</small>}<small>{t("This folder is on the AnnotAgent server, not necessarily on this device.")}</small></div>
            <details className="export-report"><summary>{t("View export report")}</summary><dl>
              <div><dt>{t("Format")}</dt><dd>{result.format}</dd></div>
              <div><dt>{t("Exported")}</dt><dd>{result.report.exported_count}</dd></div>
              <div><dt>{t("Skipped")}</dt><dd>{result.report.skipped_count}</dd></div>
              <div><dt>{t("Files")}</dt><dd>{result.report.output_files.length}</dd></div>
            </dl>
              {result.report.output_files.length > 0 && <ul>{result.report.output_files.map((file) => <li key={file}><code>{file}</code></li>)}</ul>}
              {result.report.warnings.length > 0 && <ul>{result.report.warnings.map((warning) => <li key={warning}>{warning}</li>)}</ul>}
            </details>
          </section>}
        </>
      )}
    </section>
  );
}

type TaskPreparationSection = "models" | "providers" | "plugins";

function TaskModelPreparation({ section, onSelect, onClose, onError }: {
  section: TaskPreparationSection;
  onSelect: (section: TaskPreparationSection) => void;
  onClose: () => Promise<void>;
  onError: (value: string) => void;
}) {
  const [returning, setReturning] = useState(false);
  const panel = useRef<HTMLElement>(null);
  useEffect(() => {
    const trigger = document.activeElement;
    panel.current?.scrollIntoView({ block: "start", behavior: "instant" });
    panel.current?.focus({ preventScroll: true });
    return () => { if (trigger instanceof HTMLElement && trigger.isConnected) trigger.focus(); };
  }, []);
  useEffect(() => { panel.current?.scrollIntoView({ block: "start", behavior: "instant" }); }, [section]);
  return <section ref={panel} tabIndex={-1} className="task-model-preparation panel" aria-label={t("Task model preparation")}>
    <header className="section-heading"><div><h2>{t("Prepare models without leaving your task")}</h2><p>{t("These are the same Registry forms as Settings. Your Project, images and Draft stay in place. Returning only refreshes availability; it does not start inference.")}</p></div>
      <button disabled={returning} onClick={() => {
        setReturning(true);
        void onClose().catch((error: Error) => onError(error.message)).finally(() => setReturning(false));
      }}>{t("Return to this Draft")}</button>
    </header>
    <nav className="button-row" aria-label={t("Model preparation options")}>
      <button aria-pressed={section === "models"} onClick={() => onSelect("models")}>{t("Reuse configured models")}</button>
      <button aria-pressed={section === "providers"} onClick={() => onSelect("providers")}>{t("Provider connections")}</button>
      <button aria-pressed={section === "plugins"} onClick={() => onSelect("plugins")}>{t("Local model availability")}</button>
    </nav>
    <p className="inline-notice">{t("Saving configuration is not authorization to test images. Connection checks, billable probes and downloads remain separate explicit actions. Local installation is available only when the Registry lists a compatible real Bundle.")}</p>
    {section === "models" && <ModelRegistryPage onOpenProviders={() => onSelect("providers")} onError={onError} />}
    {section === "providers" && <ProviderRegistryPage onOpenModels={() => onSelect("models")} onError={onError} />}
    {section === "plugins" && <ExpertModelPluginsPage onError={onError} />}
  </section>;
}

function WorkflowsPage({
  projects,
  runs,
  activeProjectId,
  selectedDraftId,
  selectedWorkflow,
  selectedAgentSessionId,
  selectedImprovementSessionId,
  onActivate,
  onRefresh,
  onNavigate,
  onSelectContext,
  onOpenProjects,
  onOpenProject,
  onOpenTrash,
  onNavigationGuardChange,
  onError,
}: {
  projects: ProjectSummary[];
  runs: HistoryRun[];
  activeProjectId: string;
  selectedDraftId?: string;
  selectedWorkflow?: { id: string; version: number };
  selectedAgentSessionId?: string;
  selectedImprovementSessionId?: string;
  onActivate: (id: string) => void;
  onRefresh: () => Promise<void>;
  onNavigate: (step: "data" | "labels" | "pipeline" | "test", draftId?: string) => void;
  onSelectContext: (
    context: {
      draftId?: string;
      workflowId?: string;
      workflowVersion?: number;
      agentSessionId?: string;
      improvementSessionId?: string;
    },
    replace?: boolean,
  ) => void;
  onOpenProjects: () => void;
  onOpenProject: () => void;
  onOpenTrash: () => void;
  onNavigationGuardChange: (guard?: () => boolean) => void;
  onError: (value: string) => void;
}) {
  const entries = projects.flatMap((project) =>
    project.available_workflow_versions.map((workflow) => ({
      project,
      workflow,
    })),
  );
  const selectedPublishedFromRoute = selectedWorkflow
    ? `${selectedWorkflow.id}:${selectedWorkflow.version}`
    : "";
  const [selectedPublishedKey, setSelectedPublishedKey] = useState(selectedPublishedFromRoute);
  const selected =
    entries.find(
      (entry) =>
        `${entry.workflow.workflow_id}:${entry.workflow.version}` ===
        selectedPublishedKey,
    ) ??
    entries.find(
      (entry) =>
        entry.project.id === activeProjectId &&
        entry.workflow.source.startsWith("published draft"),
    ) ??
    entries.find((entry) => entry.project.id === activeProjectId);
  const [drafts, setDrafts] = useState<WorkflowDraft[]>([]);
  const [draft, setDraft] = useState<WorkflowDraft>();
  const selectedContextRef = useRef({ draftId: selectedDraftId, agentSessionId: selectedAgentSessionId, published: selectedPublishedFromRoute });
  selectedContextRef.current = { draftId: selectedDraftId, agentSessionId: selectedAgentSessionId, published: selectedPublishedFromRoute };
  const draftEditorRef = useRef<HTMLDivElement>(null);
  const [pendingDraftFocus, setPendingDraftFocus] = useState<string>();
  const [report, setReport] = useState<WorkflowDryRunReport>();
  const [catalog, setCatalog] = useState<WorkflowCatalog>();
  const [comparison, setComparison] = useState<WorkflowVersionComparison>();
  const [advisorProposal, setAdvisorProposal] = useState<WorkflowSuggestion>();
  const [advisorProposalRecovered, setAdvisorProposalRecovered] = useState(false);
  const [proposalDiff, setProposalDiff] = useState<PipelineDraftDiff>();
  const [selectedProposalChanges, setSelectedProposalChanges] = useState<string[]>([]);
  const [undoDraft, setUndoDraft] = useState<WorkflowDraft>();
  const [activeAgentSession, setActiveAgentSession] = useState<AgentSession>();
  const [showProposalComparison, setShowProposalComparison] = useState(true);
  const [compareLeft, setCompareLeft] = useState("");
  const [compareRight, setCompareRight] = useState("");
  const advisorKind = "llm" as const;
  const [registryProviders, setRegistryProviders] = useState<ProviderProfile[]>([]);
  const [preparation, setPreparation] = useState<TaskPreparationSection>();
  const onOpenProviders = () => setPreparation("providers");
  const onOpenModels = () => setPreparation("models");
  const onOpenPlugins = () => setPreparation("plugins");
  const [compatibleModels, setCompatibleModels] = useState<
    Partial<Record<ModelBindingRole, RegistryModelProfile[]>>
  >({});
  const [projectModelBindings, setProjectModelBindings] = useState<
    ProjectModelBinding[]
  >([]);
  const [globalModelDefaults, setGlobalModelDefaults] =
    useState<GlobalModelDefaults>({});
  const [selectedAgentModelId, setSelectedAgentModelId] = useState("");
  const [modelBindingBusy, setModelBindingBusy] = useState(false);
  const [registryLoading, setRegistryLoading] = useState(true);
  const [builderConstraints, setBuilderConstraints] = useState<PipelineBuilderConstraints>(
    DEFAULT_PIPELINE_BUILDER_CONSTRAINTS,
  );
  const [buildModeKind, setBuildModeKind] = useState<PipelineBuildMode["kind"]>(
    "from_scratch",
  );
  const [templateId, setTemplateId] = useState("");
  const activeProject = projects.find((project) => project.id === activeProjectId);
  const buildSummary = useBuildSummary(activeProject, onError);
  const [targetTaskId, setTargetTaskId] = useState("");
  const [targetLabel, setTargetLabel] = useState("");
  const [busy, setBusy] = useState(false);
  const [advisorRunning, setAdvisorRunning] = useState(false);
  const advisorRequestActive = useRef(false);
  const advisorRequestStartedAt = useRef(0);
  const persistedDrafts = useRef(new Map<string, string>());
  const autosaveTimer = useRef<number | undefined>(undefined);
  const autosaveController = useRef<AbortController | undefined>(undefined);
  const autosaveGeneration = useRef(0);
  const [autosaveError, setAutosaveError] = useState("");
  const [draftConflict, setDraftConflict] = useState<{
    local: WorkflowDraft;
    server?: WorkflowDraft;
    message: string;
  }>();
  const [savedAt, setSavedAt] = useState<Date>();
  const refreshDrafts = () =>
    workspaceQueries
      .load(
        queryKeys.workflowDrafts(activeProjectId),
        (signal) => api.workflowDrafts(activeProjectId || undefined, signal),
        { force: true },
      )
      .then((value) => {
        for (const item of value.drafts)
          persistedDrafts.current.set(item.id, JSON.stringify(item));
        setDrafts(value.drafts);
        setDraft(
          (current) =>
            selectedPublishedFromRoute
              ? undefined
              :
            value.drafts.find((item) => item.id === selectedDraftId) ??
            value.drafts.find((item) => item.id === current?.id) ??
            value.drafts[0],
        );
      })
      .catch((error: Error) => {
        if (!isAbortError(error)) onError(error.message);
      });
  const refreshModelChoices = () => {
    if (!activeProjectId) {
      setRegistryProviders([]);
      setCompatibleModels({});
      setProjectModelBindings([]);
      setGlobalModelDefaults({});
      setSelectedAgentModelId("");
      setRegistryLoading(false);
      return Promise.resolve();
    }
    setRegistryLoading(true);
    return Promise.all([
      api.providers(),
      api.projectModelBindings(activeProjectId),
      api.agentModelBindings(),
      Promise.all(
        PROJECT_MODEL_CHOICES.map(async (choice) => [
          choice.role,
          (
            await api.compatibleModelProfiles({
              input_modalities: [choice.modality],
              capabilities: [choice.capability],
              ...(choice.role === "pipeline_builder"
                ? { tool_calls: true, structured_output: true }
                : {}),
            })
          ).models,
        ] as const),
      ),
    ])
      .then(([providerResult, bindingResult, defaults, choiceEntries]) => {
        const choices = Object.fromEntries(choiceEntries) as Partial<
          Record<ModelBindingRole, RegistryModelProfile[]>
        >;
        const providers = providerResult.providers;
        const liveBuilderModels = (choices.pipeline_builder ?? []).filter(
          (model) =>
            providers.find((provider) => provider.id === model.provider_id)
              ?.adapter === "open_ai_compatible",
        );
        const projectChoice = bindingResult.bindings.find(
          (binding) =>
            binding.match_kind === "role" &&
            binding.role === "pipeline_builder",
        )?.model_profile_id;
        const preferred =
          [projectChoice, defaults.pipeline_builder]
            .filter(Boolean)
            .find((id) => liveBuilderModels.some((model) => model.id === id)) ??
          liveBuilderModels[0]?.id ??
          "";
        setRegistryProviders(providers);
        setCompatibleModels(choices);
        setProjectModelBindings(bindingResult.bindings);
        setGlobalModelDefaults(defaults);
        setSelectedAgentModelId((current) =>
          liveBuilderModels.some((model) => model.id === current)
            ? current
            : preferred,
        );
      })
      .catch((error: Error) => onError(`Model Registry: ${error.message}`))
      .finally(() => setRegistryLoading(false));
  };
  const recoverAdvisorProposal = async (session: AgentSession) => {
    if (!activeProjectId || session.status === "running" || !session.draft_id) return;
    const matchesSelection = () => {
      const context = selectedContextRef.current;
      if (context.published) return false;
      if (context.agentSessionId) return context.agentSessionId === session.id;
      return !context.draftId || context.draftId === session.draft_id;
    };
    if (!matchesSelection()) return;
    const [{ drafts: latestDrafts }, sample] = await Promise.all([
      api.workflowDrafts(activeProjectId),
      api.workflowSampleTest(session.draft_id).catch(() => ({ sample_test: null, current: false })),
    ]);
    if (!matchesSelection()) return;
    const savedDraft = latestDrafts.find((candidate) => candidate.id === session.draft_id);
    if (!savedDraft || ["published", "archived"].includes(savedDraft.status)) return;
    for (const item of latestDrafts)
      persistedDrafts.current.set(item.id, JSON.stringify(item));
    setDrafts(latestDrafts);
    setDraft(savedDraft);
    const persisted = session.builder_proposal;
    const dryRun = sample.current ? sample.sample_test?.report : undefined;
    setAdvisorProposal({
      draft: savedDraft,
      rationale: persisted?.rationale ?? ["Recovered from the persisted Pipeline Builder result."],
      estimated_model_calls_per_image:
        persisted?.estimated_model_calls_per_image
        ?? savedDraft.nodes.filter((node) => node.model_binding || node.model_profile_binding).length,
      estimated_latency_ms: persisted?.estimated_latency_ms,
      estimated_cost_tier: persisted?.estimated_cost_tier ?? "unresolved",
      unresolved_model_bindings:
        persisted?.unresolved_model_bindings ?? session.unresolved_bindings ?? [],
      warnings: persisted?.warnings ?? [],
      alternatives: persisted?.alternatives ?? [],
      agent_session: session,
      agent_validation: dryRun?.validation,
      agent_dry_run: dryRun,
      approval_required: session.status === "waiting_for_human",
    });
    setAdvisorProposalRecovered(true);
    setProposalDiff(undefined);
    setSelectedProposalChanges([]);
  };
  useEffect(() => {
    if (!draft || draft.status === "published" || draft.status === "archived") return;
    if (draftConflict?.local.id === draft.id) return;
    const snapshot = JSON.stringify(draft);
    if (persistedDrafts.current.get(draft.id) === snapshot) return;
    setAutosaveError("");
    if (autosaveTimer.current) window.clearTimeout(autosaveTimer.current);
    autosaveTimer.current = window.setTimeout(() => {
      autosaveController.current?.abort();
      const controller = new AbortController();
      autosaveController.current = controller;
      const generation = ++autosaveGeneration.current;
      void api
        .saveWorkflowDraft(draft, controller.signal)
        .then((saved) => {
          if (generation !== autosaveGeneration.current) return;
          persistedDrafts.current.set(saved.id, JSON.stringify(saved));
          setDraft((current) => {
            if (!current || current.id !== saved.id) return current;
            if (JSON.stringify(current) === snapshot) return saved;
            return {
              ...current,
              revision: saved.revision,
              content_hash: saved.content_hash,
            };
          });
          setSavedAt(new Date());
          return onRefresh();
        })
        .catch((error: Error) => {
          if (controller.signal.aborted || generation !== autosaveGeneration.current) return;
          if (
            error instanceof ApiRequestError &&
            error.code === "workflow_draft_revision_conflict"
          ) {
            const local = JSON.parse(snapshot) as WorkflowDraft;
            void api
              .workflowDrafts(activeProjectId)
              .then(({ drafts: currentDrafts }) => {
                setDraftConflict({
                  local,
                  server: currentDrafts.find((item) => item.id === local.id),
                  message: error.message,
                });
              })
              .catch(() => setDraftConflict({ local, message: error.message }));
            return;
          }
          onError(`Draft autosave failed: ${error.message}`);
          setAutosaveError(error.message);
        });
    }, 800);
    return () => {
      if (autosaveTimer.current) window.clearTimeout(autosaveTimer.current);
    };
  }, [activeProjectId, draft, draftConflict]);
  useEffect(() => () => autosaveController.current?.abort(), []);
  const draftHasUnsavedChanges = Boolean(draft && !["published", "archived"].includes(draft.status)
    && persistedDrafts.current.get(draft.id) !== JSON.stringify(draft));
  useEffect(() => {
    onNavigationGuardChange(() => !draftHasUnsavedChanges || window.confirm(t("This Draft has unsaved changes. Leave without saving?")));
    const guard = (event: BeforeUnloadEvent) => { if (draftHasUnsavedChanges) event.preventDefault(); };
    window.addEventListener("beforeunload", guard);
    return () => { onNavigationGuardChange(undefined); window.removeEventListener("beforeunload", guard); };
  }, [draftHasUnsavedChanges, onNavigationGuardChange]);
  useEffect(() => {
    setAdvisorProposal(undefined);
    setAdvisorProposalRecovered(false);
    void refreshDrafts();
    void refreshModelChoices();
    if (activeProjectId) {
      void api
        .workflowCatalog(activeProjectId)
        .then(setCatalog)
        .catch((error: Error) => onError(error.message));
      void workspaceQueries
        .load(
          queryKeys.agentSessions(activeProjectId),
          (signal) => api.agentSessions(activeProjectId, signal),
          { force: true },
        )
        .then(({ sessions }) => {
          const context = selectedContextRef.current;
          const scopedSessions = sessions.filter((session) => !context.published && (
            context.agentSessionId ? session.id === context.agentSessionId
              : !context.draftId || session.draft_id === context.draftId
          ));
          const requested = scopedSessions.find(
            (session) => session.id === context.agentSessionId,
          );
          const latest = requested ?? scopedSessions.find(
            (session) =>
              session.kind === "pipeline_builder" &&
              ["running", "waiting_for_human"].includes(session.status),
          ) ?? scopedSessions.find((session) => session.kind === "pipeline_builder");
          setActiveAgentSession(latest);
          setAdvisorRunning(latest?.status === "running");
          if (latest && latest.status !== "running")
            void recoverAdvisorProposal(latest).catch((error: Error) =>
              onError(`Agent result recovery: ${error.message}`),
            );
        })
        .catch((error: Error) => {
          if (!isAbortError(error)) onError(`Agent recovery: ${error.message}`);
        });
    } else {
      setCatalog(undefined);
      setActiveAgentSession(undefined);
      setAdvisorRunning(false);
    }
    setReport(undefined);
    setSelectedPublishedKey(selectedPublishedFromRoute);
    setTargetTaskId(activeProject?.annotation_schema[0]?.id ?? "");
    setTargetLabel(activeProject?.annotation_schema[0]?.labels[0] ?? "");
    return () => {
      workspaceQueries.abort(queryKeys.workflowDrafts(activeProjectId));
      workspaceQueries.abort(queryKeys.agentSessions(activeProjectId));
    };
  }, [activeProjectId]);
  useEffect(() => {
    const selectedDraft = drafts.find((item) => item.id === selectedDraftId);
    if (selectedDraft && selectedDraft.id !== draft?.id) {
      setDraft(selectedDraft);
      setReport(undefined);
    }
  }, [selectedDraftId, drafts]);
  useEffect(() => {
    if (!pendingDraftFocus || draft?.id !== pendingDraftFocus || !draftEditorRef.current) return;
    draftEditorRef.current.closest("details")?.setAttribute("open", "");
    draftEditorRef.current.scrollIntoView({ block: "start" });
    draftEditorRef.current.focus({ preventScroll: true });
    setPendingDraftFocus(undefined);
  }, [pendingDraftFocus, draft?.id]);
  useEffect(() => {
    setSelectedPublishedKey(selectedPublishedFromRoute);
  }, [selectedPublishedFromRoute]);
  useEffect(() => {
    if (draft?.id && !selectedDraftId && !selectedPublishedFromRoute)
      onSelectContext({ draftId: draft.id }, true);
  }, [draft?.id, selectedDraftId, selectedPublishedFromRoute]);
  useEffect(() => {
    if (!advisorRunning || !activeProjectId) return;
    let stopped = false;
    let timer: number | undefined;
    let delay = 1_000;
    const poll = async () => {
      try {
        const { sessions } = await workspaceQueries.load(
          queryKeys.agentSessions(activeProjectId),
          (signal) => api.agentSessions(activeProjectId, signal),
          { force: true },
        );
        if (stopped) return;
        const context = selectedContextRef.current;
        const latest = sessions.find((session) => session.kind === "pipeline_builder" && !context.published && (
          advisorRequestActive.current ? Date.parse(session.created_at) >= advisorRequestStartedAt.current - 1_000
          : context.agentSessionId ? session.id === context.agentSessionId
            : !context.draftId || session.draft_id === context.draftId
        ));
        if (latest) {
          if (advisorRequestActive.current && latest.status !== "running") return;
          setActiveAgentSession(latest);
          onSelectContext({
            draftId: context.draftId ?? latest.draft_id,
            agentSessionId: latest.id,
          }, true);
          if (!advisorRequestActive.current && latest.status !== "running") {
            setAdvisorRunning(false);
            void recoverAdvisorProposal(latest).catch((error: Error) =>
              onError(`Agent result recovery: ${error.message}`),
            );
            return;
          }
        }
      } catch {
        // The next bounded backoff is the recovery path while SSE has no Agent event.
      }
      if (!stopped) {
        timer = window.setTimeout(() => void poll(), delay);
        delay = Math.min(10_000, Math.round(delay * 1.8));
      }
    };
    void poll();
    return () => {
      stopped = true;
      if (timer) window.clearTimeout(timer);
      workspaceQueries.abort(queryKeys.agentSessions(activeProjectId));
    };
  }, [advisorRunning, activeProjectId]);
  useEffect(() => {
    if (!draft) return;
    const key = queryKeys.sampleTest(draft.id);
    void workspaceQueries.load(
      key,
      (signal) => api.workflowSampleTest(draft.id, signal),
      { force: true },
    )
      .then(({ sample_test: sampleTest, current }) => {
        setReport(current ? sampleTest?.report : undefined);
      })
      .catch((error: unknown) => {
        if (!isAbortError(error)) setReport(undefined);
      });
    return () => workspaceQueries.abort(key);
  }, [draft?.id, draft?.updated_at]);
  const finish = (promise: Promise<unknown>) => {
    setBusy(true);
    void promise
      .then(() => Promise.all([refreshDrafts(), onRefresh()]))
      .catch((error: Error) => {
        if (!isAbortError(error)) onError(error.message);
      })
      .finally(() => setBusy(false));
  };
  const create = (fromTemplate: boolean, selectedTemplate?: string) => {
    if (!activeProjectId)
      return onError("Select a Project before creating a Workflow.");
    setBusy(true);
    void api
      .createWorkflowDraft(activeProjectId, fromTemplate, selectedTemplate)
      .then((created) => {
        persistedDrafts.current.set(created.id, JSON.stringify(created));
        setDraft(created);
        setReport(undefined);
        onSelectContext({ draftId: created.id }, true);
        setPendingDraftFocus(created.id);
        return Promise.all([refreshDrafts(), onRefresh()]);
      })
      .catch((error: Error) => {
        if (!isAbortError(error)) onError(error.message);
      })
      .finally(() => setBusy(false));
  };
  const runAdvisor = (
    target?: { task_id: string; label: string },
    retry?: { session_id?: string; base_draft_id?: string },
  ) => {
    if (!activeProjectId)
      return onError("Select a Project before suggesting a Pipeline.");
    if (advisorKind === "llm" && !selectedAgentModelId)
      return onError(
        "Provider setup required: choose an Available Pipeline Builder Model Profile.",
      );
    setBusy(true);
    setAdvisorRunning(true);
    advisorRequestActive.current = true;
    advisorRequestStartedAt.current = Date.now();
    setActiveAgentSession(undefined);
    setAdvisorProposalRecovered(false);
    setProposalDiff(undefined);
    setSelectedProposalChanges([]);
    let buildMode: PipelineBuildMode = { kind: "from_scratch" };
    if (retry?.session_id && (retry.base_draft_id ?? draft?.id)) {
      buildMode = {
        kind: "repair_draft",
        draft_id: retry.base_draft_id ?? draft!.id,
      };
    } else if (buildModeKind === "repair_draft" || buildModeKind === "resolve_bindings") {
      if (!draft || ["published", "archived"].includes(draft.status)) {
        setBusy(false);
        setAdvisorRunning(false);
        advisorRequestActive.current = false;
        return onError("Choose an editable Draft for this Build mode.");
      }
      buildMode = { kind: buildModeKind, draft_id: draft.id };
    } else if (buildModeKind === "improve_existing") {
      if (!selected?.workflow.source.startsWith("published draft")) {
        setBusy(false);
        setAdvisorRunning(false);
        advisorRequestActive.current = false;
        return onError("Choose a Published Workflow Version to improve.");
      }
      buildMode = {
        kind: "improve_existing",
        base_workflow_version_id: `${selected.workflow.workflow_id}@${selected.workflow.version.replace(/^v/, "")}`,
      };
    }
    const editableBase = buildMode.kind === "repair_draft" || buildMode.kind === "resolve_bindings"
      ? draft
      : undefined;
    const comparisonBase = buildMode.kind === "from_scratch" ? draft : editableBase;
    void Promise.resolve(comparisonBase)
      .then(async (baseDraft) => {
        const proposal = await api.suggestWorkflow(
          activeProjectId,
          advisorKind,
          target,
          {
            require_review_gate: builderConstraints.allow_human_review,
            max_cost_per_image: builderConstraints.max_cost_per_image,
            max_latency_ms: builderConstraints.max_expected_latency_ms,
          },
          builderConstraints,
          advisorKind === "llm" ? selectedAgentModelId : undefined,
          retry,
          buildMode,
        );
        setAdvisorProposal(proposal);
        persistedDrafts.current.set(proposal.draft.id, JSON.stringify(proposal.draft));
        setAdvisorProposalRecovered(false);
        setActiveAgentSession(proposal.agent_session);
        const compareDistinctDrafts = Boolean(baseDraft && baseDraft.id !== proposal.draft.id);
        if (!compareDistinctDrafts) setDraft(proposal.draft);
        onSelectContext({
          draftId: compareDistinctDrafts ? baseDraft?.id : proposal.draft.id,
          agentSessionId: proposal.agent_session?.id,
        }, true);
        setShowProposalComparison(compareDistinctDrafts);
        if (compareDistinctDrafts && baseDraft) {
          const diff = await api.workflowDraftDiff(baseDraft.id, proposal.draft.id);
          setProposalDiff(diff);
          setSelectedProposalChanges(pipelineDiffChangeIds(diff));
        }
        await refreshDrafts();
      })
      .catch((error: Error) => {
        if (!isAbortError(error)) onError(error.message);
      })
      .finally(() => {
        advisorRequestActive.current = false;
        setAdvisorRunning(false);
        setBusy(false);
      });
  };
  const suggest = () => runAdvisor();
  const suggestLabelPipeline = () =>
    targetTaskId && targetLabel
      ? runAdvisor({ task_id: targetTaskId, label: targetLabel })
      : onError("Choose a Project task and target Label first.");
  const retryAgentSession = (session: AgentSession) =>
    targetTaskId && targetLabel
      ? runAdvisor(
          { task_id: targetTaskId, label: targetLabel },
          {
            session_id: session.id,
            base_draft_id: session.draft_id ?? draft?.id,
          },
        )
      : onError("Choose the original target task and Label before retrying.");
  const openAgentDraft = (draftId: string) => {
    const selectedDraft = drafts.find((candidate) => candidate.id === draftId);
    if (selectedDraft) {
      setDraft(selectedDraft);
      onSelectContext({ draftId }, true);
      setPendingDraftFocus(draftId);
      return;
    }
    void api.workflowDrafts(activeProjectId || undefined).then(({ drafts: latest }) => {
      const recovered = latest.find((candidate) => candidate.id === draftId);
      if (recovered) {
        setDraft(recovered);
        onSelectContext({ draftId }, true);
        setPendingDraftFocus(draftId);
      }
      else onError("The saved Agent Draft is no longer available in this Project.");
    }).catch((error: Error) => onError(error.message));
  };
  const openManagedDraft = (draftId: string) => {
    const target = drafts.find((item) => item.id === draftId);
    if (!target) {
      onError(t("This Draft is unavailable. Refresh the Pipeline list and try again."));
      return;
    }
    if (draft && draft.id !== draftId && persistedDrafts.current.get(draft.id) !== JSON.stringify(draft)) {
      onError(t("Wait for the current Draft to finish saving, or resolve its save conflict before opening another Draft."));
      return;
    }
    selectedContextRef.current = { draftId, agentSessionId: undefined, published: "" };
    setAdvisorRunning(false);
    setActiveAgentSession(undefined);
    setAdvisorProposal(undefined);
    setProposalDiff(undefined);
    setDraft(target);
    setReport(undefined);
    setSelectedPublishedKey("");
    onSelectContext({ draftId });
    setPendingDraftFocus(draftId);
  };
  const applyProposalChanges = (changeIds = selectedProposalChanges) => {
    if (!draft || !advisorProposal || !proposalDiff)
      return onError("Create a Current Draft before applying Agent changes.");
    if (!changeIds.length)
      return onError("Select at least one proposed change, or reject the proposal.");
    setBusy(true);
    void api
      .applyWorkflowDraftDiff(
        draft.id,
        advisorProposal.draft.id,
        changeIds,
      )
      .then((report) => {
        persistedDrafts.current.set(report.draft.id, JSON.stringify(report.draft));
        setUndoDraft(report.previous_draft);
        setDraft(report.draft);
        setAdvisorProposal(undefined);
        setAdvisorProposalRecovered(false);
        setProposalDiff(undefined);
        setSelectedProposalChanges([]);
        setReport(undefined);
        setSavedAt(new Date());
        return onRefresh();
      })
      .catch((error: Error) => onError(error.message))
      .finally(() => setBusy(false));
  };
  const undoAgentApply = () => {
    if (!undoDraft || !draft) return;
    setBusy(true);
    void api
      .saveWorkflowDraft({ ...undoDraft, revision: draft.revision })
      .then((restored) => {
        persistedDrafts.current.set(restored.id, JSON.stringify(restored));
        setDraft(restored);
        setUndoDraft(undefined);
        setReport(undefined);
        setSavedAt(new Date());
        return onRefresh();
      })
      .catch((error: Error) => onError(`Undo failed: ${error.message}`))
      .finally(() => setBusy(false));
  };
  const targetTask = activeProject?.annotation_schema.find(
    (task) => task.id === targetTaskId,
  );
  const selectedAgentModel = (compatibleModels.pipeline_builder ?? []).find(
    (model) => model.id === selectedAgentModelId,
  );
  const selectedAgentProvider = registryProviders.find(
    (provider) => provider.id === selectedAgentModel?.provider_id,
  );
  const agentProjectBinding = projectModelBindings.find(
    (binding) =>
      binding.match_kind === "role" && binding.role === "pipeline_builder",
  );
  const liveAgentModels = (compatibleModels.pipeline_builder ?? []).filter(
    (model) =>
      registryProviders.find((provider) => provider.id === model.provider_id)
        ?.adapter === "open_ai_compatible",
  );
  const saveProjectModelChoice = (
    role: ModelBindingRole,
    capability: ModelCapability,
    modelProfileId: string,
    locked = true,
  ) => {
    if (!activeProjectId || modelBindingBusy) return;
    const inputs = projectModelBindings
      .filter(
        (binding) =>
          !(binding.match_kind === "role" && binding.role === role),
      )
      .map((binding) => ({
        capability: binding.capability,
        role: binding.role,
        match_kind: binding.match_kind,
        model_profile_id: binding.model_profile_id,
        locked: binding.locked,
      }));
    if (modelProfileId) {
      inputs.push({
        capability,
        role,
        match_kind: "role",
        model_profile_id: modelProfileId,
        locked,
      });
    }
    setModelBindingBusy(true);
    void api
      .saveProjectModelBindings(activeProjectId, inputs)
      .then(({ bindings }) => {
        setProjectModelBindings(bindings);
        if (role === "pipeline_builder") {
          setSelectedAgentModelId(modelProfileId);
        }
      })
      .catch((error: Error) => onError(`Model choice: ${error.message}`))
      .finally(() => setModelBindingBusy(false));
  };
  const discardChanges = () => {
    if (!draft) return;
    const persisted = persistedDrafts.current.get(draft.id);
    if (persisted) setDraft(JSON.parse(persisted) as WorkflowDraft);
    setReport(undefined);
  };
  const reloadConflictedDraft = () => {
    if (!draftConflict?.server) return void refreshDrafts();
    persistedDrafts.current.set(
      draftConflict.server.id,
      JSON.stringify(draftConflict.server),
    );
    setDraft(draftConflict.server);
    setDraftConflict(undefined);
    setReport(undefined);
  };
  const preserveConflictedDraft = () => {
    if (!draftConflict) return;
    const timestamp = new Date().toISOString();
    const copy: WorkflowDraft = {
      ...draftConflict.local,
      id: crypto.randomUUID(),
      name: `${draftConflict.local.name} (conflict copy)`,
      status: "editing",
      revision: 1,
      content_hash: "",
      created_at: timestamp,
      updated_at: timestamp,
    };
    setBusy(true);
    void api
      .saveWorkflowDraft(copy)
      .then((saved) => {
        persistedDrafts.current.set(saved.id, JSON.stringify(saved));
        setDraft(saved);
        setDraftConflict(undefined);
        onSelectContext({ draftId: saved.id }, true);
        return onRefresh();
      })
      .catch((error: Error) => onError(`Conflict copy failed: ${error.message}`))
      .finally(() => setBusy(false));
  };
  const archive = () => draft && finish(api.archiveWorkflowDraft(draft.id));
  const clonePublished = () =>
    selected &&
    finish(
      api.cloneWorkflowVersion(
        selected.workflow.workflow_id,
        Number(selected.workflow.version),
      ),
    );
  const createSafeDraft = () => {
    if (!selected) return onError("Select a published Workflow Version first.");
    finish(
      api.createGeometrySafeDraft(
        selected.workflow.workflow_id,
        Number(selected.workflow.version),
      ),
    );
  };
  const publishedEntries = entries.filter(({ project, workflow }) =>
    project.id === activeProjectId && workflow.source.startsWith("published draft"),
  );
  const compareVersions = () => {
    const left = publishedEntries.find(
      ({ workflow }) =>
        `${workflow.workflow_id}:${workflow.version}` === compareLeft,
    )?.workflow;
    const right = publishedEntries.find(
      ({ workflow }) =>
        `${workflow.workflow_id}:${workflow.version}` === compareRight,
    )?.workflow;
    if (!left || !right)
      return onError("Select two published Workflow Versions to compare.");
    setBusy(true);
    void api
      .compareWorkflowVersions(
        { workflow_id: left.workflow_id, version: Number(left.version) },
        { workflow_id: right.workflow_id, version: Number(right.version) },
      )
      .then(setComparison)
      .catch((error: Error) => onError(error.message))
      .finally(() => setBusy(false));
  };
  const updateNode = (
    index: number,
    patch: Partial<WorkflowDraft["nodes"][number]>,
  ) =>
    draft &&
    setDraft({
      ...draft,
      nodes: draft.nodes.map((node, nodeIndex) =>
        nodeIndex === index ? { ...node, ...patch } : node,
      ),
    });
  const addNode = () =>
    draft &&
    setDraft({
      ...draft,
      nodes: [
        ...draft.nodes,
        {
          id: `node_${draft.nodes.length + 1}`,
          node_type: "core.resize",
          kind: "transform",
          depends_on: [],
          inputs: [{ id: "image", artifact_type: "image", required: true, multiple: false }],
          outputs: [{ id: "image", artifact_type: "image", required: true, multiple: false }],
          validators: [],
          refiners: [],
          max_retries: 0,
          review_gate: false,
          parameters: { max_edge: 1600, allow_upscale: false },
        },
      ],
    });
  const removeNode = (index: number) =>
    draft &&
    setDraft({
      ...draft,
      nodes: draft.nodes.filter((_, nodeIndex) => nodeIndex !== index),
    });
  const addEdge = () =>
    draft &&
    draft.nodes.length > 1 &&
    setDraft({
      ...draft,
      edges: [
        ...(draft.edges ?? []),
        {
          from_node: draft.nodes[0].id,
          from_port: draft.nodes[0].outputs?.[0]?.id ?? "output",
          to_node: draft.nodes[1].id,
          to_port: draft.nodes[1].inputs?.[0]?.id ?? "input",
        },
      ],
    });
  const removeEdge = (index: number) =>
    draft &&
    setDraft({
      ...draft,
      edges: (draft.edges ?? []).filter((_, edgeIndex) => edgeIndex !== index),
    });
  const updateEdge = (
    index: number,
    patch: Partial<NonNullable<WorkflowDraft["edges"]>[number]>,
  ) =>
    draft &&
    setDraft({
      ...draft,
      edges: (draft.edges ?? []).map((edge, edgeIndex) =>
        edgeIndex === index ? { ...edge, ...patch } : edge,
      ),
    });
  const immutable =
    draft?.status === "published" || draft?.status === "archived";
  const registryModelProfiles = Object.values(compatibleModels).flat();
  const executedNodeIds = new Set(
    report?.samples.flatMap((sample) => sample.nodes.map((node) => node.node_id)) ?? [],
  );
  const pipelineModelCalls = (draft?.nodes ?? [])
    .filter((node) =>
      Boolean(node.model_binding || node.model_profile_binding) &&
      ["vision_model", "vision_language_model", "refiner"].includes(node.kind ?? ""),
    )
    .map((node) => {
      const profile = registryModelProfiles.find(
        (model) => model.id === node.model_profile_binding?.model_profile_id,
      );
      const instanceId = node.model_binding?.startsWith("model-instance:")
        ? node.model_binding.slice("model-instance:".length)
        : undefined;
      const expert = catalog?.expert_models.find(
        (model) =>
          model.model_id === node.model_binding ||
          model.metadata.model_instance_id === instanceId,
      );
      const executedImageCount = report?.samples.filter((sample) =>
        sample.nodes.some((result) => result.node_id === node.id && !["skipped", "cancelled", "pending"].includes(result.status)),
      ).length ?? 0;
      return {
        node,
        modelName: profile?.display_name ?? expert?.display_name ?? node.model_binding ?? "Unresolved model",
        providerName: profile
          ? registryProviders.find((provider) => provider.id === profile.provider_id)?.display_name
          : undefined,
        executedImageCount,
      };
    });
  const recoveryIds = recoveryNodeIds(draft ?? { nodes: [], edges: [] });
  const mainModelCalls = pipelineModelCalls.filter(({ node }) => !recoveryIds.has(node.id));
  const recoveryModelCalls = pipelineModelCalls.filter(({ node }) => recoveryIds.has(node.id));
  const renderModelCall = ({ node, modelName, providerName, executedImageCount }: typeof pipelineModelCalls[number], index: number) => (
    <li key={node.id} className={report && !executedImageCount ? "not-reached" : ""}>
      <span className="pipeline-model-call-index">{index + 1}</span>
      <span className="pipeline-model-call-copy"><strong>{node.node_type === "vlm_detection.detect" ? t(node.parameters.coordinate_space === "local_crop" ? "Locate within crops" : "Find candidates") : t(workflowNodeTitle(node.node_type))}</strong><small>{node.id.replaceAll("_", " ")}</small></span>
      <span className="pipeline-model-call-model"><strong>{modelName}</strong>{providerName && <small>via {providerName}</small>}</span>
      <span className={`status ${!report || executedImageCount ? "status-auto-accepted" : "status-needs-review"}`}>{!report ? t("Configured") : executedImageCount ? t("Reached on {count} images", { count: executedImageCount }) : t("Not reached")}</span>
    </li>
  );
  const latestPromptCoverage = report?.samples
    .flatMap((sample) => sample.projection?.debug_stages ?? [])
    .find((stage) => stage.stage === "prompt_coverage");
  const configuredRefinerSkipped = Boolean(report) && pipelineModelCalls.some(
    ({ node }) => node.node_type.toLowerCase().includes("segment") && !executedNodeIds.has(node.id),
  );
  const geometryBlockingCodes = new Set([
    "uncalibrated_geometry_auto_commit",
    "semantic_score_used_as_geometry_evidence",
    "geometry_acceptance_path_missing",
    "geometry_calibration_missing",
    "geometry_calibration_stale",
    "unsafe_legacy_workflow",
  ]);
  const geometryBlockingIssues = report?.validation.issues.filter(
    (issue) => issue.blocking && geometryBlockingCodes.has(issue.code),
  ) ?? [];
  if (activeProject && !buildSummary)
    return <section className="page-stack"><ProjectBreadcrumb project={activeProject} current="Build" onOpenProjects={onOpenProjects} onOpenProject={onOpenProject} /><BuildNavigation step="pipeline" onNavigate={onNavigate} /><div className="loading-banner" role="status">{t("Loading Build readiness…")}</div></section>;
  if (buildSummary && !buildStepAllowed(buildSummary.guidance, "pipeline"))
    return <section className="page-stack"><ProjectBreadcrumb project={activeProject} current="Build" onOpenProjects={onOpenProjects} onOpenProject={onOpenProject} /><BuildNavigation step="pipeline" guidance={buildSummary.guidance} onNavigate={onNavigate} /><BuildBlocker guidance={buildSummary.guidance} onNavigate={onNavigate} /></section>;
  if (preparation) return <TaskModelPreparation section={preparation} onSelect={setPreparation} onError={onError} onClose={async () => {
    await refreshModelChoices();
    setPreparation(undefined);
    requestAnimationFrame(() => document.getElementById("prepare-task-models")?.focus());
  }} />;
  return (
    <section className="page-stack">
      <ProjectBreadcrumb project={activeProject} current="Build" onOpenProjects={onOpenProjects} onOpenProject={onOpenProject} />
      <BuildNavigation step="pipeline" guidance={buildSummary?.guidance} onNavigate={(step) => onNavigate(step, step === "test" ? draft?.id : undefined)} />
      <div className="toolbar-panel workflow-designer-header">
        <div>
          <span className="eyebrow">{t("Step 3 · Automation")}</span>
          <h2>{t("How AnnotAgent will label your data")}</h2>
          <p>{t("Start from a registered recipe or Advisor suggestion, then edit the same autosaved Draft. Technical graph details remain available for expert inspection.")}</p>
        </div>
        <div className="button-row">
          <small className="save-indicator" aria-live="polite" title={savedAt?.toISOString() ?? draft?.updated_at}>{draftConflict || autosaveError ? t("Save failed") : draftHasUnsavedChanges ? t("Saving…") : draft ? t("Saved") : t("No Current Draft")}</small>
          <button
            onClick={() => create(false)}
            disabled={busy || !activeProjectId}
          >{t("New Draft")}</button>
        </div>
      </div>
      {draftConflict && (
        <section className="draft-conflict-panel" role="alert">
          <div>
            <span className="eyebrow">{t("Draft conflict")}</span>
            <h3>{t("This Draft changed in another tab")}</h3>
            <p>{t("Your revision")}{" "}{draftConflict.local.revision}{" "}{t("was not saved over server revision")}{" "}{draftConflict.server?.revision ?? "unknown"}{t(". Choose which copy to continue with; no edits were discarded.")}</p>
            <details>
              <summary>{t("Compare Draft snapshots")}</summary>
              <div className="draft-conflict-comparison">
                <div><strong>{t("Your unsaved copy")}</strong><pre>{JSON.stringify(draftConflict.local, null, 2)}</pre></div>
                <div><strong>{t("Latest server copy")}</strong><pre>{draftConflict.server ? JSON.stringify(draftConflict.server, null, 2) : draftConflict.message}</pre></div>
              </div>
            </details>
          </div>
          <div className="button-row">
            <button onClick={preserveConflictedDraft} disabled={busy}>{t("Save mine as new Draft")}</button>
            <button className="primary" onClick={reloadConflictedDraft} disabled={!draftConflict.server || busy}>{t("Reload latest Draft")}</button>
          </div>
        </section>
      )}
      {activeProject && <PipelineManagementPanel
        project={activeProject}
        currentDraftId={draft?.id}
        currentDraftDirty={Boolean(draft && persistedDrafts.current.get(draft.id) !== JSON.stringify(draft))}
        onOpenDraft={openManagedDraft}
        onChanged={() => Promise.all([refreshDrafts(), onRefresh()]).then(() => undefined)}
        onCurrentDraftRemoved={() => {
          autosaveController.current?.abort();
          if (autosaveTimer.current) window.clearTimeout(autosaveTimer.current);
          setDraft(undefined);
          onSelectContext({}, true);
        }}
        onOpenTrash={onOpenTrash}
        onError={onError}
      />}
      <section className="pipeline-model-overview" aria-labelledby="pipeline-model-overview-title">
        <header>
          <div><span className="eyebrow">{t("Runtime model calls")}</span><h2 id="pipeline-model-overview-title">{t("Models this automation will call")}</h2><p>{t("The Builder LLM plans the Draft; only the calls below process your images.")}</p></div>
          <strong aria-label={t("Main model stages")}>{mainModelCalls.length}</strong>
        </header>
        {pipelineModelCalls.length ? <ol className="pipeline-model-call-list" aria-label={t("Main model stages")}>
          {mainModelCalls.map(renderModelCall)}
        </ol> : <div className="pipeline-model-empty"><strong>{t("No image-processing model is bound")}</strong><span>Choose a Draft or ask AnnotAgent to propose a model-backed automation.</span></div>}
        {!!recoveryModelCalls.length && <details className="pipeline-recovery-stages">
          <summary>{t("Conditional recovery")} · {recoveryModelCalls.length}</summary>
          <p>{t("Runs only when the gate requests another search. These are not additional steps for every image.")}</p>
          <ol className="pipeline-model-call-list">{recoveryModelCalls.map(renderModelCall)}</ol>
        </details>}
        {!!pipelineModelCalls.length && <small>{t("Stage count is not a per-image request estimate. Crops can fan out; recovery is conditional.")}</small>}
        {configuredRefinerSkipped && <aside className="pipeline-model-warning"><strong>Segmentation was configured but did not run in the latest Sample Test.</strong><span>{latestPromptCoverage?.detail ?? t("An earlier gate routed the candidate away from refinement.")}</span></aside>}
      </section>
      <div className="workflow-command-grid">
        <details className="workflow-command-card workflow-start-options">
          <summary>{t("Start from a template")}</summary>
          <div className="workflow-template-controls">
            <p>Use a registered starting recipe instead of asking the Builder Agent.</p>
            <select
              aria-label={t("Workflow template")}
              value={templateId}
              onChange={(event) => setTemplateId(event.target.value)}
            >
              <option value="">{t("Generic project template")}</option>
              {(catalog?.workflow_templates ?? []).map((template) => (
                <option key={template.id} value={template.id}>
                  {template.name}
                </option>
              ))}
            </select>
            <button
              onClick={() => create(true, templateId || undefined)}
              disabled={busy || !activeProjectId}
            >{t("Create from Template")}</button>
          </div>
        </details>
        <section className="workflow-command-card workflow-advisor-recommendation">
          <span className="eyebrow">{t("Builder LLM · does not label images")}</span>
          <h3>{t("Build a recommended automation")}</h3>
          <p>{targetLabel ? `Set the boundaries for ${targetLabel}. The Agent may inspect, draft, validate, and Dry Run, but it cannot activate the result.` : t("Choose a Label and bounded objective before starting the Agent.")}</p>
          {registryLoading ? (
            <div className="loading-banner compact" role="status">{t("Finding compatible Agent models…")}</div>
          ) : liveAgentModels.length ? (
            <fieldset className="agent-model-choice">
              <legend>{t("Builder LLM")}</legend>
              <label>{t("Model Profile")}<select
                  value={selectedAgentModelId}
                  disabled={modelBindingBusy || advisorRunning}
                  onChange={(event) =>
                    saveProjectModelChoice(
                      "pipeline_builder",
                      "text_generation",
                      event.target.value,
                      true,
                    )
                  }
                >
                  {liveAgentModels.map((model) => {
                    const provider = registryProviders.find(
                      (candidate) => candidate.id === model.provider_id,
                    );
                    return (
                      <option key={model.id} value={model.id}>
                        {model.display_name} via {provider?.display_name ?? t("Provider")}
                      </option>
                    );
                  })}
                </select>
              </label>
              {selectedAgentModel && selectedAgentProvider && (
                <div className="agent-model-summary" aria-live="polite">
                  <span>
                    <strong>{selectedAgentModel.display_name}</strong>
                    <small>via {selectedAgentProvider.display_name}</small>
                  </span>
                  <Status status={selectedAgentModel.status} />
                  <div className="tag-group">
                    <span>{t("Plans the pipeline")}</span>
                    <span>{t("Not a Runtime image call")}</span>
                    <span>
                      {agentProjectBinding
                        ? t("Project choice")
                        : globalModelDefaults.pipeline_builder ===
                            selectedAgentModel.id
                          ? t("Global default")
                          : t("Compatible fallback")}
                    </span>
                  </div>
                </div>
              )}
              <label className="checkbox-row">
                <input
                  type="checkbox"
                  checked={agentProjectBinding?.locked ?? true}
                  disabled={
                    modelBindingBusy || !selectedAgentModelId || advisorRunning
                  }
                  onChange={(event) =>
                    saveProjectModelChoice(
                      "pipeline_builder",
                      "text_generation",
                      selectedAgentModelId,
                      event.target.checked,
                    )
                  }
                />{t("Lock this Project choice so the Agent cannot replace it")}</label>
            </fieldset>
          ) : (
            <div className="inline-provider-setup">
              <h4>{t("Provider setup required")}</h4>
              <p>{t("Pipeline Builder needs an available text model with Tool Calls and Structured Output. Your images and goal are already saved; you can prepare a model here or return later.")}</p>
              <button id="prepare-task-models" className="primary" onClick={onOpenModels}>{t("Prepare models in this task")}</button>
            </div>
          )}
          <fieldset className="agent-objective agent-objective-primary" aria-label={t("Pipeline Builder objective")}>
            <legend>{t("Annotation goal")}</legend>
            <label>{t("Target task")}<select aria-label={t("Target task")} value={targetTaskId} onChange={(event) => {
              const taskId = event.target.value;
              setTargetTaskId(taskId);
              setTargetLabel(activeProject?.annotation_schema.find((task) => task.id === taskId)?.labels[0] ?? "");
            }}>
              {(activeProject?.annotation_schema ?? []).map((task) => <option key={task.id} value={task.id}>{task.id} · {task.kind}</option>)}
            </select></label>
            <label>{t("Target Label")}<select aria-label={t("Target Label")} value={targetLabel} onChange={(event) => setTargetLabel(event.target.value)}>
              {(targetTask?.labels ?? []).map((label) => <option key={label} value={label}>{label}</option>)}
            </select></label>
            <label>{t("Priority")}<select aria-label={t("Optimization priority")} value={builderConstraints.priority} onChange={(event) => setBuilderConstraints((current) => ({ ...current, priority: event.target.value as OptimizationPriority }))}>
              <option value="balanced">{t("Balanced")}</option>
              <option value="accurate">{t("Accuracy first")}</option>
              <option value="fast">{t("Speed first")}</option>
              <option value="low_cost">{t("Lowest cost")}</option>
            </select></label>
          </fieldset>
          <details className="builder-advanced-options">
            <summary>{t("Build mode, review and limits")}</summary>
            <fieldset className="agent-objective" aria-label={t("Advanced Pipeline Builder objective")}>
              <legend>{t("Advanced constraints")}</legend>
              <label>{t("Build mode")}<select
                  aria-label={t("Pipeline Build mode")}
                  value={buildModeKind}
                  onChange={(event) =>
                    setBuildModeKind(
                      event.target.value as PipelineBuildMode["kind"],
                    )
                  }
                >
                  <option value="from_scratch">{t("Build from scratch")}</option>
                  <option value="repair_draft" disabled={!draft || ["published", "archived"].includes(draft.status)}>{t("Repair current Draft")}</option>
                  <option value="resolve_bindings" disabled={!draft || ["published", "archived"].includes(draft.status)}>{t("Resolve current bindings")}</option>
                  <option value="improve_existing" disabled={!selected?.workflow.source.startsWith("published draft")}>{t("Improve Published Version")}</option>
                </select>
                <small>
                  {buildModeKind === "from_scratch"
                    ? t("Starts with a new empty Working Draft.")
                    : buildModeKind === "improve_existing"
                      ? t("Uses the selected immutable Version as an explicit base.")
                      : buildModeKind === "resolve_bindings"
                        ? t("Keeps the graph and resolves its model bindings.")
                        : t("Keeps the selected editable Draft and its identity.")}
                </small>
              </label>
              <label>{t("Maximum cost per image")}<input aria-label={t("Maximum cost per image")} inputMode="decimal" placeholder={t("No per-image limit")} value={builderConstraints.max_cost_per_image ?? ""} onChange={(event) => setBuilderConstraints((current) => ({ ...current, max_cost_per_image: event.target.value || undefined }))} /></label>
              <label>{t("Maximum latency (ms)")}<input aria-label={t("Maximum latency")} type="number" min="1" placeholder={t("No latency limit")} value={builderConstraints.max_expected_latency_ms ?? ""} onChange={(event) => setBuilderConstraints((current) => ({ ...current, max_expected_latency_ms: event.target.value ? Number(event.target.value) : undefined }))} /></label>
              <label>{t("Desired Review workload")}<input aria-label={t("Desired review rate")} type="number" min="0" max="100" value={Math.round((builderConstraints.target_review_rate ?? 0) * 100)} onChange={(event) => setBuilderConstraints((current) => ({ ...current, target_review_rate: Number(event.target.value) / 100 }))} /><small>{t("Percent of decided candidates")}</small></label>
              <label className="checkbox-row"><input type="checkbox" checked={builderConstraints.allow_external_models} onChange={(event) => setBuilderConstraints((current) => ({ ...current, allow_external_models: event.target.checked }))} />{t("Allow configured external APIs")}</label>
              <label className="checkbox-row"><input type="checkbox" checked={builderConstraints.allow_human_review} onChange={(event) => setBuilderConstraints((current) => ({ ...current, allow_human_review: event.target.checked }))} />{t("Allow Human Review")}</label>
              <div className="agent-worker-summary">
                <span>{t("Available local expert models")}</span>
                <strong>
                  {catalog?.expert_models
                    .filter(
                      (model) =>
                        model.availability === "available" &&
                        model.availability_evidence.health_passed &&
                        model.availability_evidence.protocol_compatible &&
                        model.availability_evidence.contracts_validated &&
                        model.availability_evidence.sample_conversion_passed &&
                        model.availability_evidence.weights_ready,
                    )
                    .map((model) => model.display_name)
                    .join(", ") || t("None ready")}
                </strong>
              </div>
            </fieldset>
          </details>
          <details className="project-model-choices">
            <summary>{t("Project model choices")}</summary>
            <p>
              Reuse Available Model Profiles for each capability. Node overrides
              remain part of the Draft and are frozen when published.
            </p>
            <div>
              {PROJECT_MODEL_CHOICES.filter(
                (choice) => choice.role !== "pipeline_builder",
              ).map((choice) => {
                const binding = projectModelBindings.find(
                  (candidate) =>
                    candidate.match_kind === "role" &&
                    candidate.role === choice.role,
                );
                const models = compatibleModels[choice.role] ?? [];
                return (
                  <label key={choice.role}>
                    {t(choice.label)}
                    <select
                      aria-label={t(choice.label)}
                      value={binding?.model_profile_id ?? ""}
                      disabled={modelBindingBusy}
                      onChange={(event) =>
                        saveProjectModelChoice(
                          choice.role,
                          choice.capability,
                          event.target.value,
                          true,
                        )
                      }
                    >
                      <option value="">{t("Use a node-specific choice")}</option>
                      {models.map((model) => {
                        const provider = registryProviders.find(
                          (candidate) => candidate.id === model.provider_id,
                        );
                        return (
                          <option key={model.id} value={model.id}>
                            {model.display_name} via {provider?.display_name ?? t("Provider")}
                          </option>
                        );
                      })}
                    </select>
                    <small>
                      {models.length
                        ? `${models.length} compatible and Available`
                        : t("No compatible Profile is Available")}
                    </small>
                  </label>
                );
              })}
            </div>
            <div className="button-row">
              <button onClick={onOpenModels}>{t("Manage Model Profiles")}</button>
              <button onClick={onOpenProviders}>{t("Manage Providers")}</button>
            </div>
          </details>
          <details className="advanced-settings"><summary>{t("Agent limits and provider")}</summary><div className="workflow-advisor-fields">
            <p className="field-note">AnnotAgent uses the selected live Agent model. Test fixtures are never eligible for generated Pipelines.</p>
            <label>{t("Maximum model calls / image")}<input type="number" min="1" max="16" value={builderConstraints.max_model_calls_per_image ?? ""} onChange={(event) => setBuilderConstraints((current) => ({ ...current, max_model_calls_per_image: event.target.value ? Number(event.target.value) : undefined }))} /></label>
            <label>{t("Maximum Agent turns")}<input type="number" min="1" max="64" value={builderConstraints.maximum_agent_turns} onChange={(event) => setBuilderConstraints((current) => ({ ...current, maximum_agent_turns: Number(event.target.value) }))} /></label>
            <label>{t("Maximum Tool Calls")}<input type="number" min="1" max="128" value={builderConstraints.maximum_tool_calls} onChange={(event) => setBuilderConstraints((current) => ({ ...current, maximum_tool_calls: Number(event.target.value) }))} /></label>
            <label>{t("Maximum Dry Runs")}<input type="number" min="1" max="10" value={builderConstraints.maximum_dry_runs} onChange={(event) => setBuilderConstraints((current) => ({ ...current, maximum_dry_runs: Number(event.target.value) }))} /></label>
            <label>{t("Maximum Agent cost")}<input inputMode="decimal" value={builderConstraints.maximum_agent_cost} onChange={(event) => setBuilderConstraints((current) => ({ ...current, maximum_agent_cost: event.target.value }))} /></label>
            <button onClick={suggest} disabled={busy || !activeProjectId}>{t("Build complete Project automation")}</button>
          </div></details>
          <button className={activeAgentSession?.draft_id ? undefined : "primary"} onClick={suggestLabelPipeline} disabled={busy || advisorRunning || !activeProjectId || !targetTaskId || !targetLabel || !selectedAgentModelId}>{advisorRunning ? t("Agent is working…") : t("Ask AnnotAgent")}</button>
        </section>
        <section className="workflow-command-card workflow-version-actions">
          <span className="eyebrow">{t("Current Automation")}</span>
          <h3>{immutable ? t("Immutable Version") : t("Autosaved Draft")}</h3>
          <p>{immutable ? t("Clone this Version before making changes.") : t("Edits stay unpublished until you test and activate them in the next step.")}</p>
          <div className="button-row">
            {!immutable && undoDraft && <button onClick={undoAgentApply} disabled={busy}>{t("Undo Agent changes")}</button>}
            {!immutable && <button onClick={discardChanges} disabled={busy || !draft}>{t("Discard")}</button>}
            {!immutable && <button onClick={() => onNavigate("test", draft?.id)} disabled={busy || !draft}>{t("Open Test & Activate")}</button>}
            {immutable && <button onClick={clonePublished} disabled={busy || !selected?.workflow.source.startsWith("published draft")}>{t("Clone to Draft")}</button>}
            <button onClick={() => document.getElementById("improve-automation")?.scrollIntoView({ behavior: "smooth" })}>{t("Improve from evidence")}</button>
            {!immutable && draft && <details className="action-menu"><summary>{t("More")}</summary><div><button onClick={archive} disabled={busy}>{t("Archive")}</button></div></details>}
          </div>
        </section>
      </div>
      {geometryBlockingIssues.length > 0 && <section className="geometry-safety-blocker" role="alert">
        <div>
          <span className="eyebrow">{t("Publication blocked")}</span>
          <h2>{t("Automatic acceptance is unsafe")}</h2>
          <p>The selected model score describes semantic or relative confidence, but the bounding boxes do not have valid geometry evidence for this Project.</p>
          <ul>{geometryBlockingIssues.map((issue) => <li key={`${issue.code}:${issue.path}`}><strong>{issue.code.replaceAll("_", " ")}</strong><span>{issue.message}</span></li>)}</ul>
        </div>
        <div className="geometry-repair-actions">
          <button className="primary" disabled={busy || !selected?.workflow.source.startsWith("published draft")} onClick={createSafeDraft}>{t("Require human review")}</button>
          <button onClick={() => document.getElementById("improve-automation")?.scrollIntoView({ behavior: "smooth" })}>{t("Add compatible refiner")}</button>
          <button onClick={() => {
            document.querySelector<HTMLDetailsElement>(".geometry-calibration-panel")?.setAttribute("open", "");
            document.getElementById("improve-automation")?.scrollIntoView({ behavior: "smooth" });
          }}>{t("Run geometry calibration")}</button>
        </div>
      </section>}
      {(advisorRunning || (activeAgentSession && !advisorProposal)) && (
        <Panel title={t("Agent progress")} eyebrow={advisorRunning ? t("Live persisted Pipeline Builder session") : t("Recovered persisted Pipeline Builder session")}>
          {activeAgentSession ? (
            <AgentSessionTrace
              session={activeAgentSession}
              onRetry={() => retryAgentSession(activeAgentSession)}
              onOpenDraft={openAgentDraft}
              onConfigureProvider={onOpenProviders}
              onConfigureModel={onOpenModels}
              onCancel={() =>
                void api
                  .cancelAgentSession(activeAgentSession.id)
                  .then(({ session }) => setActiveAgentSession(session))
                  .catch((error: Error) => onError(error.message))
              }
            />
          ) : (
            <div className="loading-banner" role="status">{t("Starting the bounded Agent session…")}</div>
          )}
        </Panel>
      )}
      {advisorProposal && (
        <Panel
          title={advisorProposalRecovered ? t("Saved Agent Result") : t("Proposed Changes")}
          eyebrow={advisorProposalRecovered ? t("Recovered from server · editable Draft · not activated") : t("Advisor preview · Draft only · never activated automatically")}
        >
          {advisorProposalRecovered && <div className="saved-agent-result-summary">
            <span><strong>{advisorProposal.draft.name}</strong><small>{advisorProposal.estimated_model_calls_per_image}{" "}{t("Runtime model calls per image ·")}{" "}{advisorProposal.unresolved_model_bindings.length ? `${advisorProposal.unresolved_model_bindings.length} unresolved bindings` : "bindings resolved"}</small></span>
            <Status status={advisorProposal.draft.status} />
          </div>}
          <details
            className="advisor-result-details"
            open={
              !advisorProposalRecovered ||
              ["provider_setup_required", "blocked_draft_ready"].includes(
                advisorProposal.agent_session?.outcome ?? "",
              )
            }
          >
            <summary>{advisorProposalRecovered ? t("View Builder reasoning and diagnostics") : t("Review proposed automation")}</summary>
            <div className="advisor-result-details-body">
          <div className="advisor-proposal-grid">
            <div>
              <h3>{t("Automation Recipe")}</h3>
              <ol className="advisor-recipe-list">
                {guidedWorkflowNodes(advisorProposal.draft.nodes).map((node) => <li key={node.id}>{workflowNodeTitle(node.node_type)} <small>{node.model_binding ?? "Core"}</small></li>)}
              </ol>
            </div>
            <div className="fact-grid">
              <Fact label={t("Model calls / image")} value={advisorProposal.estimated_model_calls_per_image} />
              <Fact label={t("Estimated latency")} value={advisorProposal.estimated_latency_ms ? `${advisorProposal.estimated_latency_ms} ms` : "Unresolved"} />
              <Fact label={t("Cost tier")} value={advisorProposal.estimated_cost_tier} />
              <Fact label={t("Expected Review workload")} value={advisorProposal.agent_dry_run ? `${advisorProposal.agent_dry_run.summary.needs_review_count} of ${advisorProposal.agent_dry_run.summary.image_count} samples` : "Test required"} />
              <Fact label={advisorProposalRecovered ? t("Persistence") : t("Compared with current")} value={advisorProposalRecovered ? "Saved on server" : draft ? `${advisorProposal.draft.nodes.length - draft.nodes.length >= 0 ? "+" : ""}${advisorProposal.draft.nodes.length - draft.nodes.length} nodes` : "No Current Draft"} />
            </div>
          </div>
          {showProposalComparison && proposalDiff && (
            <fieldset className="advisor-change-preview" aria-label={t("Draft Diff")}>
              <legend>{t("Review changes")}</legend>
              {pipelineDiffRows(proposalDiff, advisorProposal.draft).map((change) => (
                <label className={change.tone} key={change.id}>
                  <input
                    type="checkbox"
                    checked={selectedProposalChanges.includes(change.id)}
                    onChange={(event) => setSelectedProposalChanges((current) =>
                      event.target.checked
                        ? [...current, change.id]
                        : current.filter((id) => id !== change.id))}
                  />
                  <span>{change.tone === "added" ? "+" : change.tone === "removed" ? "−" : "~"} {change.label}</span>
                </label>
              ))}
              {!pipelineDiffChangeIds(proposalDiff).length && <p>{t("No changes from the Current Draft.")}</p>}
            </fieldset>
          )}
          <TagGroup title={t("Why")} values={advisorProposal.rationale} />
          <TagGroup title={t("Unresolved bindings")} values={advisorProposal.unresolved_model_bindings} />
          {!!advisorProposal.unresolved_model_bindings.length && <div className="unresolved-plugin-action"><span><strong>{t("A required Expert Model capability is not Ready.")}</strong><small>Inspect compatible installed contracts, add legal checkpoint files, and run the isolated Rust process test. AnnotAgent will keep this as a blocked Draft until you retry.</small></span><button onClick={onOpenPlugins}>{t("Install or finish model setup")}</button></div>}
          <TagGroup title={t("Warnings")} values={advisorProposal.warnings} />
          <TagGroup title={t("Alternatives")} values={advisorProposal.alternatives} />
          {advisorProposal.agent_session && (
            <AgentSessionTrace
              session={advisorProposal.agent_session}
              validation={advisorProposal.agent_validation}
              dryRun={advisorProposal.agent_dry_run}
              onRetry={() => retryAgentSession(advisorProposal.agent_session!)}
              onOpenDraft={openAgentDraft}
              onConfigureProvider={onOpenProviders}
              onConfigureModel={onOpenModels}
              onCancel={() =>
                void api
                  .cancelAgentSession(advisorProposal.agent_session!.id)
                  .then(({ session }) =>
                    setAdvisorProposal((current) =>
                      current ? { ...current, agent_session: session } : current,
                    ),
                  )
                  .catch((error: Error) => onError(error.message))
              }
            />
          )}
          {!advisorProposalRecovered && <div className="button-row">
            <>
              <button className="primary" onClick={() => applyProposalChanges()} disabled={!proposalDiff || !selectedProposalChanges.length || busy}>{t("Apply selected")}</button>
              <button onClick={() => proposalDiff && applyProposalChanges(pipelineDiffChangeIds(proposalDiff))} disabled={!proposalDiff || !pipelineDiffChangeIds(proposalDiff).length || busy}>{t("Apply all")}</button>
              <button onClick={() => setShowProposalComparison((value) => !value)}>{showProposalComparison ? t("Hide comparison") : t("Compare with current")}</button>
              <button onClick={() => { setAdvisorProposal(undefined); setAdvisorProposalRecovered(false); setProposalDiff(undefined); setSelectedProposalChanges([]); }}>{t("Reject proposal")}</button>
            </>
          </div>}
            </div>
          </details>
          {advisorProposalRecovered && <div className="button-row">
            <button className="primary" onClick={() => openAgentDraft(advisorProposal.draft.id)}>{t("Open saved Draft")}</button>
            <button onClick={() => { setAdvisorProposal(undefined); setAdvisorProposalRecovered(false); }}>{t("Dismiss result")}</button>
          </div>}
        </Panel>
      )}
      {activeProject && <ImproveAutomationPanel
        project={activeProject}
        runs={runs}
        workflows={publishedEntries.map(({ workflow }) => workflow)}
        selectedSessionId={selectedImprovementSessionId}
        onSelectSession={(improvementSessionId, replace) =>
          onSelectContext({
            draftId: draft?.id,
            improvementSessionId,
            agentSessionId: activeAgentSession?.id,
          }, replace)
        }
        onDraftApplied={(draftId) => {
          void api.workflowDrafts(activeProject.id).then(({ drafts: latest }) => {
            const applied = latest.find((candidate) => candidate.id === draftId);
            if (applied) setDraft(applied);
            return refreshDrafts();
          }).catch((error: Error) => onError(error.message));
        }}
        onError={onError}
      />}
      <details className="panel version-history">
        <summary>{t("Version History")}</summary>
      <div className="toolbar-panel">
        <div>
          <span className="eyebrow">{t("Version comparison")}</span>
          <p>{t("Compare immutable node sets and content hashes.")}</p>
        </div>
        <div className="button-row">
          <select
            aria-label={t("Left Workflow Version")}
            value={compareLeft}
            onChange={(event) => setCompareLeft(event.target.value)}
          >
            <option value="">{t("Left version…")}</option>
            {publishedEntries.map(({ workflow }) => (
              <option
                key={`left-${workflow.workflow_id}-${workflow.version}`}
                value={`${workflow.workflow_id}:${workflow.version}`}
              >
                {workflow.name} · v{workflow.version}
              </option>
            ))}
          </select>
          <select
            aria-label={t("Right Workflow Version")}
            value={compareRight}
            onChange={(event) => setCompareRight(event.target.value)}
          >
            <option value="">{t("Right version…")}</option>
            {publishedEntries.map(({ workflow }) => (
              <option
                key={`right-${workflow.workflow_id}-${workflow.version}`}
                value={`${workflow.workflow_id}:${workflow.version}`}
              >
                {workflow.name} · v{workflow.version}
              </option>
            ))}
          </select>
          <button
            onClick={compareVersions}
            disabled={busy || !compareLeft || !compareRight}
          >{t("Compare")}</button>
        </div>
        {comparison && (
          <small>
            {comparison.same_content ? t("Same content") : t("Different content")} · +
            {comparison.added_nodes.length} / −{comparison.removed_nodes.length}{" "}
            / changed {comparison.changed_nodes.length}
          </small>
        )}
      </div>
      </details>
      <details className="workflow-edit-details"><summary>{t("Edit plan and advanced configuration")}</summary><div className="workflow-layout">
        <aside className="panel workflow-list">
          <span className="eyebrow">{t("Current Draft")}</span>
          <h2>{draft ? draft.name : t("No Current Draft")}</h2>
          {draft && (
            <button
              key={draft.id}
              className="active"
            >
              <span>
                <strong>{draft.name}</strong>
                <small>
                  {projects.find((project) => project.id === draft.project_id)
                    ?.name ?? draft.project_id}
                </small>
              </span>
              <Status status={draft.status} />
            </button>
          )}
          {drafts.length === 0 && (
            <Empty
              title={t("No drafts")}
              detail="Create a blank Draft, use a template, or ask the registry-bound Advisor."
            />
          )}
          {drafts.length > 1 && (
            <details className="draft-history">
              <summary>{t("Historical Drafts (")}{drafts.filter((item) => item.id !== draft?.id).length})</summary>
              {drafts.filter((item) => item.id !== draft?.id).map((item) => (
                <button key={item.id} onClick={() => { setDraft(item); setReport(undefined); onSelectContext({ draftId: item.id }); }}>
                  <span><strong>{item.name}</strong><small>{item.updated_at}</small></span>
                  <Status status={item.status} />
                </button>
              ))}
            </details>
          )}
          <span className="eyebrow workflow-published-title">{t("Default Published Version")}</span>
          {entries.filter(({ project, workflow }) => project.id === activeProjectId && workflow.is_default).map(({ project, workflow }) => (
            <button
              key={`${project.id}-${workflow.workflow_id}-${workflow.version}`}
              onClick={() => {
                onActivate(project.id);
                setSelectedPublishedKey(
                  `${workflow.workflow_id}:${workflow.version}`,
                );
                onSelectContext({
                  workflowId: workflow.workflow_id,
                  workflowVersion: Number(workflow.version),
                });
                setDraft(undefined);
                setReport(undefined);
              }}
            >
              <span>
                <strong>{workflow.name}</strong>
                <small>
                  {project.name} · v{workflow.version}
                </small>
              </span>
              <Status status={workflow.status} />
            </button>
          ))}
        </aside>
        <div ref={draftEditorRef} tabIndex={-1} role="region" aria-label={t("Draft editor")} data-draft-id={draft?.id}>
          {draft ? (
            <Panel
              title={draft.name}
              eyebrow={`${draft.status} · autosaved Automation Draft`}
            >
              {!immutable && <label className="workflow-draft-name">{t("Draft name")}<input value={draft.name} onChange={(event) => setDraft({ ...draft, name: event.target.value })} />
              </label>}
              {draft.label_pipeline && (
                <LabelPipelineEditor
                  draft={draft}
                  catalog={catalog}
                  immutable={Boolean(immutable)}
                  onChange={setDraft}
                />
              )}
              {!draft.label_pipeline && (
                <>
              <details className="natural-workflow-recipe">
                <summary>{t("Full Automation Recipe ·")}{" "}{guidedWorkflowNodes(draft.nodes).length}{" "}{t("steps")}</summary>
                <ol>{guidedWorkflowNodes(draft.nodes).map((node) => <li key={`recipe-${node.id}`}><strong>{workflowNodeTitle(node.node_type)}</strong><small>{node.model_binding ? `Model · ${node.model_binding}` : t("Reliable built-in step")}</small></li>)}</ol>
                {!draft.nodes.length && <Empty title={t("No Automation steps")} detail={t("Start from a template or preview an AnnotAgent recommendation.")} />}
              </details>
              <details className="advanced-graph">
                <summary>{t("View technical graph (read-only)")}</summary>
                <p><strong>{t("Inspection only.")}</strong> Graph-safe port selection, cycle checks, and undo are not released. Edit this Draft through the guided Automation Recipe controls.</p>
              <div className="workflow-nodes editable-workflow">
                {draft.nodes.map((node, index) => (
                  <article key={node.id}>
                    <span className="node-index">
                      {String(index + 1).padStart(2, "0")}
                    </span>
                    <div>
                      <label>{t("Node ID")}<input
                          value={node.id}
                          disabled
                          onChange={(event) =>
                            updateNode(index, { id: event.target.value })
                          }
                        />
                      </label>
                      <div className="form-grid">
                        <label>{t("Node type")}<select
                            value={node.node_type}
                            disabled
                            onChange={(event) =>
                              updateNode(index, {
                                node_type: event.target.value,
                              })
                            }
                          >
                            {!catalog?.node_catalog.some((descriptor) => descriptor.id === node.node_type) && (
                              <option value={node.node_type}>{workflowNodeTitle(node.node_type)} · legacy operation</option>
                            )}
                            {(catalog?.node_catalog ?? []).map((descriptor) => (
                              <option key={descriptor.id} value={descriptor.id}>
                                {descriptor.display_name}
                              </option>
                            ))}
                          </select>
                        </label>
                        <label>{t("Model binding")}<select
                            value={node.model_binding ?? ""}
                            disabled
                            onChange={(event) =>
                              updateNode(index, {
                                model_binding: event.target.value || undefined,
                              })
                            }
                          >
                            <option value="">{t("No model")}</option>
                            {workflowCatalogModelOptions(catalog).map((model) => (
                              <option key={model.id} value={model.id}>
                                {model.display_name}
                              </option>
                            ))}
                          </select>
                        </label>
                        <label>{t("Fallback node")}<input
                            value={node.fallback ?? ""}
                            disabled
                            placeholder={t("none")}
                            onChange={(event) =>
                              updateNode(index, {
                                fallback: event.target.value || undefined,
                              })
                            }
                          />
                        </label>
                        <label>{t("Retries")}<input
                            type="number"
                            min="0"
                            value={node.max_retries}
                            disabled
                            onChange={(event) =>
                              updateNode(index, {
                                max_retries: Number(event.target.value),
                              })
                            }
                          />
                        </label>
                      </div>
                      <label>{t("Depends on")}<input
                          value={node.depends_on.join(", ")}
                          disabled
                          onChange={(event) =>
                            updateNode(index, {
                              depends_on: event.target.value
                                .split(",")
                                .map((value) => value.trim())
                                .filter(Boolean),
                            })
                          }
                        />
                      </label>
                      <div className="form-grid">
                        <label>{t("Validators")}<input
                            value={node.validators.join(", ")}
                            disabled
                            onChange={(event) =>
                              updateNode(index, {
                                validators: event.target.value
                                  .split(",")
                                  .map((value) => value.trim())
                                  .filter(Boolean),
                              })
                            }
                          />
                        </label>
                        <label>{t("Refiners")}<input
                            value={node.refiners.join(", ")}
                            disabled
                            onChange={(event) =>
                              updateNode(index, {
                                refiners: event.target.value
                                  .split(",")
                                  .map((value) => value.trim())
                                  .filter(Boolean),
                              })
                            }
                          />
                        </label>
                      </div>
                      <label className="checkbox-line">
                        <input
                          type="checkbox"
                          checked={node.review_gate}
                          disabled
                          onChange={(event) =>
                            updateNode(index, {
                              review_gate: event.target.checked,
                            })
                          }
                        />{t("Human review gate")}</label>
                      <label>{t("Parameters (JSON)")}<textarea
                          value={JSON.stringify(node.parameters, null, 2)}
                          disabled
                          onChange={(event) => {
                            try {
                              updateNode(index, {
                                parameters: JSON.parse(
                                  event.target.value,
                                ) as Record<string, unknown>,
                              });
                            } catch {
                              // Preserve the last valid parameter object while the user edits JSON.
                            }
                          }}
                        />
                      </label>
                      <div className="node-meta">
                        <span>{t("Inputs ·")}{" "}
                          {node.inputs
                            ?.map((port) => `${port.id}:${port.artifact_type}`)
                            .join(", ") || t("none")}
                        </span>
                        <span>{t("Outputs ·")}{" "}
                          {node.outputs
                            ?.map((port) => `${port.id}:${port.artifact_type}`)
                            .join(", ") || t("none")}
                        </span>
                      </div>
                    </div>
                  </article>
                ))}
              </div>
              <div className="workflow-nodes editable-workflow">
                {(draft.edges ?? []).map((edge, index) => (
                  <article key={`${edge.from_node}-${edge.to_node}-${index}`}>
                    <span className="node-index">E{index + 1}</span>
                    <div>
                      <div className="form-grid">
                        <label>{t("From node")}<input
                            value={edge.from_node}
                            disabled
                            onChange={(event) =>
                              updateEdge(index, {
                                from_node: event.target.value,
                              })
                            }
                          />
                        </label>
                        <label>{t("From port")}<input
                            value={edge.from_port}
                            disabled
                            onChange={(event) =>
                              updateEdge(index, {
                                from_port: event.target.value,
                              })
                            }
                          />
                        </label>
                        <label>{t("To node")}<input
                            value={edge.to_node}
                            disabled
                            onChange={(event) =>
                              updateEdge(index, { to_node: event.target.value })
                            }
                          />
                        </label>
                        <label>{t("To port")}<input
                            value={edge.to_port}
                            disabled
                            onChange={(event) =>
                              updateEdge(index, { to_port: event.target.value })
                            }
                          />
                        </label>
                        <label>{t("Gate route")}<input
                            value={edge.route ?? ""}
                            disabled
                            onChange={(event) =>
                              updateEdge(index, {
                                route: event.target.value || undefined,
                              })
                            }
                          />
                        </label>
                      </div>
                    </div>
                  </article>
                ))}
              </div>
              <section className="runtime-policy-summary" aria-label={t("Runtime Policies")}>
                <span className="eyebrow">{t("Runtime behavior · not graph nodes")}</span>
                <p>Cache, replay, retry, timeout, budget, checkpoints, run control, usage, and history apply across the graph.</p>
                <div className="node-meta">
                  {(catalog?.runtime_policies ?? []).map((policy) => (
                    <span key={policy.id}>{policy.display_name} · {policy.scope}</span>
                  ))}
                </div>
              </section>
              </details>
                </>
              )}
              {report && (
                <div
                  className={
                    report.validation.valid
                      ? "validation-report valid"
                      : "validation-report invalid"
                  }
                >
                  <strong>
                    {report.validation.valid
                      ? t("Dry Run passed")
                      : `${report.validation.issues.length} validation issues`}
                  </strong>
                  {report.validation.execution_order.length > 0 && (
                    <small>
                      Order: {report.validation.execution_order.join(" → ")}
                    </small>
                  )}
                  <small>
                    {report.samples.length} sample(s) ·{" "}
                    {report.total_latency_ms} ms · estimated $
                    {report.estimated_cost}
                  </small>
                  {report.samples.map((sample) => (
                    <p key={sample.image_name}>
                      <code>{sample.image_name}</code> {sample.width}×
                      {sample.height} · {sample.nodes.length} node outputs
                    </p>
                  ))}
                  {report.validation.issues.map((issue) => (
                    <p key={`${issue.path}-${issue.code}`}>
                      <code>{issue.code}</code> {issue.path}: {issue.message}
                    </p>
                  ))}
                </div>
              )}
            </Panel>
          ) : selected ? (
            <WorkflowDetail
              project={selected.project}
              workflow={selected.workflow}
            />
          ) : (
            <Empty
              title={t("Select a Workflow")}
              detail={t("Choose a Project and create or select a Draft.")}
            />
          )}
        </div>
      </div>
      </details>
      {buildSummary && <BuildFooter previous="labels" next="test" nextEnabled={buildStepAllowed(buildSummary.guidance, "test")} nextPrimary={false} onNavigate={(step) => onNavigate(step, step === "test" ? draft?.id : undefined)} />}
    </section>
  );
}

export function workflowNodeTitle(nodeType: string): string {
  const known: Record<string, string> = {
    "core.image_input": "Read each image",
    "core.existing_annotations": "Read existing annotations",
    "core.resize": "Resize image",
    "core.tile": "Tile image",
    "capability.detect": "Find objects",
    "capability.classify": "Classify crops or images",
    "capability.segment": "Segment regions",
    "vlm_detection.detect": "Find objects",
    "yolo_detection.detect": "Find objects",
    "classification.classify": "Classify crops or images",
    "core.filter": "Select detections",
    "core.project_detection_candidates": "Select detections",
    "core.crop": "Crop candidates",
    "core.detections_to_box_prompts": "Convert detections to box prompts",
    "core.mask_to_bbox": "Convert masks to bounding boxes",
    "core.mask_to_polygon": "Convert masks to polygons",
    "core.map_label": "Select detections",
    "core.select_and_map": "Select and map results",
    "core.project_coordinates": "Project coordinates",
    "core.attach_result": "Combine model evidence",
    "core.candidate_merge": "Combine model evidence",
    "core.match_detection_sets": "Combine model evidence",
    "core.combine_evidence": "Combine model evidence",
    "core.attach_attribute": "Attach attributes",
    "core.confidence_gate": "Decision",
    "core.evidence_gate": "Decision",
    "core.validate": "Validate results",
    "core.decision": "Decision",
    "core.human_review": "Send uncertain results to Review",
    "core.commit": "Save annotations",
    "core.artifact_cache": "Keep replayable artifacts",
  };
  return known[nodeType] ?? nodeType.split(".").at(-1)?.replaceAll("_", " ") ?? nodeType;
}

function guidedWorkflowConcept(nodeType: string): string {
  if (["core.filter", "core.map_label", "core.project_detection_candidates", "core.select_and_map"].includes(nodeType))
    return "select_detections";
  if (["core.attach_result", "core.candidate_merge", "core.match_detection_sets", "core.combine_evidence"].includes(nodeType))
    return "combine_model_evidence";
  if (["core.confidence_gate", "core.evidence_gate", "core.decision"].includes(nodeType))
    return "decision";
  if (nodeType.includes("detect") || nodeType.includes("ground")) return "find_objects";
  return nodeType;
}

export function guidedWorkflowNodes<T extends { node_type: string }>(nodes: T[]): T[] {
  return nodes.filter((node, index) =>
    index === 0 ||
    guidedWorkflowConcept(node.node_type) !== guidedWorkflowConcept(nodes[index - 1].node_type),
  );
}

function pipelineStepTitle(step: PipelineStep, targetLabel?: string): string {
  const label = targetLabel || (Array.isArray(step.parameters.labels) ? step.parameters.labels.join(", ") : "targets");
  if (step.node_type.includes("detect")) return `Find ${label}`;
  if (["core.filter", "core.map_label", "core.project_detection_candidates", "core.select_and_map"].includes(step.node_type)) return "Select detections";
  if (step.node_type === "core.crop") return "Crop candidates";
  if (step.node_type.includes("classify")) return `Classify ${label}`;
  if (["core.attach_result", "core.candidate_merge", "core.match_detection_sets", "core.combine_evidence"].includes(step.node_type)) return "Combine model evidence";
  if (["core.confidence_gate", "core.evidence_gate", "core.decision"].includes(step.node_type)) return "Decision";
  if (step.kind === "human_review") return "Send uncertain results to Review";
  if (step.kind === "commit") return "Save the annotation";
  return workflowNodeTitle(step.node_type);
}

function pipelineStepDescription(step: PipelineStep, targetLabel?: string): string {
  if (step.node_type === "core.crop")
    return `${Math.round(Number(step.parameters.padding ?? 0) * 100)}% padding around the source detection`;
  if (step.node_type === "core.confidence_gate")
    return `Accept confidence ≥ ${Number(step.parameters.threshold ?? 0).toFixed(2)}; route the rest to Review`;
  if (step.node_type === "core.evidence_gate")
    return "Compare independent results and route conflicts to Review";
  if (["core.attach_result", "core.candidate_merge", "core.match_detection_sets", "core.combine_evidence"].includes(step.node_type))
    return "Keep each result attached to the same source object";
  if (step.node_type === "core.filter")
    return `Class filter · minimum confidence ${Number(step.parameters.minimum_confidence ?? 0).toFixed(2)}`;
  if (step.model_binding)
    return `Uses ${step.model_binding.model_id} for ${targetLabel || step.model_binding.capability}`;
  if (step.kind === "commit") return "Produces an editable Project annotation";
  return "Deterministic Core processing";
}

export function guidedPipelineStepGroups(steps: PipelineStep[]): Array<{
  firstIndex: number;
  steps: PipelineStep[];
}> {
  return steps.reduce<Array<{ firstIndex: number; steps: PipelineStep[] }>>(
    (groups, step, index) => {
      const previous = groups.at(-1);
      if (
        previous &&
        guidedWorkflowConcept(previous.steps[0].node_type) === guidedWorkflowConcept(step.node_type)
      ) {
        previous.steps.push(step);
      } else {
        groups.push({ firstIndex: index, steps: [step] });
      }
      return groups;
    },
    [],
  );
}

function pipelineStepOverview(steps: PipelineStep[], targetLabel: string): string {
  if (!steps.length) return `No execution steps for ${targetLabel}`;
  return `${steps.length} guided steps · ${steps.some((step) => step.model_binding) ? "Model + Core" : "Core only"} · editable Draft`;
}

function ExpertGraphEditor({
  draft,
}: {
  draft: WorkflowDraft;
  immutable: boolean;
  onChange: (draft: WorkflowDraft) => void;
}) {
  const raw = JSON.stringify(draft.label_pipeline, null, 2);
  return <details className="advanced-graph">
    <summary>{t("View technical graph (read-only)")}</summary>
    <p><strong>{t("Inspection only.")}</strong> Graph-safe editing remains unreleased. Use the guided Pipeline controls above; static validation still checks the resulting Draft before Sample Test.</p>
    <textarea aria-label={t("Technical graph JSON")} value={raw} readOnly />
  </details>;
}

function LabelPipelineEditor({
  draft,
  catalog,
  immutable,
  onChange,
}: {
  draft: WorkflowDraft;
  catalog?: WorkflowCatalog;
  immutable: boolean;
  onChange: (draft: WorkflowDraft) => void;
}) {
  const composition = draft.label_pipeline;
  const [pipelineId, setPipelineId] = useState(
    composition?.label_pipelines[0]?.id ?? "",
  );
  const [catalogNode, setCatalogNode] = useState("core.crop");
  const [drawer, setDrawer] = useState<
    | { scope: "shared"; stageIndex: number; stepIndex: number }
    | { scope: "label"; pipelineId: string; stepIndex: number }
  >();
  if (!composition) return null;
  const selected =
    composition.label_pipelines.find((pipeline) => pipeline.id === pipelineId) ??
    composition.label_pipelines[0];
  const stageConsumers = (stage: (typeof composition.shared_stages)[number]) => {
    const stepIds = new Set(stage.steps.map((step) => step.id));
    return composition.label_pipelines.filter((pipeline) =>
      pipeline.steps.some((step) =>
        Object.values(step.inputs).some((source) =>
          source.source === "shared_stage"
            ? source.stage_id === stage.id
            : source.source === "any_of_steps"
              ? source.sources.some(member => (member.source === "step" || member.source === "routed_step") && stepIds.has(member.step_id))
              : (source.source === "step" || source.source === "routed_step") && stepIds.has(source.step_id),
        ),
      ),
    );
  };
  const replaceComposition = (next: typeof composition) =>
    onChange({ ...draft, label_pipeline: next });
  const updateSharedStep = (
    stageIndex: number,
    stepIndex: number,
    next: PipelineStep,
  ) =>
    replaceComposition({
      ...composition,
      shared_stages: composition.shared_stages.map((stage, index) =>
        index === stageIndex
          ? {
              ...stage,
              steps: stage.steps.map((step, current) =>
                current === stepIndex ? next : step,
              ),
            }
          : stage,
      ),
    });
  const updatePipelineStep = (
    targetPipelineId: string,
    stepIndex: number,
    next: PipelineStep,
  ) =>
    replaceComposition({
      ...composition,
      label_pipelines: composition.label_pipelines.map((pipeline) =>
        pipeline.id === targetPipelineId
          ? {
              ...pipeline,
              steps: pipeline.steps.map((step, current) =>
                current === stepIndex ? next : step,
              ),
            }
          : pipeline,
      ),
    });
  const removePipelineStep = (targetPipelineId: string, stepIndex: number) =>
    replaceComposition({
      ...composition,
      label_pipelines: composition.label_pipelines.map((pipeline) =>
        pipeline.id === targetPipelineId
          ? {
              ...pipeline,
              steps: pipeline.steps.filter((_, current) => current !== stepIndex),
            }
          : pipeline,
      ),
    });
  const addCatalogNode = () => {
    if (!selected) return;
    const commitIndex = selected.steps.findIndex((step) => step.kind === "commit");
    const insertion = commitIndex < 0 ? selected.steps.length : commitIndex;
    const previous = selected.steps[Math.max(0, insertion - 1)];
    const previousOutput = previous
      ? Object.entries(previous.outputs)[0]
      : undefined;
    const suffix = selected.steps.length + 1;
    const id = `${selected.id}.${catalogNode.split(".").at(-1) ?? "node"}.${suffix}`;
    const output = pipelineNodeOutput(catalogNode);
    const source: PipelineSource = previousOutput
      ? {
          source: "step",
          step_id: previous.id,
          port: previousOutput[0],
          artifact_type: previousOutput[1],
        }
      : { source: "image" };
    const step: PipelineStep = {
      id,
      node_type: catalogNode,
      kind: pipelineNodeKind(catalogNode),
      inputs:
        ["core.crop", "core.project_coordinates"].includes(catalogNode)
          ? { image: { source: "image" }, detections: source }
          : { input: source },
      outputs: { [output.port]: output.type },
      model_binding: pipelineModelBinding(catalogNode, catalog),
      parameters: pipelineNodeParameters(catalogNode, selected.target_label),
      validators: [],
      refiners: [],
      retry_policy: { max_attempts: 1 },
      review_gate: { required: false, allow_manual_override: false },
      resources: {},
    };
    replaceComposition({
      ...composition,
      label_pipelines: composition.label_pipelines.map((pipeline) =>
        pipeline.id === selected.id
          ? {
              ...pipeline,
              steps: [
                ...pipeline.steps.slice(0, insertion),
                step,
                ...pipeline.steps.slice(insertion),
              ],
            }
          : pipeline,
      ),
    });
  };
  const applyDetectCropTemplate = () => {
    if (!selected) return;
    const localDetection = [...selected.steps]
      .reverse()
      .find((step) => step.kind === "vision_model" && Object.values(step.outputs).includes("detection_set"));
    const sharedDetection = composition.shared_stages
      .flatMap((stage) => stage.steps)
      .find((step) => step.kind === "vision_model" && Object.values(step.outputs).includes("detection_set"));
    const detection = localDetection ?? sharedDetection;
    const filter = selected.steps.find((step) => step.node_type === "core.filter");
    const gate = selected.steps.find((step) => step.node_type === "core.confidence_gate");
    const commit = selected.steps.find((step) => step.kind === "commit");
    if (!detection || !gate || !commit) return;
    const prefix = selected.id;
    const cropSource = filter ?? detection;
    const crop: PipelineStep = {
      id: `${prefix}.crop`,
      node_type: "core.crop",
      kind: "transform",
      inputs: {
        image: { source: "image" },
        detections: {
          source: "step",
          step_id: cropSource.id,
          port: Object.keys(cropSource.outputs)[0],
          artifact_type: "detection_set",
        },
      },
      outputs: { crops: "crop_set" },
      parameters: { padding: 0.05 },
      validators: [],
      refiners: [],
      retry_policy: { max_attempts: 1 },
      review_gate: { required: false, allow_manual_override: false },
      resources: {},
    };
    if (!localDetection) {
      const commitWithCrop: PipelineStep = {
        ...commit,
        inputs: {
          ...commit.inputs,
          preview_crops: {
            source: "step",
            step_id: crop.id,
            port: "crops",
            artifact_type: "crop_set",
          },
        },
      };
      replaceComposition({
        ...composition,
        label_pipelines: composition.label_pipelines.map((pipeline) =>
          pipeline.id === selected.id
            ? {
                ...pipeline,
                steps: [
                  ...pipeline.steps.filter(
                    (step) =>
                      step.id !== gate.id &&
                      step.id !== commit.id &&
                      step.node_type !== "core.crop" &&
                      step.node_type !== "core.artifact_cache",
                  ),
                  crop,
                  gate,
                  commitWithCrop,
                ],
              }
            : pipeline,
        ),
      });
      return;
    }
    const classifier: PipelineStep = {
      id: `${prefix}.crop_classifier`,
      node_type: "classification.classify",
      kind: "vision_model",
      inputs: {
        subjects: {
          source: "step",
          step_id: crop.id,
          port: "crops",
          artifact_type: "crop_set",
        },
      },
      outputs: { classifications: "classification_set" },
      model_binding: pipelineModelBinding("classification.classify", catalog),
      parameters: {
        labels: [selected.target_label],
      },
      validators: [],
      refiners: [],
      retry_policy: { max_attempts: 1 },
      review_gate: { required: false, allow_manual_override: false },
      resources: {},
    };
    const attach: PipelineStep = {
      id: `${prefix}.attach_result`,
      node_type: "core.attach_result",
      kind: "candidate_merge",
      inputs: {
        detections: {
          source: "step",
          step_id: detection.id,
          port: Object.keys(detection.outputs)[0],
          artifact_type: "detection_set",
        },
        classifications: {
          source: "step",
          step_id: classifier.id,
          port: "classifications",
          artifact_type: "classification_set",
        },
      },
      outputs: { candidates: "annotation_candidate_set" },
      parameters: { task_id: selected.target_task_id, class_mapping: {} },
      validators: [],
      refiners: [],
      retry_policy: { max_attempts: 1 },
      review_gate: { required: false, allow_manual_override: false },
      resources: {},
    };
    const updatedGate: PipelineStep = {
      ...gate,
      inputs: {
        candidates: {
          source: "step",
          step_id: attach.id,
          port: "candidates",
          artifact_type: "annotation_candidate_set",
        },
      },
      outputs: { candidates: "annotation_candidate_set" },
    };
    const updatedCommit: PipelineStep = {
      ...commit,
      inputs: {
        candidates: {
          source: "step",
          step_id: updatedGate.id,
          port: "candidates",
          artifact_type: "annotation_candidate_set",
        },
      },
    };
    const beforeGate = selected.steps.filter(
      (step) => step.id !== gate.id && step.id !== commit.id,
    );
    replaceComposition({
      ...composition,
      label_pipelines: composition.label_pipelines.map((pipeline) =>
        pipeline.id === selected.id
          ? {
              ...pipeline,
              steps: [
                ...beforeGate,
                crop,
                classifier,
                attach,
                updatedGate,
                updatedCommit,
              ],
            }
          : pipeline,
      ),
    });
  };
  const applyVlmDetectCropTemplate = () => {
    if (!selected) return;
    const sharedDetector = composition.shared_stages
      .flatMap((stage) => stage.steps)
      .find((step) => Object.values(step.outputs).includes("detection_set"));
    const filter = selected.steps.find((step) => step.node_type === "core.filter");
    const gate = selected.steps.find((step) => step.node_type === "core.confidence_gate");
    const commit = selected.steps.find((step) => step.kind === "commit");
    if (!sharedDetector || !filter || !gate || !commit) return;
    const vlmDetector: PipelineStep = {
      ...sharedDetector,
      node_type: "vlm_detection.detect",
      kind: "vision_model",
      model_binding: pipelineModelBinding("vlm_detection.detect", catalog),
      parameters: {
        labels: [selected.target_label],
        object_description: `A visible ${selected.target_label}. Return a tight box around each distinct instance and exclude visually similar non-target objects.`,
        instruction: `Scan the complete image and box every visible ${selected.target_label}, including small or partially occluded instances.`,
        coordinate_format: "qwen_0_1000_xyxy",
        max_detections: 10,
        grounding_assist: {
          mode: "grid",
          enabled: false,
          rows: 10,
          columns: 10,
        },
      },
    };
    const crop: PipelineStep = {
      id: `${selected.id}.crop`,
      node_type: "core.crop",
      kind: "transform",
      inputs: {
        image: { source: "image" },
        detections: {
          source: "step",
          step_id: filter.id,
          port: Object.keys(filter.outputs)[0],
          artifact_type: "detection_set",
        },
      },
      outputs: { crops: "crop_set" },
      parameters: { padding: 0.08 },
      validators: [],
      refiners: [],
      retry_policy: { max_attempts: 1 },
      review_gate: { required: false, allow_manual_override: false },
      resources: {},
    };
    const gateOutputPort = Object.keys(gate.outputs)[0];
    const gateOutputType = Object.values(gate.outputs)[0];
    const existingReview = selected.steps.find(
      (step) => step.kind === "human_review" || step.node_type === "core.human_review",
    );
    const geometryReview: PipelineStep = {
      ...(existingReview ?? {} as PipelineStep),
      id: existingReview?.id ?? `${selected.id}.geometry_review`,
      node_type: existingReview?.node_type ?? "core.human_review",
      kind: "human_review",
      inputs: {
        candidates: {
          source: "step",
          step_id: gate.id,
          port: gateOutputPort,
          artifact_type: gateOutputType,
        },
        preview_crops: {
          source: "step",
          step_id: crop.id,
          port: "crops",
          artifact_type: "crop_set",
        },
      },
      outputs: existingReview?.outputs ?? { candidates: gateOutputType },
      parameters: {
        reason: "Uncalibrated VLM box requires a geometry check",
      },
      validators: [],
      refiners: [],
      retry_policy: { max_attempts: 1 },
      review_gate: { required: true, allow_manual_override: false },
      resources: {},
    };
    const commitWithCrop: PipelineStep = {
      ...commit,
      inputs: {
        ...Object.fromEntries(
          Object.entries(commit.inputs).filter(([port]) => port !== "preview_crops"),
        ),
        candidates: {
          source: "step",
          step_id: geometryReview.id,
          port: Object.keys(geometryReview.outputs)[0],
          artifact_type: Object.values(geometryReview.outputs)[0],
        },
      },
    };
    replaceComposition({
      ...composition,
      shared_stages: composition.shared_stages.map((stage) => ({
        ...stage,
        steps: stage.steps.map((step) =>
          step.id === sharedDetector.id ? vlmDetector : step,
        ),
      })),
      label_pipelines: composition.label_pipelines.map((pipeline) =>
        pipeline.id === selected.id
          ? {
              ...pipeline,
              steps: [
                ...pipeline.steps.filter(
                  (step) =>
                    step.id !== gate.id &&
                    step.id !== commit.id &&
                    step.id !== geometryReview.id &&
                    step.node_type !== "core.crop" &&
                    step.node_type !== "core.artifact_cache" &&
                    step.node_type !== "classification.classify" &&
                    step.node_type !== "core.attach_result",
                ),
                crop,
                gate,
                geometryReview,
                commitWithCrop,
              ],
            }
          : pipeline,
      ),
    });
  };
  return (
    <div className="label-pipeline-editor">
      <div className="pipeline-section-heading">
        <div>
          <span className="eyebrow">{t("Shared Stages")}</span>
          <h3>{t("Runs once per image, then serves every Label Pipeline")}</h3>
        </div>
        <small>{composition.shared_stages.length} shared stage(s)</small>
      </div>
      {composition.shared_stages.map((stage, stageIndex) => (
        <section className="pipeline-lane shared" key={stage.id}>
          <header>
            <strong>{stage.name}</strong>
            <span>Runs once per image · used by {(stageConsumers(stage).length ? stageConsumers(stage) : composition.label_pipelines).map((pipeline) => pipeline.target_label).join(", ")}</span>
          </header>
          <div className="pipeline-step-row">
            {guidedPipelineStepGroups(stage.steps).map((group) => (
              <PipelineStepCard
                key={group.steps[0].id}
                step={group.steps[0]}
                immutable={immutable}
                shared
                mergedCount={group.steps.length}
                targetLabel={(stageConsumers(stage).length ? stageConsumers(stage) : composition.label_pipelines).map((pipeline) => pipeline.target_label).join(", ")}
                onConfigure={() => setDrawer({ scope: "shared", stageIndex, stepIndex: group.firstIndex })}
              />
            ))}
          </div>
        </section>
      ))}
      <div className="pipeline-section-heading">
        <div>
          <span className="eyebrow">{t("Label Pipelines")}</span>
          <h3>{t("One execution method per semantic Label")}</h3>
        </div>
        <div className="button-row">
          <select
            aria-label={t("Edited Label Pipeline")}
            value={selected?.id ?? ""}
            onChange={(event) => setPipelineId(event.target.value)}
          >
            {composition.label_pipelines.map((pipeline) => (
              <option key={pipeline.id} value={pipeline.id}>
                {pipeline.target_task_id} / {pipeline.target_label}
              </option>
            ))}
          </select>
          <details className="recipe-edit-menu"><summary>{t("Edit automation")}</summary><div>
          <select
            aria-label={t("Node Catalog")}
            value={catalogNode}
            onChange={(event) => setCatalogNode(event.target.value)}
          >
            {(catalog?.node_catalog ?? [])
              .filter((node) =>
                !["input", "human_and_output"].includes(node.category) &&
                node.id !== "capability.segment",
              )
              .map((node) => (
                <option key={node.id} value={node.id}>
                  {workflowNodeTitle(node.id)}
                </option>
              ))}
          </select>
          <button onClick={addCatalogNode} disabled={immutable || !selected}>{t("Add step")}</button>
          <button
            onClick={applyDetectCropTemplate}
            disabled={
              immutable ||
              !selected ||
              ![
                ...selected.steps,
                ...composition.shared_stages.flatMap((stage) => stage.steps),
              ].some((step) => step.kind === "vision_model" && Object.values(step.outputs).includes("detection_set"))
            }
            title="Internal graph: detector → filter → Core Crop; Detection remains the bbox result"
          >{t("Add detection + crop")}</button>
          <button
            onClick={applyVlmDetectCropTemplate}
            disabled={
              immutable ||
              !selected ||
              !composition.shared_stages.some((stage) =>
                stage.steps.some((step) =>
                  Object.values(step.outputs).includes("detection_set"),
                ),
              )
            }
            title="VLM DetectionSet → Filter → Core Crop; bbox Commit remains on the filtered DetectionSet"
          >{t("Use VLM detection + crop")}</button>
          </div></details>
        </div>
      </div>
      {composition.label_pipelines.map((pipeline) => (
        <section className="pipeline-lane" key={pipeline.id}>
          <header>
            <strong>{pipeline.target_label}</strong>
            <span>{pipelineStepOverview(pipeline.steps, pipeline.target_label)}</span>
          </header>
          <div className="pipeline-step-row">
            {guidedPipelineStepGroups(pipeline.steps).map((group) => (
              <PipelineStepCard
                key={group.steps[0].id}
                step={group.steps[0]}
                immutable={immutable}
                mergedCount={group.steps.length}
                targetLabel={pipeline.target_label}
                onConfigure={() => setDrawer({ scope: "label", pipelineId: pipeline.id, stepIndex: group.firstIndex })}
                onRemove={group.steps.length === 1 ? () => removePipelineStep(pipeline.id, group.firstIndex) : undefined}
              />
            ))}
          </div>
        </section>
      ))}
      <ExpertGraphEditor draft={draft} immutable={immutable} onChange={onChange} />
      {drawer && (() => {
        const step = drawer.scope === "shared"
          ? composition.shared_stages[drawer.stageIndex]?.steps[drawer.stepIndex]
          : composition.label_pipelines.find((pipeline) => pipeline.id === drawer.pipelineId)?.steps[drawer.stepIndex];
        if (!step) return null;
        return (
          <PipelineNodeDrawer
            step={step}
            catalog={catalog}
            immutable={immutable}
            onClose={() => setDrawer(undefined)}
            onChange={(next) => drawer.scope === "shared"
              ? updateSharedStep(drawer.stageIndex, drawer.stepIndex, next)
              : updatePipelineStep(drawer.pipelineId, drawer.stepIndex, next)}
          />
        );
      })()}
    </div>
  );
}

function PipelineStepCard({
  step,
  immutable,
  shared = false,
  mergedCount = 1,
  targetLabel,
  onRemove,
  onConfigure,
}: {
  step: PipelineStep;
  immutable: boolean;
  shared?: boolean;
  mergedCount?: number;
  targetLabel?: string;
  onRemove?: () => void;
  onConfigure: () => void;
}) {
  return (
    <article className="pipeline-step-card">
      <span className="pipeline-step-kind">{shared ? "shared" : t("Label pipeline")}</span>
      <strong>{pipelineStepTitle(step, targetLabel)}</strong>
      <small>{pipelineStepDescription(step, targetLabel)}</small>
      <div className="pipeline-card-summary">
        <span>{t("Runs with")}{" "}<strong>{step.model_binding?.model_id ?? "AnnotAgent Core"}</strong></span>
        {mergedCount > 1 && <span>{t("Guided action")}{" "}<strong>{mergedCount} coordinated operations</strong></span>}
        <Status status={immutable ? "published" : "valid"} />
      </div>
      <button
        aria-label={immutable ? t("Inspect node") : t("Configure node")}
        onClick={onConfigure}
      >
        {immutable ? t("Inspect") : t("Configure")}
      </button>
      {onRemove && step.kind !== "commit" && (
        <button className="danger" onClick={onRemove} disabled={immutable}>{t("Remove step")}</button>
      )}
    </article>
  );
}

function PipelineNodeDrawer({
  step,
  catalog,
  immutable,
  onClose,
  onChange,
}: {
  step: PipelineStep;
  catalog?: WorkflowCatalog;
  immutable: boolean;
  onClose: () => void;
  onChange: (step: PipelineStep) => void;
}) {
  const closeRef = useRef<HTMLButtonElement>(null);
  const [parameters, setParameters] = useState(() =>
    JSON.stringify(step.parameters, null, 2),
  );
  useEffect(() => setParameters(JSON.stringify(step.parameters, null, 2)), [step.id]);
  useEffect(() => {
    closeRef.current?.focus();
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, []);
  const updateNumber = (name: string, value: number) =>
    onChange({ ...step, parameters: { ...step.parameters, [name]: value } });
  const isDetection = step.node_type.includes("detect") || step.node_type.includes("ground");
  const rawGroundingAssist = step.parameters.grounding_assist;
  const groundingAssist =
    rawGroundingAssist && typeof rawGroundingAssist === "object"
      ? (rawGroundingAssist as Record<string, unknown>)
      : {};
  const updateGroundingAssist = (patch: Record<string, unknown>) =>
    onChange({
      ...step,
      parameters: {
        ...step.parameters,
        grounding_assist: {
          mode: "grid",
          enabled: Boolean(groundingAssist.enabled),
          rows: Number(groundingAssist.rows ?? 10),
          columns: Number(groundingAssist.columns ?? 10),
          ...patch,
        },
      },
    });
  return (
    <div className="drawer-backdrop" role="presentation" onMouseDown={(event) => event.target === event.currentTarget && onClose()}>
      <aside className="node-drawer" role="dialog" aria-modal="true" aria-labelledby="node-drawer-title">
        <header>
          <div><span className="eyebrow">{t("Pipeline step")}</span><h2 id="node-drawer-title">{pipelineStepTitle(step)}</h2></div>
          <button ref={closeRef} onClick={onClose} aria-label={t("Close node configuration")}>{t("Close")}</button>
        </header>
        <Fact label={t("Status")} value={immutable ? "Published · read only" : "Draft · editable"} />
        {step.model_binding && (
          <label>{t("Model")}<select value={step.model_binding.model_id} disabled={immutable} onChange={(event) => onChange({ ...step, model_binding: { ...step.model_binding!, model_id: event.target.value } })}>
            {workflowCatalogModelOptions(catalog).filter((model) => model.capabilities.includes(step.model_binding!.capability)).map((model) => <option key={model.id} value={model.id}>{model.display_name}</option>)}
          </select></label>
        )}
        {Array.isArray(step.parameters.labels) && (
          <label>{t("Labels")}<input value={step.parameters.labels.join(", ")} disabled={immutable} onChange={(event) => onChange({ ...step, parameters: { ...step.parameters, labels: event.target.value.split(",").map((label) => label.trim()).filter(Boolean) } })} /></label>
        )}
        {isDetection && typeof step.parameters.object_description === "string" && (
          <label>{t("What should the model find?")}<textarea value={step.parameters.object_description} disabled={immutable} onChange={(event) => onChange({ ...step, parameters: { ...step.parameters, object_description: event.target.value } })} /></label>
        )}
        {step.node_type === "core.confidence_gate" && <label>{t("Confidence threshold")}<input type="number" min="0" max="1" step="0.05" value={Number(step.parameters.threshold ?? 0)} disabled={immutable} onChange={(event) => updateNumber("threshold", Number(event.target.value))} /></label>}
        {step.node_type === "core.filter" && <label>{t("Minimum confidence")}<input type="number" min="0" max="1" step="0.05" value={Number(step.parameters.minimum_confidence ?? 0)} disabled={immutable} onChange={(event) => updateNumber("minimum_confidence", Number(event.target.value))} /></label>}
        {step.node_type === "core.crop" && <label>{t("Crop padding")}<input type="number" min="0" max="0.5" step="0.01" value={Number(step.parameters.padding ?? 0)} disabled={immutable} onChange={(event) => updateNumber("padding", Number(event.target.value))} /></label>}
        {isDetection && (
          <fieldset className="grounding-assist-fieldset">
            <legend>{t("Positioning assistance")}</legend>
            <label className="checkbox-row"><input type="checkbox" checked={Boolean(groundingAssist.enabled)} disabled={immutable} onChange={(event) => updateGroundingAssist({ enabled: event.target.checked })} />{t("Use a positioning grid to improve coordinate accuracy")}</label>
            {Boolean(groundingAssist.enabled) && <div className="form-grid">
              <label>{t("Grid rows")}<input type="number" min="2" max="16" value={Number(groundingAssist.rows ?? 10)} disabled={immutable} onChange={(event) => updateGroundingAssist({ rows: Number(event.target.value) })} /></label>
              <label>{t("Grid columns")}<input type="number" min="2" max="16" value={Number(groundingAssist.columns ?? 10)} disabled={immutable} onChange={(event) => updateGroundingAssist({ columns: Number(event.target.value) })} /></label>
            </div>}
            <small>The original image remains the source of truth; the grid is sent only as a second calibration view.</small>
          </fieldset>
        )}
        <details><summary>{t("Expert details")}</summary>
          <code>{step.node_type} · {step.id}</code>
          <label>{t("Input")}<input readOnly value={Object.entries(step.inputs).map(([name, source]) => {
            const describe = (input: PipelineSource): string => input.source === "image" ? "Image"
              : input.source === "any_of_steps" ? input.sources.map(describe).join(" OR ")
              : `${input.step_id}.${input.port}${input.source === "routed_step" ? ` [${input.route}]` : ""}`;
            return `${name}: ${describe(source)}`;
          }).join(" + ") || "None"} /></label>
          <label>{t("Output")}<input readOnly value={Object.entries(step.outputs).map(([name, type]) => `${name}: ${type}`).join(", ") || "Terminal"} /></label>
          <label>{t("Fallback node")}<input value={step.fallback ?? ""} disabled={immutable} placeholder={t("No fallback")} onChange={(event) => onChange({ ...step, fallback: event.target.value || undefined })} /></label>
          <label>{t("Raw parameters and class mapping")}<textarea value={parameters} disabled={immutable} onChange={(event) => {
            setParameters(event.target.value);
            try { onChange({ ...step, parameters: JSON.parse(event.target.value) as Record<string, unknown> }); } catch { /* Keep editing until JSON is valid. */ }
          }} /></label>
          <Fact label={t("Kind")} value={step.kind} />
          <Fact label={t("Retries")} value={step.retry_policy.max_attempts} />
          <Fact label={t("Validators")} value={step.validators.join(", ") || "None"} />
          <Fact label={t("Refiners")} value={step.refiners.join(", ") || "None"} />
        </details>
      </aside>
    </div>
  );
}

export function pipelineNodeOutput(nodeType: string): {
  port: string;
  type: PipelineArtifactType;
} {
  if (nodeType === "core.crop") return { port: "crops", type: "crop_set" };
  if (nodeType === "core.detections_to_box_prompts")
    return { port: "prompts", type: "box_prompt_set" };
  if (nodeType === "capability.segment") return { port: "masks", type: "mask_set" };
  if (nodeType === "core.mask_to_polygon") return { port: "polygons", type: "polygon_set" };
  if (nodeType === "core.resize" || nodeType === "core.tile")
    return { port: "images", type: "image" };
  if (nodeType === "classification.classify" || nodeType === "capability.classify")
    return { port: "classifications", type: "classification_set" };
  if (nodeType === "core.attach_result" || nodeType === "core.attach_attribute")
    return { port: "candidates", type: "annotation_candidate_set" };
  if (["core.confidence_gate", "core.decision", "core.validate"].includes(nodeType))
    return { port: "candidates", type: "annotation_candidate_set" };
  if (["core.match_detection_sets", "core.combine_evidence", "core.evidence_gate"].includes(nodeType))
    return { port: "candidates", type: "candidate_cluster_set" };
  return { port: "detections", type: "detection_set" };
}

export function pipelineNodeKind(nodeType: string): NonNullable<PipelineStep["kind"]> {
  if (["core.attach_result", "core.match_detection_sets", "core.combine_evidence"].includes(nodeType))
    return "candidate_merge";
  if (["core.confidence_gate", "core.evidence_gate", "core.decision"].includes(nodeType))
    return "gate";
  if (nodeType === "core.validate") return "validator";
  if (nodeType.includes("classify") || nodeType.includes("detect") || nodeType.includes("segment"))
    return "vision_model";
  return "transform";
}

export function pipelineNodeParameters(nodeType: string, label: string) {
  if (nodeType === "core.crop") return { padding: 0.05 };
  if (nodeType === "core.resize") return { max_edge: 1600, allow_upscale: false };
  if (nodeType === "core.tile")
    return { tile_size: 1024, overlap: 0.15, maximum_tiles: 64, merge_policy: "nms" };
  if (nodeType === "core.filter")
    return { labels: [label], minimum_confidence: 0.5 };
  if (nodeType === "core.map_label") return { class_mapping: {} };
  if (nodeType === "core.select_and_map")
    return { labels: [label], minimum_confidence: 0.5, class_mapping: {}, drop_unknown_labels: false };
  if (nodeType === "core.confidence_gate") return { threshold: 0.9 };
  if (nodeType === "core.decision") return { mode: "confidence", threshold: 0.9 };
  if (nodeType === "core.match_detection_sets" || nodeType === "core.combine_evidence")
    return { method: "iou", minimum_iou: 0.5, preserve_unmatched: true };
  if (nodeType === "core.evidence_gate")
    return {
      accept_when: [{ minimum_sources: 2, minimum_iou: 0.6 }],
      fallback_when: [],
      review_when: [{ geometry_conflict: true, label_conflict: true, score_missing: true }],
      reject_when: [],
    };
  if (nodeType === "classification.classify" || nodeType === "capability.classify")
    return { labels: [label] };
  if (nodeType === "vlm_detection.detect" || nodeType === "capability.detect")
    return {
      labels: [label],
      object_description: `Locate every visible ${label} and return a tight normalized bounding box.`,
      max_detections: 20,
      grounding_assist: {
        mode: "grid",
        enabled: false,
        rows: 10,
        columns: 10,
      },
    };
  return {};
}

function expertCapabilityToVision(capability: ModelCapability) {
  return capability === "image_classification" ? "classification" : capability;
}

function workflowCatalogModelOptions(catalog?: WorkflowCatalog) {
  const models = (catalog?.model_registry ?? []).map((model) => ({
    id: model.id,
    display_name: model.display_name,
    capabilities: model.capabilities,
  }));
  for (const model of catalog?.expert_models ?? []) {
    if (model.availability !== "available") continue;
    if (models.some((candidate) => candidate.id === model.model_id)) continue;
    models.push({
      id: model.model_id,
      display_name: `${model.display_name} · Rust plugin`,
      capabilities: model.capabilities.map(expertCapabilityToVision),
    });
  }
  return models;
}

export function workflowNodeModelCapability(nodeType: string): string | undefined {
  return nodeType === "vlm_detection.detect"
    ? "vision_language"
    : nodeType === "capability.segment"
    ? "prompted_segmentation"
    : nodeType.includes("classify")
    ? "classification"
    : nodeType.includes("detect")
      ? "object_detection"
      : undefined;
}

function pipelineModelBinding(nodeType: string, catalog?: WorkflowCatalog) {
  const capability = workflowNodeModelCapability(nodeType);
  if (!capability) return undefined;
  const model = workflowCatalogModelOptions(catalog).find((candidate) =>
    candidate.capabilities.includes(capability),
  );
  if (!model) return undefined;
  return {
    model_id: model.id,
    capability,
    configuration: {},
  };
}

function PipelineArtifactInspector({
  inspection,
  nodeId,
  replay,
}: {
  inspection: RunNodeArtifactInspection;
  nodeId: string;
  replay?: NodeReplayReport;
}) {
  const node = inspection.nodes.find((item) => item.node_id === nodeId);
  if (!node) return null;
  const imageUrl =
    inspection.image_index === undefined
      ? undefined
      : `/api/projects/${inspection.project_id}/images/${inspection.image_index}/content`;
  const rects = artifactRects(node.outputs);
  const crops = artifactCrops(node.outputs);
  return (
    <div className="artifact-inspector-grid">
      <div className="artifact-preview-panel">
        <span className="eyebrow">{t("Visual preview")}</span>
        {imageUrl ? (
          <div className="artifact-image-stage">
            <img src={imageUrl} alt={t("Original Pipeline input")} />
            <svg viewBox="0 0 100 100" preserveAspectRatio="none" aria-label={t("Artifact bounding boxes")}>
              {rects.map((rect, index) => (
                <rect
                  key={`${rect.x}-${rect.y}-${index}`}
                  x={rect.x * 100}
                  y={rect.y * 100}
                  width={rect.width * 100}
                  height={rect.height * 100}
                />
              ))}
            </svg>
          </div>
        ) : (
          <small>This Run predates replayable image identity.</small>
        )}
        {imageUrl && crops.length > 0 && (
          <div className="crop-preview-list">
            {crops.map((crop, index) => (
              <svg
                key={`${crop.x}-${crop.y}-${index}`}
                viewBox={`${crop.x * 100} ${crop.y * 100} ${crop.width * 100} ${crop.height * 100}`}
                aria-label={`Crop ${index + 1}`}
              >
                <image href={imageUrl} x="0" y="0" width="100" height="100" />
              </svg>
            ))}
          </div>
        )}
      </div>
      <div className="artifact-node-detail">
        <span className="eyebrow">{node.operation}</span>
        <h3>{node.node_id}</h3>
        <div className="workflow-facts">
          <Fact label={t("Status")} value={node.status} />
          <Fact label={t("Latency")} value={`${node.latency_ms} ms`} />
          <Fact label={t("Attempts")} value={node.attempts} />
          <Fact label={t("Cache")} value={node.cache_hit ? "hit" : "miss"} />
        </div>
        {node.error && (
          <p className="run-reason">
            <code>{node.error.code}</code> {node.error.summary}
          </p>
        )}
        <details open>
          <summary>{t("Configuration")}</summary>
          <pre>{JSON.stringify(node.configuration, null, 2)}</pre>
        </details>
        <details>
          <summary>{t("Inputs ·")}{" "}{node.inputs.length}</summary>
          <pre>{JSON.stringify(node.inputs, null, 2)}</pre>
        </details>
        <details open>
          <summary>{t("Outputs ·")}{" "}{node.outputs.length}</summary>
          <pre>{JSON.stringify(node.outputs, null, 2)}</pre>
        </details>
        {replay && replay.replayed_from === node.node_id && (
          <div className="validation-report valid">
            <strong>{t("Sandbox Replay completed")}</strong>
            <small>{t("Re-executed:")}{" "}{replay.reexecuted_nodes.join(", ")}</small>
            <small>{t("Preserved upstream:")}{" "}{replay.preserved_upstream_nodes.join(", ")}
            </small>
          </div>
        )}
      </div>
    </div>
  );
}

type ArtifactRect = { x: number; y: number; width: number; height: number };
type ArtifactMask = { id: string; width: number; height: number; counts: number[] };

export function decodeCocoRleMask(
  width: number,
  height: number,
  counts: number[],
): Uint8Array | undefined {
  if (!Number.isInteger(width) || !Number.isInteger(height) || width <= 0 || height <= 0)
    return undefined;
  const pixelCount = width * height;
  if (!Number.isSafeInteger(pixelCount) || pixelCount > 16_000_000) return undefined;
  if (counts.some((count) => !Number.isSafeInteger(count) || count < 0)) return undefined;
  if (counts.reduce((total, count) => total + count, 0) !== pixelCount) return undefined;
  const pixels = new Uint8Array(pixelCount);
  let sourceIndex = 0;
  counts.forEach((count, runIndex) => {
    if (runIndex % 2 === 1) {
      for (let offset = 0; offset < count; offset += 1) {
        const columnMajorIndex = sourceIndex + offset;
        const x = Math.floor(columnMajorIndex / height);
        const y = columnMajorIndex % height;
        pixels[y * width + x] = 1;
      }
    }
    sourceIndex += count;
  });
  return pixels;
}

export function artifactMasks(artifacts: PipelineArtifact[]): ArtifactMask[] {
  return artifacts.flatMap((artifact, artifactIndex) => {
    if (artifact.kind !== "mask_set" || !Array.isArray(artifact.artifact.masks)) return [];
    return artifact.artifact.masks.flatMap((item, maskIndex) => {
      if (!item || typeof item !== "object") return [];
      const record = item as Record<string, unknown>;
      if (!record.mask || typeof record.mask !== "object") return [];
      const mask = record.mask as Record<string, unknown>;
      if (mask.encoding !== "coco_rle"
        || typeof mask.width !== "number"
        || typeof mask.height !== "number"
        || typeof mask.counts !== "string") return [];
      const counts = mask.counts.trim().split(/\s+/).filter(Boolean).map(Number);
      if (!decodeCocoRleMask(mask.width, mask.height, counts)) return [];
      return [{
        id: typeof record.mask_id === "string"
          ? record.mask_id
          : `mask-${artifactIndex}-${maskIndex}`,
        width: mask.width,
        height: mask.height,
        counts,
      }];
    });
  });
}

function ArtifactMaskLayer({ masks }: { masks: ArtifactMask[] }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const first = masks[0];
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !first) return;
    canvas.width = first.width;
    canvas.height = first.height;
    const context = canvas.getContext("2d");
    if (!context) return;
    const pixels = context.createImageData(first.width, first.height);
    for (const mask of masks) {
      if (mask.width !== first.width || mask.height !== first.height) continue;
      const decoded = decodeCocoRleMask(mask.width, mask.height, mask.counts);
      if (!decoded) continue;
      decoded.forEach((active, index) => {
        if (!active) return;
        const offset = index * 4;
        pixels.data[offset] = 24;
        pixels.data[offset + 1] = 153;
        pixels.data[offset + 2] = 171;
        pixels.data[offset + 3] = Math.min(150, pixels.data[offset + 3] + 82);
      });
    }
    context.putImageData(pixels, 0, 0);
  }, [masks, first]);
  if (!first) return null;
  return <canvas ref={canvasRef} className="artifact-mask-layer" aria-hidden="true" />;
}

export function artifactRects(artifacts: PipelineArtifact[]): ArtifactRect[] {
  return artifacts.flatMap((artifact) => {
    const detections = artifact.kind === "detection_set"
      ? artifact.artifact.detections
      : artifact.kind === "candidate_cluster_set"
        ? artifact.artifact.candidates
        : artifact.kind === "box_prompt_set"
          ? artifact.artifact.prompts
        : undefined;
    if (Array.isArray(detections)) {
      return detections.flatMap((detection) => {
        if (!detection || typeof detection !== "object") return [];
        const record = detection as Record<string, unknown>;
        const rect = record.representative_bbox ?? record.bbox ?? record.rect;
        return parseArtifactRect(rect) ? [parseArtifactRect(rect)!] : [];
      });
    }
    const polygonItems = artifact.kind === "mask_set"
      ? artifact.artifact.masks
      : artifact.kind === "polygon_set"
        ? artifact.artifact.polygons
        : undefined;
    if (!Array.isArray(polygonItems)) return [];
    return polygonItems.flatMap((item) => {
      if (!item || typeof item !== "object") return [];
      const record = item as Record<string, unknown>;
      const bounds = artifactPolygonBounds(
        artifact.kind === "mask_set" ? record.mask : { rings: record.rings },
      );
      return bounds ? [bounds] : [];
    });
  });
}

function artifactPolygonBounds(value: unknown): ArtifactRect | undefined {
  if (!value || typeof value !== "object") return undefined;
  const rings = (value as Record<string, unknown>).rings;
  if (!Array.isArray(rings)) return undefined;
  const points = rings.flatMap((ring) => Array.isArray(ring) ? ring : []).flatMap((point) => {
    if (!point || typeof point !== "object") return [];
    const record = point as Record<string, unknown>;
    return typeof record.x === "number" && typeof record.y === "number"
      ? [{ x: record.x, y: record.y }]
      : [];
  });
  if (points.length === 0) return undefined;
  const xs = points.map((point) => point.x);
  const ys = points.map((point) => point.y);
  const left = Math.min(...xs);
  const top = Math.min(...ys);
  return {
    x: left,
    y: top,
    width: Math.max(...xs) - left,
    height: Math.max(...ys) - top,
  };
}

export function artifactCrops(artifacts: PipelineArtifact[]): ArtifactRect[] {
  return artifacts.flatMap((artifact) => {
    if (artifact.kind !== "crop_set") return [];
    const crops = artifact.artifact.crops;
    if (!Array.isArray(crops)) return [];
    return crops.flatMap((crop) => {
      if (!crop || typeof crop !== "object") return [];
      const rect = (crop as Record<string, unknown>).rect;
      return parseArtifactRect(rect) ? [parseArtifactRect(rect)!] : [];
    });
  });
}

function parseArtifactRect(value: unknown): ArtifactRect | undefined {
  if (Array.isArray(value) && value.length === 4 && value.every((item) => typeof item === "number"))
    return { x: value[0], y: value[1], width: value[2], height: value[3] };
  if (!value || typeof value !== "object") return undefined;
  const rect = value as Record<string, unknown>;
  const x = rect.x;
  const y = rect.y;
  const width = rect.width;
  const height = rect.height;
  return [x, y, width, height].every((item) => typeof item === "number")
    ? { x: x as number, y: y as number, width: width as number, height: height as number }
    : undefined;
}

const REGISTRY_MODEL_CAPABILITIES: { id: ModelCapability; label: string }[] = [
  { id: "text_generation", label: "Text generation" },
  { id: "vision_language", label: "Vision language" },
  { id: "image_classification", label: "Image classification" },
  { id: "object_detection", label: "Object detection" },
  { id: "open_vocabulary_detection", label: "Open-vocabulary detection" },
  { id: "phrase_grounding", label: "Phrase grounding" },
  { id: "semantic_segmentation", label: "Semantic segmentation" },
  { id: "prompted_segmentation", label: "Prompted segmentation" },
  { id: "instance_segmentation", label: "Instance segmentation" },
  { id: "keypoint_detection", label: "Keypoint detection" },
];

const PLUGIN_STATUS_GROUPS: { title: string; description: string; statuses: string[] }[] = [
  { title: "Ready to use", description: "Enabled models that passed their isolated process test.", statuses: ["ready"] },
  { title: "Finish setup", description: "Installed runtimes that still need a verified Model Bundle, smoke test, or platform support.", statuses: ["installed", "needs_weights", "unsupported_platform"] },
  { title: "Disabled", description: "Verified installations hidden from new Workflow bindings.", statuses: ["disabled"] },
  { title: "Needs attention", description: "A process, API, Manifest, or Contract check did not pass.", statuses: ["unhealthy", "crashed", "failed_smoke_test", "incompatible_api", "invalid_manifest", "invalid_contract"] },
  { title: "Updates available", description: "New versions install alongside versions used by published Workflows.", statuses: ["update_available"] },
];

function formatPluginBytes(bytes: number) {
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

interface PluginBundleInventory {
  plugin_runtime_status: string;
  available: ModelCatalogEntry[];
  installed: InstalledModelBundle[];
  setup_blockers: {
    bundle_id: string;
    bundle_version: string;
    code: string;
    message: string;
  }[];
}

const MODEL_SETUP_STEPS = [
  "Select model",
  "Review source",
  "Review license",
  "Check compatibility",
  "Install & test",
  "Ready",
];

const MODEL_INSTALL_STAGES: { id: ModelInstallOperation["stage"]; label: string }[] = [
  { id: "resolving_model", label: "Resolve model" },
  { id: "downloading_bundle", label: "Download Bundle" },
  { id: "verifying_bundle_digest", label: "Verify Bundle digest" },
  { id: "verifying_model_files", label: "Verify model files" },
  { id: "checking_onnx_contract", label: "Check ONNX Contract" },
  { id: "starting_rust_plugin", label: "Start Rust Plugin" },
  { id: "loading_model", label: "Load model" },
  { id: "running_sample_inference", label: "Run real sample inference" },
  { id: "registering_model_profile", label: "Register Model Profile" },
  { id: "ready", label: "Ready" },
];

function pluginIdentity(installation: ExpertPluginInstallation) {
  return `${installation.manifest.id}@${installation.manifest.version}`;
}

function bundleIdentity(bundle: InstalledModelBundle) {
  return `${bundle.manifest.id}@${bundle.manifest.version}`;
}

function catalogBundleIdentity(bundle: ModelCatalogEntry) {
  return `${bundle.bundle_id}@${bundle.bundle_version}`;
}

function FilePicker({
  id,
  label,
  accept,
  file,
  chooseLabel,
  emptyLabel,
  onSelect,
}: {
  id: string;
  label: string;
  accept: string;
  file?: File;
  chooseLabel: string;
  emptyLabel: string;
  onSelect: (file?: File) => void;
}) {
  const inputRef = useRef<HTMLInputElement>(null);
  const statusId = `${id}-selection`;
  const inputKey = file ? `${file.name}:${file.size}:${file.lastModified}` : "empty";
  return <div className="file-picker-control">
    <input
      key={inputKey}
      ref={inputRef}
      id={id}
      className="file-picker-native"
      type="file"
      accept={accept}
      aria-label={label}
      aria-describedby={statusId}
      onChange={(event) => onSelect(event.target.files?.[0])}
    />
    <button type="button" className="file-picker-button" onClick={() => inputRef.current?.click()} aria-describedby={statusId}>
      <span aria-hidden="true">↑</span>
      {chooseLabel}
    </button>
    <span id={statusId} className={`file-picker-selection${file ? " selected" : ""}`} role="status" title={file?.name}>
      <i aria-hidden="true">{file ? "✓" : "·"}</i>
      <span>{file?.name ?? emptyLabel}</span>
    </span>
  </div>;
}

function ExpertModelPluginsPage({ onError }: { onError: (value: string) => void }) {
  const [registry, setRegistry] = useState<ExpertPluginRegistry>();
  const [bundleInventory, setBundleInventory] = useState<Record<string, PluginBundleInventory>>({});
  const [instances, setInstances] = useState<InstalledModelInstance[]>([]);
  const [instanceProfiles, setInstanceProfiles] = useState<ModelInstanceProfile[]>([]);
  const [installOperations, setInstallOperations] = useState<ModelInstallOperation[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState("");
  const [message, setMessage] = useState("");
  const [packageFile, setPackageFile] = useState<File>();
  const [verified, setVerified] = useState<VerifiedExpertPluginPackage>();
  const [permissionsReviewed, setPermissionsReviewed] = useState(false);
  const [codeLicenseAccepted, setCodeLicenseAccepted] = useState(false);
  const [weightLicenseAccepted, setWeightLicenseAccepted] = useState(false);
  const [bundleFile, setBundleFile] = useState<File>();
  const [verifiedBundle, setVerifiedBundle] = useState<VerifiedModelBundlePackage>();
  const [bundleImportLicenseAccepted, setBundleImportLicenseAccepted] = useState(false);
  const [setupPluginIdentity, setSetupPluginIdentity] = useState("");
  const [setupEntryIdentity, setSetupEntryIdentity] = useState("");
  const [setupStep, setSetupStep] = useState(0);
  const [setupOperationId, setSetupOperationId] = useState("");
  const [setupLicenseAccepted, setSetupLicenseAccepted] = useState(false);
  const [setupError, setSetupError] = useState("");
  const setupDialogRef = useRef<HTMLElement>(null);
  const setupCloseButtonRef = useRef<HTMLButtonElement>(null);
  const setupReturnFocusRef = useRef<HTMLElement | null>(null);
  const [legacySetup, setLegacySetup] = useState<{ pluginIdentity: string; modelId: string }>();
  const [legacyContractFile, setLegacyContractFile] = useState<File>();
  const [legacyError, setLegacyError] = useState("");
  const [legacyDraft, setLegacyDraft] = useState({
    bundle_version: "1.0.0",
    display_name: "",
    upstream_project: "",
    upstream_model_id: "",
    upstream_version: "",
    source_url: "",
    exporter_name: "Existing ONNX export",
    exporter_version: "unknown",
    opset: "17",
    license_name: "",
    license_url: "",
    redistribution: "unknown" as "allowed" | "restricted" | "prohibited" | "unknown",
    commercial_use: "unknown" as "allowed" | "restricted" | "unknown",
    license_text: "",
    contract_document: "",
    license_accepted: false,
  });

  const load = async () => {
    setLoading(true);
    try {
      const pluginRegistry = await api.expertPlugins();
      const [modelRegistry, operationRegistry] = await Promise.all([
        api.modelInstances(),
        api.modelInstallOperations(),
      ]);
      const inventories = await Promise.all(pluginRegistry.installations.map(async (installation) => {
        const identity = pluginIdentity(installation);
        return [identity, await api.compatibleModelBundles(installation.manifest.id, installation.manifest.version)] as const;
      }));
      setRegistry(pluginRegistry);
      setInstances(modelRegistry.instances);
      setInstanceProfiles(modelRegistry.model_profiles);
      setInstallOperations(operationRegistry.operations);
      setBundleInventory(Object.fromEntries(inventories));
    } catch (error) {
      onError((error as Error).message);
    } finally {
      setLoading(false);
    }
  };
  useEffect(() => { void load(); }, []);

  const hasRunningInstall = installOperations.some((operation) => operation.status === "running");
  useEffect(() => {
    if (!hasRunningInstall) return;
    const timer = window.setInterval(() => {
      void api.modelInstallOperations().then((result) => {
        setInstallOperations(result.operations);
        if (!result.operations.some((operation) => operation.status === "running")) void load();
      }).catch((error) => onError((error as Error).message));
    }, 400);
    return () => window.clearInterval(timer);
  }, [hasRunningInstall]);

  const inspectPackage = async () => {
    if (!packageFile) return;
    setBusy("inspect");
    setMessage("");
    try {
      const result = await api.inspectExpertPluginPackage(packageFile);
      setVerified(result);
      setPermissionsReviewed(false);
      setCodeLicenseAccepted(false);
      setWeightLicenseAccepted(!result.manifest.weights.required);
      setMessage(result.web_installable
        ? "Package verification passed. Review every declaration before installing."
        : "Package integrity passed, but its publisher signature is not trusted for Web installation.");
    } catch (error) {
      onError((error as Error).message);
    } finally {
      setBusy("");
    }
  };

  const installPackage = async () => {
    if (!packageFile || !verified) return;
    setBusy("install");
    try {
      await api.installExpertPluginPackage(packageFile, {
        permissions_reviewed: permissionsReviewed,
        code_license_accepted: codeLicenseAccepted,
        weight_license_accepted: weightLicenseAccepted,
      });
      setPackageFile(undefined);
      setVerified(undefined);
      setMessage("Plugin runtime installed. Install a compatible verified model to make it usable.");
      await load();
    } catch (error) {
      onError((error as Error).message);
      await load();
    } finally {
      setBusy("");
    }
  };

  const inspectBundle = async () => {
    if (!bundleFile) return;
    setBusy("inspect-bundle");
    setMessage("");
    try {
      const result = await api.inspectModelBundlePackage(bundleFile);
      setVerifiedBundle(result);
      setBundleImportLicenseAccepted(!result.manifest.license.requires_acceptance);
      setMessage("Model Bundle verification passed. Review its identity and license before importing.");
    } catch (error) {
      onError((error as Error).message);
    } finally {
      setBusy("");
    }
  };

  const importBundle = async () => {
    if (!bundleFile || !verifiedBundle) return;
    setBusy("import-bundle");
    try {
      const result = await api.importModelBundlePackage(bundleFile, bundleImportLicenseAccepted);
      for (const instance of result.model_instances) await api.testModelInstance(instance.id);
      setBundleFile(undefined);
      setVerifiedBundle(undefined);
      setMessage(result.model_instances.length
        ? "Local Model Bundle imported, verified, and smoke tested."
        : "Local Model Bundle imported, but no compatible installed Plugin can create a Model Instance yet.");
      await load();
    } catch (error) {
      onError((error as Error).message);
      await load();
    } finally {
      setBusy("");
    }
  };

  const setupInstallation = registry?.installations.find((installation) => pluginIdentity(installation) === setupPluginIdentity);
  const setupInventory = bundleInventory[setupPluginIdentity];
  const setupEntry = setupInventory?.available.find((entry) => catalogBundleIdentity(entry) === setupEntryIdentity);
  const setupOperation = installOperations.find((operation) => operation.id === setupOperationId);
  const setupFailure = setupError || setupOperation?.error || "";
  const setupInstallStageIndex = setupOperation
    ? Math.max(0, MODEL_INSTALL_STAGES.findIndex((stage) => stage.id === setupOperation.stage))
    : 0;
  const setupDownloadPercent = setupOperation?.bytes_total
    ? Math.min(100, Math.round((setupOperation.bytes_completed / setupOperation.bytes_total) * 100))
    : undefined;
  const legacyInstallation = registry?.installations.find((installation) => pluginIdentity(installation) === legacySetup?.pluginIdentity);

  useEffect(() => {
    if (setupOperation?.status === "succeeded") {
      setSetupStep(5);
      setSetupError("");
      setMessage(`${setupEntry?.display_name ?? "The prompted-segmentation model"} is Ready and selectable by new Workflow Drafts.`);
    } else if (setupOperation?.status === "failed") {
      setSetupStep(4);
      setSetupError(setupOperation.error ?? "Model installation failed.");
    }
  }, [setupOperation?.status, setupOperation?.error]);

  useEffect(() => {
    if (!setupPluginIdentity) return;
    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    const focusFrame = window.requestAnimationFrame(() => setupCloseButtonRef.current?.focus());
    const handleKeyboard = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        setSetupPluginIdentity("");
        return;
      }
      if (event.key !== "Tab" || !setupDialogRef.current) return;
      const focusable = Array.from(setupDialogRef.current.querySelectorAll<HTMLElement>(
        'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
      )).filter((element) => !element.hidden);
      if (!focusable.length) return;
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };
    window.addEventListener("keydown", handleKeyboard);
    return () => {
      window.cancelAnimationFrame(focusFrame);
      window.removeEventListener("keydown", handleKeyboard);
      document.body.style.overflow = previousOverflow;
      const returnTarget = setupReturnFocusRef.current;
      window.requestAnimationFrame(() => returnTarget?.focus());
    };
  }, [setupPluginIdentity]);

  const openModelSetup = (installation: ExpertPluginInstallation, entry?: ModelCatalogEntry) => {
    setupReturnFocusRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const identity = pluginIdentity(installation);
    const selectedEntry = entry ?? bundleInventory[identity]?.available.find((candidate) => !candidate.fixture) ?? bundleInventory[identity]?.available[0];
    const latestOperation = installOperations.find((operation) =>
      operation.plugin_id === installation.manifest.id
      && operation.plugin_version === installation.manifest.version
      && (!selectedEntry || (operation.bundle_id === selectedEntry.bundle_id && operation.bundle_version === selectedEntry.bundle_version)),
    );
    const matchingInstance = selectedEntry && instances.find((instance) =>
      instance.plugin_id === installation.manifest.id
      && instance.plugin_version === installation.manifest.version
      && instance.model_bundle_id === selectedEntry.bundle_id
      && instance.model_bundle_version === selectedEntry.bundle_version,
    );
    setSetupPluginIdentity(identity);
    setSetupEntryIdentity(selectedEntry ? catalogBundleIdentity(selectedEntry) : "");
    setSetupOperationId(latestOperation?.id ?? "");
    setSetupStep(latestOperation?.status === "running" || latestOperation?.status === "failed"
      ? 4
      : matchingInstance?.status === "ready" || latestOperation?.status === "succeeded"
        ? 5
        : 0);
    setSetupLicenseAccepted(false);
    setSetupError(latestOperation?.status === "failed" ? latestOperation.error ?? "Model installation failed." : "");
  };

  const closeModelSetup = () => setSetupPluginIdentity("");

  const installSelectedBundle = async () => {
    if (!setupInstallation || !setupEntry?.catalog_id) return;
    setBusy("install-model-bundle");
    setSetupError("");
    try {
      if (setupEntry.license_summary.requires_acceptance) {
        await api.acceptModelBundleLicense(setupEntry.bundle_id, setupEntry.bundle_version, setupEntry.license_summary.license_digest);
      }
      setSetupStep(4);
      const operation = await api.startModelInstallOperation({
        catalog_id: setupEntry.catalog_id,
        bundle_id: setupEntry.bundle_id,
        bundle_version: setupEntry.bundle_version,
        plugin_id: setupInstallation.manifest.id,
        plugin_version: setupInstallation.manifest.version,
      });
      setSetupOperationId(operation.id);
      setInstallOperations((current) => [operation, ...current.filter((item) => item.id !== operation.id)]);
    } catch (error) {
      setSetupError((error as Error).message);
      await load();
    } finally {
      setBusy("");
    }
  };

  const openLegacyBundleSetup = (installation: ExpertPluginInstallation, modelId: string) => {
    const model = installation.manifest.models.find((item) => item.id === modelId);
    setLegacySetup({ pluginIdentity: pluginIdentity(installation), modelId });
    setLegacyContractFile(undefined);
    setLegacyError("");
    setLegacyDraft((current) => ({
      ...current,
      display_name: `${model?.display_name ?? modelId} · Local Bundle`,
      upstream_model_id: modelId,
      license_name: installation.manifest.license.weights,
      license_accepted: false,
      contract_document: "",
    }));
  };

  const createLegacyBundle = async () => {
    const installation = registry?.installations.find((item) => pluginIdentity(item) === legacySetup?.pluginIdentity);
    if (!installation || !legacySetup) return;
    setBusy("create-legacy-bundle");
    setLegacyError("");
    try {
      const result = await api.createLegacyLocalModelBundle(installation.manifest.id, installation.manifest.version, {
        model_id: legacySetup.modelId,
        bundle_version: legacyDraft.bundle_version,
        display_name: legacyDraft.display_name,
        upstream_project: legacyDraft.upstream_project,
        upstream_model_id: legacyDraft.upstream_model_id,
        ...(legacyDraft.upstream_version.trim() ? { upstream_version: legacyDraft.upstream_version.trim() } : {}),
        ...(legacyDraft.source_url.trim() ? { source_url: legacyDraft.source_url.trim() } : {}),
        exporter_name: legacyDraft.exporter_name,
        exporter_version: legacyDraft.exporter_version,
        opset: Number(legacyDraft.opset),
        license_name: legacyDraft.license_name,
        ...(legacyDraft.license_url.trim() ? { license_url: legacyDraft.license_url.trim() } : {}),
        redistribution: legacyDraft.redistribution,
        commercial_use: legacyDraft.commercial_use,
        license_text: legacyDraft.license_text,
        contract_document: legacyDraft.contract_document,
        license_accepted: legacyDraft.license_accepted,
      });
      const ready = result.model_instances.some((instance) => instance.status === "ready");
      setMessage(ready
        ? `Local Model Bundle created and smoke tested. Export retained at ${result.local_bundle_path}.`
        : `Local Model Bundle was created at ${result.local_bundle_path}, but its Model Instance did not pass the fixed smoke test. Legacy files were preserved.`);
      setLegacySetup(undefined);
      await load();
    } catch (error) {
      setLegacyError((error as Error).message);
      await load();
    } finally {
      setBusy("");
    }
  };

  const perform = async (key: string, action: () => Promise<unknown>, success: string) => {
    setBusy(key);
    try {
      await action();
      setMessage(success);
      await load();
    } catch (error) {
      onError((error as Error).message);
    } finally {
      setBusy("");
    }
  };

  const installations = registry?.installations ?? [];
  const hasWorkflowReadyModel = (installation: (typeof installations)[number]) => instances.some((instance) =>
    instance.plugin_id === installation.manifest.id
    && instance.plugin_version === installation.manifest.version
    && instance.status === "ready"
    && bundleInventory[pluginIdentity(installation)]?.installed.some((bundle) =>
      bundle.manifest.id === instance.model_bundle_id
      && bundle.manifest.version === instance.model_bundle_version
      && bundle.manifest.publishable),
  );
  const readyModels = instanceProfiles.filter((model) => model.selectable).length;
  const setupInstallations = installations.filter((installation) =>
    !hasWorkflowReadyModel(installation)
    && ["installed", "needs_weights", "unsupported_platform"].includes(installation.status),
  ).length;
  const attentionInstallations = installations.filter((installation) =>
    ["unhealthy", "crashed", "failed_smoke_test", "incompatible_api", "invalid_manifest", "invalid_contract"].includes(installation.status),
  ).length;
  return <section className="registry-page expert-plugin-page">
    <header className="plugin-page-hero">
      <div className="plugin-page-heading">
        <span className="eyebrow">{t("Local model runtime")}</span>
        <h2>{t("Expert Model Plugins")}</h2>
        <p>Install an isolated Rust runtime, then pair it with a verified, versioned Model Bundle. Files, licenses, Contracts, and smoke-test evidence stay visible.</p>
        <div className="plugin-trust-line" aria-label={t("Plugin runtime properties")}><span>{t("Isolated Rust process")}</span><span>{t("Verified Model Bundles")}</span><span>{t("Immutable versions")}</span></div>
      </div>
      <dl className="plugin-registry-summary" aria-label={t("Plugin Registry summary")}>
        <div><dt>{t("Ready models")}</dt><dd>{readyModels}</dd><small>{t("selectable now")}</small></div>
        <div><dt>{t("Finish setup")}</dt><dd>{setupInstallations}</dd><small>{t("installed packages")}</small></div>
        <div className={attentionInstallations ? "attention" : ""}><dt>{t("Needs attention")}</dt><dd>{attentionInstallations}</dd><small>{t("failed checks")}</small></div>
      </dl>
    </header>

    <section className="plugin-page-guidance" aria-label={t("Plugin setup overview")}>
      <article className="plugin-setup-roadmap">
        <div><span className="eyebrow">{t("How setup works")}</span><strong>{t("From local package to selectable model")}</strong></div>
        <ol>
          <li><span>1</span><small>{t("Install package")}</small></li>
          <li><span>2</span><small>{t("Install model")}</small></li>
          <li><span>3</span><small>{t("Smoke test")}</small></li>
          <li><span>4</span><small>{t("Use in Workflow")}</small></li>
        </ol>
      </article>
      <article className="plugin-agent-policy" aria-label={t("Agent plugin permissions")}>
        <span className="plugin-policy-mark" aria-hidden="true">A</span>
        <div><strong>{t("Agent discovery is read only")}</strong><p>Pipeline Builder can recommend compatible Ready models. Installation, licenses, model assets, and executables always remain manual actions.</p></div>
      </article>
    </section>

    <details className="plugin-install-wizard" open={!installations.length}>
      <summary><span className="plugin-install-summary"><i aria-hidden="true">+</i><span><strong>{t("Install plugin runtime")}</strong><small>Advanced setup for a local .annotplugin package. Model assets are installed separately.</small></span></span><b>{t("Advanced")}</b></summary>
      <div className="plugin-install-body">
        <ol className="plugin-install-steps" aria-label={t("Installation steps")}>
          {['Select package', 'Verify package', 'Review permissions', 'Install runtime'].map((step, index) => <li className={verified && index < 2 ? "complete" : ""} key={step}><span>{index + 1}</span>{step}</li>)}
        </ol>
        <div className="plugin-package-picker">
          <label htmlFor="expert-plugin-package">{t("Plugin package")}</label>
          <FilePicker id="expert-plugin-package" label={t("Plugin package")} accept=".annotplugin" file={packageFile} chooseLabel="Choose plugin package" emptyLabel="No .annotplugin selected" onSelect={(file) => { setPackageFile(file); setVerified(undefined); setMessage(""); }} />
          <button onClick={inspectPackage} disabled={!packageFile || Boolean(busy)}>{busy === "inspect" ? t("Verifying…") : t("Verify package")}</button>
        </div>
        {verified && <div className="plugin-review-grid">
          <section>
            <span className="eyebrow">{t("Package identity")}</span>
            <h3>{verified.manifest.display_name} <small>v{verified.manifest.version}</small></h3>
            <p>{verified.manifest.description}</p>
            <dl className="registry-facts">
              <div><dt>{t("Publisher")}</dt><dd>{verified.manifest.publisher}</dd></div>
              <div><dt>{t("Package hash")}</dt><dd title={verified.package_sha256}>{verified.package_sha256.slice(0, 12)}…</dd></div>
              <div><dt>{t("Plugin API")}</dt><dd>{verified.manifest.plugin_api}</dd></div>
              <div><dt>{t("Runtime")}</dt><dd>{t("Native Rust process")}</dd></div>
              <div><dt>{t("Targets")}</dt><dd>{verified.manifest.compatibility.targets.join(", ")}</dd></div>
              <div><dt>{t("Implementation")}</dt><dd>{verified.manifest.implementation_status.replaceAll("_", " ")}</dd></div>
              <div><dt>{t("Publisher signature")}</dt><dd>{verified.signature_trusted ? t("Trusted") : verified.signature.replaceAll("_", " ")}</dd></div>
            </dl>
          </section>
          <fieldset className="plugin-review-checks">
            <legend>{t("Required human review")}</legend>
            <div className="plugin-permission-summary">
              <span>{t("Network")}{" "}<strong>{verified.manifest.permissions.network.replaceAll("_", " ")}</strong></span>
              <span>{t("Provider secrets")}{" "}<strong>{verified.manifest.permissions.provider_secrets ? t("Requested") : t("Denied")}</strong></span>
              <span>{t("Project files")}{" "}<strong>{verified.manifest.permissions.project_files ? t("Requested") : t("Denied")}</strong></span>
              <span>{t("Subprocesses")}{" "}<strong>{verified.manifest.permissions.subprocesses ? t("Requested") : t("Denied")}</strong></span>
            </div>
            <label className="checkbox-line"><input type="checkbox" checked={permissionsReviewed} onChange={(event) => setPermissionsReviewed(event.target.checked)} /><span>I reviewed the publisher, target, runtime resources, and requested permissions.</span></label>
            <label className="checkbox-line"><input type="checkbox" checked={codeLicenseAccepted} onChange={(event) => setCodeLicenseAccepted(event.target.checked)} /><span>{t("I accept the code license:")}{" "}<strong>{verified.manifest.license.code}</strong>.</span></label>
            {verified.manifest.weights.required && <label className="checkbox-line"><input type="checkbox" checked={weightLicenseAccepted} onChange={(event) => setWeightLicenseAccepted(event.target.checked)} /><span>{t("I accept the weight license:")}{" "}<strong>{verified.manifest.license.weights}</strong>.</span></label>}
            {!verified.web_installable && <p className="setup-requirement" role="status">{verified.install_guidance}</p>}
            <button className="primary" onClick={installPackage} disabled={Boolean(busy) || !verified.web_installable || !permissionsReviewed || !codeLicenseAccepted || (verified.manifest.weights.required && !weightLicenseAccepted)} title={!verified.web_installable ? verified.install_guidance : undefined}>{busy === "install" ? t("Installing…") : t("Install trusted package")}</button>
          </fieldset>
        </div>}
      </div>
    </details>

    <details className="plugin-install-wizard model-bundle-import">
      <summary><span className="plugin-install-summary"><i aria-hidden="true">⇧</i><span><strong>{t("Import .annotmodel")}</strong><small>Advanced local import for an already prepared data-only Model Bundle.</small></span></span><b>{t("Advanced")}</b></summary>
      <div className="plugin-install-body">
        <div className="plugin-package-picker">
          <label htmlFor="expert-model-bundle">{t("Model Bundle")}</label>
          <FilePicker id="expert-model-bundle" label={t("Model Bundle")} accept=".annotmodel" file={bundleFile} chooseLabel="Choose model bundle" emptyLabel="No .annotmodel selected" onSelect={(file) => { setBundleFile(file); setVerifiedBundle(undefined); setMessage(""); }} />
          <button onClick={inspectBundle} disabled={!bundleFile || Boolean(busy)}>{busy === "inspect-bundle" ? t("Verifying…") : t("Verify Bundle")}</button>
        </div>
        {verifiedBundle && <div className="bundle-import-review">
          <div><span className="eyebrow">{t("Verified package")}</span><h3>{verifiedBundle.manifest.display_name}</h3><p>{verifiedBundle.manifest.source.upstream_project} · {verifiedBundle.manifest.architecture} · {verifiedBundle.file_count} files</p><code>{verifiedBundle.bundle_sha256}</code></div>
          <div><strong>{verifiedBundle.manifest.license.name}</strong><small>{verifiedBundle.manifest.license.redistribution} redistribution · {verifiedBundle.manifest.license.commercial_use} commercial use</small>{verifiedBundle.manifest.license.requires_acceptance && <label className="checkbox-line"><input type="checkbox" checked={bundleImportLicenseAccepted} onChange={(event) => setBundleImportLicenseAccepted(event.target.checked)} /><span>I accept this exact license digest.</span></label>}<button className="primary" onClick={importBundle} disabled={Boolean(busy) || !bundleImportLicenseAccepted}>{busy === "import-bundle" ? t("Importing and testing…") : t("Import verified Bundle")}</button></div>
        </div>}
      </div>
    </details>

    {setupInstallation && <div className="modal-backdrop model-setup-backdrop">
      <section ref={setupDialogRef} className="model-setup-wizard" role="dialog" aria-modal="true" aria-labelledby="model-setup-title" aria-describedby="model-setup-description">
        <header><div><span className="eyebrow">{t("Model Setup")}</span><h3 id="model-setup-title">{setupInstallation.manifest.display_name}</h3><p id="model-setup-description">Only a verified Bundle that passes the exact Rust Plugin smoke test becomes selectable.</p></div><button ref={setupCloseButtonRef} type="button" className="icon-button" aria-label={t("Close model setup")} onClick={closeModelSetup}>×</button></header>
        <div className="model-setup-scroll">
          <ol className="model-setup-progress" aria-label={t("Model installation progress")}>
            {MODEL_SETUP_STEPS.map((step, index) => <li className={index < setupStep ? "complete" : index === setupStep ? "current" : ""} key={step}><span>{index < setupStep ? "✓" : index + 1}</span><small>{step}</small></li>)}
          </ol>
          {setupStep === 0 && <div className="model-setup-content"><span className="eyebrow">{t("Select model")}</span>{setupInventory?.available.length ? <div className="compatible-model-list">{setupInventory.available.map((entry) => <label className={setupEntryIdentity === catalogBundleIdentity(entry) ? "selected" : ""} key={catalogBundleIdentity(entry)}><input type="radio" name="setup-model" checked={setupEntryIdentity === catalogBundleIdentity(entry)} onChange={() => setSetupEntryIdentity(catalogBundleIdentity(entry))} /><span><strong>{entry.display_name}</strong><small>{entry.description}</small><small>{formatPluginBytes(entry.bundle_size_bytes)} · {entry.license_summary.name}</small></span><Status status={entry.fixture ? "Fixture" : "Ready to install"} /></label>)}</div> : setupInventory?.setup_blockers?.length ? <div className="model-setup-blocker" role="status"><span className="model-setup-blocker-label">{t("Update required")}</span><strong>{t("Plugin runtime update required")}</strong><p>{setupInventory.setup_blockers[0].message}</p><small>Model files were not downloaded again. The installed Bundle and previous Plugin version remain unchanged.</small></div> : <Empty title={t("No verified bundle is available for this platform")} detail="The Plugin remains installed, but AnnotAgent will not suggest raw ONNX downloads or an unverified model." />}</div>}
          {setupStep === 1 && setupEntry && <div className="model-setup-content"><span className="eyebrow">{t("Review source")}</span><dl className="bundle-review-facts"><div><dt>{t("Model")}</dt><dd>{setupEntry.display_name}</dd></div><div><dt>{t("Model family")}</dt><dd>{setupEntry.model_family ?? t("Declared by the verified Bundle")}</dd></div><div><dt>{t("Capability")}</dt><dd>{setupEntry.capabilities.map((value) => value.replaceAll("_", " ")).join(", ")}</dd></div><div><dt>{t("Publisher")}</dt><dd>{setupEntry.publisher.display_name}{setupEntry.publisher.verified ? " · verified" : " · unverified"}</dd></div><div><dt>{t("Curated Catalog")}</dt><dd>{setupEntry.catalog_id}</dd></div><div><dt>{t("Bundle digest")}</dt><dd>{setupEntry.bundle_sha256}</dd></div><div><dt>{t("Download size")}</dt><dd>{formatPluginBytes(setupEntry.bundle_size_bytes)}</dd></div><div><dt>{t("Installed size")}</dt><dd>{formatPluginBytes(setupEntry.installed_size_bytes ?? setupEntry.platform_requirements[0]?.minimum_disk_bytes ?? setupEntry.bundle_size_bytes)}</dd></div><div><dt>{t("Delivery")}</dt><dd>{setupEntry.fixture ? "Built-in deterministic local Catalog" : setupEntry.bundle_url}</dd></div><div><dt>{t("Release status")}</dt><dd>{setupEntry.fixture ? "Fixture only · not publishable" : t("Real model · production eligible")}</dd></div></dl></div>}
          {setupStep === 2 && setupEntry && <div className="model-setup-content"><span className="eyebrow">{t("Review license")}</span><div className="license-review-card"><h4>{setupEntry.license_summary.name}</h4><p>Redistribution: {setupEntry.license_summary.redistribution.replaceAll("_", " ")} · Commercial use: {setupEntry.license_summary.commercial_use.replaceAll("_", " ")}</p><code>{setupEntry.license_summary.license_digest}</code>{setupEntry.license_summary.license_url && <a href={setupEntry.license_summary.license_url} target="_blank" rel="noreferrer">{t("Read license source")}</a>}{setupEntry.license_summary.requires_acceptance && <label className="checkbox-line"><input type="checkbox" checked={setupLicenseAccepted} onChange={(event) => setSetupLicenseAccepted(event.target.checked)} /><span>{t("I accept this exact model license and digest.")}</span></label>}</div></div>}
          {setupStep === 3 && setupEntry && <div className="model-setup-content"><span className="eyebrow">{t("Check compatibility")}</span><div className="compatibility-checks"><span className="passed"><b>✓</b><strong>{t("Plugin")}</strong><small>{setupInstallation.manifest.id}@{setupInstallation.manifest.version}</small></span><span className="passed"><b>✓</b><strong>{t("Model binding")}</strong><small>{setupEntry.compatible_plugins.map((item) => `${item.model_id} · ${item.required_file_roles.join(" + ")}`).join(", ")}</small></span><span className={setupEntry.platform_requirements.length ? "passed" : "blocked"}><b>{setupEntry.platform_requirements.length ? "✓" : "—"}</b><strong>{t("Platform")}</strong><small>{setupEntry.platform_requirements.map((item) => item.target).join(", ") || t("No supported platform")}</small></span><span className={setupInventory?.plugin_runtime_status === "incompatible" ? "blocked" : "passed"}><b>{setupInventory?.plugin_runtime_status === "incompatible" ? "—" : "✓"}</b><strong>{t("Execution provider")}</strong><small>{setupEntry.platform_requirements.flatMap((item) => item.execution_providers).join(", ").toUpperCase()} · Rust {setupInventory?.plugin_runtime_status.replaceAll("_", " ")}</small></span></div></div>}
          {setupStep === 4 && <div className="model-setup-content install-stage"><span className="eyebrow">{t("Installation evidence")}</span><div className="model-install-live"><div><h4>{setupOperation?.status === "failed" ? t("Setup stopped safely") : setupOperation ? MODEL_INSTALL_STAGES[setupInstallStageIndex]?.label : t("Ready to retry")}</h4><p>{setupOperation?.detail ?? t("Review the failure below, then retry from the verified Catalog entry.")}</p></div>{setupOperation?.status === "running" && <Status status="Running" />}</div>{setupDownloadPercent !== undefined && setupOperation?.status === "running" && <div className="model-install-meter" aria-label={`Model download ${setupDownloadPercent}%`}><span style={{ width: `${setupDownloadPercent}%` }} /></div>}<ol className="model-install-stage-list" aria-label="Real model installation stages">{MODEL_INSTALL_STAGES.map((stage, index) => <li className={setupOperation?.status === "failed" && index === setupInstallStageIndex ? "failed" : index < setupInstallStageIndex || setupOperation?.status === "succeeded" ? "complete" : index === setupInstallStageIndex ? "current" : "pending"} key={stage.id}><i aria-hidden="true">{index < setupInstallStageIndex || setupOperation?.status === "succeeded" ? "✓" : index === setupInstallStageIndex && setupOperation?.status === "failed" ? "!" : index + 1}</i><span>{stage.label}</span></li>)}</ol></div>}
          {setupStep === 5 && <div className="model-setup-content install-stage ready"><span className="eyebrow">{t("Installation evidence")}</span><h4>{setupEntry?.fixture ? "Fixture Model Instance Ready" : t("Real Model Instance Ready")}</h4><p>{setupEntry?.fixture ? "The Fixture passed its deterministic Rust Plugin test but remains ineligible for Published Workflows." : "The exact Bundle, real ONNX graphs, Rust Plugin, bbox-prompt sample inference, mask validation, and immutable Model Profile are verified. This model is now selectable by Workflow Drafts."}</p>{setupOperation?.model_instance_ids.length ? <code>{setupOperation.model_instance_ids.join(", ")}</code> : null}</div>}
          {setupFailure && <div className="model-setup-error" role="alert"><strong>{t("Setup stopped at")}{" "}{setupOperation ? MODEL_INSTALL_STAGES[setupInstallStageIndex]?.label : MODEL_SETUP_STEPS[setupStep]}</strong><span>{setupFailure}</span><small>{setupOperation?.suggested_action ?? "Review the selected Catalog entry and Plugin compatibility, then retry. Existing verified assets were preserved."}</small></div>}
        </div>
        <footer>{setupStep > 0 && setupStep < 4 && <button onClick={() => { setSetupStep((value) => value - 1); setSetupError(""); }} disabled={Boolean(busy)}>{t("Back")}</button>}<span />{setupStep < 3 && <button className="primary" onClick={() => setSetupStep((value) => value + 1)} disabled={!setupEntry || (setupStep === 2 && setupEntry.license_summary.requires_acceptance && !setupLicenseAccepted)}>{t("Continue")}</button>}{setupStep === 3 && <button className="primary" onClick={installSelectedBundle} disabled={Boolean(busy) || !setupEntry}>{busy === "install-model-bundle" ? t("Starting installation…") : t("Install model")}</button>}{setupStep === 5 && <button className="primary" onClick={closeModelSetup}>{t("Done")}</button>}{setupFailure && setupStep === 4 && <button className="primary" onClick={() => { setSetupStep(3); setSetupOperationId(""); setSetupError(""); }}>{t("Review and retry")}</button>}</footer>
      </section>
    </div>}

    {legacyInstallation && legacySetup && <section className="legacy-bundle-creator" aria-label={t("Create local model bundle")}>
      <header><div><span className="eyebrow">{t("Legacy migration")}</span><h3>{t("Create local model bundle")}</h3><p>AnnotAgent copies the existing files into a data-only Bundle, verifies every hash and ONNX tensor Contract, then runs the exact Rust Plugin smoke test. The originals are never deleted.</p></div><button className="icon-button" aria-label="Close local bundle creator" onClick={() => setLegacySetup(undefined)}>×</button></header>
      <div className="legacy-bundle-form">
        <fieldset><legend>Bundle identity</legend><label>{t("Display name")}<input value={legacyDraft.display_name} onChange={(event) => setLegacyDraft((current) => ({ ...current, display_name: event.target.value }))} /></label><label>Bundle version<input value={legacyDraft.bundle_version} onChange={(event) => setLegacyDraft((current) => ({ ...current, bundle_version: event.target.value }))} placeholder="1.0.0" /></label><label>{t("Model")}<input value={legacySetup.modelId} readOnly /></label></fieldset>
        <fieldset><legend>Upstream source</legend><label>Upstream project<input value={legacyDraft.upstream_project} onChange={(event) => setLegacyDraft((current) => ({ ...current, upstream_project: event.target.value }))} placeholder="Project or organization" /></label><label>Upstream model ID<input value={legacyDraft.upstream_model_id} onChange={(event) => setLegacyDraft((current) => ({ ...current, upstream_model_id: event.target.value }))} /></label><label>Upstream version<input value={legacyDraft.upstream_version} onChange={(event) => setLegacyDraft((current) => ({ ...current, upstream_version: event.target.value }))} /></label><label>Source URL<input type="url" value={legacyDraft.source_url} onChange={(event) => setLegacyDraft((current) => ({ ...current, source_url: event.target.value }))} placeholder="https://…" /></label></fieldset>
        <fieldset><legend>Export provenance</legend><label>Exporter name<input value={legacyDraft.exporter_name} onChange={(event) => setLegacyDraft((current) => ({ ...current, exporter_name: event.target.value }))} /></label><label>Exporter version<input value={legacyDraft.exporter_version} onChange={(event) => setLegacyDraft((current) => ({ ...current, exporter_version: event.target.value }))} /></label><label>ONNX opset<input type="number" min="1" max="21" value={legacyDraft.opset} onChange={(event) => setLegacyDraft((current) => ({ ...current, opset: event.target.value }))} /></label></fieldset>
        <fieldset><legend>Model license</legend><label>License name<input value={legacyDraft.license_name} onChange={(event) => setLegacyDraft((current) => ({ ...current, license_name: event.target.value }))} /></label><label>License URL<input type="url" value={legacyDraft.license_url} onChange={(event) => setLegacyDraft((current) => ({ ...current, license_url: event.target.value }))} /></label><label>Redistribution<select value={legacyDraft.redistribution} onChange={(event) => setLegacyDraft((current) => ({ ...current, redistribution: event.target.value as typeof current.redistribution }))}><option value="allowed">Allowed</option><option value="restricted">Restricted</option><option value="unknown">{t("Unknown")}</option><option value="prohibited">Prohibited</option></select></label><label>Commercial use<select value={legacyDraft.commercial_use} onChange={(event) => setLegacyDraft((current) => ({ ...current, commercial_use: event.target.value as typeof current.commercial_use }))}><option value="allowed">Allowed</option><option value="restricted">Restricted</option><option value="unknown">{t("Unknown")}</option></select></label><label className="wide">Exact license text<textarea value={legacyDraft.license_text} onChange={(event) => setLegacyDraft((current) => ({ ...current, license_text: event.target.value }))} rows={7} /></label>{legacyDraft.redistribution === "prohibited" && <p className="wide">A redistribution-prohibited asset cannot be packaged as a publishable local Bundle. Keep the legacy files unchanged and resolve the license terms first.</p>}</fieldset>
        <fieldset><legend>ONNX Model Contract</legend><div className="wide legacy-contract-file"><label htmlFor="legacy-contract-json">Contract JSON file</label><FilePicker id="legacy-contract-json" label="Contract JSON file" accept=".json,application/json" file={legacyContractFile} chooseLabel="Choose Contract JSON" emptyLabel="No Contract JSON selected" onSelect={(file) => { setLegacyContractFile(file); if (!file) { setLegacyDraft((current) => ({ ...current, contract_document: "" })); return; } void file.text().then((contract_document) => setLegacyDraft((current) => ({ ...current, contract_document }))); }} /></div><p className="wide">The Contract must use schema version 1 and exactly declare these file roles: <code>{legacyInstallation.manifest.models.find((model) => model.id === legacySetup.modelId)?.required_file_roles.join(", ")}</code>.</p>{legacyDraft.contract_document && <pre className="wide">{legacyDraft.contract_document.slice(0, 800)}{legacyDraft.contract_document.length > 800 ? "…" : ""}</pre>}</fieldset>
      </div>
      <label className="checkbox-line legacy-license-acceptance"><input type="checkbox" checked={legacyDraft.license_accepted} onChange={(event) => setLegacyDraft((current) => ({ ...current, license_accepted: event.target.checked }))} /><span>I supplied and accept the exact license above. I understand this local migration is not publisher-verified.</span></label>
      {legacyError && <div className="model-setup-error" role="alert"><strong>Local Bundle creation stopped</strong><span>{legacyError}</span><small>The legacy model files were not changed or removed.</small></div>}
      <footer><button onClick={() => setLegacySetup(undefined)} disabled={Boolean(busy)}>{t("Cancel")}</button><button className="primary" onClick={createLegacyBundle} disabled={Boolean(busy) || legacyDraft.redistribution === "prohibited" || !legacyDraft.display_name.trim() || !legacyDraft.upstream_project.trim() || !legacyDraft.upstream_model_id.trim() || !legacyDraft.license_name.trim() || !legacyDraft.license_text.trim() || !legacyDraft.contract_document.trim() || !legacyDraft.license_accepted}>{busy === "create-legacy-bundle" ? t("Hashing, packing, and testing…") : t("Create and test local Bundle")}</button></footer>
    </section>}

    {message && <p className="registry-message" role="status" aria-live="polite">{message}</p>}
    {loading && <div className="loading-banner" role="status">{t("Loading the local Plugin Registry…")}</div>}
    {!loading && !installations.length && <Empty title={t("No Expert Model Plugins installed")} detail="Install a verified .annotplugin package above. No model becomes selectable until its real process test passes." />}

    {PLUGIN_STATUS_GROUPS.map((group) => {
      const groupItems = installations.filter((installation) => {
        return group.statuses.includes(hasWorkflowReadyModel(installation) ? "ready" : installation.status);
      });
      if (!groupItems.length) return null;
      return <section className="plugin-status-group" key={group.title} aria-labelledby={`plugin-group-${group.title.replaceAll(" ", "-")}`}>
        <header><div><h3 id={`plugin-group-${group.title.replaceAll(" ", "-")}`}>{t(group.title)}</h3><p>{t(group.description)}</p></div><span>{groupItems.length}</span></header>
        <div className="plugin-card-grid">
          {groupItems.map((installation) => {
            const identity = `${installation.manifest.id}@${installation.manifest.version}`;
            const inventory = bundleInventory[identity] ?? { plugin_runtime_status: "installed", available: [], installed: [], setup_blockers: [] };
            const setupBlocker = inventory.setup_blockers[0];
            const pluginInstances = instances.filter((instance) => instance.plugin_id === installation.manifest.id && instance.plugin_version === installation.manifest.version);
            const readyInstances = pluginInstances.filter((instance) => instance.status === "ready");
            const workflowReadyInstances = readyInstances.filter((instance) => inventory.installed.some((bundle) => bundle.manifest.id === instance.model_bundle_id && bundle.manifest.version === instance.model_bundle_version && bundle.manifest.publishable));
            const fixtureReadyInstances = readyInstances.filter((instance) => inventory.installed.some((bundle) => bundle.manifest.id === instance.model_bundle_id && bundle.manifest.version === instance.model_bundle_version && bundle.manifest.fixture));
            const latestInstallOperation = installOperations.find((operation) => operation.plugin_id === installation.manifest.id && operation.plugin_version === installation.manifest.version);
            const setupState = latestInstallOperation?.status === "running"
              ? { tone: "setup", eyebrow: "Installation in progress", title: MODEL_INSTALL_STAGES.find((stage) => stage.id === latestInstallOperation.stage)?.label ?? "Installing model", detail: latestInstallOperation.detail }
              : latestInstallOperation?.status === "failed" && !workflowReadyInstances.length
                ? { tone: "blocked", eyebrow: "Setup needs attention", title: `Stopped at ${MODEL_INSTALL_STAGES.find((stage) => stage.id === latestInstallOperation.stage)?.label ?? "model setup"}`, detail: latestInstallOperation.suggested_action ?? latestInstallOperation.error ?? "Review the structured failure and retry." }
                : workflowReadyInstances.length
              ? { tone: "ready", eyebrow: "Ready for Workflows", title: `${workflowReadyInstances.length} verified model${workflowReadyInstances.length === 1 ? "" : t("s")} available`, detail: "Plugin, Bundle, Contract, and sample inference evidence are registered." }
              : fixtureReadyInstances.length
                ? { tone: "setup", eyebrow: "Offline Fixture verified", title: "A real model is still required", detail: "The Rust provisioning path works, but the Fixture is not SAM, accuracy evidence, or publishable." }
              : installation.status === "unsupported_platform"
                ? { tone: "blocked", eyebrow: "Unavailable on this platform", title: "No compatible native runtime", detail: "This package remains visible but cannot be enabled or selected." }
                : setupBlocker
                  ? { tone: "blocked", eyebrow: "Runtime update required", title: "Install a compatible Plugin version", detail: setupBlocker.message }
                : inventory.installed.length
                  ? { tone: "setup", eyebrow: "Next step", title: "Finish Model Bundle verification", detail: "Run the fixed smoke test or inspect the latest structured failure." }
                  : { tone: "setup", eyebrow: "Model required", title: "No compatible model installed", detail: "This Plugin cannot run until a verified Model Bundle is installed." };
            return <article className="plugin-card" key={identity}>
              <header><div className="registry-monogram">RS</div><div><strong>{installation.manifest.display_name}</strong><small>{installation.manifest.id} · v{installation.manifest.version}</small></div><Status status={workflowReadyInstances.length ? "Ready" : installation.status === "needs_weights" ? "Model required" : installation.status.replaceAll("_", " ")} /></header>
              <div className={`plugin-next-action ${setupState.tone}`}><span>{setupState.eyebrow}</span><strong>{setupState.title}</strong><small>{setupState.detail}</small></div>
              <p>{installation.manifest.description}</p>
              <dl className="plugin-card-facts">
                <div><dt>{t("Capabilities")}</dt><dd>{installation.manifest.models.flatMap((model) => model.capabilities).map((value) => value.replaceAll("_", " ")).join(", ")}</dd></div>
                <div><dt>{t("Runtime")}</dt><dd>Rust native process</dd></div>
                <div><dt>{t("Runtime models")}</dt><dd>{installation.manifest.models.map((model) => model.display_name).join(", ")}</dd></div>
                <div><dt>{t("Device")}</dt><dd>{installation.manifest.compatibility.accelerators.join(", ") || "CPU"}</dd></div>
                <div><dt>{t("Installed Bundles")}</dt><dd>{inventory.installed.length}</dd></div>
                <div><dt>{t("Used by")}</dt><dd>{installation.references.length} Published Workflow reference{installation.references.length === 1 ? "" : t("s")}</dd></div>
              </dl>
              <div className="registry-card-actions">
                {!workflowReadyInstances.length && <button className="primary" onClick={() => openModelSetup(installation)} disabled={Boolean(busy) || installation.status === "unsupported_platform"}>{latestInstallOperation?.status === "running" ? t("View installation") : latestInstallOperation?.status === "failed" ? t("Review failed setup") : setupBlocker ? t("Review required update") : t("Install compatible model")}</button>}
                <button onClick={() => perform(`${identity}:toggle`, () => api.setExpertPluginEnabled(installation.manifest.id, installation.manifest.version, !installation.enabled), installation.enabled ? "Plugin disabled." : "Plugin enabled; test evidence is preserved.")} disabled={Boolean(busy) || installation.status === "unsupported_platform"}>{installation.enabled ? t("Disable") : t("Enable")}</button>
                <button className="danger-button" onClick={() => { if (window.confirm(`Uninstall ${identity}? Installed Model Bundles remain in the shared model store.`)) void perform(`${identity}:uninstall`, () => api.uninstallExpertPlugin(installation.manifest.id, installation.manifest.version), "Plugin version uninstalled."); }} disabled={Boolean(busy) || installation.references.length > 0} title={installation.references.length ? t("Published Workflow references protect this exact version") : undefined}>{t("Uninstall")}</button>
              </div>
              <details className="registry-card-section" open>
                <summary><span>{t("Runtime")}</span><small>{inventory.plugin_runtime_status.replaceAll("_", " ")}</small></summary>
                <dl className="plugin-detail-list"><div><dt>{t("Process")}</dt><dd>Isolated native Rust</dd></div><div><dt>{t("Protocol")}</dt><dd>{installation.manifest.runtime.protocol}</dd></div><div><dt>{t("Plugin version")}</dt><dd>{installation.manifest.version}</dd></div><div><dt>Package SHA-256</dt><dd>{installation.package_sha256}</dd></div></dl>
              </details>
              <details className="registry-card-section" open={!inventory.installed.length}>
                <summary><span>{t("Compatible Models")}</span><small>{inventory.available.length}{" "}{t("available")}</small></summary>
                {inventory.available.length ? <div className="compatible-model-list compact">{inventory.available.map((entry) => { const installedInstance = pluginInstances.find((instance) => instance.model_bundle_id === entry.bundle_id && instance.model_bundle_version === entry.bundle_version); const entryOperation = installOperations.find((operation) => operation.plugin_id === installation.manifest.id && operation.plugin_version === installation.manifest.version && operation.bundle_id === entry.bundle_id && operation.bundle_version === entry.bundle_version); return <div key={catalogBundleIdentity(entry)}><span><strong>{entry.display_name}</strong><small>{entry.model_family ?? entry.bundle_id} · {entry.publisher.display_name} · {formatPluginBytes(entry.bundle_size_bytes)} · {entry.license_summary.name}</small></span><Status status={entry.fixture ? "Fixture" : installedInstance?.status === "ready" ? "Ready" : entryOperation?.status === "running" ? "Installing" : "Ready to install"} /><button onClick={() => openModelSetup(installation, entry)}>{entryOperation?.status === "running" ? t("View progress") : installedInstance?.status === "ready" ? t("View evidence") : entryOperation?.status === "failed" ? t("Review failure") : t("Install model")}</button></div>; })}</div> : setupBlocker ? <div className="bundle-empty-state warning"><strong>{t("Plugin runtime update required")}</strong><p>{setupBlocker.message}</p><button onClick={() => openModelSetup(installation)}>{t("Review required update")}</button></div> : <div className="bundle-empty-state"><strong>{t("No verified bundle is available for this platform")}</strong><p>Unpublished SAM 2 and unverified checkpoints stay in Labs; AnnotAgent will not turn them into a selectable model.</p></div>}
              </details>
              <details className="registry-card-section">
                <summary><span>{t("Installed Models")}</span><small>{pluginInstances.length} instances</small></summary>
                {inventory.installed.length ? <div className="installed-bundle-list">{inventory.installed.map((bundle) => { const matching = pluginInstances.filter((instance) => instance.model_bundle_id === bundle.manifest.id && instance.model_bundle_version === bundle.manifest.version); const instanceReady = matching.some((instance) => instance.status === "ready"); return <div key={bundleIdentity(bundle)}><header><span><strong>{bundle.manifest.display_name}</strong><small>{bundleIdentity(bundle)} · {bundle.manifest.variant}</small></span><Status status={bundle.manifest.fixture && instanceReady ? "Fixture" : instanceReady ? "Ready" : bundle.status.replaceAll("_", " ")} /></header><code>{bundle.bundle_sha256}</code>{bundle.manifest.fixture && <p>Offline contract test only · not selectable for Published Workflows</p>}{matching.map((instance) => <p key={instance.id}>{instance.execution_provider.toUpperCase()} · {instance.status.replaceAll("_", " ")} · profile revision {instance.model_profile_revision}</p>)}</div>; })}</div> : <div className={`bundle-empty-state${setupBlocker ? " warning" : ""}`}><strong>{setupBlocker ? t("Installed model cannot bind to this Plugin version") : t("No compatible model installed")}</strong><p>{setupBlocker ? "The existing Bundle is preserved. Update the immutable Plugin runtime before creating a Model Instance." : t("This Plugin cannot run until a verified model is installed.")}</p><button className="primary" onClick={() => openModelSetup(installation)}>{setupBlocker ? t("Review required update") : t("Install compatible model")}</button></div>}
              </details>
              <details className="registry-card-section">
                <summary><span>{t("Model Setup")}</span><small>{workflowReadyInstances.length ? t("Ready") : fixtureReadyInstances.length ? "Fixture only" : t("Action required")}</small></summary>
                {pluginInstances.map((instance) => <div className="model-instance-evidence" key={instance.id}><span><strong>{instance.model_bundle_id}</strong><small>{instance.contract_inspection.valid ? t("ONNX Contract verified") : instance.contract_inspection.errors.join(" · ")}</small></span><Status status={instance.status.replaceAll("_", " ")} />{instance.status !== "ready" && <button onClick={() => perform(`${instance.id}:smoke`, () => api.testModelInstance(instance.id), "Fixed Bundle smoke test completed.")} disabled={Boolean(busy)}>{busy === `${instance.id}:smoke` ? t("Testing…") : t("Run Smoke Test")}</button>}</div>)}
                {!pluginInstances.length && <p>{setupBlocker ? <>No verified Model Instance exists for this Plugin version. Review and install the required immutable runtime update first.</> : <>No verified Model Instance exists. Choose <strong>{t("Install compatible model")}</strong> to review an available Bundle.</>}</p>}
              </details>
              <details className="registry-card-section"><summary><span>{t("References")}</span><small>{installation.references.length}{" "}{t("protected")}</small></summary>{installation.references.length ? <ul className="plugin-reference-list">{installation.references.map((reference) => <li key={`${reference.kind}:${reference.location}`}><strong>{reference.kind.replaceAll("_", " ")}</strong><span>{reference.location}</span></li>)}</ul> : <p>No Published Workflow currently protects this Plugin version. Bundle references are tracked independently by exact digest.</p>}</details>
              {installation.weights.length > 0 && <details className="registry-card-section legacy-provisioning"><summary><span>Legacy manual provisioning</span><small>{t("Not recommended")}</small></summary><div className="legacy-model-warning"><strong>LegacyUnbundledModel</strong><p>These files predate Model Bundles. Their hashes are preserved, but they have no Bundle source, license document, Contract, or reproducible smoke-test identity and are not treated as trusted assets.</p>{installation.weights.map((weight) => <code key={`${weight.model_id}:${weight.component_id}`}>{weight.component_id} · {weight.original_filename} · {weight.checkpoint_sha256}</code>)}<p>Create a local Bundle only after supplying source, license, and a complete Model Contract. A failed conversion never removes these files.</p>{Array.from(new Set(installation.weights.map((weight) => weight.model_id))).map((modelId) => <button key={modelId} onClick={() => openLegacyBundleSetup(installation, modelId)}>{t("Create local model bundle")}</button>)}</div></details>}
            </article>;
          })}
        </div>
      </section>;
    })}
  </section>;
}

function ProviderRegistryPage({
  onOpenModels,
  onError,
}: {
  onOpenModels: () => void;
  onError: (value: string) => void;
}) {
  const [providers, setProviders] = useState<ProviderProfile[]>([]);
  const [presets, setPresets] = useState<ProviderPresetProfile[]>([]);
  const [models, setModels] = useState<RegistryModelProfile[]>([]);
  const [legacyImport, setLegacyImport] = useState<LegacyRegistryImportPreview>();
  const [presetId, setPresetId] = useState("dashscope");
  const [displayName, setDisplayName] = useState("Alibaba DashScope");
  const [baseUrl, setBaseUrl] = useState("https://dashscope.aliyuncs.com/compatible-mode/v1");
  const [adding, setAdding] = useState(false);
  const [busy, setBusy] = useState("");
  const [notice, setNotice] = useState("");
  const refresh = () =>
    Promise.all([
      api.providers(),
      api.providerPresets(),
      api.modelProfiles(),
      api.legacyRegistryImport(),
    ])
      .then(([providerResult, presetResult, modelResult, legacyResult]) => {
        setProviders(providerResult.providers);
        setPresets(presetResult.presets);
        setModels(modelResult.models);
        setLegacyImport(legacyResult.migration);
      })
      .catch((error: Error) => onError(error.message));
  useEffect(() => {
    void refresh();
  }, []);
  const choosePreset = (id: string) => {
    const preset = presets.find((candidate) => candidate.id === id);
    setPresetId(id);
    if (preset) {
      setDisplayName(preset.display_name);
      setBaseUrl(preset.base_url);
    }
  };
  const create = () => {
    setBusy("create");
    void api
      .createProvider({
        display_name: displayName,
        preset_id: presetId,
        adapter: "open_ai_compatible",
        base_url: baseUrl,
      })
      .then(() => {
        setAdding(false);
        setNotice("Provider saved. Add a credential, then run a passive connection check.");
        return refresh();
      })
      .catch((error: Error) => onError(error.message))
      .finally(() => setBusy(""));
  };
  const importLegacy = () => {
    if (
      !window.confirm(
        "Import the compatibility Provider, model and default-vision Project bindings? The credential value and historical Runs will not be moved or changed.",
      )
    ) return;
    setBusy("legacy-import");
    void api
      .applyLegacyRegistryImport()
      .then((result) => {
        setNotice(
          `Imported Provider and Model Profile. ${result.migration.bindings_created} Project binding${result.migration.bindings_created === 1 ? "" : t("s")} created; ${result.migration.bindings_preserved} existing choice${result.migration.bindings_preserved === 1 ? " was" : "s were"} preserved.`,
        );
        return refresh();
      })
      .catch((error: Error) => onError(error.message))
      .finally(() => setBusy(""));
  };
  return (
    <section className="registry-page">
      <div className="toolbar-panel">
        <div>
          <span className="eyebrow">{t("Reusable connections")}</span>
          <h2>{t("Providers")}</h2>
          <p>{t("Configure each API connection once. Credentials are write-only and never returned to this page.")}</p>
        </div>
        <button className="primary" onClick={() => setAdding((value) => !value)}>
          {adding ? t("Cancel") : t("Add provider")}
        </button>
      </div>
      {notice && <div className="positive-empty" role="status"><strong>{notice}</strong></div>}
      {adding && (
        <Panel title={t("New Provider")} eyebrow={t("Connection profile")}>
          <div className="form-grid">
            <label>{t("Preset")}<select value={presetId} onChange={(event) => choosePreset(event.target.value)}>{presets.map((preset) => <option key={preset.id} value={preset.id}>{preset.display_name}</option>)}</select></label>
            <label>{t("Display name")}<input value={displayName} onChange={(event) => setDisplayName(event.target.value)} /></label>
            <label>{t("Adapter")}<select value="open_ai_compatible" disabled><option value="open_ai_compatible">{t("OpenAI compatible")}</option></select></label>
            <label>{t("Base URL")}<input type="url" value={baseUrl} onChange={(event) => setBaseUrl(event.target.value)} /></label>
          </div>
          <div className="button-row"><button className="primary" disabled={busy === "create" || !displayName.trim() || !baseUrl.trim()} onClick={create}>{busy === "create" ? t("Saving…") : t("Save Provider")}</button></div>
        </Panel>
      )}
      {providers.length ? (
        <div className="registry-card-grid">
          {providers.map((provider) => (
            <ProviderRegistryCard
              key={provider.id}
              provider={provider}
              models={models.filter((model) => model.provider_id === provider.id)}
              onChanged={refresh}
              onOpenModels={onOpenModels}
              onError={onError}
            />
          ))}
        </div>
      ) : (
        <Empty title={t("No Providers configured")} detail={t("Connect an OpenAI-compatible API before asking AnnotAgent to build a Pipeline.")} />
      )}
      {legacyImport && !legacyImport.already_applied && (
        <details className="legacy-registry-import">
          <summary>
            <span><strong>{t("Legacy compatibility")}</strong><small>{t("Optional import available from older workspace settings")}</small></span>
            <b>{t("Optional")}</b>
          </summary>
          <div className="legacy-registry-import-body">
            <p>Import older compatibility settings into the Provider Registry. Current Providers continue to work if you leave this untouched.</p>
            <dl>
              <div><dt>{t("Provider")}</dt><dd>{legacyImport.provider_display_name}</dd></div>
              <div><dt>{t("Model Profile")}</dt><dd>{legacyImport.model_display_name}</dd></div>
              <div><dt>{t("Project bindings")}</dt><dd>{legacyImport.project_binding_count}</dd></div>
            </dl>
            <div className="legacy-registry-import-footer">
              <small>The credential remains a {legacyImport.credential_source?.replaceAll("_", " ") ?? "non-secret configuration"} reference. No secret or Run history is moved.</small>
              <button disabled={Boolean(busy)} onClick={importLegacy}>
                {busy === "legacy-import" ? t("Importing…") : t("Review and import")}
              </button>
            </div>
          </div>
        </details>
      )}
    </section>
  );
}

function ProviderRegistryCard({
  provider,
  models,
  onChanged,
  onOpenModels,
  onError,
}: {
  provider: ProviderProfile;
  models: RegistryModelProfile[];
  onChanged: () => Promise<void>;
  onOpenModels: () => void;
  onError: (value: string) => void;
}) {
  const [busy, setBusy] = useState("");
  const [credentialSource, setCredentialSource] = useState<"system_keyring" | "environment_variable" | "workspace_file" | "session_only">("workspace_file");
  const [secret, setSecret] = useState("");
  const [environmentVariable, setEnvironmentVariable] = useState("");
  const [selectedModel, setSelectedModel] = useState(models[0]?.id ?? "");
  const [editDisplayName, setEditDisplayName] = useState(provider.display_name);
  const [editBaseUrl, setEditBaseUrl] = useState(provider.base_url);
  const [discovery, setDiscovery] = useState<{ models: { remote_model_id: string }[]; warning: string }>();
  const [message, setMessage] = useState("");
  useEffect(() => {
    if (!models.some((model) => model.id === selectedModel)) setSelectedModel(models[0]?.id ?? "");
  }, [models, selectedModel]);
  const run = (name: string, action: () => Promise<unknown>, success: string) => {
    setBusy(name);
    setMessage("");
    void action()
      .then(() => onChanged())
      .then(() => setMessage(success))
      .catch((error: Error) => onError(error.message))
      .finally(() => setBusy(""));
  };
  const saveCredential = () => {
    if (
      credentialSource === "environment_variable" &&
      !isEnvironmentVariableName(environmentVariable)
    ) {
      onError(
        "Enter an environment variable name such as DASHSCOPE_API_KEY, not the API key itself. To paste a key directly, choose Local workspace file.",
      );
      return;
    }
    run(
      "credential",
      () => api.saveProviderCredential(provider.id, {
        source: credentialSource,
        ...(credentialSource === "environment_variable"
          ? { environment_variable: environmentVariable.trim() }
          : { secret }),
      }),
      "Credential reference saved. Run Check connection next.",
    );
  };
  const discover = () => {
    setBusy("discover");
    void api.discoverProviderModels(provider.id)
      .then((result) => {
        setDiscovery(result);
        setMessage(`Discovered ${result.models.length} model ID${result.models.length === 1 ? "" : t("s")}.`);
      })
      .catch((error: Error) => onError(error.message))
      .finally(() => setBusy(""));
  };
  const probe = () => {
    if (!selectedModel || !window.confirm("This sends a minimal generation request and may incur Provider charges. Continue?")) return;
    run("probe", () => api.activeProbe(provider.id, selectedModel), "Billable model probe succeeded and usage was recorded.");
  };
  const remove = () => {
    if (!window.confirm(`Delete ${provider.display_name}? Referenced Providers cannot be deleted.`)) return;
    run("delete", () => api.deleteProvider(provider.id), "Provider deleted.");
  };
  const credentialFieldId = `provider-${provider.id}-credential`;
  const credentialStorageHelp = credentialSource === "workspace_file"
    ? "Encrypted transport is unchanged. The key is written to this Git-ignored workspace with owner-only file permissions and remains available after a restart."
    : credentialSource === "environment_variable"
      ? "Use the name of a variable that already exists in the server environment. The key itself is never copied into AnnotAgent."
      : credentialSource === "session_only"
        ? "The key stays in this server process and is cleared whenever the server stops."
        : "The key is stored by the operating system credential service.";
  return (
    <article className="registry-provider-card">
      <header>
        <span className="registry-monogram" aria-hidden="true">{provider.display_name.slice(0, 2).toUpperCase()}</span>
        <span><strong>{provider.display_name}</strong><small>{provider.preset_id ?? "custom"} · {provider.adapter.replaceAll("_", " ")}</small></span>
        <Status status={provider.health.status.replaceAll("_", " ")} />
      </header>
      <dl className="registry-facts">
        <div><dt>{t("Endpoint")}</dt><dd title={provider.base_url}>{provider.endpoint_summary}</dd></div>
        <div><dt>{t("Credential")}</dt><dd>{provider.credential_configured ? `${provider.credential_source?.replaceAll("_", " ")} configured` : t("Missing")}</dd></div>
        <div><dt>{t("Models")}</dt><dd>{provider.model_count}</dd></div>
        <div><dt>{t("Last checked")}</dt><dd>{provider.health.checked_at ? new Date(provider.health.checked_at).toLocaleString(localeTag()) : t("Never")}</dd></div>
      </dl>
      {provider.health.safe_message && <p className="registry-safe-message">{provider.health.safe_message}</p>}
      <div className="registry-card-actions">
        <button disabled={Boolean(busy) || !provider.enabled} onClick={() => run("check", () => api.checkProvider(provider.id), "Connection check succeeded without a generation request.")}>{busy === "check" ? t("Checking…") : t("Check connection")}</button>
        <button disabled={Boolean(busy) || !provider.enabled} onClick={discover}>{busy === "discover" ? t("Discovering…") : t("Discover models")}</button>
        <button disabled={Boolean(busy)} onClick={() => run("toggle", () => api.updateProvider(provider.id, { enabled: !provider.enabled }), provider.enabled ? "Provider disabled." : "Provider enabled; run a connection check.")}>{provider.enabled ? t("Disable") : t("Enable")}</button>
      </div>
      <details className="registry-card-section">
        <summary>{t("Edit connection")}</summary>
        <div className="form-grid">
          <label>{t("Display name")}<input value={editDisplayName} onChange={(event) => setEditDisplayName(event.target.value)} /></label>
          <label>{t("Base URL")}<input type="url" value={editBaseUrl} onChange={(event) => setEditBaseUrl(event.target.value)} /></label>
        </div>
        <div className="button-row"><button disabled={Boolean(busy) || !editDisplayName.trim() || !editBaseUrl.trim()} onClick={() => run("edit", () => api.updateProvider(provider.id, { display_name: editDisplayName, base_url: editBaseUrl }), "Provider connection updated.")}>{busy === "edit" ? t("Saving…") : t("Save connection")}</button></div>
        <small>Changing an endpoint is blocked while Model Profiles reference this Provider; create a new Provider and rebind instead.</small>
      </details>
      <details className="registry-card-section">
        <summary>{provider.credential_configured ? t("Rotate or remove credential") : t("Add credential")}</summary>
        <>
          <div className="credential-editor">
            <div className="credential-field">
              <label htmlFor={`${credentialFieldId}-storage`}>{t("Storage")}</label>
              <select id={`${credentialFieldId}-storage`} aria-describedby={`${credentialFieldId}-storage-help`} value={credentialSource} onChange={(event) => setCredentialSource(event.target.value as typeof credentialSource)}>
                <option value="workspace_file">{t("Local workspace file")}</option>
                <option value="environment_variable">{t("Environment variable")}</option>
                <option value="session_only">{t("This server session only")}</option>
                <option value="system_keyring">{t("System credential store")}</option>
              </select>
              <p className="credential-field-help" id={`${credentialFieldId}-storage-help`}>{t(credentialStorageHelp)}</p>
            </div>
            {credentialSource === "environment_variable" ? <div className="credential-field">
              <label htmlFor={`${credentialFieldId}-variable`}>{t("Variable name")}</label>
              <input id={`${credentialFieldId}-variable`} aria-describedby={`${credentialFieldId}-variable-help`} value={environmentVariable} onChange={(event) => setEnvironmentVariable(event.target.value)} placeholder="DASHSCOPE_API_KEY" />
              <p className="credential-field-help" id={`${credentialFieldId}-variable-help`}>{t("Enter only a variable name, such as")}{" "}<code>DASHSCOPE_API_KEY</code>{t(". Do not paste the API key into this field.")}</p>
            </div> : <div className="credential-field">
              <label htmlFor={`${credentialFieldId}-secret`}>{t("API key")}</label>
              <input id={`${credentialFieldId}-secret`} aria-describedby={`${credentialFieldId}-secret-help`} type="password" autoComplete="new-password" value={secret} onChange={(event) => setSecret(event.target.value)} placeholder={provider.credential_configured ? t("Enter a replacement key") : t("Paste API key")} />
              <p className="credential-field-help" id={`${credentialFieldId}-secret-help`}>For security, an existing key is never shown here. Saving replaces the current key.</p>
            </div>}
          </div>
          <div className="credential-actions"><button className="primary" disabled={busy === "credential" || (credentialSource === "environment_variable" ? !environmentVariable.trim() : !secret.trim())} onClick={saveCredential}>{busy === "credential" ? t("Saving…") : provider.credential_configured ? t("Rotate credential") : t("Save credential")}</button><button disabled={!provider.credential_configured || Boolean(busy)} onClick={() => run("remove-credential", () => api.deleteProviderCredential(provider.id), "Credential reference removed.")}>{t("Remove credential")}</button>{provider.credential_source === "legacy_workspace_file" && <button disabled={Boolean(busy)} onClick={() => run("migrate", () => api.migrateProviderCredential(provider.id, false), "Credential copied to the system credential store. The legacy source was preserved.")}>{t("Migrate legacy credential")}</button>}</div>
        </>
      </details>
      <details className="registry-card-section">
        <summary>{t("Billable model test")}</summary>
        <p>This is separate from Check connection and sends a real generation request.</p>
        {models.length ? <div className="button-row"><select aria-label={t("Model Profile for active probe")} value={selectedModel} onChange={(event) => setSelectedModel(event.target.value)}>{models.map((model) => <option key={model.id} value={model.id}>{model.display_name} · r{model.revision}</option>)}</select><button disabled={Boolean(busy) || !provider.enabled} onClick={probe}>{busy === "probe" ? t("Testing…") : t("Run billable test")}</button></div> : <button onClick={onOpenModels}>{t("Add a Model Profile")}</button>}
      </details>
      {discovery && <details className="registry-discovery" open><summary>Discovered model IDs · {discovery.models.length}</summary><p>{discovery.warning}</p><div className="discovered-model-list">{discovery.models.slice(0, 100).map((model) => <code key={model.remote_model_id}>{model.remote_model_id}</code>)}</div><button onClick={onOpenModels}>{t("Create a verified Model Profile")}</button></details>}
      {message && <small className="registry-message" role="status">{message}</small>}
      <details className="advanced-settings"><summary>{t("Advanced and destructive actions")}</summary><div className="button-row"><button className="danger-button" disabled={Boolean(busy)} onClick={remove}>{t("Delete Provider")}</button></div><small>Deletion is blocked when Models, Drafts, published Workflows, Runs, or bindings reference this Provider.</small></details>
    </article>
  );
}

function ModelRegistryPage({
  onOpenProviders,
  onError,
}: {
  onOpenProviders: () => void;
  onError: (value: string) => void;
}) {
  const [providers, setProviders] = useState<ProviderProfile[]>([]);
  const [models, setModels] = useState<RegistryModelProfile[]>([]);
  const [globalDefaults, setGlobalDefaults] = useState<GlobalModelDefaults>({});
  const [defaultChoices, setDefaultChoices] = useState<{
    pipeline_builder: RegistryModelProfile[];
    vision_language: RegistryModelProfile[];
    text_generation: RegistryModelProfile[];
  }>({ pipeline_builder: [], vision_language: [], text_generation: [] });
  const [adding, setAdding] = useState(false);
  const [editingId, setEditingId] = useState("");
  const [busy, setBusy] = useState("");
  const [providerId, setProviderId] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [remoteModelId, setRemoteModelId] = useState("");
  const [modalities, setModalities] = useState<InputModality[]>(["text"]);
  const [capabilities, setCapabilities] = useState<ModelCapability[]>(["text_generation"]);
  const [toolCalls, setToolCalls] = useState(false);
  const [structuredOutput, setStructuredOutput] = useState(false);
  const [jsonSchema, setJsonSchema] = useState(false);
  const [inputPrice, setInputPrice] = useState("");
  const [outputPrice, setOutputPrice] = useState("");
  const [requestPrice, setRequestPrice] = useState("");
  const [providerFilter, setProviderFilter] = useState("all");
  const [capabilityFilter, setCapabilityFilter] = useState("all");
  const [statusFilter, setStatusFilter] = useState("all");
  const [modalityFilter, setModalityFilter] = useState("all");
  const [enabledFilter, setEnabledFilter] = useState("all");
  const [costFilter, setCostFilter] = useState("all");
  const refresh = () =>
    Promise.all([
      api.providers(),
      api.modelProfiles(),
      api.agentModelBindings(),
      api.compatibleModelProfiles({
        input_modalities: ["text"],
        capabilities: ["text_generation"],
        tool_calls: true,
        structured_output: true,
      }),
      api.compatibleModelProfiles({
        input_modalities: ["image"],
        capabilities: ["vision_language"],
      }),
      api.compatibleModelProfiles({
        input_modalities: ["text"],
        capabilities: ["text_generation"],
      }),
    ])
      .then(([
        providerResult,
        modelResult,
        defaults,
        pipelineBuilder,
        visionLanguage,
        textGeneration,
      ]) => {
        setProviders(providerResult.providers);
        setModels(modelResult.models);
        setGlobalDefaults(defaults);
        setDefaultChoices({
          pipeline_builder: pipelineBuilder.models,
          vision_language: visionLanguage.models,
          text_generation: textGeneration.models,
        });
        setProviderId((current) => current || providerResult.providers[0]?.id || "");
      })
      .catch((error: Error) => onError(error.message));
  useEffect(() => {
    void refresh();
  }, []);
  const toggle = <T,>(value: T, values: T[], update: (values: T[]) => void) =>
    update(values.includes(value) ? values.filter((candidate) => candidate !== value) : [...values, value]);
  const resetEditor = () => {
    setAdding(false);
    setEditingId("");
    setDisplayName("");
    setRemoteModelId("");
    setModalities(["text"]);
    setCapabilities(["text_generation"]);
    setToolCalls(false);
    setStructuredOutput(false);
    setJsonSchema(false);
    setInputPrice("");
    setOutputPrice("");
    setRequestPrice("");
  };
  const save = () => {
    setBusy("save");
    const value = {
      provider_id: providerId,
      display_name: displayName,
      remote_model_id: remoteModelId,
      input_modalities: modalities,
      task_capabilities: capabilities,
      protocol_features: {
        tool_calls: toolCalls,
        parallel_tool_calls: false,
        structured_output: structuredOutput,
        json_schema: jsonSchema,
        usage_reporting: true,
        streaming: false,
        reasoning_controls: false,
      },
      pricing: {
        currency: "USD",
        input_per_million_tokens: inputPrice || undefined,
        output_per_million_tokens: outputPrice || undefined,
        per_request: requestPrice || undefined,
        source: inputPrice || outputPrice || requestPrice ? "user_configured" as const : "unknown" as const,
      },
    };
    const operation = editingId
      ? api.updateModelProfile(editingId, value)
      : api.createModelProfile(value);
    void operation.then(() => {
      resetEditor();
      return refresh();
    }).catch((error: Error) => onError(error.message)).finally(() => setBusy(""));
  };
  const edit = (model: RegistryModelProfile) => {
    setEditingId(model.id);
    setAdding(true);
    setProviderId(model.provider_id);
    setDisplayName(model.display_name);
    setRemoteModelId(model.remote_model_id);
    setModalities(model.input_modalities);
    setCapabilities(model.task_capabilities);
    setToolCalls(model.protocol_features.tool_calls);
    setStructuredOutput(model.protocol_features.structured_output);
    setJsonSchema(model.protocol_features.json_schema);
    setInputPrice(model.pricing.input_per_million_tokens ?? "");
    setOutputPrice(model.pricing.output_per_million_tokens ?? "");
    setRequestPrice(model.pricing.per_request ?? "");
    window.scrollTo({ top: 0, behavior: "smooth" });
  };
  const filtered = models.filter((model) =>
    (providerFilter === "all" || model.provider_id === providerFilter) &&
    (capabilityFilter === "all" || model.task_capabilities.includes(capabilityFilter as ModelCapability)) &&
    (statusFilter === "all" || model.status === statusFilter) &&
    (modalityFilter === "all" || model.input_modalities.includes(modalityFilter as InputModality)) &&
    (enabledFilter === "all" || model.enabled === (enabledFilter === "enabled")) &&
    (costFilter === "all" || (costFilter === "configured") === (model.pricing.source !== "unknown")),
  );
  const change = (model: RegistryModelProfile, value: Partial<RegistryModelProfile>, success?: string) => {
    setBusy(model.id);
    void api.updateModelProfile(model.id, value).then(refresh).then(() => success && undefined).catch((error: Error) => onError(error.message)).finally(() => setBusy(""));
  };
  const probe = (model: RegistryModelProfile) => {
    if (!window.confirm("This sends a minimal generation request and may incur Provider charges. Continue?")) return;
    setBusy(model.id);
    void api.activeProbe(model.provider_id, model.id).then(refresh).catch((error: Error) => onError(error.message)).finally(() => setBusy(""));
  };
  const saveGlobalDefault = (
    key: keyof GlobalModelDefaults,
    modelProfileId: string,
  ) => {
    const next = {
      ...globalDefaults,
      [key]: modelProfileId || undefined,
    };
    setBusy("defaults");
    void api
      .saveAgentModelBindings(next)
      .then((saved) => setGlobalDefaults(saved))
      .catch((error: Error) => onError(error.message))
      .finally(() => setBusy(""));
  };
  const defaultOption = (model: RegistryModelProfile) => {
    const provider = providers.find(
      (candidate) => candidate.id === model.provider_id,
    );
    return `${model.display_name} via ${provider?.display_name ?? "Provider"}`;
  };
  return (
    <section className="registry-page">
      <div className="toolbar-panel"><div><span className="eyebrow">{t("Reusable capability contracts")}</span><h2>{t("Models")}</h2><p>Model Profiles bind a Provider model ID to explicit modalities, protocol features, capabilities, pricing, and an immutable revision.</p></div><button className="primary" disabled={!providers.length} onClick={() => adding ? resetEditor() : setAdding(true)}>{adding ? t("Cancel") : t("Add model")}</button></div>
      {!providers.length && <div className="guided-callout"><strong>{t("Provider setup required")}</strong><p>{t("Add a Provider before creating a Model Profile.")}</p><button onClick={onOpenProviders}>{t("Connect a Provider")}</button></div>}
      <Panel title={t("Default model choices")} eyebrow={t("Reusable workspace defaults")}>
        <p>
          Projects may override these choices. Published Workflows still freeze
          the final Model Profile revision.
        </p>
        <div className="registry-default-models">
          <label>{t("Default Pipeline Builder model")}<select
              aria-label={t("Default Pipeline Builder model")}
              value={globalDefaults.pipeline_builder ?? ""}
              disabled={busy === "defaults"}
              onChange={(event) =>
                saveGlobalDefault("pipeline_builder", event.target.value)
              }
            >
              <option value="">{t("No global default")}</option>
              {defaultChoices.pipeline_builder.map((model) => (
                <option key={model.id} value={model.id}>
                  {defaultOption(model)}
                </option>
              ))}
            </select>
            <small>Text · Structured Output · Tool Calls · Available</small>
          </label>
          <label>{t("Default Vision Language model")}<select
              aria-label={t("Default Vision Language model")}
              value={globalDefaults.vision_language ?? ""}
              disabled={busy === "defaults"}
              onChange={(event) =>
                saveGlobalDefault("vision_language", event.target.value)
              }
            >
              <option value="">{t("No global default")}</option>
              {defaultChoices.vision_language.map((model) => (
                <option key={model.id} value={model.id}>
                  {defaultOption(model)}
                </option>
              ))}
            </select>
            <small>Image · Vision Language · Available</small>
          </label>
          <label>{t("Default Text model")}<select
              aria-label={t("Default Text model")}
              value={globalDefaults.text_generation ?? ""}
              disabled={busy === "defaults"}
              onChange={(event) =>
                saveGlobalDefault("text_generation", event.target.value)
              }
            >
              <option value="">{t("No global default")}</option>
              {defaultChoices.text_generation.map((model) => (
                <option key={model.id} value={model.id}>
                  {defaultOption(model)}
                </option>
              ))}
            </select>
            <small>Text Generation · Available</small>
          </label>
        </div>
      </Panel>
      {adding && <Panel title={editingId ? t("Edit Model Profile") : t("New Model Profile")} eyebrow={t("Manual capability declaration")}>
        <div className="registry-model-editor">
          <section className="registry-form-section">
            <header><strong>{t("Model identity")}</strong><small>Choose the connection and enter the exact model identifier exposed by that Provider.</small></header>
            <div className="registry-model-identity">
              <label><span>{t("Provider")}</span><select value={providerId} onChange={(event) => setProviderId(event.target.value)}>{providers.map((provider) => <option value={provider.id} key={provider.id}>{provider.display_name}</option>)}</select></label>
              <label><span>{t("Display name")}</span><input value={displayName} onChange={(event) => setDisplayName(event.target.value)} /></label>
              <label><span>{t("Remote model ID")}</span><input value={remoteModelId} onChange={(event) => setRemoteModelId(event.target.value)} placeholder={t("Exact Provider model ID")} /></label>
            </div>
          </section>
          <div className="registry-option-sections">
            <fieldset className="registry-check-group"><legend>{t("Input modalities")}</legend>{(["text", "image", "video"] as InputModality[]).map((value) => <label className="checkbox-line" key={value}><input type="checkbox" checked={modalities.includes(value)} onChange={() => toggle(value, modalities, setModalities)} /><span>{value}</span></label>)}</fieldset>
            <fieldset className="registry-check-group"><legend>{t("Protocol features")}</legend><label className="checkbox-line"><input type="checkbox" checked={toolCalls} onChange={(event) => setToolCalls(event.target.checked)} /><span>{t("Tool calls")}</span></label><label className="checkbox-line"><input type="checkbox" checked={structuredOutput} onChange={(event) => setStructuredOutput(event.target.checked)} /><span>{t("Structured output")}</span></label><label className="checkbox-line"><input type="checkbox" checked={jsonSchema} onChange={(event) => setJsonSchema(event.target.checked)} /><span>{t("JSON Schema")}</span></label></fieldset>
            <fieldset className="registry-check-group registry-capability-group"><legend>{t("Task capabilities")}</legend>{REGISTRY_MODEL_CAPABILITIES.map((capability) => <label className="checkbox-line" key={capability.id}><input type="checkbox" checked={capabilities.includes(capability.id)} onChange={() => toggle(capability.id, capabilities, setCapabilities)} /><span>{t(capability.label)}</span></label>)}</fieldset>
          </div>
          <section className="registry-form-section">
            <header><strong>{t("Pricing")}</strong><small>Optional USD estimates used for Run previews and persisted usage summaries.</small></header>
            <div className="registry-pricing-grid">
              <label><span>{t("Input / 1M tokens")}</span><input aria-label={t("Input / 1M tokens (USD)")} inputMode="decimal" value={inputPrice} onChange={(event) => setInputPrice(event.target.value)} placeholder={t("Unknown")} /><small>USD</small></label>
              <label><span>{t("Output / 1M tokens")}</span><input aria-label={t("Output / 1M tokens (USD)")} inputMode="decimal" value={outputPrice} onChange={(event) => setOutputPrice(event.target.value)} placeholder={t("Unknown")} /><small>USD</small></label>
              <label><span>{t("Per request")}</span><input aria-label={t("Per request (USD)")} inputMode="decimal" value={requestPrice} onChange={(event) => setRequestPrice(event.target.value)} placeholder={t("Unknown")} /><small>USD</small></label>
            </div>
          </section>
          <footer className="registry-model-editor-footer">
            <p>Manual capabilities remain unverified until an explicit active probe succeeds.</p>
            <button className="primary" disabled={busy === "save" || !providerId || !displayName.trim() || !remoteModelId.trim() || !modalities.length || !capabilities.length} onClick={save}>{busy === "save" ? t("Saving…") : editingId ? t("Save as next revision if needed") : t("Save Model Profile")}</button>
          </footer>
        </div>
      </Panel>}
      <div className="registry-filter-bar"><label>{t("Provider")}<select value={providerFilter} onChange={(event) => setProviderFilter(event.target.value)}><option value="all">{t("All Providers")}</option>{providers.map((provider) => <option key={provider.id} value={provider.id}>{provider.display_name}</option>)}</select></label><label>{t("Capability")}<select value={capabilityFilter} onChange={(event) => setCapabilityFilter(event.target.value)}><option value="all">{t("All capabilities")}</option>{REGISTRY_MODEL_CAPABILITIES.map((capability) => <option key={capability.id} value={capability.id}>{t(capability.label)}</option>)}</select></label><label>{t("Health")}<select value={statusFilter} onChange={(event) => setStatusFilter(event.target.value)}><option value="all">{t("All statuses")}</option><option value="available">{t("Available")}</option><option value="unverified">{t("Unverified")}</option><option value="disabled">{t("Disabled")}</option><option value="unavailable">{t("Unavailable")}</option></select></label><label>{t("Input modality")}<select value={modalityFilter} onChange={(event) => setModalityFilter(event.target.value)}><option value="all">{t("All modalities")}</option><option value="text">{t("Text")}</option><option value="image">{t("Image")}</option><option value="video">{t("Video")}</option></select></label><label>{t("Enabled")}<select value={enabledFilter} onChange={(event) => setEnabledFilter(event.target.value)}><option value="all">{t("Enabled and disabled")}</option><option value="enabled">{t("Enabled")}</option><option value="disabled">{t("Disabled")}</option></select></label><label>{t("Pricing")}<select value={costFilter} onChange={(event) => setCostFilter(event.target.value)}><option value="all">{t("Any pricing status")}</option><option value="configured">{t("Configured")}</option><option value="unknown">{t("Unknown")}</option></select></label></div>
      {filtered.length ? <div className="registry-card-grid">{filtered.map((model) => {
        const provider = providers.find((candidate) => candidate.id === model.provider_id);
        return <article className="registry-model-card" key={model.id}><header><span><strong>{model.display_name}</strong><small>{provider?.display_name ?? t("Missing Provider")} · revision {model.revision}</small></span><Status status={model.status} /></header><code>{model.remote_model_id}</code><div className="tag-group">{model.input_modalities.map((value) => <span key={value}>{value}{" "}{t("input")}</span>)}{model.task_capabilities.map((value) => <span key={value}>{value.replaceAll("_", " ")}</span>)}</div><dl className="registry-facts"><div><dt>{t("Protocol")}</dt><dd>{[model.protocol_features.tool_calls && "tools", model.protocol_features.structured_output && "structured", model.protocol_features.json_schema && "JSON Schema"].filter(Boolean).join(" · ") || "basic"}</dd></div><div><dt>{t("Capability source")}</dt><dd>{model.capability_source.replaceAll("_", " ")}</dd></div><div><dt>{t("Pricing")}</dt><dd>{model.pricing.source === "unknown" ? t("Unknown") : `${model.pricing.currency} · ${model.pricing.source.replaceAll("_", " ")}`}</dd></div><div><dt>{t("Binding lock")}</dt><dd>{model.locked ? t("Locked") : t("Editable")}</dd></div></dl><ModelQualityContracts modelId={model.id} onError={onError} /><div className="registry-card-actions"><button disabled={busy === model.id} onClick={() => edit(model)}>{t("Edit")}</button><button disabled={busy === model.id || !provider?.enabled} onClick={() => probe(model)}>{busy === model.id ? t("Working…") : t("Run billable test")}</button><button disabled={busy === model.id} onClick={() => change(model, { enabled: !model.enabled })}>{model.enabled ? t("Disable") : t("Enable")}</button><button disabled={busy === model.id} onClick={() => change(model, { locked: !model.locked })}>{model.locked ? t("Unlock") : t("Lock")}</button></div><details className="advanced-settings"><summary>{t("Revision and destructive actions")}</summary><pre>{JSON.stringify({ limits: model.limits, generation_defaults: model.generation_defaults, pricing: model.pricing }, null, 2)}</pre><button className="danger-button" disabled={busy === model.id} onClick={() => { if (window.confirm(`Delete ${model.display_name}? Referenced profiles cannot be deleted.`)) { setBusy(model.id); void api.deleteModelProfile(model.id).then(refresh).catch((error: Error) => onError(error.message)).finally(() => setBusy("")); } }}>{t("Delete Model Profile")}</button></details></article>;
      })}</div> : <Empty title={t("No matching Model Profiles")} detail={t("Change the filters or add a manually declared model.")} />}
    </section>
  );
}

function ModelQualityContracts({
  modelId,
  onError,
}: {
  modelId: string;
  onError: (message: string) => void;
}) {
  const [contracts, setContracts] = useState<ModelCapabilityQualityContract[]>();
  const [loading, setLoading] = useState(false);
  const load = () => {
    if (contracts || loading) return;
    setLoading(true);
    void api.modelQualityContracts(modelId)
      .then((result) => setContracts(result.contracts))
      .catch((error: Error) => onError(error.message))
      .finally(() => setLoading(false));
  };
  return <details className="model-quality-contracts" onToggle={(event) => event.currentTarget.open && load()}>
    <summary><span><strong>{t("Score and box quality")}</strong><small>{t("Operation-scoped safety contract")}</small></span><b>{contracts?.length ?? t("View")}</b></summary>
    {loading && <p>{t("Loading quality contracts…")}</p>}
    {contracts?.map((contract) => <article key={`${contract.operation}:${contract.capability}`}>
      <header><strong>{contract.operation.replaceAll("_", " ")}</strong><small>{t("Model revision")}{" "}{contract.model_profile_revision}</small></header>
      <dl>
        <div><dt>{t("Geometry output")}</dt><dd>{geometrySemanticsLabel(contract.output_geometry)}</dd></div>
        <div><dt>{t("Score meaning")}</dt><dd>{scoreSemanticsLabel(contract.score_semantics)}</dd></div>
        <div><dt>{t("Automatic acceptance")}</dt><dd>{contract.auto_accept_eligibility === "never_from_score_alone" ? t("Never from score alone") : contract.auto_accept_eligibility.replaceAll("_", " ")}</dd></div>
        <div><dt>{t("Evidence")}</dt><dd>{contract.evidence_source.replaceAll("_", " ")}</dd></div>
      </dl>
      {contract.requires_geometry_verification && <p>Project calibration, measured refinement, or Human Review is required before box auto-acceptance.</p>}
    </article>)}
    {contracts && !contracts.length && <p>This Model Profile has no geometric operation contract.</p>}
  </details>;
}

function VisionWorkersRegistryPage({
  models,
  onOpenSettings,
  onError,
}: {
  models: ModelBinding[];
  onOpenSettings: () => void;
  onError: (value: string) => void;
}) {
  const [workers, setWorkers] = useState(models.filter((model) => model.scope === "workspace_worker"));
  const [testing, setTesting] = useState("");
  const [results, setResults] = useState<Record<string, DetectionWorkerTestResult>>({});
  useEffect(() => {
    void api.models().then((result) => setWorkers(result.models.filter((model) => model.scope === "workspace_worker"))).catch((error: Error) => onError(error.message));
  }, []);
  const test = (worker: ModelBinding) => {
    setTesting(worker.id);
    void api.testModel(worker.id).then((result) => setResults((current) => ({ ...current, [worker.id]: result }))).catch((error: Error) => onError(error.message)).finally(() => setTesting(""));
  };
  return <section className="registry-page"><div className="toolbar-panel"><div><span className="eyebrow">Read-only migration compatibility</span><h2>Legacy HTTP models</h2><p>Existing versioned HTTP Vision Protocol bindings remain inspectable. New native expert models should be installed as isolated Rust packages.</p></div><button onClick={onOpenSettings}>Open compatibility settings</button></div>{workers.length ? <div className="registry-card-grid">{workers.map((worker) => <article className="registry-model-card" key={worker.id}><header><span><strong>{worker.id}</strong><small>{worker.model} · {worker.role}</small></span><Status status={worker.health_status} /></header><code>{worker.endpoint ?? t("No endpoint")}</code><div className="tag-group">{worker.capabilities?.map((capability) => <span key={capability}>{t(capability.replaceAll("_", " "))}</span>)}</div><div className="worker-contract-summary">{worker.score_semantics && <small>{t("Confidence")}{" "}{worker.score_semantics.replaceAll("_", " ")}</small>}{worker.label_space?.length ? <small>Label space · {worker.label_space.join(" · ")}</small> : null}{worker.checkpoint_sha256 && <small>Checkpoint · {worker.checkpoint_sha256.slice(0, 12)}…</small>}{worker.architecture && <small>Architecture · {worker.architecture}</small>}{worker.cost_per_request !== undefined && <small>Estimated cost · ${worker.cost_per_request} / request</small>}</div><p>{worker.health_detail}</p><button disabled={testing === worker.id} onClick={() => test(worker)}>{testing === worker.id ? t("Discovering…") : t("Refresh discovery")}</button>{results[worker.id] && <div className="registry-safe-message" role="status"><strong>{results[worker.id].passed ? t("Discovery passed") : `Stopped at ${results[worker.id].failed_stage ?? "discovery"}`}</strong><span>{results[worker.id].capabilities?.capabilities.join(" · ") || results[worker.id].error}</span><span>{results[worker.id].evidence?.detail}</span></div>}</article>)}</div> : <Empty title="No legacy HTTP models configured" detail="Install a native Rust Expert Model Plugin for new Workflows." />}</section>;
}

function RegistryUsagePage({ onError }: { onError: (value: string) => void }) {
  const [models, setModels] = useState<RegistryModelProfile[]>([]);
  const [usage, setUsage] = useState<ProviderProbeUsage[]>([]);
  useEffect(() => {
    void api.modelProfiles().then(async (result) => {
      setModels(result.models);
      const records = await Promise.all(result.models.map((model) => api.modelProfileUsage(model.id)));
      setUsage(records.flatMap((record) => record.active_probes).sort((left, right) => right.created_at.localeCompare(left.created_at)));
    }).catch((error: Error) => onError(error.message));
  }, []);
  const totals = usage.reduce((current, record) => ({ tokens: current.tokens + (record.total_tokens ?? 0), cost: current.cost + Number(record.cost || 0) }), { tokens: 0, cost: 0 });
  return <section className="registry-page"><div className="toolbar-panel"><div><span className="eyebrow">Recorded Registry operations</span><h2>{t("Usage")}</h2><p>Active model probes are listed separately from normal Run usage because each probe requires explicit billable confirmation.</p></div></div><div className="metrics-grid"><Metric label="Active probes" value={usage.length} detail="explicitly confirmed" /><Metric label="Probe tokens" value={totals.tokens.toLocaleString(localeTag())} detail="reported by Providers" /><Metric label="Estimated probe cost" value={`$${totals.cost.toFixed(6)}`} detail="configured pricing snapshots" /></div>{usage.length ? <div className="registry-usage-list">{usage.map((record) => { const model = models.find((candidate) => candidate.id === record.model_profile_id); return <article key={record.id}><span><strong>{model?.display_name ?? record.model_profile_id}</strong><small>{new Date(record.created_at).toLocaleString(localeTag())} · revision {record.model_profile_revision}</small></span><span>{record.total_tokens ?? t("Unknown")}{" "}{t("tokens")}</span><span>{record.currency} {record.cost}</span><Status status={record.succeeded ? "succeeded" : "failed"} /></article>; })}</div> : <Empty title="No active probe usage" detail="Passive connection checks do not generate usage records." />}</section>;
}

function WorkflowDetail({
  project,
  workflow,
}: {
  project: ProjectSummary;
  workflow: WorkflowVersion;
}) {
  return (
    <Panel
      title={`${workflow.name} · v${workflow.version}`}
      eyebrow={`${project.name} · ${workflow.status}`}
    >
      <div className="workflow-facts">
        <Fact label={t("Validation")} value={workflow.validation_status} />
        <Fact label={t("Default")} value={workflow.is_default ? "Yes" : "No"} />
        <Fact label={t("Source")} value={workflow.source} />
        <Fact
          label={t("Enabled Skills")}
          value={
            project.enabled_skills
              .map((skill) => `${skill.id}@${skill.version}`)
              .join(", ") || "None"
          }
        />
      </div>
      <div className="workflow-nodes">
        {workflow.nodes.map((node, index) => (
          <article key={node.id}>
            <span className="node-index">
              {String(index + 1).padStart(2, "0")}
            </span>
            <div>
              <span className="eyebrow">{node.node_type}</span>
              <h3>{node.id}</h3>
              <small>Depends on: {node.depends_on.join(", ") || "start"}</small>
              <div className="node-meta">
                <span>Model · {node.model_binding || "unbound"}</span>
                <span>Fallback · {node.fallback || t("none")}</span>
                <span>
                  Human review ·{" "}
                  {node.human_review_gate ? "gate enabled" : "not configured"}
                </span>
              </div>
              <TagGroup title={t("Validators")} values={node.validators} />
              <TagGroup title={t("Refiners")} values={node.refiners} />
            </div>
          </article>
        ))}
      </div>
    </Panel>
  );
}

function ModelsPage({
  models,
  onConfigure,
  onError,
}: {
  models: ModelBinding[];
  onConfigure: () => void;
  onError: (value: string) => void;
}) {
  const [catalogModels, setCatalogModels] = useState(models);
  const [testingModel, setTestingModel] = useState<string>();
  const [testResults, setTestResults] = useState<Record<string, DetectionWorkerTestResult>>({});
  useEffect(() => {
    void api.models().then((value) => setCatalogModels(value.models)).catch((error: Error) => onError(error.message));
  }, []);
  const testWorker = (modelId: string) => {
    setTestingModel(modelId);
    void api.testModel(modelId)
      .then((result) => setTestResults((current) => ({ ...current, [modelId]: result })))
      .catch((error: Error) => onError(error.message))
      .finally(() => setTestingModel(undefined));
  };
  const modelGroups = [
    { id: "ready", title: "Ready", detail: "Runnable now" },
    { id: "configured_unavailable", title: "Configured but unavailable", detail: "Verify credentials or connection" },
    { id: "labs", title: "Experimental / Labs", detail: "Requires an explicitly installed local Worker and weights" },
    { id: "disabled", title: "Disabled", detail: "Excluded from recommendations" },
  ] as const;
  return (
    <section className="page-stack">
      <div className="toolbar-panel">
        <div>
          <span className="eyebrow">Provider catalog and bindings</span>
          <h2>{t("Models")}</h2>
          <p>
            Credentials stay in the native system credential store; Workflows refer to
            stable binding IDs.
          </p>
        </div>
        <button className="primary" onClick={onConfigure}>
          Configure provider
        </button>
      </div>
      <div className="split-grid">
        <Panel title="Configured bindings" eyebrow="Workspace default">
          {catalogModels.length ? (
            <div className="binding-list">
              {modelGroups.map((group) => {
                const bindings = catalogModels.filter((binding) => binding.availability_group === group.id);
                if (!bindings.length) return null;
                return <section className="model-availability-group" key={group.id} aria-labelledby={`model-group-${group.id}`}>
                  <header><div><strong id={`model-group-${group.id}`}>{t(group.title)}</strong><small>{group.detail}</small></div><b>{bindings.length}</b></header>
                  {bindings.map((binding) => (
                <article key={binding.id}>
                  <span className="catalog-monogram">AI</span>
                  <div>
                    <strong>{binding.id}</strong>
                    <small>
                      {binding.role} · {binding.scope.replaceAll("_", " ")}
                    </small>
                    <code>
                      {binding.provider} / {binding.model}
                    </code>
                    <small
                      title={binding.health_detail}
                    >{`Health · ${binding.health_status}`}</small>
                    {binding.capabilities?.length ? <small>Configured contract · {binding.capabilities.join(" · ")}</small> : null}
                    {binding.score_semantics && <small>Score · {binding.score_semantics.replaceAll("_", " ")}</small>}
                    {binding.architecture && <small>Architecture · {binding.architecture}</small>}
                    {binding.model_version && <small>Version · {binding.model_version}</small>}
                    {binding.checkpoint_sha256 && <small title={binding.checkpoint_sha256}>Checkpoint · {binding.checkpoint_sha256.slice(0, 12)}…</small>}
                    {binding.label_space?.length ? <small>Label space · {binding.label_space.join(" · ")}</small> : null}
                    {binding.endpoint && <small>Endpoint · {binding.endpoint}</small>}
                    {binding.cost_per_request !== undefined && <small>Estimated cost · ${binding.cost_per_request} / request</small>}
                    {binding.license_summary && <small>License · {binding.license_summary}</small>}
                    {binding.scope === "workspace_worker" && <div className="worker-actions">
                      <button
                        onClick={() => testWorker(binding.id)}
                        disabled={testingModel === binding.id}
                        title="Read health, capabilities, models, and contracts from the Worker"
                      >
                        {testingModel === binding.id ? t("Discovering…") : t("Refresh discovery")}
                      </button>
                    </div>}
                    {testResults[binding.id] && <div className="worker-discovery" role="status">
                      <strong>{testResults[binding.id].passed ? t("Discovery passed") : `Stopped at ${testResults[binding.id].failed_stage ?? "discovery"}`}</strong>
                      <small>{testResults[binding.id].capabilities?.capabilities.join(" · ") || testResults[binding.id].error}</small>
                      <small>{testResults[binding.id].evidence?.detail}</small>
                    </div>}
                    {binding.scope === "workspace_worker" && <details className="worker-setup-instructions">
                      <summary>View setup instructions</summary>
                      <p>Start a protocol v1 HTTP Vision Worker at this endpoint, then enable and test it. AnnotAgent never downloads model weights during Server startup.</p>
                      <code>{binding.endpoint ?? "Configure a Worker URL in Settings"}</code>
                    </details>}
                  </div>
                </article>
                  ))}
                </section>;
              })}
            </div>
          ) : (
            <Empty
              title="No model bindings"
              detail="Choose a provider in Settings."
            />
          )}
        </Panel>
        <Panel title="Provider catalog" eyebrow="Curated compatible options">
          <div className="catalog-list">
            {PROVIDER_PRESETS.map(
              (preset) => (
                <article key={preset.id}>
                  <span className="catalog-monogram">
                    {preset.shortLabel.slice(0, 2).toUpperCase()}
                  </span>
                  <span>
                    <strong>{preset.label}</strong>
                    <small>
                      {preset.models.length
                        ? `${preset.models.length} curated models`
                        : t("Custom model IDs")}
                    </small>
                  </span>
                </article>
              ),
            )}
          </div>
        </Panel>
      </div>
    </section>
  );
}

type ManagementDialogState = {
  request: ManagementRequest;
  preview: ManagementPreview;
  replacementOptions?: { label: string; value: { workflow_id: string; version: number } }[];
};

function managementRequest(
  projectId: string,
  objects: ManagementObjectRef[],
  action: ManagementAction,
): ManagementRequest {
  return {
    project_id: projectId,
    objects,
    action,
    idempotency_key: crypto.randomUUID(),
  };
}

function PipelineManagementPanel({
  project,
  currentDraftId,
  currentDraftDirty,
  onOpenDraft,
  onChanged,
  onCurrentDraftRemoved,
  onOpenTrash,
  onError,
}: {
  project: ProjectSummary;
  currentDraftId?: string;
  currentDraftDirty: boolean;
  onOpenDraft: (draftId: string) => void;
  onChanged: () => Promise<void>;
  onCurrentDraftRemoved: () => void;
  onOpenTrash: () => void;
  onError: (value: string) => void;
}) {
  const [pipelines, setPipelines] = useState<PipelineLifecycleSummary[]>([]);
  const [showArchived, setShowArchived] = useState(false);
  const [selected, setSelected] = useState<Map<string, ManagementObjectRef>>(new Map());
  const [dialog, setDialog] = useState<ManagementDialogState>();
  const [receipt, setReceipt] = useState<ManagementReceipt>();
  const [renaming, setRenaming] = useState<PipelineLifecycleSummary>();
  const [displayName, setDisplayName] = useState("");
  const [busy, setBusy] = useState(false);
  const load = () => api.pipelineLifecycle(project.id, true, false).then(({ pipelines: items }) => {
    setPipelines(items);
    setSelected(new Map());
  });
  useEffect(() => {
    void load().catch((error: Error) => onError(error.message));
  }, [project.id]);
  const visible = pipelines.filter((pipeline) => showArchived || !pipeline.archived_at);
  const availableVersions = pipelines.flatMap((pipeline) => pipeline.versions
    .filter((version) => !version.archived_at && !version.deleted_at)
    .map((version) => ({
      label: `${pipeline.display_name} · v${version.object.version}`,
      value: { workflow_id: version.object.id, version: version.object.version! },
    })));
  const key = (object: ManagementObjectRef) => `${object.kind}:${object.id}:${object.version ?? 0}`;
  const selectableObjects = visible.flatMap((pipeline): ManagementObjectRef[] => [
    { kind: "pipeline", id: pipeline.workflow_id, expected_revision: pipeline.lifecycle_revision },
    ...pipeline.drafts.map((draft) => draft.object),
    ...pipeline.versions.map((version) => version.object),
  ]);
  const allSelected = selectableObjects.length > 0 && selectableObjects.every((object) => selected.has(key(object)));
  const selectAll = (checked: boolean) => setSelected(new Map(
    checked ? selectableObjects.map((object) => [key(object), object]) : [],
  ));
  const toggle = (object: ManagementObjectRef, checked: boolean) => setSelected((current) => {
    const next = new Map(current);
    if (checked) next.set(key(object), object);
    else next.delete(key(object));
    return next;
  });
  const blocksDirtyDraft = (objects: ManagementObjectRef[]) => currentDraftDirty && objects.some((object) =>
    (object.kind === "workflow_draft" && object.id === currentDraftId)
      || (object.kind === "pipeline" && object.id === currentDraftId));
  const openAction = (action: ManagementAction, objects: ManagementObjectRef[]) => {
    const coveredChildren = new Set(pipelines
      .filter((pipeline) => objects.some((object) => object.kind === "pipeline" && object.id === pipeline.workflow_id))
      .flatMap((pipeline) => [...pipeline.drafts, ...pipeline.versions].map((child) => key(child.object))));
    objects = objects.filter((object) => !coveredChildren.has(key(object)));
    if (!objects.length) return;
    if (blocksDirtyDraft(objects)) {
      onError("Save or discard the current Draft changes before removing its Draft or Pipeline.");
      return;
    }
    const request = managementRequest(project.id, objects, action);
    void api.previewManagement(project.id, request)
      .then((preview) => setDialog({ request, preview, replacementOptions: availableVersions.filter(({ value }) =>
        !objects.some((object) => object.kind === "workflow_version" && object.id === value.workflow_id && object.version === value.version)
          && !objects.some((object) => object.kind === "pipeline" && object.id === value.workflow_id)) }))
      .catch((error: Error) => onError(error.message));
  };
  const openRename = () => {
    if (!renaming || !displayName.trim()) return;
    const request = {
      ...managementRequest(project.id, [{
        kind: "pipeline" as const,
        id: renaming.workflow_id,
        expected_revision: renaming.lifecycle_revision,
      }], "rename"),
      display_name: displayName.trim(),
    };
    void api.previewManagement(project.id, request)
      .then((preview) => {
        setRenaming(undefined);
        setDialog({ request, preview });
      })
      .catch((error: Error) => onError(error.message));
  };
  const cloneVersion = (workflowId: string, version: number) => {
    setBusy(true);
    void api.cloneWorkflowVersion(workflowId, version)
      .then((draft) => Promise.all([onChanged(), load()]).then(() => onOpenDraft(draft.id)))
      .catch((error: Error) => onError(error.message))
      .finally(() => setBusy(false));
  };
  const complete = (nextReceipt: ManagementReceipt, preview: ManagementPreview) => {
    setDialog(undefined);
    setReceipt(nextReceipt);
    const removedCurrent = nextReceipt.action === "move_to_trash" && preview.objects.some((object) =>
      (object.kind === "workflow_draft" && object.id === currentDraftId)
        || (object.kind === "pipeline" && object.id === currentDraftId));
    if (removedCurrent) onCurrentDraftRemoved();
    void Promise.all([load(), onChanged()]).catch((error: Error) => onError(error.message));
  };
  return <section className="panel pipeline-management" aria-labelledby="pipeline-management-title">
    <header className="pipeline-management-header">
      <div><span className="eyebrow">{t("Saved work")}</span><h2 id="pipeline-management-title">{t("Pipelines and Versions")}</h2><p>{t("Manage display aliases and lifecycle state without changing immutable published content.")}</p></div>
      <div className="button-row"><label className="checkbox-row"><input type="checkbox" checked={showArchived} onChange={(event) => { setShowArchived(event.target.checked); setSelected(new Map()); }} />{t("Show archived")}</label><button onClick={onOpenTrash}>{t("Trash")}</button></div>
    </header>
    {receipt && <div className="operation-receipt" role="status"><span><strong>{receipt.action.replaceAll("_", " ")}{" "}{t("completed")}</strong><small>{t("Operation")}{" "}{receipt.operation_id.slice(0, 8)}{" "}{t("is persisted.")}</small></span><button onClick={() => setReceipt(undefined)}>{t("Dismiss")}</button></div>}
    <div className="pipeline-selection-toolbar">
      <label className="checkbox-row"><input type="checkbox" aria-label={t("Select all Pipelines and Versions")} checked={allSelected} ref={(input) => { if (input) input.indeterminate = selected.size > 0 && !allSelected; }} disabled={selectableObjects.length === 0} onChange={(event) => selectAll(event.target.checked)} />{t("Select all")}</label>
      <strong>{t("{count} items selected", { count: selected.size })}</strong>
      {selected.size > 0 && <><button className="danger-button" onClick={() => openAction("move_to_trash", [...selected.values()])}>{t("Move selected to Trash…")}</button><button onClick={() => selectAll(false)}>{t("Clear selection")}</button></>}
    </div>
    <div className="pipeline-management-list">
      {visible.map((pipeline) => {
        const pipelineObject: ManagementObjectRef = { kind: "pipeline", id: pipeline.workflow_id, expected_revision: pipeline.lifecycle_revision };
        return <details key={pipeline.workflow_id} className="pipeline-management-item">
          <summary>
            <input type="checkbox" aria-label={t("Select Pipeline {name}", { name: pipeline.display_name })} checked={selected.has(key(pipelineObject))} onClick={(event) => event.stopPropagation()} onChange={(event) => toggle(pipelineObject, event.target.checked)} />
            <span><strong>{pipeline.display_name}</strong><small>{pipeline.drafts.length}{" "}{t("Draft")}{pipeline.drafts.length === 1 ? "" : t("s")} · {pipeline.versions.length}{" "}{t("Published Version")}{pipeline.versions.length === 1 ? "" : t("s")}</small></span>
            {pipeline.default_version && <span className="status status-auto-accepted">{t("Default v")}{pipeline.default_version}</span>}
            {pipeline.archived_at && <span className="status status-archived">{t("Archived")}</span>}
            <span className="row-arrow" aria-hidden="true">⌄</span>
          </summary>
          <div className="pipeline-management-body">
            <div className="pipeline-parent-actions">
              <code>{pipeline.workflow_id}</code>
              <div className="button-row"><button onClick={() => { setRenaming(pipeline); setDisplayName(pipeline.display_name); }}>{t("Rename alias")}</button><button onClick={() => openAction(pipeline.archived_at ? "unarchive" : "archive", [pipelineObject])}>{pipeline.archived_at ? t("Unarchive") : t("Archive")}</button><button className="danger-button" onClick={() => openAction("move_to_trash", [pipelineObject])}>{t("Delete Pipeline…")}</button></div>
            </div>
            <section><header><strong>{t("Drafts")}</strong><span>{t("Editable working copies")}</span></header>{pipeline.drafts.map((draft) => <article key={key(draft.object)}><input type="checkbox" aria-label={t("Select Draft {name}", { name: draft.display_name })} checked={selected.has(key(draft.object))} onChange={(event) => toggle(draft.object, event.target.checked)} /><span><strong>{draft.display_name}</strong><small>{draft.archived_at ? t("Archived") : t("Editing")} · {draft.content_hash.slice(0, 10) || "not hashed"}</small></span><div className="button-row"><button onClick={() => onOpenDraft(draft.object.id)}>{t("Open")}</button><button onClick={() => openAction(draft.archived_at ? "unarchive" : "archive", [draft.object])}>{draft.archived_at ? t("Unarchive") : t("Archive")}</button><button className="danger-button" onClick={() => openAction("move_to_trash", [draft.object])}>{t("Delete Draft…")}</button></div></article>)}{pipeline.drafts.length === 0 && <small>{t("No visible Drafts.")}</small>}</section>
            <section><header><strong>{t("Published Versions")}</strong><span>{t("Immutable execution definitions")}</span></header>{pipeline.versions.map((version) => <article key={key(version.object)}><input type="checkbox" aria-label={t("Select Published Version {name} v{version}", { name: pipeline.display_name, version: version.object.version! })} checked={selected.has(key(version.object))} onChange={(event) => toggle(version.object, event.target.checked)} /><span><strong>{t("Version")}{" "}{version.object.version}</strong><small title={version.content_hash}>{t("Hash")}{" "}{version.content_hash.slice(0, 12)} · {version.historical_run_references}{" "}{t("historical Run reference")}{version.historical_run_references === 1 ? "" : t("s")}</small></span>{version.is_default && <span className="status status-auto-accepted">{t("Default")}</span>}<div className="button-row"><button disabled={busy} onClick={() => cloneVersion(version.object.id, version.object.version!)}>{t("Copy as Draft")}</button>{version.is_default ? <button onClick={() => openAction("clear_default", [version.object])}>{t("Clear default")}</button> : <button onClick={() => openAction("set_default", [version.object])}>{t("Set default")}</button>}<button onClick={() => openAction(version.archived_at ? "unarchive" : "archive", [version.object])}>{version.archived_at ? t("Unarchive") : t("Archive")}</button><button className="danger-button" onClick={() => openAction("move_to_trash", [version.object])}>{t("Delete Version…")}</button></div></article>)}{pipeline.versions.length === 0 && <small>{t("No visible Published Versions.")}</small>}</section>
          </div>
        </details>;
      })}
      {visible.length === 0 && <Empty title={t("No saved Pipelines")} detail={showArchived ? t("Create a Draft to begin.") : t("Show archived Pipelines or create a new Draft.")} />}
    </div>
    {renaming && <div className="modal-backdrop" role="presentation" onMouseDown={(event) => event.target === event.currentTarget && setRenaming(undefined)}><section className="modal pipeline-rename-dialog" role="dialog" aria-modal="true" aria-labelledby="pipeline-rename-title"><header><div><span className="eyebrow">{t("Display alias only")}</span><h2 id="pipeline-rename-title">{t("Rename Pipeline")}</h2></div><button onClick={() => setRenaming(undefined)}>{t("Close")}</button></header><label>{t("Pipeline display name")}<input autoFocus maxLength={160} value={displayName} onChange={(event) => setDisplayName(event.target.value)} /></label><p>{t("Published nodes, bindings, prompts, Versions and content hashes remain unchanged.")}</p><footer className="button-row"><button onClick={() => setRenaming(undefined)}>{t("Cancel")}</button><button className="primary" disabled={!displayName.trim()} onClick={openRename}>{t("Preview rename")}</button></footer></section></div>}
    {dialog && <ManagementImpactDialog state={dialog} onClose={() => setDialog(undefined)} onComplete={complete} onError={onError} />}
  </section>;
}

function ManagementImpactDialog({
  state,
  onClose,
  onComplete,
  onError,
}: {
  state: ManagementDialogState;
  onClose: () => void;
  onComplete: (receipt: ManagementReceipt, preview: ManagementPreview) => void;
  onError: (value: string) => void;
}) {
  const [request, setRequest] = useState(state.request);
  const [preview, setPreview] = useState(state.preview);
  const [busy, setBusy] = useState(false);
  const [purgeConfirmation, setPurgeConfirmation] = useState("");
  const needsDefaultChoice = preview.blockers.some(
    (blocker) => blocker.code === "default_replacement_required",
  );
  const refreshPreview = (next: ManagementRequest) => {
    setBusy(true);
    void api.previewManagement(next.project_id, next)
      .then((value) => {
        setRequest(next);
        setPreview(value);
      })
      .catch((error: Error) => onError(error.message))
      .finally(() => setBusy(false));
  };
  const execute = () => {
    setBusy(true);
    void api.executeManagement(request.project_id, {
      ...request,
      confirmation_token: preview.confirmation_token,
    })
      .then((receipt) => onComplete(receipt, preview))
      .catch((error: Error) => onError(error.message))
      .finally(() => setBusy(false));
  };
  const titleKey = request.action === "purge"
    ? "Permanently clean up {count} items?"
    : request.action === "restore" ? "Restore {count} items?"
    : request.action === "archive" ? "Archive {count} items?"
    : request.action === "unarchive" ? "Unarchive {count} items?"
    : request.action === "rename" ? "Rename Pipeline?"
    : request.action === "set_default" ? "Use this Published Version by default?"
    : request.action === "clear_default" ? "Clear the default Automation?"
    : request.action === "cancel_and_delete" ? "Cancel and move {count} items to Trash?"
    : "Move {count} items to Trash?";
  const title = t(preview.objects.length === 1 ? titleKey.replace("{count} items", "{count} item") : titleKey, { count: preview.objects.length });
  const confirmLabel = request.action === "purge"
    ? "Permanently clean up"
    : request.action === "move_to_trash"
      ? "Move to Trash"
      : request.action === "set_default"
        ? "Set default"
      : request.action === "clear_default"
          ? "Clear default"
          : request.action === "cancel_and_delete"
            ? "Cancel and move to Trash"
          : request.action[0].toUpperCase() + request.action.slice(1).replaceAll("_", " ");
  return <div className="modal-backdrop" role="presentation" onMouseDown={(event) => event.target === event.currentTarget && onClose()}>
    <section className="modal management-dialog" role="dialog" aria-modal="true" aria-labelledby="management-dialog-title">
      <header>
        <div><span className="eyebrow">{t("Project lifecycle management")}</span><h2 id="management-dialog-title">{title}</h2></div>
        <button aria-label={t("Close management dialog")} onClick={onClose}>{t("Close")}</button>
      </header>
      <p className="management-summary">{preview.summary}</p>
      <dl className="management-impact-grid">
        <div><dt>{t("Selected")}</dt><dd>{preview.impact.top_level_objects}</dd></div>
        <div><dt>{t("Owned child Runs")}</dt><dd>{preview.impact.child_runs}</dd></div>
        <div><dt>{t("Reviews hidden")}</dt><dd>{preview.impact.unresolved_reviews_hidden}</dd></div>
        <div><dt>{t("Annotations retained")}</dt><dd>{preview.impact.confirmed_annotations_retained}</dd></div>
        <div><dt>{t("Historical references")}</dt><dd>{preview.impact.historical_run_references}</dd></div>
        <div><dt>{t("Debug rows")}</dt><dd>{preview.impact.debug_rows}</dd></div>
      </dl>
      <aside className="management-storage-note"><strong>{request.action === "purge" ? t("Cleanup estimate") : t("Storage is unchanged")}</strong><span>{preview.impact.estimate_note}</span></aside>
      {needsDefaultChoice && <fieldset className="management-default-choice">
        <legend>{t("Current Project default")}</legend>
        <p>{t("This item is the default Automation. Choose a replacement or explicitly leave this Project without a default.")}</p>
        <label>{t("Replacement Version")}<select value={request.replacement_default_version ? `${request.replacement_default_version.workflow_id}@${request.replacement_default_version.version}` : ""} onChange={(event) => {
          const option = state.replacementOptions?.find(({ value }) => `${value.workflow_id}@${value.version}` === event.target.value);
          refreshPreview({ ...request, replacement_default_version: option?.value, clear_default: false, confirmation_token: undefined });
        }}><option value="">{t("Choose another Version…")}</option>{state.replacementOptions?.map((option) => <option key={`${option.value.workflow_id}@${option.value.version}`} value={`${option.value.workflow_id}@${option.value.version}`}>{option.label}</option>)}</select></label>
        <button onClick={() => refreshPreview({ ...request, replacement_default_version: undefined, clear_default: true, confirmation_token: undefined })}>{t("Clear default Automation")}</button>
      </fieldset>}
      {preview.blockers.length > 0 && <div className="management-blockers" role="alert">
        {preview.blockers.map((blocker) => <article key={`${blocker.code}-${blocker.object.kind}-${blocker.object.id}-${blocker.object.version ?? 0}`}><strong>{blocker.code.replaceAll("_", " ")}</strong><span>{blocker.message}</span>{blocker.related_ids.length > 0 && <small>{blocker.related_ids.join(", ")}</small>}</article>)}
      </div>}
      {request.action === "purge" && <label className="management-purge-confirmation">{t("Type")}{" "}<strong>DELETE</strong>{" "}{t("to confirm permanent cleanup")}<input value={purgeConfirmation} onChange={(event) => setPurgeConfirmation(event.target.value)} autoComplete="off" /></label>}
      <footer className="button-row">
        <button onClick={onClose} disabled={busy}>{t("Cancel")}</button>
        <button className={request.action === "purge" ? "danger" : "primary"} disabled={busy || !preview.can_execute || (request.action === "purge" && purgeConfirmation !== "DELETE")} onClick={execute}>{busy ? t("Working…") : t(confirmLabel)}</button>
      </footer>
    </section>
  </div>;
}

function TrashWorkspace({
  project,
  route,
  onNavigate,
  onRefresh,
  onError,
}: {
  project?: ProjectSummary;
  route: Extract<WorkspaceRoute, { kind: "projectTrash" }>;
  onNavigate: (path: string, replace?: boolean) => void;
  onRefresh: () => Promise<void>;
  onError: (value: string) => void;
}) {
  const [items, setItems] = useState<TrashEntry[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [dialog, setDialog] = useState<ManagementDialogState>();
  const [receipt, setReceipt] = useState<ManagementReceipt>();
  const kind = route.objectKind === "all" ? undefined : route.objectKind as ManagementObjectKind | undefined;
  const load = () => api.trash(route.projectId, kind).then((value) => setItems(value.items));
  useEffect(() => {
    setSelected(new Set());
    void load().catch((error: Error) => onError(error.message));
  }, [route.projectId, kind]);
  const selectionKey = (item: TrashEntry) => `${item.object.kind}:${item.object.id}:${item.object.version ?? 0}`;
  const selectedItems = items.filter((item) => selected.has(selectionKey(item)));
  const openAction = (action: "restore" | "purge", targets = selectedItems) => {
    const request = managementRequest(route.projectId, targets.map((item) => item.object), action);
    void api.previewManagement(route.projectId, request)
      .then((preview) => setDialog({ request, preview }))
      .catch((error: Error) => onError(error.message));
  };
  if (!project) return <section className="page-stack"><Empty title={t("Project not found")} detail={t("Trash is always scoped to a stable Project.")} /></section>;
  return <section className="page-stack management-page">
    <ProjectBreadcrumb project={project} current="Trash" onOpenProjects={() => onNavigate("/projects")} onOpenProject={() => onNavigate(`/projects/${encodeURIComponent(project.id)}`)} />
    <div className="toolbar-panel"><div><span className="eyebrow">{t("Project management")}</span><h2>{t("Trash")}</h2><p>Restore removed work or explicitly clean up eligible records. Original images, accepted annotations, exports, models, and credentials are not removed here.</p></div><button onClick={() => onNavigate(projectRunsPath(project.id))}>{t("Back to Runs")}</button></div>
    {receipt && <div className="operation-receipt" role="status"><span><strong>{receipt.action.replaceAll("_", " ")}{" "}{t("completed")}</strong><small>{t("Operation")}{" "}{receipt.operation_id.slice(0, 8)} · persisted on the server</small></span><button onClick={() => setReceipt(undefined)}>{t("Dismiss")}</button></div>}
    <Panel title={t("Removed items")} eyebrow={`${items.length} recoverable item${items.length === 1 ? "" : t("s")}`}>
      <div className="management-list-toolbar">
        <label>{t("Object type")}<select value={kind ?? "all"} onChange={(event) => onNavigate(projectTrashPath(project.id, event.target.value))}><option value="all">{t("All objects")}</option><option value="run">{t("Runs")}</option><option value="batch">{t("Dataset Runs")}</option><option value="workflow_draft">{t("Drafts")}</option><option value="workflow_version">{t("Published Versions")}</option><option value="pipeline">{t("Pipelines")}</option></select></label>
        <span>{selected.size}{" "}{t("selected")}</span>
        <button disabled={selectedItems.length === 0} onClick={() => openAction("restore")}>{t("Restore selected")}</button>
        <button className="danger" disabled={selectedItems.length === 0} onClick={() => openAction("purge")}>{t("Permanently clean up…")}</button>
      </div>
      <div className="management-object-list">
        {items.map((item) => <article key={selectionKey(item)}>
          <input type="checkbox" aria-label={`Select ${item.display_name}`} checked={selected.has(selectionKey(item))} onChange={(event) => setSelected((current) => { const next = new Set(current); if (event.target.checked) next.add(selectionKey(item)); else next.delete(selectionKey(item)); return next; })} />
          <span><strong>{item.display_name}</strong><small>{item.object.kind.replaceAll("_", " ")} · removed {new Date(item.deleted_at).toLocaleString(localeTag())}</small><code>{t("Operation")}{" "}{item.deletion_operation_id.slice(0, 8)}</code></span>
          <button onClick={() => openAction("restore", [item])}>{t("Restore")}</button>
          <button className="danger-button" onClick={() => openAction("purge", [item])}>{t("Clean up…")}</button>
        </article>)}
        {items.length === 0 && <Empty title={t("Trash is empty")} detail="Items moved to Trash remain recoverable until you explicitly clean them up." />}
      </div>
    </Panel>
    {dialog && <ManagementImpactDialog state={dialog} onClose={() => setDialog(undefined)} onError={onError} onComplete={(nextReceipt) => { setDialog(undefined); setReceipt(nextReceipt); setSelected(new Set()); void Promise.all([load(), onRefresh()]).catch((error: Error) => onError(error.message)); }} />}
  </section>;
}

function RunsPage({
  onNavigationGuardChange,
  runs,
  projects,
  activeProject: scopeProject,
  route,
  onNavigate,
  onRefresh,
  onError,
}: {
  onNavigationGuardChange: (guard?: () => boolean) => void;
  runs: HistoryRun[];
  projects: ProjectSummary[];
  activeProject?: ProjectSummary;
  route: Extract<
    WorkspaceRoute,
    { kind: "runs" | "projectRuns" | "projectRun" }
  >;
  onNavigate: (path: string, replace?: boolean) => void;
  onRefresh: () => Promise<void>;
  onError: (value: string) => void;
}) {
  const [batches, setBatches] = useState<DatasetBatchSummary[]>([]);
  const [indexedRuns, setIndexedRuns] = useState<HistoryRun[]>(runs);
  const [runTotal, setRunTotal] = useState(runs.length);
  const [nextRunOffset, setNextRunOffset] = useState<number | null>(null);
  const [selectedManagement, setSelectedManagement] = useState<Map<string, ManagementObjectRef>>(new Map());
  const [managementDialog, setManagementDialog] = useState<ManagementDialogState>();
  const [managementReceipt, setManagementReceipt] = useState<ManagementReceipt>();
  const [undoObjects, setUndoObjects] = useState<ManagementObjectRef[]>([]);
  const [purgedRun, setPurgedRun] = useState<RunProvenanceSummary>();
  const [usageSummary, setUsageSummary] = useState<ManagementUsageSummary>();
  const loadRunPage = (offset = 0, append = false) => {
    const controller = new AbortController();
    void api
      .runs(controller.signal, offset, scopeProject?.id)
      .then((value) => {
        setIndexedRuns((current) => append
          ? [...current, ...value.runs.filter((run) => !current.some((item) => item.id === run.id))]
          : value.runs);
        setRunTotal(value.page.total);
        setNextRunOffset(value.page.next_offset);
      })
      .catch((error: Error) => {
        if (!isAbortError(error)) onError(error.message);
      });
    return controller;
  };
  useEffect(() => {
    void api.batches().then((value) => setBatches(value.batches)).catch((error: Error) => onError(error.message));
  }, [runs.length, runs[0]?.updated_at]);
  useEffect(() => {
    if (!scopeProject) return setUsageSummary(undefined);
    void api.managementUsage(scopeProject.id).then(setUsageSummary).catch((error: Error) => onError(error.message));
  }, [scopeProject?.id, runs.length, managementReceipt?.operation_id]);
  useEffect(() => {
    if (route.kind === "projectRun" || (route.kind === "runs" && route.runId)) return;
    const controller = loadRunPage();
    return () => controller.abort();
  }, [route.kind, scopeProject?.id, runs.length, runs[0]?.updated_at]);
  useEffect(() => setSelectedManagement(new Map()), [scopeProject?.id, route.kind === "projectRuns" ? route.status : undefined]);
  const detailRoute =
    route.kind === "runs" || route.kind === "projectRun" ? route : undefined;
  const routeRunId = detailRoute?.runId;
  const availableRuns = [...indexedRuns, ...runs.filter((run) => !indexedRuns.some((item) => item.id === run.id))];
  const run = availableRuns.find((item) => item.id === routeRunId);
  useEffect(() => {
    setPurgedRun(undefined);
    if (!routeRunId || run) return;
    const controller = new AbortController();
    void api
      .run(routeRunId, controller.signal)
      .then((value) => setIndexedRuns((current) => [
        value.run,
        ...current.filter((item) => item.id !== value.run.id),
      ]))
      .catch((error: Error) => {
        if (isAbortError(error)) return;
        if (error instanceof ApiRequestError && error.status === 404) {
          void api.runProvenance(routeRunId, controller.signal)
            .then(setPurgedRun)
            .catch((provenanceError: Error) => {
              if (!isAbortError(provenanceError)) onError(error.message);
            });
          return;
        }
        onError(error.message);
      });
    return () => controller.abort();
  }, [routeRunId, run?.id]);
  const runOwner = run
    ? projects.find((item) => item.project_id === run.project_id)
    : undefined;
  useEffect(() => {
    if (
      (route.kind === "projectRun" || route.kind === "runs") &&
      route.runId &&
      resolvedRunProjectId(run) &&
      (route.kind === "runs" || route.projectId !== resolvedRunProjectId(run))
    )
      onNavigate(
        projectRunPath(resolvedRunProjectId(run)!, route.runId, {
          annotationId: route.annotationId, canvasView: route.canvasView,
          imageId: route.imageId,
          nodeId: route.nodeId,
          artifactId: route.artifactId,
          view: route.view,
        }),
        true,
      );
  }, [route.kind, routeRunId, run?.project_id, run?.ownership_status]);
  if (detailRoute && routeRunId && run)
    return (
      <RunDetailWorkspace
        onNavigationGuardChange={onNavigationGuardChange}
        key={run.id}
        run={run}
        project={runOwner}
        route={detailRoute}
        onNavigate={onNavigate}
        onRefresh={onRefresh}
        onError={onError}
      />
    );
  if (detailRoute && routeRunId && purgedRun) return <section className="page-stack management-page">
    {scopeProject && <ProjectBreadcrumb project={scopeProject} current="Deleted Run source" onOpenProjects={() => onNavigate("/projects")} onOpenProject={() => onNavigate(`/projects/${encodeURIComponent(scopeProject.id)}`)} />}
    <button className="text-button run-back" onClick={() => onNavigate(scopeProject ? projectRunsPath(scopeProject.id) : "/runs")}>{t("← Run history")}</button>
    <div className="toolbar-panel"><div><span className="eyebrow">Retained provenance · Run {purgedRun.run_id.slice(0, 8)}</span><h2>{t("Source Run was permanently cleaned up")}</h2><p>The original trace and model transcript are gone. This minimal read-only summary remains because user annotation data refers to the source.</p></div></div>
    <Panel title={t("Retained source summary")} eyebrow={new Date(purgedRun.purged_at).toLocaleString(localeTag())}><dl className="management-impact-grid"><div><dt>{t("Provider")}</dt><dd>{purgedRun.provider}</dd></div><div><dt>{t("Model")}</dt><dd>{purgedRun.model}</dd></div><div><dt>{t("Workflow")}</dt><dd>{purgedRun.workflow_id ? `${purgedRun.workflow_id}@v${purgedRun.workflow_version ?? "?"}` : "Legacy"}</dd></div></dl>{purgedRun.workflow_content_hash && <p><strong>{t("Frozen content hash")}</strong><br /><code>{purgedRun.workflow_content_hash}</code></p>}</Panel>
  </section>;
  const projectRuns = runsForContext(indexedRuns, scopeProject);
  const statusFilter =
    route.kind === "runs" || route.kind === "projectRuns"
      ? route.status ?? "all"
      : "all";
  const projectBatches = batches.filter(
    (batch) => !scopeProject || batch.project_id === scopeProject.id,
  );
  const childRunIds = new Set(projectBatches.flatMap((batch) => batch.child_run_ids));
  const standaloneRuns = projectRuns.filter((item) => !childRunIds.has(item.id));
  const visibleBatches = projectBatches.filter(
    (batch) => statusFilter === "all" || batch.status === statusFilter,
  );
  const visibleStandaloneRuns = standaloneRuns.filter(
    (item) => statusFilter === "all" || item.status === statusFilter,
  );
  const visibleExecutions = [
    ...visibleBatches.map((batch) => ({ kind: "batch" as const, created_at: batch.created_at, batch })),
    ...visibleStandaloneRuns.map((item) => ({ kind: "run" as const, created_at: item.created_at, run: item })),
  ].sort((left, right) => right.created_at.localeCompare(left.created_at));
  const availableStatuses = [...new Set([
    ...projectBatches.map((batch) => batch.status),
    ...standaloneRuns.map((item) => item.status),
  ])];
  const setListFilters = (projectId: string, status: string) => {
    setSelectedManagement(new Map());
    const params = new URLSearchParams();
    if (status !== "all") params.set("status", status);
    onNavigate(
      projectId
        ? projectRunsPath(projectId, status)
        : `/runs${params.size ? `?${params.toString()}` : ""}`,
    );
  };
  const managementKey = (object: ManagementObjectRef) => `${object.kind}:${object.id}:${object.version ?? 0}`;
  const toggleManagement = (object: ManagementObjectRef, checked: boolean) =>
    setSelectedManagement((current) => {
      const next = new Map(current);
      if (checked) next.set(managementKey(object), object);
      else next.delete(managementKey(object));
      return next;
    });
  const openDelete = (objects = [...selectedManagement.values()]) => {
    if (!scopeProject || objects.length === 0) return;
    const request = managementRequest(scopeProject.id, objects, "move_to_trash");
    void api.previewManagement(scopeProject.id, request)
      .then((preview) => setManagementDialog({ request, preview }))
      .catch((error: Error) => onError(error.message));
  };
  const reloadManagedRuns = () => Promise.all([
    api.runs(undefined, 0, scopeProject?.id).then((value) => {
      setIndexedRuns(value.runs);
      setRunTotal(value.page.total);
      setNextRunOffset(value.page.next_offset);
    }),
    api.batches().then((value) => setBatches(value.batches)),
    onRefresh(),
  ]);
  const undo = () => {
    if (!scopeProject || undoObjects.length === 0) return;
    const request = managementRequest(scopeProject.id, undoObjects, "restore");
    void api.previewManagement(scopeProject.id, request)
      .then((preview) => {
        if (!preview.can_execute) throw new Error(preview.blockers.map((blocker) => blocker.message).join(" "));
        return api.executeManagement(scopeProject.id, { ...request, confirmation_token: preview.confirmation_token });
      })
      .then((receipt) => {
        setManagementReceipt(receipt);
        setUndoObjects([]);
        return reloadManagedRuns();
      })
      .catch((error: Error) => onError(error.message));
  };
  return (
    <section className="page-stack">
      {scopeProject && <ProjectBreadcrumb
        project={scopeProject}
        current="Runs"
        onOpenProjects={() => onNavigate("/projects")}
        onOpenProject={() => onNavigate(`/projects/${encodeURIComponent(scopeProject.id)}`)}
      />}
      <div className="toolbar-panel"><div><span className="eyebrow">{t("Immutable execution history")}</span><h2>{t("Runs")}</h2><p>Open a Run to inspect its exact Pipeline Version, progress, image, node Artifacts, errors, usage, and Replay.</p></div>{scopeProject && <div className="button-row"><button onClick={() => onNavigate(projectTrashPath(scopeProject.id))}>{t("Trash")}</button><button className="danger-button" disabled={selectedManagement.size === 0} onClick={() => openDelete()}>Delete selected ({selectedManagement.size})</button></div>}</div>
      {usageSummary && <div className="run-lifecycle-usage" aria-label={t("Project Run usage")}><span><small>{t("Visible Run usage")}</small><strong>{usageSummary.visible_runs.total_tokens.toLocaleString(localeTag())} tokens · ${usageSummary.visible_runs.cost}</strong></span><span><small>{t("Historical actual usage")}</small><strong>{usageSummary.historical_total.total_tokens.toLocaleString(localeTag())} tokens · ${usageSummary.historical_total.cost}</strong></span><span><small>{t("Retained after cleanup")}</small><strong>{usageSummary.cleaned_up_runs.total_tokens.toLocaleString(localeTag())} tokens · ${usageSummary.cleaned_up_runs.cost}</strong></span></div>}
      {managementReceipt && <div className="operation-receipt" role="status"><span><strong>{managementReceipt.action === "restore" ? t("Items restored") : t("Moved to Trash")}</strong><small>Server operation {managementReceipt.operation_id.slice(0, 8)} is durable.</small></span>{undoObjects.length > 0 && <button onClick={undo}>{t("Undo")}</button>}<button onClick={() => setManagementReceipt(undefined)}>{t("Dismiss")}</button></div>}
      <Panel title={t("Run history")} eyebrow={`${visibleExecutions.length} executions visible · ${runTotal} image Runs recorded`}>
        <div className="list-filters">
          <label>{t("Project")}<select
              aria-label={t("Project filter")}
              value={scopeProject?.id ?? ""}
              onChange={(event) => setListFilters(event.target.value, statusFilter)}
            >
              <option value="">{t("All projects")}</option>
              {projects.map((project) => <option key={project.id} value={project.id}>{project.name}</option>)}
            </select>
          </label>
          <label>{t("Status")}<select aria-label={t("Status filter")} value={statusFilter} onChange={(event) => setListFilters(scopeProject?.id ?? "", event.target.value)}>
              <option value="all">{t("All statuses")}</option>
              {availableStatuses.map((status) => <option key={status} value={status}>{status.replaceAll("_", " ")}</option>)}
            </select>
          </label>
        </div>
        <div className="runs-table">
          {visibleExecutions.map((execution) => execution.kind === "batch"
            ? <BatchRunGroup key={execution.batch.id} batch={execution.batch} runs={availableRuns} project={projects.find((project) => project.id === execution.batch.project_id)} onNavigate={onNavigate} management={scopeProject ? { selected: selectedManagement.has(`batch:${execution.batch.id}:0`), onToggle: toggleManagement, onDelete: openDelete } : undefined} />
            : <RunHistoryRow key={execution.run.id} run={execution.run} projectId={projects.find((project) => project.project_id === execution.run.project_id)?.id} onNavigate={onNavigate} management={scopeProject ? { selected: selectedManagement.has(`run:${execution.run.id}:0`), onToggle: toggleManagement, onDelete: openDelete } : undefined} />)}
          {visibleExecutions.length === 0 && <Empty title={t("No matching runs")} detail="Change the explicit Project or status filter to see more Run history." />}
        </div>
        {nextRunOffset !== null && <button className="text-button" onClick={() => loadRunPage(nextRunOffset, true)}>{t("Load older Runs")}</button>}
      </Panel>
      {routeRunId && !run && <Empty title={t("Run not found")} detail={t("The linked Run is not available in this workspace.")} />}
      {managementDialog && <ManagementImpactDialog state={managementDialog} onClose={() => setManagementDialog(undefined)} onError={onError} onComplete={(receipt, preview) => { setManagementDialog(undefined); setManagementReceipt(receipt); setSelectedManagement(new Map()); setUndoObjects(preview.objects.map((object) => ({ ...object, expected_revision: object.expected_revision + 1 }))); void reloadManagedRuns().catch((error: Error) => onError(error.message)); }} />}
    </section>
  );
}

function BatchRunGroup({
  batch,
  runs,
  project,
  onNavigate,
  management,
}: {
  batch: DatasetBatchSummary;
  runs: HistoryRun[];
  project?: ProjectSummary;
  onNavigate: (path: string) => void;
  management?: {
    selected: boolean;
    onToggle: (object: ManagementObjectRef, checked: boolean) => void;
    onDelete: (objects: ManagementObjectRef[]) => void;
  };
}) {
  const childRuns = batch.child_run_ids.flatMap((id) => {
    const run = runs.find((candidate) => candidate.id === id);
    return run ? [run] : [];
  });
  const workflowName = childRuns[0]?.workflow_name
    ?? batch.workflow_snapshot.workflow?.draft?.name
    ?? batch.workflow_snapshot.draft?.name
    ?? "Published workflow";
  const workflowVersion = childRuns[0]?.workflow_version ?? batch.workflow_version.split("@").at(-1) ?? "unknown";
  const usage = batch.budget_ledger.consumed;
  const object: ManagementObjectRef = { kind: "batch", id: batch.id, expected_revision: batch.lifecycle_revision };
  return <div className={`managed-execution-row${management ? " with-management" : ""}`}>
    {management && <input type="checkbox" aria-label={`Select Dataset Run ${batch.id.slice(0, 8)}`} checked={management.selected} onChange={(event) => management.onToggle(object, event.target.checked)} />}
    <details className="batch-run-group">
    <summary className="batch-run-row">
      <span className="event-rail" />
      <div><strong>{project?.name ?? batch.project_id}</strong><small>Dataset Run · {workflowName}@v{workflowVersion}</small><code>{batch.progress.completed_images}/{batch.progress.total_images} images completed · Batch {batch.id.slice(0, 8)}</code></div>
      <div className="run-usage"><span>{usage.total_tokens.toLocaleString(localeTag())}{" "}{t("tokens")}</span><span>${usage.cost}</span></div>
      <Status status={batch.status} />
      <span className="row-arrow" aria-hidden="true">⌄</span>
    </summary>
    <div className="batch-run-children">
      <div className="batch-run-explanation"><strong>One Dataset Run</strong><span>AnnotAgent created {batch.progress.total_images} image Runs so each image keeps its own Artifacts, errors, Replay and Review history.</span></div>
      <button className="text-button" onClick={() => onNavigate(projectBatchPath(batch.project_id, batch.id))}>{t("Open Dataset Run detail →")}</button>
      {childRuns.map((run, index) => <RunHistoryRow key={run.id} run={run} projectId={project?.id} onNavigate={onNavigate} childLabel={`Image ${index + 1} of ${batch.progress.total_images}`} />)}
      {childRuns.length === 0 && <Empty title={t("No image Runs recorded")} detail="This Dataset Run stopped before an image Run was created." />}
    </div>
    </details>
    <button className="execution-detail-button" onClick={() => onNavigate(projectBatchPath(batch.project_id, batch.id, { view: "history" }))} aria-label={`${t("Execution details")} · ${batch.id.slice(0, 8)}`}>{t("Execution details")}</button>
    {management && <details className="row-menu"><summary aria-label={`Manage Dataset Run ${batch.id.slice(0, 8)}`}>•••</summary><div><button onClick={() => onNavigate(projectBatchPath(batch.project_id, batch.id))}>{t("View")}</button><button className="danger-button" onClick={() => management.onDelete([object])}>{t("Delete…")}</button></div></details>}
  </div>;
}

function BatchDetailWorkspace({
  onNavigationGuardChange,
  route,
  runs,
  projects,
  onNavigate,
  onRefresh,
  onError,
}: {
  onNavigationGuardChange: (guard?: () => boolean) => void;
  route: Extract<WorkspaceRoute, { kind: "projectBatch" }>;
  runs: HistoryRun[];
  projects: ProjectSummary[];
  onNavigate: (path: string, replace?: boolean) => void;
  onRefresh: () => Promise<void>;
  onError: (value: string) => void;
}) {
  const [batch, setBatch] = useState<DatasetBatchSummary>();
  const [loaded, setLoaded] = useState(false);
  const [statusFilter, setStatusFilter] = useState("all");
  const [busy, setBusy] = useState(false);
  const [managementDialog, setManagementDialog] = useState<ManagementDialogState>();
  const requestGeneration = useRef(0);
  const load = (signal?: AbortSignal) => {
    const generation = ++requestGeneration.current;
    return (
    api
      .batch(route.batchId, signal)
      .then((value) => {
        if (signal?.aborted || generation !== requestGeneration.current) return;
        setBatch(value.batch);
        setLoaded(true);
      })
    );
  };
  useEffect(() => {
    const controller = new AbortController();
    setBatch(undefined);
    setLoaded(false);
    setStatusFilter("all");
    void load(controller.signal).catch((error: Error) => { if (!controller.signal.aborted) onError(error.message); });
    return () => { controller.abort(); requestGeneration.current += 1; };
  }, [route.batchId]);
  const owner = batch
    ? projects.find((project) => project.id === batch.project_id)
    : projects.find((project) => project.id === route.projectId);
  useEffect(() => {
    if (batch && batch.project_id !== route.projectId)
      onNavigate(projectBatchPath(batch.project_id, batch.id, { imageId: route.imageId, status: route.status, view: route.view, annotationId: route.annotationId, canvasView: route.canvasView }), true);
  }, [batch?.id, batch?.project_id, route.projectId, route.imageId, route.status, route.view]);
  if (!loaded)
    return <div className="loading-banner" role="status">{t("Loading Dataset Run…")}</div>;
  if (!batch)
    return (
      <section className="page-stack">
        <ProjectBreadcrumb
          project={owner}
          current="Dataset Run not found"
          onOpenProjects={() => onNavigate("/projects")}
          onOpenProject={owner ? () => onNavigate(`/projects/${encodeURIComponent(owner.id)}`) : undefined}
        />
        <Empty
          title={t("Dataset Run not found")}
          detail="The linked Batch is not available in this workspace."
        />
      </section>
    );
  if (batch.project_id !== route.projectId)
    return <div className="loading-banner" role="status">{t("Opening the owning Project…")}</div>;
  if (route.view !== "history" && !batch.in_trash)
    return <JourneyBatch key={batch.id} project={owner} onNavigationGuardChange={onNavigationGuardChange} batch={batch} route={route} onNavigate={onNavigate} onReload={() => load()} />;
  const childRuns = batch.child_run_ids.flatMap((id) => {
    const run = runs.find((candidate) => candidate.id === id);
    return run ? [run] : [];
  });
  const workflowName =
    childRuns[0]?.workflow_name ??
    batch.workflow_snapshot.workflow?.draft?.name ??
    batch.workflow_snapshot.draft?.name ??
    "Published workflow";
  const workflowVersion =
    childRuns[0]?.workflow_version ??
    batch.workflow_version.split("@").at(-1) ??
    "unknown";
  const control = (action: "pause" | "resume" | "cancel") => {
    setBusy(true);
    void api
      .controlBatch(batch.id, action)
      .then(() => Promise.all([load(), onRefresh()]))
      .catch((error: Error) => onError(error.message))
      .finally(() => setBusy(false));
  };
  const progress = batch.progress;
  const visibleImages = batch.images.filter(
    (image) => statusFilter === "all" || image.status === statusFilter,
  );
  const statusOptions = [...new Set(batch.images.map((image) => image.status))];
  const usage = batch.budget_ledger.consumed;
  const manageBatch = (action: "move_to_trash" | "restore" | "cancel_and_delete") => {
    const request = managementRequest(batch.project_id, [{
      kind: "batch",
      id: batch.id,
      expected_revision: batch.lifecycle_revision,
    }], action);
    void api.previewManagement(batch.project_id, request)
      .then((preview) => setManagementDialog({ request, preview }))
      .catch((error: Error) => onError(error.message));
  };
  return (
    <section className="page-stack batch-detail-page">
      <ProjectBreadcrumb
        project={owner}
        current={`Dataset Run ${batch.id.slice(0, 8)}`}
        onOpenProjects={() => onNavigate("/projects")}
        onOpenProject={owner ? () => onNavigate(`/projects/${encodeURIComponent(owner.id)}`) : undefined}
      />
      <button
        className="text-button run-back"
        onClick={() => onNavigate(projectRunsPath(batch.project_id))}
      >{t("← Run history")}</button>
      {!batch.in_trash && <button onClick={() => onNavigate(projectBatchPath(batch.project_id, batch.id, { imageId: route.imageId, status: route.status, annotationId: route.annotationId, canvasView: route.canvasView }))}>{t("View results")}</button>}
      {batch.in_trash && <div className="trash-state-banner" role="status"><span><strong>{t("This Dataset Run is in Trash")}</strong><small>Its child results are hidden from normal Run history but remain recoverable.</small></span><button onClick={() => manageBatch("restore")}>{t("Restore Dataset Run")}</button><button onClick={() => onNavigate(projectTrashPath(batch.project_id, "batch"))}>{t("Open Trash")}</button></div>}
      <div className="toolbar-panel run-detail-header">
        <div>
          <span className="eyebrow">Dataset Run · {batch.id.slice(0, 8)}</span>
          <h2>{workflowName}@v{workflowVersion}</h2>
          <div className="context-line">
            <Status status={batch.status} />
            <span>{progress.completed_images}/{progress.total_images} images completed</span>
            <span>{batch.max_concurrency} concurrent</span>
            <span>{usage.total_tokens.toLocaleString(localeTag())}{" "}{t("tokens")}</span>
            <span>${usage.cost}</span>
          </div>
        </div>
        <div className="button-row">
          {batch.status === "running" && <button disabled={busy} onClick={() => control("pause")}>{t("Pause")}</button>}
          {batch.status === "paused" && <button disabled={busy} onClick={() => control("resume")}>{t("Resume")}</button>}
          {(batch.status === "running" || batch.status === "paused" || batch.status === "pending") && <button className="danger" disabled={busy} onClick={() => control("cancel")}>{t("Cancel")}</button>}
          {!batch.in_trash && <button className="danger-button" disabled={busy} onClick={() => manageBatch(["running", "paused", "pending", "awaiting_review"].includes(batch.status) ? "cancel_and_delete" : "move_to_trash")}>{["running", "paused", "pending", "awaiting_review"].includes(batch.status) ? t("Cancel and delete…") : t("Delete…")}</button>}
        </div>
      </div>
      <dl className="run-result-metrics" aria-label={t("Dataset Run progress")}>
        <div><dt>{t("Total")}</dt><dd>{progress.total_images}</dd><small>{t("images")}</small></div>
        <div><dt>{t("Completed")}</dt><dd>{progress.completed_images}</dd><small>{t("ready")}</small></div>
        <div><dt>{t("Running")}</dt><dd>{progress.running_images}</dd><small>{t("in progress")}</small></div>
        <div><dt>{t("Review")}</dt><dd>{progress.review_images}</dd><small>{t("needs attention")}</small></div>
        <div><dt>{t("Failed")}</dt><dd>{progress.failed_images}</dd><small>{t("images")}</small></div>
        <div><dt>{t("Pending")}</dt><dd>{progress.pending_images}</dd><small>{t("queued")}</small></div>
      </dl>
      <Panel title={t("Images")} eyebrow={`${visibleImages.length} of ${progress.total_images} shown`}>
        <div className="batch-image-toolbar">
          <label>{t("Image status")}<select aria-label={t("Filter Dataset Run images")} value={statusFilter} onChange={(event) => setStatusFilter(event.target.value)}><option value="all">{t("All statuses")}</option>{statusOptions.map((status) => <option key={status} value={status}>{status.replaceAll("_", " ")}</option>)}</select></label>
          <button disabled={busy} onClick={() => void load().catch((error: Error) => onError(error.message))}>{t("Refresh")}</button>
        </div>
        <div className="runs-table batch-image-runs">
          {visibleImages.map((image) => {
            const childRun = image.child_run_id
              ? runs.find((candidate) => candidate.id === image.child_run_id)
              : undefined;
            return <button
              key={image.image_id}
              className="run-row batch-child-run"
              disabled={!image.child_run_id}
              onClick={() => image.child_run_id && onNavigate(projectRunPath(batch.project_id, image.child_run_id, { imageId: image.image_id }))}
            >
              <span className="event-rail" />
              <div><strong>{image.name}</strong><small>{t("Image")}{" "}{image.position + 1} · {image.annotation_count} annotations · {image.review_count}{" "}{t("reviews")}</small><code>{image.image_id}</code>{image.failure && <small className="run-reason">{image.failure}</small>}</div>
              <div className="run-usage"><span>{image.usage.total_tokens.toLocaleString(localeTag())}{" "}{t("tokens")}</span><span>${image.usage.cost}</span></div>
              <Status status={image.status} />
              <span className="row-arrow" aria-hidden="true">{childRun ? "→" : "·"}</span>
            </button>;
          })}
          {visibleImages.length === 0 && (
            <Empty
              title={t("No images match this status")}
              detail={t("Choose another status or refresh the Dataset Run.")}
            />
          )}
        </div>
      </Panel>
      {managementDialog && <ManagementImpactDialog state={managementDialog} onClose={() => setManagementDialog(undefined)} onError={onError} onComplete={(receipt) => { setManagementDialog(undefined); void onRefresh(); if (receipt.action === "move_to_trash") onNavigate(projectRunsPath(batch.project_id)); else void load().catch((error: Error) => onError(error.message)); }} />}
    </section>
  );
}

function RunHistoryRow({
  run,
  projectId,
  onNavigate,
  childLabel,
  management,
}: {
  run: HistoryRun;
  projectId?: string;
  onNavigate: (path: string) => void;
  childLabel?: string;
  management?: {
    selected: boolean;
    onToggle: (object: ManagementObjectRef, checked: boolean) => void;
    onDelete: (objects: ManagementObjectRef[]) => void;
  };
}) {
  const object: ManagementObjectRef = { kind: "run", id: run.id, expected_revision: run.lifecycle_revision };
  return <div className={`managed-execution-row${childLabel ? " managed-child-row" : ""}${management ? " with-management" : ""}`}>
    {management && <input type="checkbox" aria-label={`Select Run ${run.id.slice(0, 8)}`} checked={management.selected} onChange={(event) => management.onToggle(object, event.target.checked)} />}
    <button className={`run-row${childLabel ? " batch-child-run" : ""}`} onClick={() => onNavigate(projectId ? projectRunPath(projectId, run.id) : `/runs/${encodeURIComponent(run.id)}`)}>
    <span className="event-rail" />
    <div><strong>{childLabel ?? run.project_name}</strong><small>{run.workflow_name}@v{run.workflow_version}</small><code>{run.model_identity} · {run.artifact_count}{" "}{t("Artifacts")}</code>{run.terminal_reason && <small className="run-reason">{run.terminal_reason}</small>}</div>
    <div className="run-usage"><span>{(run.input_tokens + run.output_tokens).toLocaleString(localeTag())}{" "}{t("tokens")}</span><span>${run.cost}</span></div>
    <Status status={run.status} />
    <span className="row-arrow" aria-hidden="true">→</span>
    </button>
    <button className="execution-detail-button" onClick={() => onNavigate(projectId ? projectRunPath(projectId, run.id, { view: "debug", imageId: run.image_id }) : `/runs/${encodeURIComponent(run.id)}?view=debug`)} aria-label={`${t("Execution details")} · ${run.id.slice(0, 8)}`}>{t("Execution details")}</button>
    {management && <details className="row-menu"><summary aria-label={`Manage Run ${run.id.slice(0, 8)}`}>•••</summary><div><button onClick={() => onNavigate(projectId ? projectRunPath(projectId, run.id) : `/runs/${encodeURIComponent(run.id)}`)}>{t("View")}</button><button className="danger-button" onClick={() => management.onDelete([object])}>{t("Delete…")}</button></div></details>}
  </div>;
}

function RunDetailWorkspace({
  onNavigationGuardChange,
  run,
  project,
  route,
  onNavigate,
  onRefresh,
  onError,
}: {
  onNavigationGuardChange: (guard?: () => boolean) => void;
  run: HistoryRun;
  project?: ProjectSummary;
  route: Extract<WorkspaceRoute, { kind: "runs" | "projectRun" }>;
  onNavigate: (path: string, replace?: boolean) => void;
  onRefresh: () => Promise<void>;
  onError: (value: string) => void;
}) {
  const view = route.view ?? "results";
  const [replay, setReplay] = useState<NodeReplayReport>();
  const [busy, setBusy] = useState(false);
  const [managementDialog, setManagementDialog] = useState<ManagementDialogState>();
  const alive = useRef(true);
  const actionPending = useRef(false);
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  const resultQuery = useRouteQuery(
    queryKeys.runResults(run.id),
    (signal) => api.runResultSummary(run.id, signal),
    { staleTime: run.controllable ? 0 : 30_000 },
  );
  const annotationQuery = useRouteQuery(
    queryKeys.runAnnotations(run.id),
    (signal) => api.runAnnotations(run.id, signal),
    { staleTime: run.controllable ? 0 : 30_000 },
  );
  const debugQuery = useRouteQuery(
    view === "debug" ? queryKeys.runDebug(run.id) : undefined,
    (signal) => api.runDebugSummary(run.id, signal),
    { staleTime: run.controllable ? 0 : 30_000 },
  );
  const artifactQuery = useRouteQuery(
    view === "debug" ? `${queryKeys.runDebug(run.id)}/artifacts` : undefined,
    (signal) => api.pipelineArtifacts(run.id, signal),
    { staleTime: run.controllable ? 0 : 30_000 },
  );
  const imageQuery = useRouteQuery(
    project ? queryKeys.projectImages(project.id) : undefined,
    (signal) => api.images(project!.id, signal),
    { staleTime: 30_000 },
  );
  const inspection = replay?.inspection ?? artifactQuery.data;
  const annotationInspection = annotationQuery.data?.run_id === run.id && (!project || annotationQuery.data.project_id === project.id) ? annotationQuery.data : undefined;
  const resultSummary = resultQuery.data?.run_id === run.id && (!project || resultQuery.data.project_id === project.id) ? resultQuery.data : undefined;
  const debugSummary = debugQuery.data;
  const images = imageQuery.data?.images ?? [];
  const runPath = (context: {
    imageId?: string;
    nodeId?: string;
    artifactId?: string;
    view?: "results" | "debug";
    annotationId?: string;
    canvasView?: "original";
  } = {}) =>
    project
      ? projectRunPath(project.id, run.id, { annotationId: route.annotationId, canvasView: route.canvasView, ...context })
      : `/runs/${encodeURIComponent(run.id)}${(() => {
          const params = new URLSearchParams();
          if (context.view === "debug" || context.nodeId || context.artifactId)
            params.set("view", "debug");
          if (context.imageId) params.set("image", context.imageId);
          if (context.nodeId) params.set("node", context.nodeId);
          if (context.artifactId) params.set("artifact", context.artifactId);
          if (context.annotationId ?? route.annotationId) params.set("annotation", context.annotationId ?? route.annotationId!);
          if ((context.canvasView ?? route.canvasView) === "original") params.set("display", "original");
          return params.size ? `?${params.toString()}` : "";
        })()}`;
  useEffect(() => setReplay(undefined), [run.id]);
  useEffect(() => {
    const error = resultQuery.error ?? annotationQuery.error ?? debugQuery.error ?? artifactQuery.error ?? imageQuery.error;
    if (error) onError(error.message);
  }, [resultQuery.error, annotationQuery.error, debugQuery.error, artifactQuery.error, imageQuery.error]);
  useEffect(() => {
    if (view !== "debug") return;
    if (!route.nodeId && inspection?.nodes[0]) {
      onNavigate(
        runPath({
          view: "debug",
          imageId: inspection.image_id ?? annotationInspection?.image_id ?? run.image_id,
          nodeId: inspection.nodes[0].node_id,
        }),
        true,
      );
    }
  }, [view, run.id, route.nodeId, inspection, annotationInspection?.image_id, run.image_id]);
  const selectedNode = inspection?.nodes.find((node) => node.node_id === route.nodeId) ?? inspection?.nodes[0];
  const lineageStageNodes = (["coarse", "search_region", "relocalized", "prompt_coverage", "mask", "refined", "final"] as ResultLineageStage[]).flatMap((stage) => {
    const node = [...(inspection?.nodes ?? [])].reverse().find((candidate) => resultLineageStageForOperation(candidate.operation) === stage);
    return node ? [{ stage, node }] : [];
  });
  const selectedArtifacts = selectedNode
    ? selectedNode.outputs.filter(
        (artifact, index) =>
          !route.artifactId || pipelineArtifactIdentity(artifact, index) === route.artifactId,
      )
    : [];
  const ownedImageId = resultSummary?.image.image_id
    ?? annotationInspection?.image_id
    ?? inspection?.image_id
    ?? run.image_id;
  const ownedImage = images.find((image) => image.image_id === ownedImageId);
  useEffect(() => {
    if (ownedImageId && route.imageId !== ownedImageId)
      onNavigate(runPath({
        view,
        imageId: ownedImageId,
        nodeId: view === "debug" ? route.nodeId : undefined,
        artifactId: view === "debug" ? route.artifactId : undefined,
      }), true);
  }, [ownedImageId, route.imageId, route.nodeId, route.artifactId, view]);
  const runAnnotations = annotationInspection?.image_id === ownedImageId ? annotationInspection?.annotations ?? [] : [];
  const finalAnnotationIds = new Set([
    ...(resultSummary?.projection.committed_annotation_ids ?? []),
    ...(resultSummary?.projection.review_candidate_ids ?? []),
  ]);
  const finalAnnotations = runAnnotations.filter((annotation) => finalAnnotationIds.has(annotation.id));
  const runReviewId = resultSummary?.projection.review_candidate_ids[0];
  const selectedPreviewArtifacts = selectedNode
    ? [...selectedNode.inputs, ...(selectedArtifacts.length ? selectedArtifacts : selectedNode.outputs)]
    : [];
  const previewProjectId = inspection?.project_id ?? annotationInspection?.project_id ?? project?.id;
  const canPreview = Boolean(
    previewProjectId && ownedImageId && (ownedImage || inspection || runAnnotations.length),
  );
  const setContext = (context: { node?: string; artifact?: string }) => {
    onNavigate(
      runPath({
        view: "debug",
        imageId: ownedImageId,
        nodeId: context.node ?? selectedNode?.node_id,
        artifactId: context.artifact,
      }),
    );
  };
  const setView = (next: "results" | "debug") => {
    onNavigate(
      runPath({
        view: next,
        imageId: ownedImageId,
        nodeId: next === "debug" ? selectedNode?.node_id : undefined,
      }),
    );
  };
  const control = (action: "pause" | "resume" | "cancel") => {
    if (actionPending.current) return;
    actionPending.current = true;
    setBusy(true);
    void api.control(run.id, action).then(() => { if (alive.current) return onRefresh(); }).catch((error: Error) => { if (alive.current) onError(error.message); }).finally(() => { actionPending.current = false; if (alive.current) setBusy(false); });
  };
  const manageRun = (action: "move_to_trash" | "restore" | "cancel_and_delete") => {
    if (!project) return onError("This Run does not have a resolvable owning Project.");
    const request = managementRequest(project.id, [{
      kind: "run",
      id: run.id,
      expected_revision: run.lifecycle_revision,
    }], action);
    void api.previewManagement(project.id, request)
      .then((preview) => { if (alive.current) setManagementDialog({ request, preview }); })
      .catch((error: Error) => { if (alive.current) onError(error.message); });
  };
  const replayNode = () => {
    if (!selectedNode || actionPending.current) return;
    actionPending.current = true;
    setBusy(true);
    void api.replayNode(run.id, selectedNode.node_id).then((value) => {
      if (alive.current) setReplay(value);
      workspaceQueries.invalidate(queryKeys.runDebug(run.id));
      workspaceQueries.invalidate(queryKeys.runResults(run.id));
    }).catch((error: Error) => { if (alive.current) onError(error.message); }).finally(() => { actionPending.current = false; if (alive.current) setBusy(false); });
  };
  const duration = Math.max(0, new Date(run.updated_at).getTime() - new Date(run.created_at).getTime());
  const completedNodes = inspection?.nodes.filter((node) =>
    ["succeeded", "completed", "skipped"].includes(node.status),
  ).length;
  const nodeProgress = inspection?.nodes.length
    ? `${completedNodes}/${inspection.nodes.length} nodes`
    : run.current_node
      ? `Current: ${run.current_node}`
      : "No node trace";
  const resultHeadline = run.status === "running"
    ? "Run in progress"
    : run.status === "paused"
      ? "Run paused"
      : run.status === "completed" || run.status === "completed_with_review"
        ? "Run completed"
        : run.status === "awaiting_review"
          ? "Results need review"
          : `Run ${run.status.replaceAll("_", " ")}`;
  if (view === "results" && !run.in_trash && project)
    return <JourneyRun key={run.id} project={project} onNavigationGuardChange={onNavigationGuardChange} run={run} image={ownedImage} summary={resultSummary} annotations={finalAnnotations} annotationsReady={Boolean(annotationInspection && annotationInspection.image_id === ownedImageId)} selectedId={route.annotationId} original={route.canvasView === "original"} onSelect={(annotationId) => onNavigate(runPath({ imageId: ownedImageId, annotationId }), true)} onOriginal={(original) => onNavigate(runPath({ imageId: ownedImageId, canvasView: original ? "original" : undefined }), true)} busy={busy} onControl={control} onNavigate={onNavigate} />;
  return (
    <section className="page-stack run-detail-page">
      <ProjectBreadcrumb
        project={project}
        current={`Run ${run.id.slice(0, 8)}`}
        onOpenProjects={() => onNavigate("/projects")}
        onOpenProject={project ? () => onNavigate(`/projects/${encodeURIComponent(project.id)}`) : undefined}
      />
      <button className="text-button run-back" onClick={() => onNavigate(project ? projectRunsPath(project.id) : "/runs")}>{t("← Run history")}</button>
      {run.in_trash && project && <div className="trash-state-banner" role="status"><span><strong>{t("This Run is in Trash")}</strong><small>Its unfinished Review work is hidden. Accepted annotations and source data are preserved.</small></span><button onClick={() => manageRun("restore")}>{t("Restore Run")}</button><button onClick={() => onNavigate(projectTrashPath(project.id, "run"))}>{t("Open Trash")}</button></div>}
      <nav className="run-view-tabs" aria-label={t("Run workspace view")}>
        <button className={view === "results" ? "active" : ""} aria-current={view === "results" ? "page" : undefined} onClick={() => setView("results")}>{t("Results")}</button>
        <button className={view === "debug" ? "active" : ""} aria-current={view === "debug" ? "page" : undefined} onClick={() => setView("debug")}>{t("Debug")}</button>
      </nav>
      <div className="toolbar-panel run-detail-header">
        {view === "results" ? <div><span className="eyebrow">{run.project_name} · {run.workflow_name}@v{run.workflow_version}</span><h2>{resultHeadline}</h2><div className="context-line"><Status status={run.status} /><span>{formatSampleDuration(resultSummary?.duration_ms ?? duration)}</span><span>${resultSummary?.usage.estimated_cost ?? run.cost}</span></div></div> : <div><span className="eyebrow">Debug · Run {run.id.slice(0, 8)}</span><h2>{run.workflow_name}@v{run.workflow_version}</h2><div className="context-line"><Status status={run.status} /><span>{nodeProgress}</span><span>{run.artifact_count}{" "}{t("Artifacts")}</span><span>{(run.input_tokens + run.output_tokens).toLocaleString(localeTag())}{" "}{t("tokens")}</span><span>${run.cost}</span></div></div>}
        <div className="button-row">
          {project && <button onClick={() => onNavigate(`/projects/${encodeURIComponent(project.id)}/build/pipeline`)}>{t("Improve automation")}</button>}
          {runReviewId && <button onClick={() => onNavigate(project ? projectReviewPath(project.id, runReviewId) : `/review/${encodeURIComponent(runReviewId)}`)}>{t("Review")}{" "}{resultSummary?.needs_review_count || 1} result</button>}
          {run.status === "running" && <button disabled={busy} onClick={() => control("pause")}>{t("Pause")}</button>}
          {run.status === "paused" && <button disabled={busy} onClick={() => control("resume")}>{t("Resume")}</button>}
          {run.controllable && <button className="danger" disabled={busy} onClick={() => control("cancel")}>{t("Cancel")}</button>}
          {!run.in_trash && project && <button className="danger-button" disabled={busy} onClick={() => manageRun(run.controllable || ["pending", "running", "paused", "awaiting_review"].includes(run.status) ? "cancel_and_delete" : "move_to_trash")}>{run.controllable || ["pending", "running", "paused", "awaiting_review"].includes(run.status) ? t("Cancel and delete…") : t("Delete…")}</button>}
        </div>
      </div>
      {view === "results" ? <>
        <dl className="run-result-metrics" aria-label={t("Run result summary")}>
          <div><dt>{t("Images")}</dt><dd>{resultSummary?.image_count ?? 1}</dd><small>processed</small></div>
          <div><dt>{t("Accepted")}</dt><dd>{resultSummary?.ready_count ?? 0}</dd><small>{resultSummary?.result_count ?? runAnnotations.length}{" "}{t("detections")}</small></div>
          <div><dt>{t("Needs review")}</dt><dd>{resultSummary?.needs_review_count ?? 0}</dd><small>{t("human decision")}</small></div>
          <div><dt>{t("Fallbacks")}</dt><dd>{resultSummary?.fallback_count ?? run.fallback_nodes.length}</dd><small>open-vocabulary</small></div>
          <div><dt>{t("Cache hits")}</dt><dd>{resultSummary?.cache_hit_count ?? 0}</dd><small>{t("model calls reused")}</small></div>
          <div><dt>{t("Failed")}</dt><dd>{resultSummary?.failed_count ?? 0}</dd><small>{resultSummary?.no_target_count ?? 0}{" "}{t("no-target")}</small></div>
        </dl>
        <div className="run-results-workspace">
          <aside className="panel run-image-identity"><span className="eyebrow">{t("Run image")}</span>{ownedImage ? <><img src={ownedImage.url} alt="" /><strong>{ownedImage.name}</strong><Status status={resultSummary?.image.status ?? run.status} /><code>{ownedImage.image_id}</code></> : ownedImageId ? <><strong>{t("Project image")}</strong><Status status={resultSummary?.image.status ?? run.status} /><code>{ownedImageId}</code></> : <small>Resolving stable Image identity…</small>}</aside>
          <main className="panel run-visual-workspace run-result-preview"><span className="eyebrow">{t("Result Preview")}</span>{resultSummary?.labels.length ? <div className="run-result-labels" aria-label={t("Result labels")}>{resultSummary.labels.map((item) => <span key={item.label}>{item.label}<b>{item.count}</b></span>)}</div> : null}
            {canPreview ? <>{!finalAnnotations.length && <p className="sample-risk-notice" role="status">{t(resultSummary?.failed_count ? "No final annotation was produced. Inspect the image and failure details." : "No final target was returned. Inspect the original image for possible missed objects.")}</p>}<RunArtifactCanvas projectId={previewProjectId!} project={project} artifacts={[]} annotations={finalAnnotations} imageId={ownedImageId!} /></> : <Empty title={t("No visual result")} detail={t("The source image is unavailable. No other image was substituted.")} />}
          </main>
          <aside className="panel run-needs-attention"><span className="eyebrow">{t("Needs Attention")}</span>{runReviewId ? <><h3>{resultSummary?.needs_review_count || 1} result needs a decision</h3><p>The current final candidate is waiting for a human decision.</p><button className="primary" onClick={() => onNavigate(project ? projectReviewPath(project.id, runReviewId) : `/review/${encodeURIComponent(runReviewId)}`)}>{t("Review result")}</button></> : resultSummary?.failed_count ? <><h3>{t("Run needs repair")}</h3><p>{run.terminal_reason ?? t("A Pipeline step did not produce a usable result.")}</p><button className="primary" onClick={() => setView("debug")}>{t("Open Debug")}</button></> : <div className="positive-empty"><strong>{t("No results need attention")}</strong><span>{resultSummary?.no_target_count ? t("No target returned is not proof that the image contains no target.") : t("All results passed the configured gates.")}</span></div>}</aside>
        </div>
      </> : <>
        <div className="debug-summary-strip" aria-label={t("Run debug summary")}><span>{debugSummary?.succeeded_node_count ?? completedNodes ?? 0}/{debugSummary?.node_count ?? inspection?.nodes.length ?? 0}{" "}{t("steps complete")}</span><span>{debugSummary?.failed_node_count ?? 0}{" "}{t("failed")}</span><span>{debugSummary?.issues.length ?? 0}{" "}{t("issues")}</span><span>{formatSampleDuration(debugSummary?.duration_ms ?? duration)}</span></div>
        {lineageStageNodes.length > 0 && <nav className="run-lineage-stage-selector" aria-label={t("Artifact lineage stages")}>{lineageStageNodes.map(({ stage, node }) => <button key={`${stage}-${node.node_id}`} className={selectedNode?.node_id === node.node_id ? "active" : ""} aria-current={selectedNode?.node_id === node.node_id ? "step" : undefined} onClick={() => setContext({ node: node.node_id })}><strong>{stage === "search_region" ? t("Search region") : stage === "prompt_coverage" ? t("Prompt coverage") : stage[0].toUpperCase() + stage.slice(1)}</strong><small>{node.operation}</small></button>)}</nav>}
        <div className="run-workspace">
          <aside className="panel run-image-identity"><span className="eyebrow">{t("Run image")}</span>{ownedImage ? <><img src={ownedImage.url} alt="" /><strong>{ownedImage.name}</strong><Status status={resultSummary?.image.status ?? run.status} /><code>{ownedImage.image_id}</code></> : ownedImageId ? <><strong>{t("Project image")}</strong><Status status={resultSummary?.image.status ?? run.status} /><code>{ownedImageId}</code></> : <small>Resolving stable Image identity…</small>}</aside>
          <main className="panel run-visual-workspace"><span className="eyebrow">{t("Artifact Preview")}</span>{canPreview ? <RunArtifactCanvas projectId={previewProjectId!} project={project} artifacts={selectedPreviewArtifacts} annotations={finalAnnotations} imageId={ownedImageId!} /> : <Empty title={t("No visual Artifact")} detail={run.checkpoint_present ? t("Loading the persisted checkpoint and annotations.") : t("This Run has no visual Artifact to preview.")} />}</main>
          <aside className="panel run-node-timeline"><span className="eyebrow">{t("Pipeline Steps")}</span>{inspection?.nodes.map((node, index) => <button key={node.node_id} className={node.node_id === selectedNode?.node_id ? "active" : ""} onClick={() => setContext({ node: node.node_id })}><span>{index + 1}</span><span><strong title={node.operation}>{node.operation}</strong><small title={`${node.status} · ${node.latency_ms} ms`}>{node.status} · {node.latency_ms} ms</small></span>{node.error && <i title={node.error.summary}>!</i>}</button>)}{!inspection && <small>{t("No node trace available.")}</small>}</aside>
        </div>
      {selectedNode && (
        <section className="panel run-node-inspector" aria-label={t("Node inspector")}>
          <header className="run-node-inspector-header">
            <div>
              <span className="eyebrow">{t("Node inspector")}</span>
              <h2>{selectedNode.operation}</h2>
              <code>Node ID · {selectedNode.node_id}</code>
            </div>
            <button disabled={busy} onClick={replayNode}>{t("Replay from this node")}</button>
          </header>
          <div className="run-node-metrics" aria-label={t("Node execution summary")}>
            <article><span>{t("Status")}</span><Status status={selectedNode.status} /></article>
            <article><span>{t("Duration")}</span><strong>{selectedNode.latency_ms.toLocaleString(localeTag())} ms</strong></article>
            <article><span>{t("Model usage")}</span><strong>{selectedNode.usage.input_tokens + selectedNode.usage.output_tokens}{" "}{t("tokens")}</strong><small>${selectedNode.usage.cost}</small></article>
          </div>
          <section className="run-node-artifacts" aria-labelledby="node-output-artifacts">
            <header><div><span className="eyebrow">{t("Artifacts")}</span><h3 id="node-output-artifacts">{t("Node outputs")}</h3></div><b>{selectedNode.outputs.length}</b></header>
            <div className="artifact-choice" aria-label={t("Node output Artifacts")}>{selectedNode.outputs.map((artifact, index) => { const id = pipelineArtifactIdentity(artifact, index); return <button key={id} className={route.artifactId === id ? "active" : ""} onClick={() => setContext({ artifact: id })}><span>{artifact.kind.replaceAll("_", " ")}</span><code>{id.slice(0, 8)}</code></button>; })}</div>
            {selectedNode.outputs.length === 0 && <p className="node-payload-empty">{t("This node did not produce an Artifact.")}</p>}
          </section>
          <EvidenceDecisionCard metadata={selectedNode.metadata ?? {}} route={selectedNode.route} />
          {selectedNode.error && <div className="run-repair-card"><div><strong>{selectedNode.error.code}</strong><p>{selectedNode.error.summary}</p></div><div className="button-row">{selectedNode.error.retryable && <button className="primary" disabled={busy} onClick={replayNode}>{t("Replay failed step")}</button>}{project && <button onClick={() => onNavigate(`/projects/${encodeURIComponent(project.id)}/build/pipeline`)}>{t("Fix automation")}</button>}</div></div>}
          <div className="node-payload-sections">
            {selectedNode.metadata?.model_asset != null && <NodePayloadSection title={t("Model identity")} description="Immutable Plugin, Bundle, files, Contract, Model Instance, revision, and execution provider frozen by this Workflow Version" badge="Frozen" value={selectedNode.metadata.model_asset} open />}
            <NodePayloadSection title={t("Input")} description="Artifacts received from upstream nodes" badge={selectedNode.inputs.length} value={selectedNode.inputs} />
            <NodePayloadSection title={t("Output")} description="Artifacts emitted by this node" badge={selectedNode.outputs.length} value={selectedNode.outputs} open />
            <NodePayloadSection title={t("Configuration")} description="Resolved runtime configuration" badge="JSON" value={selectedNode.configuration} />
            <NodePayloadSection title={t("Provider request")} description="Recorded provider context; credentials and image bytes are redacted" badge={run.provider} value={{ provider: run.provider, model: run.model, operation: selectedNode.operation, parameters: selectedNode.configuration.parameters }} />
            {selectedNode.error && <NodePayloadSection title={t("Raw error")} description="Structured Runtime failure" badge={selectedNode.error.code} value={selectedNode.error} />}
          </div>
          {replay?.replayed_from === selectedNode.node_id && <div className="validation-report valid"><strong>{t("Sandbox Replay completed")}</strong><small>{t("Preserved upstream:")}{" "}{replay.preserved_upstream_nodes.join(", ") || t("None")}</small><small>{t("Re-executed:")}{" "}{replay.reexecuted_nodes.join(", ")}</small></div>}
        </section>
      )}
      </>}
      {managementDialog && <ManagementImpactDialog state={managementDialog} onClose={() => setManagementDialog(undefined)} onError={onError} onComplete={(receipt) => { setManagementDialog(undefined); void onRefresh(); if (project) onNavigate(projectRunsPath(project.id)); }} />}
    </section>
  );
}

function resultLineageStageForOperation(operation: string): ResultLineageStage | undefined {
  if (["vlm_detection.detect", "capability.detect", "object_detection.detect", "yolo.detect"].includes(operation)) return "coarse";
  if (["core.expand_region", "core.crop", "core.tile"].includes(operation)) return "search_region";
  if (operation === "core.project_coordinates") return "relocalized";
  if (operation === "core.prompt_coverage_gate") return "prompt_coverage";
  if (operation === "capability.segment") return "mask";
  if (["core.mask_to_bbox", "core.geometry_quality_evaluation"].includes(operation)) return "refined";
  if (["core.geometry_decision", "core.human_review", "commit"].includes(operation)) return "final";
  return undefined;
}

export function evidenceGateReport(
  metadata: Record<string, unknown>,
): EvidenceGateReportDto | undefined {
  const value = metadata.evidence_gate;
  if (!value || typeof value !== "object") return undefined;
  const report = value as Record<string, unknown>;
  if (!(["accept", "fallback", "review", "reject"] as const).includes(
    report.decision as "accept" | "fallback" | "review" | "reject",
  )) return undefined;
  if (!Array.isArray(report.reasons)) return undefined;
  const reasons = report.reasons.flatMap((reason) => {
    if (!reason || typeof reason !== "object") return [];
    const item = reason as Record<string, unknown>;
    if (typeof item.code !== "string" || typeof item.message !== "string") return [];
    return [{
      code: item.code,
      message: item.message,
      candidate_id: typeof item.candidate_id === "string" ? item.candidate_id : undefined,
      source_model_ids: Array.isArray(item.source_model_ids)
        ? item.source_model_ids.filter((source): source is string => typeof source === "string")
        : [],
      metrics: item.metrics && typeof item.metrics === "object"
        ? Object.fromEntries(
          Object.entries(item.metrics as Record<string, unknown>)
            .filter((entry): entry is [string, number] =>
              typeof entry[1] === "number" && Number.isFinite(entry[1]),
            ),
        )
        : {},
    }];
  });
  return {
    decision: report.decision as EvidenceGateReportDto["decision"],
    reasons,
    candidate_count: typeof report.candidate_count === "number" ? report.candidate_count : 0,
    validation_issue_count:
      typeof report.validation_issue_count === "number" ? report.validation_issue_count : 0,
  };
}

function EvidenceDecisionCard({
  metadata,
  route,
}: {
  metadata: Record<string, unknown>;
  route?: string | null;
}) {
  const report = evidenceGateReport(metadata);
  if (!report) return null;
  return (
    <section className={`evidence-decision-card decision-${report.decision}`} aria-label={t("Evidence decision")}>
      <header>
        <div><span className="eyebrow">{t("Evidence decision")}</span><h3>{report.decision}</h3></div>
        <span>{report.candidate_count} candidate{report.candidate_count === 1 ? "" : t("s")}</span>
      </header>
      <ul>
        {report.reasons.map((reason, index) => (
          <li key={`${reason.code}-${reason.candidate_id ?? index}`}>
            <strong>{reason.message}</strong>
            <small>
              {reason.code.replaceAll("_", " ")}
              {reason.source_model_ids.length ? ` · ${reason.source_model_ids.join(" + ")}` : ""}
            </small>
          </li>
        ))}
      </ul>
      <footer>
        <span>Route · {route ?? report.decision}</span>
        <span>Domain issues · {report.validation_issue_count}</span>
      </footer>
    </section>
  );
}

function NodePayloadSection({
  title,
  description,
  badge,
  value,
  open = false,
}: {
  title: string;
  description: string;
  badge: string | number;
  value: unknown;
  open?: boolean;
}) {
  const empty = Array.isArray(value)
    ? value.length === 0
    : Boolean(value) && typeof value === "object"
      ? Object.keys(value as Record<string, unknown>).length === 0
      : value === undefined || value === null;
  return (
    <details className="node-payload-section" open={open}>
      <summary>
        <span><strong>{title}</strong><small>{description}</small></span>
        <b>{badge}</b>
        <i aria-hidden="true" />
      </summary>
      <div>
        {empty
          ? <p className="node-payload-empty">{t("No")}{" "}{title.toLowerCase()} data recorded.</p>
          : <pre>{JSON.stringify(value, null, 2)}</pre>}
      </div>
    </details>
  );
}

function pipelineArtifactIdentity(artifact: PipelineArtifact, index: number): string {
  const reference = artifact.artifact.reference;
  if (reference && typeof reference === "object") {
    const id = (reference as Record<string, unknown>).artifact_id;
    if (typeof id === "string") return id;
  }
  return `${artifact.kind}-${index}`;
}

type ArtifactMark = ArtifactRect & {
  id: string;
  label: string;
  confidence?: number;
  color: string;
  parentId?: string;
  parentArtifact?: string;
  sourceNode?: string;
  evidence: DetectionEvidenceDto[];
  scoreSemantics?: string;
  geometrySemantics?: string;
  calibrationStatus?: string;
  geometryReportId?: string;
  geometryIssues?: string[];
  agreement?: "single_source" | "geometry_conflict" | "label_conflict" | { multi_source_agreement: { minimum_iou: number; mean_iou: number } };
};

export function geometrySemanticsLabel(value?: string): string {
  const labels: Record<string, string> = {
    not_applicable: "Not applicable",
    coarse_hypothesis: "Uncalibrated coarse proposal",
    predicted_geometry: "Predicted box",
    refined_geometry: "Refined by prompted segmentation",
    mask_refined_geometry: "Refined by prompted segmentation",
    calibrated_geometry: "Project-calibrated geometry",
    human_verified: "Human-verified geometry",
  };
  return value ? labels[value] ?? value.replaceAll("_", " ") : "Geometry source not recorded";
}

export function scoreSemanticsLabel(value?: string): string {
  const labels: Record<string, string> = {
    semantic_confidence: "Semantic confidence",
    detection_confidence: "Detection confidence",
    calibrated_probability: "Calibrated probability",
    relative_confidence: "Relative model score",
    ranking_score: "Ranking score",
    not_provided: "Model score",
    unknown: "Model score",
  };
  return value ? labels[value] ?? value.replaceAll("_", " ") : "Model score";
}

function geometryStateFromDetection(
  detection: Record<string, unknown>,
  evidence: DetectionEvidenceDto[],
): Pick<ArtifactMark, "scoreSemantics" | "geometrySemantics" | "calibrationStatus" | "geometryReportId" | "geometryIssues"> {
  const quality = detection.quality && typeof detection.quality === "object"
    ? detection.quality as Record<string, unknown>
    : undefined;
  const geometry = quality?.geometry && typeof quality.geometry === "object"
    ? quality.geometry as Record<string, unknown>
    : undefined;
  const sourceCapability = evidence[0]?.source_capability ?? detection.source_capability;
  const conservativeGeometry = sourceCapability === "vision_language"
    ? "coarse_hypothesis"
    : typeof sourceCapability === "string" && (sourceCapability.includes("detect") || sourceCapability.includes("ground"))
      ? "predicted_geometry"
      : undefined;
  return {
    scoreSemantics: typeof (detection.score as Record<string, unknown> | undefined)?.semantics === "string"
      ? String((detection.score as Record<string, unknown>).semantics)
      : evidence[0]?.score.semantics,
    geometrySemantics: typeof geometry?.semantics === "string"
      ? geometry.semantics
      : typeof detection.geometry_semantics === "string"
        ? detection.geometry_semantics
        : conservativeGeometry,
    calibrationStatus: typeof geometry?.calibration_status === "string"
      ? geometry.calibration_status
      : typeof detection.calibration_status === "string"
        ? detection.calibration_status
        : "uncalibrated",
    geometryReportId: typeof geometry?.report_id === "string" ? geometry.report_id : undefined,
    geometryIssues: Array.isArray(detection.geometry_issue_codes)
      ? detection.geometry_issue_codes.filter((value): value is string => typeof value === "string")
      : [],
  };
}

function artifactVisualContext(project?: ProjectSummary) {
  const schemaVisuals: Record<string, LabelVisualMapping> = {};
  project?.annotation_schema.flatMap((task) => task.labels).forEach((label, index) => {
    schemaVisuals[label] = { slot: ((index % 8) + 1) as LabelVisualMapping["slot"] };
  });
  return {
    projectOverrides: project?.annotation_visuals as
      | Record<string, LabelVisualMapping>
      | undefined,
    skillProfiles: visualProfilesForSkills(project?.enabled_skills.map((skill) => skill.id) ?? []),
    schemaVisuals,
  };
}

function markColor(label: string, project?: ProjectSummary): string {
  const visual = annotationVisual(
    {
      id: "preview",
      image_id: "preview",
      task_id: label,
      label,
      value: { kind: "bounding_box", rect: [0, 0, 1, 1] },
      attributes: {},
      source: "model",
      review_status: "draft",
      provenance: {},
      created_at: "",
    },
    artifactVisualContext(project),
  );
  return annotationColor(visual.slot);
}

function detectionScoreValue(detection: Record<string, unknown>): number | undefined {
  const score = detection.score;
  if (score && typeof score === "object") {
    const value = (score as Record<string, unknown>).value;
    return typeof value === "number" ? value : undefined;
  }
  return typeof detection.confidence === "number" ? detection.confidence : undefined;
}

function parseDetectionEvidence(value: unknown): DetectionEvidenceDto[] {
  if (!Array.isArray(value)) return [];
  return value.flatMap((item) => {
    if (!item || typeof item !== "object") return [];
    const evidence = item as Record<string, unknown>;
    const rect = parseArtifactRect(evidence.bbox);
    if (!rect || typeof evidence.source_model_id !== "string") return [];
    const score = evidence.score && typeof evidence.score === "object"
      ? evidence.score as Record<string, unknown>
      : {};
    return [{
      source_model_id: evidence.source_model_id,
      source_model_display_name: typeof evidence.source_model_display_name === "string" ? evidence.source_model_display_name : undefined,
      source_artifact_id: typeof evidence.source_artifact_id === "string" ? evidence.source_artifact_id : "unknown",
      bbox: [rect.x, rect.y, rect.width, rect.height],
      score: {
        value: typeof score.value === "number" ? score.value : undefined,
        semantics: typeof score.semantics === "string" ? score.semantics as DetectionEvidenceDto["score"]["semantics"] : "unknown",
      },
      query_id: typeof evidence.query_id === "string" ? evidence.query_id : undefined,
      model_label: typeof evidence.model_label === "string" ? evidence.model_label : undefined,
      project_label: typeof evidence.project_label === "string" ? evidence.project_label : undefined,
      source_capability: typeof evidence.source_capability === "string" ? evidence.source_capability : "object_detection",
      raw_output_ref: evidence.raw_output_ref as DetectionEvidenceDto["raw_output_ref"],
    }];
  });
}

function sourceModelLabel(
  evidence: DetectionEvidenceDto,
  models: ModelBinding[] = [],
): string {
  return evidence.source_model_display_name ??
    models.find((model) =>
      model.id === evidence.source_model_id ||
      model.model === evidence.source_model_id,
    )?.model ??
    evidence.source_model_id.replaceAll("_", " ").replaceAll("-", " ");
}

function evidenceIdentity(item: DetectionEvidenceDto): string {
  return `${item.source_model_id}:${item.bbox.join(",")}`;
}

function uniqueEvidence(items: DetectionEvidenceDto[]): DetectionEvidenceDto[] {
  return items.filter((item, index) =>
    items.findIndex((candidate) => evidenceIdentity(candidate) === evidenceIdentity(item)) === index,
  );
}

function artifactMarkSummary(mark: ArtifactMark): string {
  const sources = uniqueEvidence(mark.evidence);
  if (typeof mark.agreement === "object")
    return `${sources.length} models agree · IoU ${mark.agreement.multi_source_agreement.minimum_iou.toFixed(2)}`;
  if (mark.agreement === "geometry_conflict") return `${sources.length} models disagree on location`;
  if (mark.agreement === "label_conflict") return `${sources.length} models disagree on label`;
  const source = sources[0];
  if (!source) return mark.confidence === undefined ? "Bounding box" : `Bounding box · ${Math.round(mark.confidence * 100)}%`;
  return source.score.value == null
    ? `${sourceModelLabel(source)} · score not provided`
    : `${sourceModelLabel(source)} · ${scoreSemanticsLabel(source.score.semantics)} ${source.score.value.toFixed(2)}`;
}

export function artifactDetectionMarks(
  artifacts: PipelineArtifact[],
  project?: ProjectSummary,
): ArtifactMark[] {
  return artifacts.flatMap((artifact) => {
    if (artifact.kind !== "detection_set" && artifact.kind !== "candidate_cluster_set") return [];
    const clusterSet = artifact.kind === "candidate_cluster_set";
    const detections = clusterSet ? artifact.artifact.candidates : artifact.artifact.detections;
    const reference = artifact.artifact.reference as Record<string, unknown> | undefined;
    if (!Array.isArray(detections)) return [];
    return detections.flatMap((value, index) => {
      if (!value || typeof value !== "object") return [];
      const detection = value as Record<string, unknown>;
      const rect = parseArtifactRect(detection.representative_bbox ?? detection.bbox ?? detection.rect);
      if (!rect) return [];
      const label = typeof detection.target_label === "string"
        ? detection.target_label
        : typeof detection.project_label === "string"
        ? detection.project_label
        : typeof detection.model_label === "string"
          ? detection.model_label
          : typeof detection.label === "string"
            ? detection.label
            : typeof detection.class_id === "string"
              ? detection.class_id
              : "detection";
      const evidence = parseDetectionEvidence(detection.members ?? detection.evidence);
      if (!evidence.length && typeof detection.source_model_id === "string") {
        evidence.push({
          source_model_id: detection.source_model_id,
          source_artifact_id: typeof reference?.artifact_id === "string" ? reference.artifact_id : "unknown",
          bbox: [rect.x, rect.y, rect.width, rect.height],
          score: {
            value: detectionScoreValue(detection),
            semantics: detection.score && typeof detection.score === "object" &&
              typeof (detection.score as Record<string, unknown>).semantics === "string"
              ? (detection.score as Record<string, unknown>).semantics as DetectionEvidenceDto["score"]["semantics"]
              : "unknown",
          },
          query_id: typeof detection.query_id === "string" ? detection.query_id : undefined,
          model_label: typeof detection.model_label === "string" ? detection.model_label : undefined,
          project_label: label,
          source_capability: typeof detection.source_capability === "string" ? detection.source_capability : "object_detection",
        });
      }
      const geometryState = geometryStateFromDetection(detection, evidence);
      return [{
        ...rect,
        id: typeof detection.detection_id === "string"
          ? detection.detection_id
          : typeof detection.id === "string"
            ? detection.id
            : `detection-${index}`,
        label,
        confidence: detectionScoreValue(detection),
        color: markColor(label, project),
        parentArtifact: typeof reference?.artifact_id === "string" ? reference.artifact_id : undefined,
        sourceNode: typeof reference?.source_node === "string" ? reference.source_node : undefined,
        evidence,
        ...geometryState,
        agreement: clusterSet ? detection.agreement as ArtifactMark["agreement"] : undefined,
      }];
    });
  });
}

export function artifactCropMarks(
  artifacts: PipelineArtifact[],
  detections: ArtifactMark[],
): ArtifactMark[] {
  return artifacts.flatMap((artifact) => {
    if (artifact.kind !== "crop_set") return [];
    const crops = artifact.artifact.crops;
    const reference = artifact.artifact.reference as Record<string, unknown> | undefined;
    if (!Array.isArray(crops)) return [];
    return crops.flatMap((value, index) => {
      if (!value || typeof value !== "object") return [];
      const crop = value as Record<string, unknown>;
      const rect = parseArtifactRect(crop.rect);
      if (!rect) return [];
      const parent = crop.parent as Record<string, unknown> | undefined;
      const parentId = typeof parent?.item_id === "string" ? parent.item_id : undefined;
      const detection = detections.find((item) => item.id === parentId);
      return [{
        ...rect,
        id: typeof crop.id === "string" ? crop.id : `crop-${index}`,
        parentId,
        label: detection?.label ?? "crop",
        confidence: detection?.confidence,
        color: detection?.color ?? markColor("crop"),
        parentArtifact: typeof parent?.artifact_id === "string" ? parent.artifact_id : undefined,
        sourceNode: typeof reference?.source_node === "string" ? reference.source_node : undefined,
        evidence: detection?.evidence ?? [],
        agreement: detection?.agreement,
      }];
    });
  });
}

export function annotationDetectionMarks(
  annotations: Annotation[],
  project?: ProjectSummary,
): ArtifactMark[] {
  return annotations.flatMap((annotation) => {
    if (annotation.value.kind !== "bounding_box") return [];
    const [x, y, width, height] = annotation.value.rect;
    const label = annotation.label ?? annotation.task_id;
    return [{
      x,
      y,
      width,
      height,
      id: annotation.id,
      label,
      confidence: annotation.confidence,
      color: markColor(label, project),
      sourceNode: "committed annotation",
      evidence: [],
      scoreSemantics: typeof annotation.provenance.score_semantics === "string"
        ? annotation.provenance.score_semantics
        : annotation.confidence === undefined ? "not_provided" : "unknown",
      geometrySemantics: annotation.review_status === "human_accepted" || annotation.source === "human"
        ? "human_verified"
        : typeof annotation.provenance.geometry_semantics === "string"
          ? annotation.provenance.geometry_semantics
          : undefined,
      calibrationStatus: typeof annotation.provenance.geometry_calibration_status === "string"
        ? annotation.provenance.geometry_calibration_status
        : annotation.review_status === "human_accepted" ? "passed" : "uncalibrated",
    }];
  });
}

function sameMark(left: ArtifactMark, right: ArtifactMark): boolean {
  return left.label === right.label
    && Math.abs(left.x - right.x) < 0.0001
    && Math.abs(left.y - right.y) < 0.0001
    && Math.abs(left.width - right.width) < 0.0001
    && Math.abs(left.height - right.height) < 0.0001;
}

function uniqueMarks(marks: ArtifactMark[]): ArtifactMark[] {
  return marks.reduce<ArtifactMark[]>((items, mark) => {
    const existing = items.find((candidate) => sameMark(candidate, mark));
    if (!existing) return [...items, mark];
    existing.evidence = uniqueEvidence([...existing.evidence, ...mark.evidence]);
    existing.agreement = existing.agreement ?? mark.agreement;
    existing.scoreSemantics ??= mark.scoreSemantics;
    existing.geometrySemantics ??= mark.geometrySemantics;
    existing.calibrationStatus ??= mark.calibrationStatus;
    existing.geometryReportId ??= mark.geometryReportId;
    existing.geometryIssues = [...new Set([...(existing.geometryIssues ?? []), ...(mark.geometryIssues ?? [])])];
    return items;
  }, []);
}

function RunArtifactCanvas({ projectId, project, artifacts, annotations, imageId }: { projectId: string; project?: ProjectSummary; artifacts: PipelineArtifact[]; annotations: Annotation[]; imageId: string }) {
  const imageUrl = `/api/projects/${projectId}/images/${encodeURIComponent(imageId)}/content`;
  const masks = artifactMasks(artifacts);
  const artifactDetections = uniqueMarks(artifactDetectionMarks(artifacts, project));
  const annotationDetections = annotationDetectionMarks(annotations, project);
  const detections = [
    ...artifactDetections,
    ...annotationDetections.filter((annotation) =>
      !artifactDetections.some((artifact) => sameMark(annotation, artifact))),
  ];
  const crops = uniqueMarks(artifactCropMarks(artifacts, detections));
  const [mode, setMode] = useState<"original" | "result" | "compare" | "crops">("result");
  const [zoom, setZoom] = useState(1);
  const [selectedId, setSelectedId] = useState(detections[0]?.id ?? crops[0]?.parentId ?? "");
  useEffect(() => {
    if (![...detections.map((item) => item.id), ...crops.map((item) => item.parentId)].includes(selectedId))
      setSelectedId(detections[0]?.id ?? crops[0]?.parentId ?? "");
  }, [artifacts, annotations]);
  const selectOffset = (offset: number) => {
    const ids = detections.map((item) => item.id);
    if (!ids.length) return;
    const current = Math.max(0, ids.indexOf(selectedId));
    setSelectedId(ids[(current + offset + ids.length) % ids.length]);
  };
  const legend = [...new Map(detections.map((item) => [item.label, item])).values()];
  const selectedMark = detections.find((item) => item.id === selectedId);
  const imageStage = (showResults: boolean, label: string) => <div className="canvas-pan"><div className="artifact-image-stage" style={{ transform: `scale(${zoom})` }}><img src={imageUrl} alt={label} />{showResults && masks.length > 0 && <ArtifactMaskLayer masks={masks} />}{showResults && <svg viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true" focusable="false">{detections.map((rect) => <g key={rect.id} focusable="false" className={rect.id === selectedId ? "selected" : ""} style={{ color: rect.color }} onMouseDown={(event) => event.preventDefault()} onClick={() => setSelectedId(rect.id)}><rect x={rect.x * 100} y={rect.y * 100} width={rect.width * 100} height={rect.height * 100} /><text x={rect.x * 100} y={Math.max(3, rect.y * 100 - 1)}>{rect.label}</text></g>)}</svg>}</div></div>;
  return (
    <div className="run-artifact-canvas" role="region" aria-label="Run result annotation viewer" onKeyDown={(event) => { if (!workspaceShortcutAllowed(event.nativeEvent, isTextEditingTarget(event.target), Boolean(document.querySelector('dialog[open], [role="dialog"]'))) || event.metaKey || event.ctrlKey || event.altKey) return; if (event.key === "ArrowRight" || event.key === "ArrowDown") { event.preventDefault(); selectOffset(1); } if (event.key === "ArrowLeft" || event.key === "ArrowUp") { event.preventDefault(); selectOffset(-1); } }}>
      <div className="preview-toggle">
        <button className={mode === "original" ? "active" : ""} onClick={() => setMode("original")}>{t("Original")}</button>
        <button className={mode === "result" ? "active" : ""} onClick={() => setMode("result")}>{t("Result")}</button>
        <button className={mode === "compare" ? "active" : ""} onClick={() => setMode("compare")}>{t("Compare")}</button>
        <button className={mode === "crops" ? "active" : ""} disabled={!crops.length} onClick={() => setMode("crops")}>Crop ({crops.length})</button>
        <label className="preview-zoom-control">
          <span>{t("Zoom")}</span>
          <input aria-label={t("Preview zoom")} type="range" min="1" max="3" step="0.25" value={zoom} onChange={(event) => setZoom(Number(event.target.value))} />
          <output aria-live="polite">{Math.round(zoom * 100)}%</output>
        </label>
      </div>
      {(legend.length > 0 || masks.length > 0) && <div className="bbox-legend" aria-label={t("Annotation color legend")}>{legend.map((item) => <span key={item.label}><i style={{ background: item.color }} />{item.label}</span>)}{masks.length > 0 && <span><i className="mask-overlay-swatch" />Mask overlay · {masks.length}</span>}</div>}
      {detections.length > 0 && <ul className="canvas-annotation-list" aria-label={t("Run result annotations")}>{detections.map((item) => <li key={item.id}><button aria-pressed={item.id === selectedId} onClick={() => setSelectedId(item.id)}><i aria-hidden="true" style={{ borderColor: item.color }} /><span><strong>{item.label}</strong><small>{artifactMarkSummary(item)}</small></span></button></li>)}</ul>}
      {selectedMark && <section className="geometry-quality-facts" aria-label={t("Semantic and geometry quality")}>
        <article><span>{scoreSemanticsLabel(selectedMark.scoreSemantics)}</span><strong>{selectedMark.confidence === undefined ? t("Not provided") : selectedMark.confidence.toFixed(2)}</strong><small>{t("This score describes model belief, not box geometry.")}</small></article>
        <article><span>{t("Box quality")}</span><strong>{geometrySemanticsLabel(selectedMark.geometrySemantics)}</strong><small>{t("A model score is not box IoU or tightness.")}</small></article>
        <article className={selectedMark.calibrationStatus === "passed" || selectedMark.geometrySemantics === "human_verified" ? "verified" : "needs-check"}><span>{t("Geometry verification")}</span><strong>{selectedMark.geometrySemantics === "human_verified" ? t("Verified by a reviewer") : selectedMark.calibrationStatus === "passed" ? t("Project calibration passed") : t("Not performed")}</strong><small>{selectedMark.geometryReportId ? `Quality report ${selectedMark.geometryReportId.slice(0, 8)}` : `${(selectedMark.calibrationStatus ?? "uncalibrated").replaceAll("_", " ")} · review or measured evidence required`}</small></article>
        {selectedMark.geometryIssues?.length ? <article className="needs-check"><span>{t("Geometry issues")}</span><strong>{selectedMark.geometryIssues.map((issue) => issue.replaceAll("_", " ")).join(", ")}</strong></article> : null}
      </section>}
      {selectedMark?.evidence.length ? <section className="evidence-inspector" aria-label="Detection evidence inspector">
        <header><span className="eyebrow">{t("Evidence inspector")}</span><strong>{artifactMarkSummary(selectedMark)}</strong></header>
        <div>{uniqueEvidence(selectedMark.evidence).map((item) => <article key={evidenceIdentity(item)}>
          <span><strong>{sourceModelLabel(item)}</strong><small>{item.source_capability.replaceAll("_", " ")}</small></span>
          <span><strong>{item.score.value == null ? t("Score not provided") : item.score.value.toFixed(2)}</strong><small>{scoreSemanticsLabel(item.score.semantics)}</small></span>
          <code>[{item.bbox.map((value) => value.toFixed(3)).join(", ")}]</code>
          {(item.query_id || item.model_label) && <small>{item.query_id ? `Query · ${item.query_id}` : ""}{item.query_id && item.model_label ? " · " : ""}{item.model_label ? `Model label · ${item.model_label}` : ""}</small>}
        </article>)}</div>
      </section> : null}
      {mode === "original" ? imageStage(false, "Original Run input") : mode === "result" ? imageStage(true, "Run result") : mode === "compare" ? <div className="run-result-compare"><section><span>{t("Original")}</span>{imageStage(false, "Original Run input")}</section><section><span>{t("Result")}</span>{imageStage(true, "Run result")}</section></div> : (
        <div className="crop-preview-list enlarged">{crops.map((crop, index) => <button className={crop.parentId === selectedId ? "selected" : ""} key={crop.id} onClick={() => setSelectedId(crop.parentId ?? crop.id)}><svg style={{ transform: `scale(${zoom})` }} viewBox={`${crop.x * 100} ${crop.y * 100} ${crop.width * 100} ${crop.height * 100}`} aria-label={`Crop ${index + 1}: ${crop.label}`}><image href={imageUrl} x="0" y="0" width="100" height="100" /></svg><span><strong>{crop.label}</strong>{crop.confidence !== undefined && <small>{Math.round(crop.confidence * 100)}%</small>}<small>Parent: {crop.parentArtifact?.slice(0, 8) ?? crop.parentId ?? t("Unknown")}</small><small>{t("Source:")}{" "}{crop.sourceNode ?? t("Unknown")}</small></span></button>)}</div>
      )}
    </div>
  );
}

const GENERIC_REVIEW_REASONS = [
  { value: "too_loose", label: "Too loose" },
  { value: "too_tight", label: "Too tight" },
  { value: "shifted", label: "Shifted" },
  { value: "wrong_object", label: "Wrong object" },
  { value: "missed_object", label: "Missed object" },
  { value: "duplicate", label: "Duplicate" },
  { value: "wrong_label", label: "Wrong label" },
  { value: "other", label: "Other" },
] as const;

function reviewReasonExplanation(item: ReviewItem) {
  if (item.review_explanation) return item.review_explanation.summary;
  if (item.review_reason === "low_confidence")
    return "The model confidence is below this Automation's acceptance threshold.";
  if (item.review_reason === "validation_issue")
    return `Validation needs a human decision${item.validation_issues.length ? `: ${item.validation_issues.join(", ").replaceAll("_", " ")}.` : "."}`;
  return "This Automation routes the result through a Human Review gate.";
}

function ReviewPage({
  project,
  projects,
  models,
  events,
  route,
  onNavigate,
  onNavigationGuardChange,
  onError,
}: {
  project?: ProjectSummary;
  projects: ProjectSummary[];
  models: ModelBinding[];
  events: RunEvent[];
  route: Extract<WorkspaceRoute, { kind: "review" | "projectReview" }>;
  onNavigate: (path: string, replace?: boolean) => boolean;
  onNavigationGuardChange: (guard?: () => boolean) => void;
  onError: (value: string) => void;
}) {
  const guidedReview = !!route.reviewItemId && route.view !== "audit";
  const [reviews, setReviews] = useState<ReviewItem[]>([]);
  const [queueLoaded, setQueueLoaded] = useState(false);
  const [nextReviewOffset, setNextReviewOffset] = useState<number | null>(null);
  const [progress, setProgress] = useState<ReviewQueueProgress>({
    reviewed_count: 0,
    total_count: 0,
    remaining_count: 0,
  });
  const [queueNavigation, setQueueNavigation] = useState<ReviewNavigation>();
  const [selectedId, setSelectedId] = useState(route.reviewItemId ?? "");
  const [draft, setDraft] = useState<Annotation>();
  const loadedReviewAnnotation = useRef<Annotation | undefined>(undefined);
  const [past, setPast] = useState<Annotation[]>([]);
  const [future, setFuture] = useState<Annotation[]>([]);
  const [isNew, setIsNew] = useState(false);
  const [editing, setEditing] = useState(false);
  const [decisionBusy, setDecisionBusy] = useState(false);
  const reviewMutationPending = useRef(false);
  const [rejectOpen, setRejectOpen] = useState(false);
  const [rejectReason, setRejectReason] = useState("wrong_object");
  const [completedProject, setCompletedProject] = useState<ProjectSummary>();
  const [compareMode, setCompareMode] = useState<"after" | "before" | "split">("after");
  const [inspectorCollapsed, setInspectorCollapsed] = useState(() =>
    !route.reviewItemId || window.localStorage.getItem("annotagent.reviewInspectorCollapsed") !== "false",
  );
  const [queueOpen, setQueueOpen] = useState(!route.reviewItemId);
  const [attributesText, setAttributesText] = useState("{}");
  const [reason, setReason] = useState("");
  const [skillReasonOptions, setSkillReasonOptions] = useState<
    { value: string; label: string; skillId: string }[]
  >([]);
  const [correctionSkillId, setCorrectionSkillId] = useState("");
  const [note, setNote] = useState("");
  const [revisionHistory, setRevisionHistory] = useState<AnnotationRevision[]>([]);
  const [revisionHistoryOpen, setRevisionHistoryOpen] = useState(false);
  const [revisionHistoryLoading, setRevisionHistoryLoading] = useState(false);
  const [images, setImages] = useState<ImageItem[]>([]);
  const queueLoadGeneration = useRef(0);
  const detailResponse = useRef<{sequence:number;id?:string}>({sequence:0});
  const itemLoadGeneration = useRef(0);
  const reviewProgressRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!rejectOpen) return;
    const previousFocus = document.activeElement;
    document.querySelector<HTMLSelectElement>(".review-reject-panel select")?.focus();
    return () => {
      if (previousFocus instanceof HTMLElement && previousFocus.isConnected) previousFocus.focus({ preventScroll: true });
      else reviewProgressRef.current?.focus({ preventScroll: true });
    };
  }, [rejectOpen]);
  useEffect(() => setSelectedId(route.reviewItemId ?? ""), [route.reviewItemId]);
  const routeReview = route.reviewItemId
    ? reviews.find((review) => review.id === route.reviewItemId)
    : undefined;
  const scopedProject = route.projectId
    ? projects.find((candidate) => candidate.id === route.projectId) ?? project
    : undefined;
  const routeReviewProject = projectForReview(projects, routeReview);
  const visibleReviews = scopedProject
    ? reviews.filter(
        (review) => review.project_id === scopedProject.project_id,
      )
    : reviews;
  const selected = route.reviewItemId
    ? routeReview &&
      (!scopedProject || routeReview.project_id === scopedProject.project_id)
      ? routeReview
      : undefined
    : visibleReviews.find((review) => review.id === selectedId) ??
      visibleReviews[0];
  const reviewProject = projectForReview(projects, selected) ?? scopedProject;
  const reviewHref = (reviewId: string, item?: ReviewItem) => {
    const owner = projectForReview(
      projects,
      item ?? reviews.find((review) => review.id === reviewId),
    );
    if (owner) return projectReviewPath(owner.id, reviewId, route.view);
    if (route.projectId) return projectReviewPath(route.projectId, reviewId, route.view);
    return `/review/${encodeURIComponent(reviewId)}`;
  };
  useEffect(() => {
    if (
      route.reviewItemId &&
      routeReviewProject &&
      (route.kind !== "projectReview" ||
        route.projectId !== routeReviewProject.id)
    )
      onNavigate(
        projectReviewPath(routeReviewProject.id, route.reviewItemId, route.view),
        true,
      );
  }, [route.kind, route.reviewItemId, route.projectId, routeReviewProject?.id]);
  const refresh = (offset = 0, append = false) => {
    const generation = ++queueLoadGeneration.current;
    const priorDetailSequence = detailResponse.current.sequence;
    const key = `${queryKeys.reviewQueue(route.projectId)}:${offset}`;
    if (!append) setQueueLoaded(false);
    return workspaceQueries
      .load(key, (signal) => api.reviews(route.projectId, signal, offset), { force: true })
      .then((value) => {
        if (generation !== queueLoadGeneration.current) return;
        const newerDetailId=detailResponse.current.sequence!==priorDetailSequence?detailResponse.current.id:undefined;
        setReviews((current) => mergeReviewQueue(current,value.reviews,{append,keepId:route.reviewItemId,newerDetailId}));
        setProgress(value.progress);
        setNextReviewOffset(value.page.next_offset);
      })
      .catch((error: Error) => {
        if (generation === queueLoadGeneration.current && !isAbortError(error)) onError(error.message);
      })
      .finally(() => {
        if (generation === queueLoadGeneration.current) setQueueLoaded(true);
      });
  };
  const detailReviewId = route.reviewItemId || selectedId || visibleReviews[0]?.id;
  useEffect(() => {
    if (!detailReviewId) return;
    const reviewId = detailReviewId;
    const generation = ++itemLoadGeneration.current;
    const key = queryKeys.review(reviewId, route.projectId);
    void workspaceQueries
      .load(
        key,
        (signal) => api.review(
          reviewId,
          signal,
          route.kind === "projectReview" ? route.projectId : undefined,
        ),
        { force: true },
      )
      .then((review) => {
        if (generation !== itemLoadGeneration.current) return;
        detailResponse.current={sequence:detailResponse.current.sequence+1,id:review.id};
        setReviews((items) => items.some((item) => item.id === review.id)
          ? items.map((item) => item.id === review.id ? review : item)
          : [review, ...items]);
      })
      .catch((error: Error) => {
        if (generation === itemLoadGeneration.current && !isAbortError(error)) onError(error.message);
      });
    return () => { itemLoadGeneration.current += 1; workspaceQueries.abort(key); };
  }, [route.kind, route.projectId, detailReviewId]);
  useEffect(() => {
    void refresh();
    const key = `${queryKeys.reviewQueue(route.projectId)}:0`;
    return () => { queueLoadGeneration.current += 1; workspaceQueries.abort(key); };
  }, [route.projectId]);
  useEffect(() => {
    if (!selected?.id) {
      setQueueNavigation(undefined);
      return;
    }
    let current = true;
    void api
      .reviewNext(selected.id, route.projectId)
      .then((value) => {
        if (!current) return;
        setQueueNavigation(value);
        setProgress(value.progress);
      })
      .catch((error: Error) => { if (current) onError(error.message); });
    return () => { current = false; };
  }, [selected?.id, route.projectId]);
  useEffect(() => {
    let current = true;
    void api
      .skills()
      .then((skills) => {
        if (!current) return;
        const ids = reviewProject?.enabled_skills.map((skill) => skill.id) ?? [];
        const enabled = skills.filter((skill) => ids.includes(skill.id));
        const options = enabled.flatMap((skill) => skill.correction_taxonomy.map((value) => ({
          value,
          label: value.replaceAll("_", " "),
          skillId: skill.id,
        })));
        setCorrectionSkillId(
          selected?.source_skill_id && ids.includes(selected.source_skill_id)
            ? selected.source_skill_id
            : enabled[0]?.id ?? "",
        );
        setSkillReasonOptions(options);
        setReason("other");
      })
      .catch((error: Error) => { if (current) onError(error.message); });
    return () => { current = false; };
  }, [reviewProject?.id, selected?.source_skill_id]);
  useEffect(() => {
    let current = true;
    setImages([]);
    if (reviewProject) {
      const key = queryKeys.projectImages(reviewProject.id);
      void workspaceQueries
        .load(key, (signal) => api.images(reviewProject.id, signal), { staleTime: 30_000 })
        .then((value) => { if (current) setImages(value.images); })
        .catch((error: Error) => {
          if (current && !isAbortError(error)) onError(error.message);
        });
      return () => { current = false; workspaceQueries.abort(key); };
    }
    setImages([]);
  }, [reviewProject?.id]);
  useEffect(() => {
    const previous = loadedReviewAnnotation.current;
    if (!canRefreshReviewDraft(previous, selected?.annotation, draft, attributesText, Boolean(note.trim() || rejectOpen))) return;
    loadedReviewAnnotation.current = selected?.annotation;
    setDraft(selected?.annotation);
    setAttributesText(JSON.stringify(selected?.annotation.attributes ?? {}, null, 2));
    setPast([]);
    setFuture([]);
    setIsNew(false);
    setEditing(false);
    setRejectOpen(false);
    setRejectReason("wrong_object");
    setReason("other");
    setNote("");
    setCorrectionSkillId("");
    setRevisionHistory([]);
    setRevisionHistoryOpen(false);
  }, [selected?.id, selected?.annotation]);
  const beginEdit = () => {
    if (!draft) return;
    setPast((items) => [...items, structuredClone(draft)]);
    setFuture([]);
  };
  const edit = (next: Annotation) => {
    beginEdit();
    setDraft(next);
  };
  const useEvidenceBox = (evidence: DetectionEvidenceDto) => {
    if (!draft || draft.value.kind !== "bounding_box") return;
    edit({
      ...draft,
      value: { kind: "bounding_box", rect: evidence.bbox },
      provenance: {
        ...draft.provenance,
        selected_geometry_evidence: {
          source_model_id: evidence.source_model_id,
          source_capability: evidence.source_capability,
          score: evidence.score,
        },
      },
      attributes: {
        ...draft.attributes,
        selected_detection_evidence: evidence,
      },
    });
    setAttributesText(JSON.stringify({
      ...draft.attributes,
      selected_detection_evidence: evidence,
    }, null, 2));
    setEditing(true);
    setReason("shifted");
    if (!note.trim()) setNote(`Used the ${sourceModelLabel(evidence, models)} source box.`);
  };
  const undo = () => {
    const previous = past.at(-1);
    if (!previous || !draft) return;
    setFuture((items) => [structuredClone(draft), ...items]);
    setDraft(previous);
    setIsNew(previous.id !== selected?.annotation.id);
    setAttributesText(JSON.stringify(previous.attributes, null, 2));
    setPast((items) => items.slice(0, -1));
  };
  const redo = () => {
    const next = future[0];
    if (!next || !draft) return;
    setPast((items) => [...items, structuredClone(draft)]);
    setDraft(next);
    setIsNew(next.id !== selected?.annotation.id);
    setAttributesText(JSON.stringify(next.attributes, null, 2));
    setFuture((items) => items.slice(1));
  };
  useEffect(() => {
    const keyboard = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey) || event.key.toLowerCase() !== "z") return;
      if (!workspaceShortcutAllowed(event, isTextEditingTarget(event.target), Boolean(document.querySelector('dialog[open], [role="dialog"]')))) return;
      event.preventDefault();
      if (event.shiftKey) redo();
      else undo();
    };
    window.addEventListener("keydown", keyboard);
    return () => window.removeEventListener("keydown", keyboard);
  }, [draft, past, future]);
  const persistDraft = async (): Promise<boolean> => {
    if (!draft || !selected) return false;
    let attributes: Record<string, unknown>;
    try {
      attributes = JSON.parse(attributesText) as Record<string, unknown>;
    } catch {
      onError("Attributes must be a valid JSON object.");
      return false;
    }
    if (!attributes || Array.isArray(attributes) || typeof attributes !== "object") {
      onError("Attributes must be a JSON object.");
      return false;
    }
    const annotation = { ...draft, attributes };
    try {
      if (isNew) await api.createAnnotation(selected.run_id, annotation);
      else await api.revise(annotation, reason);
      const savedReview = await api.review(annotation.id, undefined, reviewProject?.id);
      loadedReviewAnnotation.current = savedReview.annotation;
      setReviews((items) => [...items.filter((item) => item.id !== savedReview.id), savedReview]);
      setDraft(savedReview.annotation);
      setAttributesText(JSON.stringify(savedReview.annotation.attributes ?? {}, null, 2));
      if (isNew) {
        onNavigationGuardChange();
        setSelectedId(annotation.id);
        onNavigate(reviewHref(annotation.id), true);
      }
      setIsNew(false);
      setPast([]);
      setFuture([]);
      await refresh();
      return true;
    } catch (error) {
      onError((error as Error).message);
      return false;
    }
  };
  const save = async () => {
    if (reviewMutationPending.current) return;
    reviewMutationPending.current = true; setDecisionBusy(true); onError("");
    try { await persistDraft(); }
    finally { reviewMutationPending.current = false; setDecisionBusy(false); }
  };
  const createShape = (kind: "bounding_box" | "keypoints" | "polyline" | "polygon") => {
    if (!selected) return onError("Select a review item before creating an annotation.");
    const task = reviewProject?.annotation_schema.find((candidate) => candidate.kind === kind);
    if (!task)
      return onError(`This Project has no ${kind.replace("_", " ")} task.`);
    const label = task.labels[0];
    if (!label) return onError(`Task ${task.id} has no label configured.`);
    const value: Annotation["value"] =
      kind === "bounding_box"
        ? { kind, rect: [0.35, 0.35, 0.3, 0.3] }
        : kind === "keypoints"
          ? { kind, points: [{ name: "point", point: [0.5, 0.5], visible: true }] }
          : kind === "polyline"
            ? { kind, points: [[0.35, 0.5], [0.65, 0.5]] }
            : { kind, rings: [[[0.35, 0.35], [0.65, 0.35], [0.5, 0.65]]] };
    const annotation: Annotation = {
      ...structuredClone(selected.annotation),
      id: crypto.randomUUID(),
      task_id: task.id,
      label,
      value,
      attributes: {},
      confidence: undefined,
      source: "human",
      review_status: "needs_review",
      provenance: { tool_names: [], artifact_ids: [] },
      created_at: new Date().toISOString(),
    };
    if (draft) setPast((items) => [...items, structuredClone(draft)]);
    setFuture([]);
    setDraft(annotation);
    setAttributesText("{}");
    setIsNew(true);
    setEditing(true);
  };
  const hasUnsavedAnnotationChanges = Boolean(
    isNew ||
    (draft && selected && (
      JSON.stringify(draft) !== JSON.stringify(selected.annotation) ||
      attributesText !== JSON.stringify(selected.annotation.attributes ?? {}, null, 2)
    )),
  );
  useEffect(() => {
    const warnBeforeUnload = (event: BeforeUnloadEvent) => {
      if (!hasUnsavedAnnotationChanges) return;
      event.preventDefault();
      event.returnValue = "";
    };
    window.addEventListener("beforeunload", warnBeforeUnload);
    return () => window.removeEventListener("beforeunload", warnBeforeUnload);
  }, [hasUnsavedAnnotationChanges]);
  useEffect(() => {
    onNavigationGuardChange(() =>
      !hasUnsavedAnnotationChanges ||
      window.confirm("Discard unsaved Review changes and leave this result?"),
    );
    return () => onNavigationGuardChange();
  }, [hasUnsavedAnnotationChanges, onNavigationGuardChange]);
  const navigateFromReview = (path: string, replace?: boolean) =>
    onNavigate(path, replace);
  const moveQueueSelection = (item?: ReviewItem) => {
    if (!item) return;
    if (onNavigate(reviewHref(item.id, item))) setSelectedId(item.id);
  };
  const openRevisionHistory = () => {
    if (!draft || !reviewProject) return;
    setRevisionHistoryOpen(true);
    setRevisionHistoryLoading(true);
    void api
      .revisions(draft.id, reviewProject.id)
      .then(({ revisions }) => setRevisionHistory(revisions))
      .catch((error: Error) => onError(`Revision history: ${error.message}`))
      .finally(() => setRevisionHistoryLoading(false));
  };
  const decideAndAdvance = async (
    decision: "accept" | "reject",
    reasonCode: string,
  ) => {
    if (reviewMutationPending.current || decisionBusy || (selected && ["human_accepted", "rejected"].includes(selected.annotation.review_status) && !hasUnsavedAnnotationChanges)) return;
    if (!selected || !reviewProject) {
      onError("Select the Review item's Project before recording a decision.");
      return;
    }
    if (isNew) {
      onError("Create the new annotation before deciding the original result.");
      return;
    }
    reviewMutationPending.current = true;
    setDecisionBusy(true);
    onError("");
    try {
      if (hasUnsavedAnnotationChanges && !(await persistDraft())) return;
      const reasonSkill = skillReasonOptions.find((option) => option.value === reasonCode)?.skillId;
      const outcome = await api.decideAndNext(
        selected.id,
        reviewProject.id,
        decision,
        reasonCode,
        note,
        reasonSkill || selected.source_skill_id || correctionSkillId || undefined,
        route.projectId,
      );
      setCompletedProject(reviewProject);
      setProgress(outcome.progress);
      setRejectOpen(false);
      setEditing(false);
      loadedReviewAnnotation.current = outcome.annotation;
      setDraft(outcome.annotation);
      setAttributesText(JSON.stringify(outcome.annotation.attributes ?? {}, null, 2));
      setPast([]); setFuture([]);
      setReviews((items) => items.map((item) => item.id === selected.id ? { ...item, annotation: outcome.annotation } : item));
      onNavigationGuardChange();
      if (outcome.next_review) {
        setReviews((items) => [...items.filter((item) => item.id !== outcome.next_review!.id), outcome.next_review!]);
        setSelectedId(outcome.next_review.id);
        onNavigate(reviewHref(outcome.next_review.id, outcome.next_review), true);
      } else {
        // Keep the last saved result on its stable URL. Export is a separate action.
        setQueueNavigation(undefined);
      }
    } catch (error) {
      onError((error as Error).message);
    } finally {
      reviewMutationPending.current = false;
      setDecisionBusy(false);
    }
  };
  useEffect(() => {
    const keyboard = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (!workspaceShortcutAllowed(event, isTextEditingTarget(target), Boolean(document.querySelector('dialog[open], [role="dialog"]')))) return;
      if (target?.closest("input, textarea, select, button, [contenteditable='true']")) return;
      if (event.metaKey || event.ctrlKey || event.altKey || decisionBusy || !selected) return;
      const key = event.key.toLowerCase();
      if (key === "a") {
        event.preventDefault();
        void decideAndAdvance("accept", hasUnsavedAnnotationChanges ? reason : "accepted_as_is");
      } else if (key === "r") {
        event.preventDefault();
        setRejectOpen(true);
      } else if (key === "e") {
        event.preventDefault();
        setEditing(true);
        if (!guidedReview) setInspectorVisibility(false);
      } else if (event.key === " ") {
        event.preventDefault();
        setCompareMode((mode) => mode === "before" ? "after" : "before");
      } else if (event.key === "ArrowLeft") {
        event.preventDefault();
        moveQueueSelection(queueNavigation?.previous_review);
      } else if (event.key === "ArrowRight") {
        event.preventDefault();
        moveQueueSelection(queueNavigation?.next_review);
      }
    };
    window.addEventListener("keydown", keyboard);
    return () => window.removeEventListener("keydown", keyboard);
  }, [selected?.id, queueNavigation, decisionBusy, hasUnsavedAnnotationChanges, reason]);
  const visualContext = {
    skillProfiles: visualProfilesForSkills(
      reviewProject?.enabled_skills.map((skill) => skill.id) ?? [],
    ),
  };
  const availableShapeKinds = reviewProject?.annotation_schema
    .map((task) => task.kind)
    .filter((kind): kind is "bounding_box" | "keypoints" | "polyline" | "polygon" =>
      ["bounding_box", "keypoints", "polyline", "polygon"].includes(kind),
    ) ?? [];
  const shapeLabels = {
    bounding_box: "Box",
    keypoints: "Keypoint",
    polyline: "Polyline",
    polygon: "Polygon",
  } as const;
  const setInspectorVisibility = (collapsed: boolean) => {
    setInspectorCollapsed(collapsed);
    if (!collapsed) setQueueOpen(false);
    window.localStorage.setItem(
      "annotagent.reviewInspectorCollapsed",
      String(collapsed),
    );
  };
  const reviewScoreSemantics = selected?.detection_evidence[0]?.score.semantics ??
    (typeof selected?.annotation.provenance.score_semantics === "string"
      ? selected.annotation.provenance.score_semantics
      : "unknown");
  const reviewGeometrySemantics = selected?.annotation.review_status === "human_accepted" || selected?.annotation.source === "human"
    ? "human_verified"
    : typeof selected?.annotation.provenance.geometry_semantics === "string"
      ? selected.annotation.provenance.geometry_semantics
      : selected?.detection_evidence.some((evidence) => evidence.source_capability === "vision_language")
        ? "coarse_hypothesis"
        : selected?.detection_evidence.length
          ? "predicted_geometry"
          : undefined;
  const reviewCalibrationStatus = typeof selected?.annotation.provenance.geometry_calibration_status === "string"
    ? selected.annotation.provenance.geometry_calibration_status
    : reviewGeometrySemantics === "human_verified" ? "passed" : "uncalibrated";
  return (
    <section className={`review-layout${inspectorCollapsed || guidedReview ? " inspector-collapsed" : ""}${!queueOpen ? " queue-collapsed" : ""}`}>
      {queueOpen && <aside className="review-queue panel">
        <span className="eyebrow">{t("Human attention")}</span>
        <h2>{t("Review queue")}{" "}<b>{queueLoaded ? progress.remaining_count : "…"}</b>
        </h2>
        <label className="review-project-filter">{t("Project")}<select
            aria-label={t("Project filter")}
            value={scopedProject?.id ?? ""}
            onChange={(event) =>
              navigateFromReview(event.target.value
                ? projectReviewPath(event.target.value)
                : "/review")
            }
          >
            <option value="">{t("All projects")}</option>
            {projects.map((candidate) => (
              <option key={candidate.id} value={candidate.id}>{candidate.name}</option>
            ))}
          </select>
        </label>
        <div className="queue-items" aria-label={t("Annotations requiring review")}>
          {visibleReviews.map((review) => (<Fragment key={review.id}>
            <button
              key={review.id}
              aria-pressed={selected?.id === review.id}
              className={selected?.id === review.id ? "active" : ""}
              onClick={() => {
                moveQueueSelection(review);
              }}
            >
              <span aria-hidden="true">
                {review.image_index === undefined ? "–" : review.image_index + 1}
              </span>
              <span>
                <strong>
                  {review.annotation.label ?? review.annotation.task_id}
                </strong>
                <small>
                  {!project && `${review.project_name} · `}{t("Image")}{" "}{review.image_index === undefined ? "?" : review.image_index + 1} ·{" "}
                  {review.annotation.confidence === undefined ? t("No confidence") : `${Math.round(review.annotation.confidence * 100)}%`}
                </small>
              </span>
            </button>
            {!route.reviewItemId && <button className="text-button review-audit-entry" onClick={() => {
              const owner = projectForReview(projects, review);
              if (owner) navigateFromReview(projectReviewPath(owner.id, review.id, "audit"));
            }}>{t("Review audit and sources")} · {review.annotation.label ?? review.annotation.task_id}</button>}
            </Fragment>
          ))}
        </div>
        {nextReviewOffset !== null && (
          <button
            className="text-button"
            onClick={() => void refresh(nextReviewOffset, true)}
          >{t("Load more review items")}</button>
        )}
        {visibleReviews.length === 0 && (
          <Empty
            title={t("Queue is clear")}
            detail="Low confidence or conflicting evidence will route candidates here."
          />
        )}
      </aside>}
      <div className="review-center">
        <div ref={reviewProgressRef} tabIndex={-1} className="review-progress-header" aria-label={t("Review progress")} role="status">
          <div>
            <span className="eyebrow">{t("Inbox progress")}</span>
            <strong>{queueLoaded ? `${progress.reviewed_count} of ${progress.total_count} results reviewed` : t("Loading review progress…")}</strong>
            <small>{queueLoaded ? `${progress.remaining_count} remaining${progress.current_position ? ` · item ${progress.current_position}` : ""}` : t("Reading the persisted Project queue")}</small>
          </div>
          <div className="review-progress-navigation" aria-label={t("Review queue navigation")}>
            <button aria-expanded={queueOpen} onClick={() => { setQueueOpen(!queueOpen); if (!queueOpen) setInspectorVisibility(true); }}>{t(queueOpen ? "Hide review queue" : "Show review queue")}</button>
            <button aria-label={t("Previous review result")} disabled={!queueNavigation?.previous_review} onClick={() => moveQueueSelection(queueNavigation?.previous_review)}>←</button>
            <button aria-label={t("Next review result")} disabled={!queueNavigation?.next_review} onClick={() => moveQueueSelection(queueNavigation?.next_review)}>→</button>
          </div>
        </div>
        {selected && <div className="review-edit-toolbar" aria-label={t("Annotation editing controls")}>
          <button className={editing ? "active" : ""} aria-pressed={editing} onClick={() => { setEditing((value) => !value); if (!guidedReview) setInspectorVisibility(false); }}>{t("Edit")}{" "}<kbd>E</kbd></button>
          {(editing || guidedReview) && availableShapeKinds.length > 0 && (
            <details className="review-add-menu">
              <summary aria-label={t("Add annotation")}>
                <span className="review-add-icon" aria-hidden="true" />
                <span className="review-add-label">{t("Add")}</span>
                <span className="review-add-caret" aria-hidden="true" />
              </summary>
              <div role="menu" aria-label={t("Annotation types")}>
                {availableShapeKinds.map((kind) => (
                  <button
                    key={kind}
                    role="menuitem"
                    onClick={(event) => {
                      event.currentTarget.closest("details")?.removeAttribute("open");
                      createShape(kind);
                    }}
                  >
                    {shapeLabels[kind]}
                  </button>
                ))}
              </div>
            </details>
          )}
          {(past.length > 0 || future.length > 0) && (
            <div className="review-history-tools" aria-label={t("Edit history")}>
              <button
                onClick={undo}
                disabled={!past.length}
                aria-label={t("Undo annotation edit")}
                title={t("Undo (⌘Z)")}
              ><span aria-hidden="true">↶</span></button>
              <button
                onClick={redo}
                disabled={!future.length}
                aria-label={t("Redo annotation edit")}
                title={t("Redo (⇧⌘Z)")}
              ><span aria-hidden="true">↷</span></button>
            </div>
          )}
          <div className="review-view-controls" aria-label={t("Canvas view controls")}>
            <select
              aria-label={t("Canvas view")}
              value={compareMode}
              onChange={(event) => setCompareMode(event.target.value as typeof compareMode)}
            >
              <option value="after">{t("Result")}</option>
              <option value="before">{t("Original")}</option>
              <option value="split">{t("Compare")}</option>
            </select>
            {!guidedReview && <button
              className="details-toggle"
              onClick={() => setInspectorVisibility(!inspectorCollapsed)}
              aria-label={inspectorCollapsed ? t("Show details") : t("Hide details")}
              aria-expanded={!inspectorCollapsed}
            >{t("Details")}{" "}<span aria-hidden="true">{inspectorCollapsed ? "›" : "‹"}</span>
            </button>}
          </div>
        </div>}
        {guidedReview && editing && draft && <div className="journey-review-edit" aria-label={t("Annotation edit details")}>
          <label>{t(draft.value.kind === "classification" ? "Categories (comma-separated)" : "Label")}<input value={reviewLabelText(draft)} disabled={decisionBusy} onChange={(event) => edit(withReviewLabel(draft, event.target.value))} /></label>
          <label>{t("Correction reason")}<select value={reason} disabled={decisionBusy} onChange={(event) => setReason(event.target.value)}>{GENERIC_REVIEW_REASONS.map((option) => <option key={option.value} value={option.value}>{t(option.label)}</option>)}</select></label>
          {hasUnsavedAnnotationChanges && <p>{t(draft.value.kind === "classification" ? "This category correction will be saved in the annotation revision history. It is not a geometry-quality measurement." : "This correction will be saved as geometry-quality evidence for calibration and future Automation improvements.")}</p>}
        </div>}
        {selected && <div className="review-canvas-risk"><strong>{t(["human_accepted", "rejected"].includes(selected.annotation.review_status) ? "Decision saved" : "Why this needs review")}</strong><p>{["human_accepted", "rejected"].includes(selected.annotation.review_status) ? t(selected.annotation.review_status) : reviewReasonExplanation(selected)}</p><small>{t("Current decision applies to this object, not every object in the image.")}</small></div>}
        {selected ? <div
          className={`review-canvas-stage${compareMode === "split" ? " review-canvas-compare" : ""}`}
        >
          {(compareMode === "before" || compareMode === "split") && (
            <div>{compareMode === "split" && <small>{t("Original")}</small>}<AnnotationCanvas
              compactList
              imageUrl={images.find((image) => image.image_id === selected.annotation.image_id)?.url}
              annotations={[]}
              readOnly
              selectedId={selected?.annotation.id}
              visualContext={visualContext}
              onSelect={() => undefined}
              onChange={() => undefined}
            /></div>
          )}
          {(compareMode === "after" || compareMode === "split") && (
            <div>{compareMode === "split" && <small>{t("Result")}</small>}<AnnotationCanvas
              compactList
              imageUrl={images.find((image) => image.image_id === selected.annotation.image_id)?.url}
              annotations={draft ? [draft] : []}
              selectedId={draft?.id}
              readOnly={decisionBusy}
              visualContext={visualContext}
              onSelect={() => undefined}
              onEditStart={() => { setEditing(true); setReason((value) => value || "shifted"); beginEdit(); }}
              onChange={setDraft}
            /></div>
          )}
        </div> : queueLoaded ? <section className="review-complete panel">
          <span className="eyebrow">{t("Inbox complete")}</span>
          <h2>{progress.total_count > 0 ? t("Review complete") : t("Nothing needs review")}</h2>
          <p>{progress.total_count > 0 ? `All ${progress.total_count} queued results have a human decision.` : t("Uncertain results will appear here when an Automation routes them to Human Review.")}</p>
          {(completedProject ?? scopedProject) && progress.total_count > 0 && <button className="primary" onClick={() => navigateFromReview(`/projects/${encodeURIComponent((completedProject ?? scopedProject)!.id)}/export`)}>{t("Continue to export")}</button>}
        </section> : <section className="review-complete panel" aria-busy="true">
          <span className="eyebrow">{t("Review inbox")}</span>
          <h2>{t("Loading review results…")}</h2>
          <p>{t("Reading the persisted queue and human decisions.")}</p>
        </section>}
        <div className="review-footer-stack">
          {rejectOpen && selected && <section className="review-reject-panel" role="dialog" aria-labelledby="reject-review-title">
            <div>
              <span className="eyebrow">{t("Reject result")}</span>
              <h3 id="reject-review-title">{t("Why is this result incorrect?")}</h3>
              <p>{t("A reason is required before the result leaves the Inbox.")}</p>
            </div>
            <label>{t("Reason")}<select aria-label={t("Reject reason")} value={rejectReason} onChange={(event) => setRejectReason(event.target.value)}>
                <optgroup label={t("Common reasons")}>
                  {GENERIC_REVIEW_REASONS.map((option) => <option key={option.value} value={option.value}>{t(option.label)}</option>)}
                </optgroup>
                {skillReasonOptions.length > 0 && <optgroup label={t("Enabled Skill reasons")}>
                  {skillReasonOptions.map((option) => <option key={`${option.skillId}:${option.value}`} value={option.value}>{option.label}</option>)}
                </optgroup>}
              </select>
            </label>
            <label>{t("Note")}{" "}{rejectReason === "other" ? "(required)" : "(optional)"}
              <textarea value={note} onChange={(event) => setNote(event.target.value)} placeholder={t("Add useful context for this decision")} />
            </label>
            <div className="button-row">
              <button onClick={() => setRejectOpen(false)}>{t("Cancel")}</button>
              <button className="danger" disabled={decisionBusy || (rejectReason === "other" && !note.trim())} onClick={() => void decideAndAdvance("reject", rejectReason)}>{decisionBusy ? t("Rejecting…") : t("Reject & next")}</button>
            </div>
          </section>}
          {draft && selected && !rejectOpen && (!(["human_accepted", "rejected"].includes(draft.review_status)) || hasUnsavedAnnotationChanges) && (
            <div className="review-action-bar" aria-label={t("Review decision controls")}>
              <span className="review-shortcuts" aria-label={t("Keyboard shortcuts")}><kbd>A</kbd>{" "}{t("accept")}{" "}<kbd>R</kbd>{" "}{t("reject")}{" "}<kbd>Space</kbd>{" "}{t("original/result")}</span>
              {editing && hasUnsavedAnnotationChanges && (
                <button disabled={decisionBusy} onClick={()=>void save()}>{decisionBusy ? t("Saving…") : isNew ? t("Create annotation") : t("Save changes")}</button>
              )}
              <button onClick={() => setRejectOpen(true)} disabled={decisionBusy}>{t("Reject & next")}</button>
              <button className="primary" disabled={decisionBusy || isNew} onClick={() => void decideAndAdvance("accept", hasUnsavedAnnotationChanges ? reason : "accepted_as_is")} aria-label={t("Accept and next")}>{decisionBusy ? t("Saving decision…") : t("Accept & next")}</button>
            </div>
          )}
          {draft && ["human_accepted", "rejected"].includes(draft.review_status) && progress.remaining_count === 0 && <section className="review-saved-result">
            <h2>{t("Review complete")}</h2><p role="status">{t("Your last decision is saved. You can keep inspecting this image or export the confirmed results.")}</p>
            {reviewProject && <button className="primary" onClick={() => navigateFromReview(`/projects/${encodeURIComponent(reviewProject.id)}/export`)}>{t("Continue to export")}</button>}
          </section>}
        </div>
      </div>
      {!guidedReview && !inspectorCollapsed && <aside className="inspector panel review-inspector">
        <div className="review-inspector-header">
          <div>
            <span className="eyebrow">{t("Review details")}</span>
            <h2>{draft?.label ?? t("No selection")}</h2>
          </div>
        </div>
        {draft && selected && (
          <>
            <div className="review-reason-summary">
              <span className="eyebrow">{t("Review details")}</span>
              <h3>{selected.review_explanation?.title ?? t("Needs review")}</h3>
              {selected.review_explanation?.details.length ? <ul>{selected.review_explanation.details.map((detail) => <li key={detail}>{detail}</li>)}</ul> : null}
            </div>
            <dl className="review-essential-facts">
              <div><dt>{scoreSemanticsLabel(reviewScoreSemantics)}</dt><dd>{(selected.confidence ?? draft.confidence) === undefined ? t("Not provided") : `${Math.round((selected.confidence ?? draft.confidence ?? 0) * 100)}%`}</dd></div>
              <div><dt>{t("Box quality")}</dt><dd>{geometrySemanticsLabel(reviewGeometrySemantics)}</dd></div>
              <div><dt>{t("Geometry verification")}</dt><dd>{reviewGeometrySemantics === "human_verified" ? t("Human verified") : reviewCalibrationStatus === "passed" ? t("Project calibration passed") : t("Needs geometry check")}</dd></div>
              <div><dt>{t("Source Run")}</dt><dd>{selected.run_id.slice(0, 8)}</dd></div>
              <div><dt>{t("Automation Version")}</dt><dd>{selected.workflow_id ? `${selected.workflow_id}@v${selected.workflow_version}` : `v${selected.workflow_version}`}</dd></div>
              <div><dt>{t("Source Step")}</dt><dd>{selected.source_node ?? t("Unknown")}</dd></div>
            </dl>
            {selected.detection_evidence?.length ? <section className="review-evidence" aria-label={t("Source model evidence")}>
              <header><span className="eyebrow">{t("Source evidence")}</span><strong>{uniqueEvidence(selected.detection_evidence).length} detector result{uniqueEvidence(selected.detection_evidence).length === 1 ? "" : t("s")}</strong></header>
              <div>{uniqueEvidence(selected.detection_evidence).map((evidence) => <article key={evidenceIdentity(evidence)}>
                <span><strong>{sourceModelLabel(evidence, models)}</strong><small>{evidence.source_capability.replaceAll("_", " ")}</small></span>
                <span><strong>{evidence.score.value == null ? t("Score not provided") : evidence.score.value.toFixed(2)}</strong><small>{scoreSemanticsLabel(evidence.score.semantics)}</small></span>
                <code>[{evidence.bbox.map((value) => value.toFixed(3)).join(", ")}]</code>
                <button onClick={() => useEvidenceBox(evidence)}>Use {sourceModelLabel(evidence, models)} box</button>
              </article>)}</div>
              {uniqueEvidence(selected.detection_evidence).length > 1 && <button onClick={() => setEditing(true)}>{t("Merge manually")}</button>}
            </section> : null}
            <button onClick={() => navigateFromReview(reviewProject ? projectRunPath(reviewProject.id, selected.run_id, { nodeId: selected.source_node, artifactId: selected.source_artifact_id, view: "debug" }) : `/runs/${encodeURIComponent(selected.run_id)}`)}>{t("Open run context")}</button>
            {reviewProject && <button onClick={() => navigateFromReview(`/projects/${encodeURIComponent(reviewProject.id)}/build/pipeline`)}>{t("Improve automation")}</button>}
            {editing && <section className="review-edit-details" aria-label={t("Annotation edit details")}>
              <div><span className="eyebrow">{t("Manual correction")}</span><strong>{t("Edit result")}</strong></div>
                <label>{t(draft.value.kind === "classification" ? "Categories (comma-separated)" : "Label")}<input value={reviewLabelText(draft)} disabled={decisionBusy} onChange={(event) => edit(withReviewLabel(draft, event.target.value))} />
              </label>
              <label>{t("Correction reason")}<select aria-label={t("Correction reason")} value={reason} onChange={(event) => setReason(event.target.value)}>
                  {GENERIC_REVIEW_REASONS.map((option) => <option key={option.value} value={option.value}>{t(option.label)}</option>)}
                  {skillReasonOptions.map((option) => <option key={`${option.skillId}:${option.value}`} value={option.value}>{option.label}</option>)}
                </select>
              </label>
              <label>{t("Reviewer note")}<textarea value={note} onChange={(event) => setNote(event.target.value)} placeholder={t("What changed, and why?")} />
              </label>
              {hasUnsavedAnnotationChanges && <div className="correction-impact" role="status">
                <strong>{t("Correction impact")}</strong>
                <span>This correction will be saved as geometry-quality evidence for calibration and future Automation improvements.</span>
              </div>
              }
            </section>}
            <details className="review-execution-details">
              <summary>{t("Execution details")}</summary>
              <div className="fact-grid">
                <Fact label={t("Refinement")} value={selected.refinement_chain?.map((refiner) => refiner.replaceAll("_", " ")).join(" → ") || "None recorded"} />
                <Fact label={t("Validation issue")} value={selected.validation_issues.join(", ") || "None"} />
                <Fact label={t("Task")} value={draft.task_id} />
                <Fact label={t("Status")} value={draft.review_status} />
              </div>
              {editing && <label>{t("Attributes (JSON)")}<textarea aria-label="Annotation attributes JSON" value={attributesText} onChange={(event) => setAttributesText(event.target.value)} />
              </label>}
              <button
                aria-expanded={revisionHistoryOpen}
                aria-controls="review-revision-history"
                onClick={openRevisionHistory}
              >{t("View revision history")}</button>
              <Trace events={events.filter((event) => event.run_id === selected.run_id)} />
            </details>
            {revisionHistoryOpen && <section
              id="review-revision-history"
              className="review-revision-drawer"
              role="dialog"
              aria-modal="false"
              aria-labelledby="review-revision-title"
            >
              <header>
                <span><span className="eyebrow">{t("Audit history")}</span><h3 id="review-revision-title">{t("Annotation revisions")}</h3></span>
                <button aria-label={t("Close revision history")} onClick={() => setRevisionHistoryOpen(false)}>×</button>
              </header>
              {revisionHistoryLoading ? <p role="status">{t("Loading revisions…")}</p> : revisionHistory.length ? <ol>
                {[...revisionHistory].reverse().map((revision) => <li key={revision.revision_id}>
                  <header><strong>{revision.reason?.replaceAll("_", " ") ?? t("Annotation updated")}</strong><time dateTime={revision.created_at}>{new Date(revision.created_at).toLocaleString(localeTag())}</time></header>
                  <dl>
                    <div><dt>{t("Actor")}</dt><dd>{revision.actor}</dd></div>
                    <div><dt>{t("Label")}</dt><dd>{revision.before?.label ?? t("None")} → {revision.after?.label ?? t("None")}</dd></div>
                    <div><dt>{t("Status")}</dt><dd>{revision.before?.review_status?.replaceAll("_", " ") ?? "created"} → {revision.after?.review_status?.replaceAll("_", " ") ?? "deleted"}</dd></div>
                    <div><dt>{t("Geometry")}</dt><dd>{revision.before?.value.kind ?? t("none")} → {revision.after?.value.kind ?? t("none")}</dd></div>
                  </dl>
                </li>)}
              </ol> : <p>{t("No revisions have been recorded for this annotation.")}</p>}
            </section>}
          </>
        )}
      </aside>}
    </section>
  );
}

function Trace({ events }: { events: RunEvent[] }) {
  const icon = (kind: string) =>
    kind.includes("model")
      ? "model-call"
      : kind.includes("tool") || kind.includes("artifact")
        ? "tool-call"
        : kind.includes("validation")
          ? "validate"
          : kind.includes("review")
            ? "review"
            : "agent-trace";
  const summary = (event: RunEvent) =>
    typeof event.payload.data.summary === "string"
      ? event.payload.data.summary
      : undefined;
  return (
    <div className="trace-panel panel">
      <div>
        <span className="eyebrow">{t("Visible execution events")}</span>
        <h3>{t("Agent trace")}</h3>
        <small>
          Original candidates, refined Artifacts, validation, and final commits
          · no hidden chain-of-thought
        </small>
      </div>
      <div className="trace-strip" aria-label="Agent trace events">
        {events.slice(-10).map((event) => (
          <article key={event.event_id}>
            <span>
              <img
                src={`/brand/core/icons/${icon(event.kind)}.svg`}
                alt=""
                aria-hidden="true"
              />
            </span>
            <div>
              <strong>{event.kind.replaceAll("_", " ")}</strong>
              <small>
                {summary(event) ??
                  `${event.task_id ?? "run"} · ${new Date(event.occurred_at).toLocaleTimeString(localeTag())}`}
              </small>
            </div>
          </article>
        ))}
      </div>
    </div>
  );
}

function agentStageLabel(session: AgentSession): string {
  const phaseLabels: Record<NonNullable<AgentSession["phase"]>, string> = {
    context_loading: "Loading bounded context",
    feasibility_analysis: "Resolving feasibility",
    candidate_selection: "Selecting the best compatible plan",
    drafting: "Building the Draft",
    validating: "Validating the Draft",
    dry_running: "Testing sample images",
    revising: "Revising from evidence",
    draft_salvage: "Creating a Draft from the preserved plan",
    finalizing: "Saving the outcome",
    waiting_for_human: "Ready for your review",
    completed: "Completed",
    cancelled: "Cancelled",
    failed: "Needs attention",
  };
  if (session.phase) return phaseLabels[session.phase];
  if (session.status === "waiting_for_human") return "Ready for your review";
  if (session.status === "cancelled") return "Cancelled";
  if (session.status === "budget_exceeded") return "Stopped at budget";
  if (session.status === "failed") return "Needs attention";
  const last = session.steps.at(-1)?.tool_name ?? "inspect_project";
  if (last.includes("inspect") || last.includes("list_")) return "Inspecting Project and Registry";
  if (last.includes("draft") || last.includes("node") || last.includes("connect")) return "Building the Draft";
  if (last.includes("validate")) return "Validating the Draft";
  if (last.includes("dry_run")) return "Testing on sample images";
  return "Revising the recommendation";
}

function agentOutcomeLabel(session: AgentSession): string {
  const labels: Record<NonNullable<AgentSession["outcome"]>, string> = {
    draft_ready_for_human_review: "Draft ready for human review",
    blocked_draft_ready: "Blocked Draft saved",
    provider_setup_required: "Provider setup required",
    unsupported_request: "Request is not supported by the current catalog",
    cancelled: "Agent cancelled",
    budget_exceeded: "Progress-safety budget reached",
    failed: "Agent needs attention",
  };
  if (session.outcome) return t(labels[session.outcome]);
  if (session.status === "running") return "Agent is running";
  if (session.status === "waiting_for_human") return "Waiting for your action";
  return session.status.replaceAll("_", " ");
}

function AgentSessionTrace({
  session,
  validation,
  dryRun,
  onCancel,
  onRetry,
  onOpenDraft,
  onConfigureProvider,
  onConfigureModel,
}: {
  session: AgentSession;
  validation?: WorkflowDryRunReport["validation"];
  dryRun?: WorkflowDryRunReport;
  onCancel?: () => void;
  onRetry?: () => void;
  onOpenDraft?: (draftId: string) => void;
  onConfigureProvider?: () => void;
  onConfigureModel?: () => void;
}) {
  const cancellable = ["running", "waiting_for_human"].includes(session.status);
  const stage = agentStageLabel(session);
  const totalCalls = session.total_tool_calls ?? session.usage.tool_calls;
  const maximumCalls = session.builder_budget?.max_total_tool_calls ?? session.budget.max_tool_calls;
  const remainingCalls = session.remaining_tool_calls ?? Math.max(0, maximumCalls - totalCalls);
  const reservedCalls = session.builder_budget
    ? (session.builder_budget.reserved_materialization_calls ?? 0)
      + (session.builder_budget.reserved_validation_calls ?? 0)
      + (session.builder_budget.reserved_finalization_calls ?? session.reserved_finalization_calls ?? 0)
    : session.reserved_finalization_calls ?? 0;
  const progress = Math.min(100, Math.round((totalCalls / Math.max(1, maximumCalls)) * 100));
  const needsSetup = ["provider_setup_required", "blocked_draft_ready"].includes(session.outcome ?? "");
  const retryable = ["failed", "budget_exceeded"].includes(session.status) || needsSetup;
  return (
    <div className="agent-session-trace" aria-label={`${session.kind} Agent trace`}>
      <div className="context-line">
        <strong>{t("Pipeline Builder")}</strong>
        <Status status={session.status} />
        <span>{stage}</span>
        <span>{totalCalls} tool calls</span>
        <span>{session.usage.input_tokens + session.usage.output_tokens}{" "}{t("tokens")}</span>
        <span>${session.usage.cost}</span>
        {onCancel && (
          <button className="danger" disabled={!cancellable} onClick={onCancel}>{t("Cancel Agent")}</button>
        )}
      </div>
      <div className="agent-progress" role="progressbar" aria-label={t("Tool budget used")} aria-valuemin={0} aria-valuemax={100} aria-valuenow={progress}>
        <span style={{ width: `${progress}%` }} />
      </div>
      {session.salvage_outcome && (
        <section className="agent-salvage-notice" aria-label={t("Draft salvage result")}>
          <div>
            <span className="eyebrow">{t("Discovery completed")}</span>
            <strong>
              {session.salvage_outcome === "runnable_draft_materialized"
                ? t("Best compatible plan saved as a Draft")
                : t("Best available plan saved with explicit blockers")}
            </strong>
          </div>
          <p>
            AnnotAgent stopped searching, preserved the plans already found, selected one
            deterministically, and ran static validation.
          </p>
        </section>
      )}
      {!!session.plan_candidates?.length &&
        !session.selected_candidate_id &&
        !session.salvage_outcome && (
          <section className="agent-plan-warning" role="alert">
            <div>
              <span className="eyebrow">{t("Plan discovered but not applied")}</span>
              <strong>AnnotAgent preserved the candidates for diagnosis</strong>
            </div>
            <p>
              No candidate was materialized into the Working Draft. Inspect the
              planning events, then retry from the saved Draft.
            </p>
          </section>
        )}
      <details className="agent-execution-facts"><summary>{t("Execution details and budget")}</summary><div className="fact-grid">
        <Fact label={t("Current stage")} value={stage} />
        <Fact label={t("Model turns")} value={session.model_turns ?? session.model_calls.length} />
        <Fact label={t("Tool budget")} value={`${remainingCalls} remaining · ${reservedCalls} reserved`} />
        <Fact label={t("Phase calls")} value={session.phase_tool_calls ?? "Not recorded"} />
        <Fact label={t("Cache reuse")} value={session.cache_hits ?? 0} />
        <Fact label={t("Duplicates blocked")} value={session.duplicate_tool_calls ?? 0} />
        <Fact
          label={t("Provider")}
          value={session.model_selection?.provider_display_name ?? "Not recorded"}
        />
        <Fact
          label={t("Agent model")}
          value={session.model_selection?.model_display_name ?? "Offline deterministic"}
        />
        <Fact
          label={t("Model choice")}
          value={
            session.model_selection
              ? `${session.model_selection.binding_source.replaceAll("_", " ")}${session.model_selection.locked ? " · locked" : ""}`
              : "Offline mode"
          }
        />
        <Fact
          label={t("Validation issues")}
          value={validation?.issues.length ?? "Not recorded"}
        />
        <Fact
          label={t("Dry Run")}
          value={dryRun ? `${dryRun.summary.image_count} image · ${dryRun.summary.failed_count} failed` : "Not run"}
        />
        <Fact label={t("Stop reason")} value={t(builderStopLabel(session))} />
        <Fact
          label={t("Human action")}
          value={session.pending_human_action ?? "None"}
        />
        {session.builder_constraints && <Fact label={t("Priority")} value={session.builder_constraints.priority.replaceAll("_", " ")} />}
        {session.build_mode && <Fact label={t("Build mode")} value={session.build_mode.kind.replaceAll("_", " ")} />}
      </div></details>
      {!!session.plan_candidates?.length && (
        <details className="agent-plan-candidates" aria-label={t("Pipeline plan candidates")}>
          <summary>{t("Alternative plans and selection details")} · {session.plan_candidates?.length}</summary>
          <p>{t("Alternatives are not extra pipelines being executed. Structural ranking does not measure annotation accuracy.")}</p>
          <div className="section-heading compact">
            <div>
              <span className="eyebrow">{t("Preserved plans")}</span>
              <h4>{session.plan_candidates.length} candidate{session.plan_candidates.length === 1 ? "" : t("s")}</h4>
            </div>
            <span>{session.discovered_conversion_paths?.length ?? 0} typed path{session.discovered_conversion_paths?.length === 1 ? "" : t("s")}</span>
          </div>
          <div className="agent-candidate-grid">
            {session.plan_candidates.map((candidate) => {
              const selected = candidate.id === session.selected_candidate_id;
              return (
                <article key={candidate.id} className={selected ? "selected" : ""}>
                  <div className="context-line">
                    <strong>{candidate.name}</strong>
                    <Status status={candidate.status === "materialized" ? "succeeded" : candidate.status} />
                    {selected && <span className="candidate-selected">{t("Selected")}</span>}
                  </div>
                  <p>{candidate.source.replaceAll("_", " ")} · {candidate.geometry_safety.replaceAll("_", " ")} · {candidate.model_bindings.length} model call{candidate.model_bindings.length === 1 ? "" : t("s")}</p>
                  {!!candidate.node_blueprints.length && (
                    <div className="candidate-node-chain" aria-label={t("Candidate node chain")}>
                      {candidate.node_blueprints.map((node, index) => (
                        <span key={`${candidate.id}-${node.id}`}>
                          {index > 0 && <b aria-hidden="true">→</b>}
                          <code>{node.node_type}</code>
                        </span>
                      ))}
                    </div>
                  )}
                  <div className="candidate-contracts">
                    <span>{candidate.has_commit_path ? t("Commit path") : t("No Commit path")}</span>
                    <span>{candidate.has_review_path ? t("Review path") : t("No Review path")}</span>
                    <span>{candidate.sufficiency}</span>
                  </div>
                  {!!candidate.unresolved_bindings.length && (
                    <small>{candidate.unresolved_bindings.length} unresolved binding{candidate.unresolved_bindings.length === 1 ? "" : t("s")}</small>
                  )}
                  {!!candidate.model_bindings.length && (
                    <ul className="candidate-model-list">
                      {candidate.model_bindings.map((binding) => (
                        <li key={`${candidate.id}-${binding.node_id}-${binding.capability}`}>
                          <span>{binding.capability.replaceAll("_", " ")}</span>
                          <code>{binding.model_profile_id?.slice(0, 8) ?? t("Unresolved")}</code>
                          <small>{binding.availability.replaceAll("_", " ")}</small>
                        </li>
                      ))}
                    </ul>
                  )}
                  {!!candidate.score.reasons.length && <small>{candidate.score.reasons.join(" · ")}</small>}
                </article>
              );
            })}
          </div>
          {!!session.planning_events?.length && (
            <details>
              <summary>Planning events ({session.planning_events.length})</summary>
              <ol className="agent-action-list">
                {session.planning_events.map((event) => (
                  <li key={`${event.sequence}-${event.kind}`}>
                    <strong>{event.sequence}. {event.kind.replaceAll("_", " ")}</strong>
                    <small>{event.detail}</small>
                  </li>
                ))}
              </ol>
            </details>
          )}
        </details>
      )}
      {session.status !== "running" && (
        <section className={`agent-outcome-card ${needsSetup ? "setup" : ""}`} aria-label={t("Pipeline Builder outcome")}>
          <div>
            <span className="eyebrow">{t("Outcome")}</span>
            <h4>{agentOutcomeLabel(session)}</h4>
            <p>{session.salvage_outcome === "runnable_draft_materialized" ? t("Review the statically checked Draft, then test sample images. Annotation quality has not been verified.") : t(session.next_action ?? readableErrorMessage(session.stop_reason ?? "Open the saved session for details."))}</p>
          </div>
          <div className="agent-outcome-facts">
            {session.draft_id && <span><small>{t("Draft")}</small><strong>{session.draft_id.slice(0, 8)}</strong></span>}
            <span><small>{t("Stop reason")}</small><strong>{t(builderStopLabel(session))}</strong></span>
            <span><small>{t("Plan source")}</small><strong>{t(builderPlanSource(session))}</strong></span>
            {!!session.unresolved_bindings?.length && <span><small>{t("Unresolved")}</small><strong>{session.unresolved_bindings.length} model binding{session.unresolved_bindings.length === 1 ? "" : t("s")}</strong></span>}
          </div>
          <div className="button-row">
            {session.draft_id && onOpenDraft && <button className="primary" onClick={() => onOpenDraft(session.draft_id!)}>{session.outcome === "draft_ready_for_human_review" ? t("Review Draft") : needsSetup ? t("Open blocked Draft") : t("Open Draft")}</button>}
            {needsSetup && onConfigureProvider && <button onClick={onConfigureProvider}>{t("Configure Provider")}</button>}
            {needsSetup && onConfigureModel && <button onClick={onConfigureModel}>{t("Configure Model")}</button>}
            {retryable && onRetry && <button onClick={onRetry}>{t("Retry from current Draft")}</button>}
          </div>
        </section>
      )}
      <details className="agent-tool-trace">
        <summary>Tool actions ({session.steps.length})</summary>
      <ol className="agent-action-list">
        {session.steps.map((step) => (
          <li key={step.call_id}>
            <strong>{step.sequence}. {step.tool_name.replaceAll("_", " ")}</strong>
            <small>{step.success ? t("Completed") : t("Failed")}</small>
            <details>
              <summary>{t("Observable inputs and result")}</summary>
              <pre>{JSON.stringify({ arguments: step.arguments, result: step.result }, null, 2)}</pre>
            </details>
          </li>
        ))}
      </ol>
      </details>
      {session.model_calls.length > 0 && (
        <details className="agent-model-call-trace">
          <summary>Model requests ({session.model_calls.length})</summary>
          <ol className="agent-action-list">
            {session.model_calls.map((call) => (
              <li key={`${call.sequence}-${call.request_id ?? call.created_at}`}>
                <strong>
                  {call.sequence}. {call.provider_name} · {call.remote_model_id}
                </strong>
                <small>
                  {call.succeeded ? t("Succeeded") : t("Failed")} · {call.duration_ms} ms
                  · {call.input_tokens + call.output_tokens} tokens · {call.currency}{" "}
                  {call.cost} · {call.retry_count} retries
                </small>
                {call.safe_error && <p role="alert">{call.safe_error}</p>}
              </li>
            ))}
          </ol>
        </details>
      )}
    </div>
  );
}

function ProjectAgentActivity({
  projectId,
  onError,
}: {
  projectId: string;
  onError: (value: string) => void;
}) {
  const [sessions, setSessions] = useState<AgentSession[]>([]);
  const [memory, setMemory] = useState<CorrectionMemoryRecord[]>([]);
  const load = () =>
    Promise.all([api.agentSessions(projectId), api.correctionMemory(projectId)])
      .then(([agentData, memoryData]) => {
        setSessions(agentData.sessions);
        setMemory(memoryData.records);
      })
      .catch((error: Error) => onError(error.message));
  useEffect(() => {
    void load();
  }, [projectId]);
  if (!sessions.length && !memory.length) return null;
  return (
    <div className="split-grid agent-activity">
      <Panel title={t("Agent activity")} eyebrow={t("Advisor and recovery sessions")}>
        {sessions.length ? (
          sessions.slice(0, 5).map((session) => (
            <AgentSessionTrace
              key={session.id}
              session={session}
              onCancel={() =>
                void api
                  .cancelAgentSession(session.id)
                  .then(load)
                  .catch((error: Error) => onError(error.message))
              }
            />
          ))
        ) : (
          <Empty title={t("No Agent sessions")} detail="Deterministic Workflow execution remains the fast path." />
        )}
      </Panel>
      <Panel title={t("Correction Memory")} eyebrow={t("Project-scoped structured evidence")}>
        {memory.length ? (
          <div className="catalog-list">
            {memory.map((record) => (
              <article key={record.id}>
                <span className="catalog-monogram">M</span>
                <span>
                  <strong>{record.reason_code.replaceAll("_", " ")}</strong>
                  <small>{record.skill_id} · {record.task_id} · {record.predicted_label ?? t("any Label")}</small>
                  <small>This evidence can raise recovery risk for the same Project, Skill, task and Label.</small>
                </span>
              </article>
            ))}
          </div>
        ) : (
          <Empty title={t("No correction evidence")} detail={t("Human corrections will appear here after Review.")} />
        )}
      </Panel>
    </div>
  );
}

function SkillsPage({ onError }: { onError: (value: string) => void }) {
  const [skills, setSkills] = useState<SkillDetail[]>([]);
  useEffect(() => {
    void api
      .skills()
      .then(setSkills)
      .catch((error: Error) => onError(error.message));
  }, []);
  const groups: { kind: SkillDetail["kind"]; title: string; detail: string }[] = [
    { kind: "capability", title: "Capability Skills", detail: "Reusable model and processing abilities" },
    { kind: "domain", title: "Domain Skills", detail: "Domain validation, policy, recovery and memory" },
    { kind: "pack", title: "Skill Packs", detail: "Versioned collections of Domain and Capability Skills" },
  ];
  return (
    <section className="page-stack">
      <div className="boundary-note">
        <span>AnnotAgent</span>
        <i>Tool · Core Node · Model · Skill</i>
        <span>Layered Skill Registry</span>
      </div>
      {groups.map((group) => {
        const items = skills.filter((skill) => skill.kind === group.kind);
        if (!items.length) return null;
        return (
          <section className="skill-group" key={group.kind}>
            <div><span className="eyebrow">{group.detail}</span><h2>{t(group.title)}</h2></div>
            {items.map((skill) => (
              <Panel key={skill.id} title={`${skill.display_name} · v${skill.version}`} eyebrow={`${skill.kind} · ${skill.id}`}>
                <p className="lede">{skill.description}</p>
                <div className="skill-columns">
                  <TagGroup title="Provided Nodes" values={skill.nodes} />
                  <TagGroup title="Registered tools" values={skill.tools} />
                  <TagGroup title={t("Capabilities")} values={skill.capabilities} />
                  <TagGroup title="Capability requirements" values={skill.capability_requirements} />
                  <TagGroup title={t("Validators")} values={skill.validators} />
                  <TagGroup title={t("Refiners")} values={skill.refiners} />
                  <TagGroup title="Policies" values={skill.policies} />
                  <TagGroup title="Templates" values={skill.workflow_templates.map((template) => template.id)} />
                  <TagGroup title="Correction taxonomy" values={skill.correction_taxonomy} />
                  <TagGroup title="Prompt resources" values={skill.resources} />
                  <TagGroup title="Used by Projects" values={skill.projects} />
                </div>
              </Panel>
            ))}
          </section>
        );
      })}
      {skills.length === 0 && (
        <Empty
          title="No Skills installed"
          detail="Install a registered extension before creating a runnable Project."
        />
      )}
    </section>
  );
}

type ExpertWorkerDraft = Record<string, any>;

const EXPERT_WORKER_PRESETS = [
  ["sam", "SAM", "Prompted segmentation from a box or point prompt"],
  ["yolo", "YOLO", "Fixed-label object detection"],
  ["rfdetr", "RF-DETR", "Specialist object detection"],
  ["locate_anything", "LocateAnything", "Open-vocabulary detection and phrase grounding"],
  ["pidnet", "PIDNet", "Semantic segmentation"],
  ["grounding_dino", "Grounding DINO", "Open-vocabulary detection and phrase grounding"],
  ["custom", "Custom", "A protocol-compatible Expert Vision Worker"],
] as const;

function expertWorkerPreset(preset: string, suffix: number): ExpertWorkerDraft {
  const profiles: Record<string, { name: string; model: string; port: number; capabilities: string[]; score: string; architecture: string }> = {
    sam: { name: "SAM prompted segmentation", model: "sam2.1-hiera-tiny", port: 8790, capabilities: ["prompted_segmentation"], score: "not_provided", architecture: "sam2.1-hiera-tiny" },
    yolo: { name: "YOLO detector", model: "yolo-specialist", port: 8793, capabilities: ["object_detection"], score: "relative_confidence", architecture: "yolo" },
    rfdetr: { name: "RF-DETR specialist", model: "rfdetr-specialist", port: 8792, capabilities: ["object_detection"], score: "relative_confidence", architecture: "rf-detr" },
    locate_anything: { name: "LocateAnything", model: "locate-anything", port: 8791, capabilities: ["open_vocabulary_detection", "phrase_grounding"], score: "not_provided", architecture: "locateanything" },
    pidnet: { name: "PIDNet semantic segmentation", model: "pidnet-specialist", port: 8794, capabilities: ["semantic_segmentation"], score: "not_provided", architecture: "pidnet" },
    grounding_dino: { name: "Grounding DINO", model: "grounding-dino", port: 8795, capabilities: ["open_vocabulary_detection", "phrase_grounding"], score: "relative_confidence", architecture: "grounding-dino" },
    custom: { name: `Expert Vision Worker ${suffix}`, model: `expert-model-${suffix}`, port: 8795 + suffix, capabilities: ["object_detection"], score: "unknown", architecture: "" },
  };
  const profile = profiles[preset] ?? profiles.custom;
  return {
    id: `expert-${preset.replaceAll("_", "-")}-${suffix}`,
    display_name: profile.name,
    model_id: profile.model,
    base_url: `http://127.0.0.1:${profile.port}`,
    authentication_reference: null,
    enabled: false,
    allow_remote: false,
    requires_checkpoint_metadata: true,
    expected_capabilities: profile.capabilities,
    score_semantics: profile.score,
    version: {
      architecture: profile.architecture || null,
      model_version: "unconfigured",
      checkpoint_sha256: null,
      training_dataset_version: null,
      backend_protocol_version: "1",
    },
    label_space: [],
    runtime_requirements: { devices: ["cpu", "cuda"], dependencies: [], supports_batch: false },
    license: {
      code_license: null,
      weight_license: null,
      source_url: null,
      commercial_use: "unknown",
      redistribution: "unknown",
      usage_notes: [],
      verified_from_official_source: false,
    },
    timeout_seconds: 120,
    max_request_bytes: 44_000_000,
    max_response_bytes: preset === "sam" ? 16_000_000 : 2_000_000,
    max_retries: 0,
    cost_per_request: "0",
    availability: "missing_weights",
    availability_evidence: {
      health_passed: false,
      protocol_compatible: false,
      contracts_validated: false,
      sample_conversion_passed: false,
      weights_ready: false,
    },
  };
}

function ExpertModelSetupWizard({
  settings,
  onSaved,
  onClose,
  onError,
}: {
  settings: Record<string, any>;
  onSaved: (value: Record<string, any>, message: string) => void;
  onClose: () => void;
  onError: (value: string) => void;
}) {
  const existingWorkers = Array.isArray(settings.detection_workers) ? settings.detection_workers : [];
  const [step, setStep] = useState(1);
  const [method, setMethod] = useState<"preset" | "http">("preset");
  const [preset, setPreset] = useState("sam");
  const [worker, setWorker] = useState<ExpertWorkerDraft>(() => expertWorkerPreset("sam", existingWorkers.length + 1));
  const [discovery, setDiscovery] = useState<DetectionWorkerTestResult>();
  const [sample, setSample] = useState<DetectionWorkerSampleTestResult>();
  const [projects, setProjects] = useState<ProjectSummary[]>([]);
  const [nextProjectOffset, setNextProjectOffset] = useState<number | null>(0);
  const [projectPageLoading, setProjectPageLoading] = useState(false);
  const [projectPageError, setProjectPageError] = useState<string>();
  const projectRead = useRef<AbortController | undefined>(undefined);
  const projectReadPending = useRef(false);
  const [projectId, setProjectId] = useState("");
  const [images, setImages] = useState<ImageItem[]>([]);
  const [imageIndex, setImageIndex] = useState(0);
  const [query, setQuery] = useState("football");
  const [busy, setBusy] = useState("");

  async function loadProjectPage(offset: number) {
    if (projectReadPending.current) return;
    projectReadPending.current = true;
    const controller = new AbortController(); projectRead.current = controller;
    setProjectPageLoading(true); setProjectPageError(undefined);
    try {
      const value = await api.projectPage(offset, controller.signal);
      if (controller.signal.aborted) return;
      const available = value.projects.filter(project => project.image_count > 0);
      setProjects(current => offset === 0 ? available : [...current, ...available.filter(project => !current.some(item => item.id === project.id))]);
      setNextProjectOffset(value.page?.projects.next_offset ?? null);
      if (offset === 0) setProjectId(current => current || available[0]?.id || "");
    } catch (error) { if (!controller.signal.aborted) setProjectPageError((error as Error).message); }
    finally { projectReadPending.current = false; if (!controller.signal.aborted) setProjectPageLoading(false); }
  }
  useEffect(() => () => projectRead.current?.abort(), []);
  useEffect(() => {
    if (step === 5 && nextProjectOffset === 0) void loadProjectPage(0);
  }, [step]);
  useEffect(() => {
    if (!projectId) {
      setImages([]);
      return;
    }
    const controller = new AbortController();
    setImages([]);
    void api.images(projectId, controller.signal).then((value) => {
      if (controller.signal.aborted) return;
      setImages(value.images);
      setImageIndex(value.images[0]?.index ?? 0);
    }).catch((error: Error) => { if (!controller.signal.aborted) onError(error.message); });
    return () => controller.abort();
  }, [projectId]);

  const choosePreset = (value: string) => {
    setPreset(value);
    setWorker(expertWorkerPreset(value, existingWorkers.length + 1));
    setDiscovery(undefined);
    setSample(undefined);
  };
  const setField = (field: string, value: unknown) => setWorker((current) => ({ ...current, [field]: value }));
  const setVersion = (field: string, value: unknown) => setWorker((current) => ({ ...current, version: { ...(current.version ?? {}), [field]: value || null } }));
  const setLicense = (field: string, value: unknown) => setWorker((current) => ({ ...current, license: { ...(current.license ?? {}), [field]: value || null } }));

  const persistDraft = async (draft: ExpertWorkerDraft) => {
    const latest = await api.settings();
    const latestWorkers = Array.isArray(latest.detection_workers) ? latest.detection_workers : [];
    const existingIndex = latestWorkers.findIndex((candidate: ExpertWorkerDraft) => candidate.id === draft.id || candidate.model_id === draft.model_id);
    const observed = existingIndex >= 0 ? latestWorkers[existingIndex] : undefined;
    const merged = observed ? {
      ...observed,
      ...draft,
      availability: observed.availability ?? draft.availability,
      availability_evidence: observed.availability_evidence ?? draft.availability_evidence,
    } : draft;
    const nextWorkers = existingIndex >= 0
      ? latestWorkers.map((candidate: ExpertWorkerDraft, index: number) => index === existingIndex ? merged : candidate)
      : [...latestWorkers, merged];
    const saved = await api.saveSettings({ ...latest, detection_workers: nextWorkers });
    onSaved(saved, "Saved Expert Model setup locally.");
    const savedWorkers = Array.isArray(saved.detection_workers) ? saved.detection_workers : [];
    const savedWorker = savedWorkers.find((candidate: ExpertWorkerDraft) => candidate.id === merged.id) ?? merged;
    setWorker(savedWorker);
    return savedWorker;
  };

  const discover = async () => {
    setBusy("discovery");
    try {
      const savedWorker = await persistDraft(worker);
      const result = await api.testModel(String(savedWorker.model_id));
      setDiscovery(result);
      const latest = await api.settings();
      const latestWorkers = Array.isArray(latest.detection_workers) ? latest.detection_workers : [];
      const observed = latestWorkers.find((candidate: ExpertWorkerDraft) => candidate.model_id === savedWorker.model_id);
      if (observed) setWorker(observed);
      setStep(3);
    } catch (error) {
      onError((error as Error).message);
    } finally {
      setBusy("");
    }
  };
  const saveIdentity = async () => {
    setBusy("identity");
    try {
      await persistDraft(worker);
      setStep(5);
    } catch (error) {
      onError((error as Error).message);
    } finally {
      setBusy("");
    }
  };
  const runSample = async () => {
    if (!projectId || !images.length) return;
    setBusy("sample");
    try {
      const savedWorker = await persistDraft(worker);
      const refreshedDiscovery = await api.testModel(String(savedWorker.model_id));
      setDiscovery(refreshedDiscovery);
      if (!refreshedDiscovery.passed) {
        throw new Error(refreshedDiscovery.error ?? refreshedDiscovery.evidence?.detail ?? "Worker discovery no longer matches the saved model identity.");
      }
      const result = await api.sampleTestModel(String(savedWorker.model_id), {
        project_id: projectId,
        image_index: imageIndex,
        query: query.trim() || undefined,
        box_prompt: savedWorker.expected_capabilities?.includes("prompted_segmentation") ? [0.25, 0.25, 0.5, 0.5] : undefined,
      });
      setSample(result);
      const latest = await api.settings();
      const latestWorkers = Array.isArray(latest.detection_workers) ? latest.detection_workers : [];
      const observed = latestWorkers.find((candidate: ExpertWorkerDraft) => candidate.model_id === savedWorker.model_id);
      if (observed) setWorker(observed);
    } catch (error) {
      onError((error as Error).message);
    } finally {
      setBusy("");
    }
  };
  const readyEvidence = sample?.evidence;
  const canRegister = Boolean(readyEvidence?.health_passed && readyEvidence.protocol_compatible && readyEvidence.contracts_validated && readyEvidence.sample_conversion_passed && readyEvidence.weights_ready);
  const register = async () => {
    setBusy("register");
    try {
      const latest = await api.settings();
      const latestWorkers = Array.isArray(latest.detection_workers) ? latest.detection_workers : [];
      const nextWorkers = latestWorkers.map((candidate: ExpertWorkerDraft) => candidate.model_id === worker.model_id ? { ...candidate, enabled: true } : candidate);
      const saved = await api.saveSettings({ ...latest, detection_workers: nextWorkers });
      onSaved(saved, `${worker.display_name} is registered and available to the Pipeline Builder.`);
      onClose();
    } catch (error) {
      onError((error as Error).message);
    } finally {
      setBusy("");
    }
  };

  const title = ["Choose an integration", "Connect the Worker", "Discover live capabilities", "Complete model identity", "Run a selected-image sample", "Register the Expert Model"][step - 1];
  return <div className="modal-backdrop"><div className="modal expert-model-wizard" role="dialog" aria-modal="true" aria-labelledby="expert-model-wizard-title">
    <header><span className="eyebrow">Expert Model · Step {step} of 6</span><h2 id="expert-model-wizard-title">{title}</h2><div className="wizard-progress six" aria-label={`Step ${step} of 6`}>{[1, 2, 3, 4, 5, 6].map((item) => <i key={item} className={item <= step ? "complete" : ""} />)}</div></header>
    {step === 5 && <div className="button-row" aria-label="Sample Project pages">
      <span>{projects.length} Projects with images loaded</span>
      {projectPageError && <p role="alert">{projectPageError}</p>}
      {nextProjectOffset !== null && <button disabled={projectPageLoading} onClick={() => void loadProjectPage(nextProjectOffset)}>{projectPageLoading ? "Loading Projects…" : projectPageError ? "Retry Project loading" : "Load more Projects"}</button>}
    </div>}
    {step === 1 && <div className="wizard-step"><div className="choice-grid expert-methods" role="radiogroup" aria-label="Expert Model integration method">{([
      ["preset", "Use preset", "Start with a known capability contract"],
      ["http", "Generic HTTP Worker", "Connect any Vision Protocol v1 service"],
    ] as const).map(([value, label, detail]) => <label className={method === value ? "selected" : ""} key={value}><input type="radio" name="expert-method" checked={method === value} onChange={() => { setMethod(value); if (value === "http") choosePreset("custom"); }} /><span><strong>{label}</strong><small>{detail}</small></span></label>)}</div>{method === "preset" && <label>{t("Preset")}<select value={preset} onChange={(event) => choosePreset(event.target.value)}>{EXPERT_WORKER_PRESETS.map(([value, label, detail]) => <option key={value} value={value}>{label} — {detail}</option>)}</select></label>}</div>}
    {step === 2 && <div className="wizard-step"><div className="form-grid"><label>{t("Endpoint")}<input type="url" value={String(worker.base_url ?? "")} onChange={(event) => setField("base_url", event.target.value)} /></label><label>Timeout seconds<input type="number" min="1" value={Number(worker.timeout_seconds ?? 120)} onChange={(event) => setField("timeout_seconds", Number(event.target.value))} /></label><label>Authentication reference<input value={String(worker.authentication_reference ?? "")} onChange={(event) => setField("authentication_reference", event.target.value || null)} placeholder="env:ANNOTAGENT_SAM_TOKEN" /></label><label className="checkbox-line"><input type="checkbox" checked={Boolean(worker.allow_remote)} onChange={(event) => setField("allow_remote", event.target.checked)} /><span>Allow remote HTTPS Worker</span></label></div><div className="wizard-summary"><strong>Trust boundary</strong><span>Loopback is allowed by default. Remote endpoints require HTTPS and explicit permission. Authentication is a reference; no secret is written to Settings.</span></div></div>}
    {step === 3 && <div className="wizard-step"><div className={`expert-test-banner ${discovery?.passed ? "passed" : "failed"}`} role="status"><strong>{discovery?.passed ? t("Discovery passed") : `Discovery stopped at ${discovery?.failed_stage ?? "an unknown stage"}`}</strong><span>{discovery?.error ?? discovery?.evidence?.detail ?? "The Worker returned all required protocol resources."}</span></div><div className="expert-check-grid"><Fact label={t("Health")} value={discovery?.health?.status ?? "Not available"} /><Fact label={t("Protocol")} value={discovery?.evidence?.protocol_compatible ? "Compatible" : "Not verified"} /><Fact label={t("Models")} value={discovery?.models?.models.length ?? 0} /><Fact label="Contracts" value={discovery?.evidence?.contracts_validated ? "Valid" : "Not verified"} /></div>{discovery?.capabilities && <div className="tag-group">{discovery.capabilities.capabilities.map((capability) => <span key={capability}>{t(capability.replaceAll("_", " "))}</span>)}</div>}<details className="advanced-settings"><summary>Raw discovery response</summary><pre>{JSON.stringify(discovery, null, 2)}</pre></details></div>}
    {step === 4 && <div className="wizard-step"><div className="form-grid"><label>{t("Display name")}<input value={String(worker.display_name ?? "")} onChange={(event) => setField("display_name", event.target.value)} /></label><label>Model ID<input value={String(worker.model_id ?? "")} onChange={(event) => setField("model_id", event.target.value)} /></label><label>Architecture<input value={String(worker.version?.architecture ?? "")} onChange={(event) => setVersion("architecture", event.target.value)} /></label><label>{t("Version")}<input value={String(worker.version?.model_version ?? "")} onChange={(event) => setVersion("model_version", event.target.value)} /></label><label>Checkpoint SHA-256<input value={String(worker.version?.checkpoint_sha256 ?? "")} onChange={(event) => setVersion("checkpoint_sha256", event.target.value.trim())} placeholder="64 hexadecimal characters" /></label><label>Training dataset version<input value={String(worker.version?.training_dataset_version ?? "")} onChange={(event) => setVersion("training_dataset_version", event.target.value)} /></label><label>Label space<input value={Array.isArray(worker.label_space) ? worker.label_space.join(", ") : ""} onChange={(event) => setField("label_space", event.target.value.split(",").map((value) => value.trim()).filter(Boolean))} placeholder="football, robot" /></label><label>Checkpoint license<input value={String(worker.license?.weight_license ?? "")} onChange={(event) => setLicense("weight_license", event.target.value)} /></label></div><div className="expert-test-banner missing"><strong>Missing weights until identity is complete</strong><span>A filename is not a checkpoint identity. SAM and specialist models remain unavailable without a version, SHA-256, and concrete weight license.</span></div></div>}
    {step === 5 && <div className="wizard-step"><div className="form-grid"><label>{t("Project")}<select aria-label={t("Project")} value={projectId} onChange={(event) => setProjectId(event.target.value)}><option value="">Choose a Project with images</option>{projects.map((project) => <option key={project.id} value={project.id}>{project.name}</option>)}</select></label><label>Sample image<select value={imageIndex} onChange={(event) => setImageIndex(Number(event.target.value))}>{images.map((image) => <option key={image.index} value={image.index}>{image.name}</option>)}</select></label><label>Text query<input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="football" /></label></div>{projectId && images.length > 0 && <div className="expert-sample-layout"><img src={`/api/projects/${projectId}/images/${imageIndex}/content`} alt="Selected Worker sample input" /><div><button className="primary" disabled={busy === "sample"} onClick={() => void runSample()}>{busy === "sample" ? t("Running sample…") : t("Run sample test")}</button><small>Prompted segmentation uses a visible centered sample box. Detection Workers use the selected image and query.</small></div></div>}{sample && <div className="expert-sample-result"><div className={`expert-test-banner ${sample.passed ? "passed" : "failed"}`} role="status"><strong>{sample.passed ? t("Sample conversion passed") : t("Sample conversion failed")}</strong><span>{sample.error ?? sample.evidence.detail}</span></div>{sample.input?.image_url && <img src={sample.input.image_url} alt="Worker sample result source" />}<div className="expert-check-grid"><Fact label={t("Artifacts")} value={Array.isArray(sample.converted_artifacts) ? sample.converted_artifacts.length : 0} /><Fact label={t("Duration")} value={`${sample.duration_ms} ms`} /><Fact label="Score semantics" value={sample.score_semantics?.replaceAll("_", " ") ?? "Unknown"} /><Fact label={t("Geometry")} value={sample.geometry_semantics?.replaceAll("_", " ") ?? "Unknown"} /></div><details className="advanced-settings"><summary>Converted Artifact and coordinates</summary><pre>{JSON.stringify({ raw_output_summary: sample.raw_output_summary, converted_artifacts: sample.converted_artifacts, coordinates: sample.coordinates, warnings: sample.warnings }, null, 2)}</pre></details></div>}</div>}
    {step === 6 && <div className="wizard-step"><div className={`expert-test-banner ${canRegister ? "passed" : "missing"}`}><strong>{canRegister ? t("Ready to register") : t("Registration is blocked")}</strong><span>{canRegister ? "Health, protocol, contracts, model identity, weights, and sample conversion all have active evidence." : sample?.evidence.detail ?? "Run a successful selected-image sample after discovery and identity setup."}</span></div><div className="expert-checklist">{[["Health", readyEvidence?.health_passed], ["Protocol", readyEvidence?.protocol_compatible], ["Contracts", readyEvidence?.contracts_validated], ["Weights", readyEvidence?.weights_ready], ["Sample conversion", readyEvidence?.sample_conversion_passed]].map(([label, passed]) => <span key={String(label)} className={passed ? "complete" : "blocked"}><b>{passed ? "✓" : "—"}</b>{label}</span>)}</div></div>}
    <div className="wizard-actions"><button disabled={Boolean(busy)} onClick={step === 1 ? onClose : () => setStep((value) => value - 1)}>{step === 1 ? t("Cancel") : t("Back")}</button>{step === 1 ? <button className="primary" onClick={() => setStep(2)}>{t("Continue")}</button> : step === 2 ? <button className="primary" disabled={busy === "discovery" || !String(worker.base_url ?? "").trim()} onClick={() => void discover()}>{busy === "discovery" ? t("Discovering…") : t("Save and discover")}</button> : step === 3 ? <button className="primary" onClick={() => setStep(4)}>Configure identity</button> : step === 4 ? <button className="primary" disabled={busy === "identity"} onClick={() => void saveIdentity()}>{busy === "identity" ? t("Saving…") : t("Save identity and test")}</button> : step === 5 ? <button className="primary" disabled={!sample?.passed} onClick={() => setStep(6)}>Review registration</button> : <button className="primary" disabled={!canRegister || busy === "register"} onClick={() => void register()}>{busy === "register" ? t("Registering…") : t("Register Expert Model")}</button>}</div>
  </div></div>;
}

function SettingsPage({ view, onError }: { view: "workers" | "storage"; onError: (value: string) => void }) {
  const [settings, setSettings] = useState<Record<string, any>>();
  const [savedSignature, setSavedSignature] = useState("");
  const [message, setMessage] = useState("");
  const [saving, setSaving] = useState(false);
  const [showExpertWizard, setShowExpertWizard] = useState(false);
  useEffect(() => {
    void api
      .settings()
      .then((value) => {
        setSettings(value);
        setSavedSignature(JSON.stringify(value));
      })
      .catch((error: Error) => onError(error.message));
  }, []);
  if (!settings)
    return (
      <section className="page-stack">
        <Empty
          title="Loading settings"
          detail="Reading the saved workspace configuration."
        />
      </section>
    );
  const pricing = settings.pricing ?? {};
  const budget = settings.budget ?? {};
  const detectionWorkers = Array.isArray(settings.detection_workers)
    ? settings.detection_workers
    : [];
  const setDetectionWorker = (index: number, field: string, value: unknown) =>
    setSettings({
      ...settings,
      detection_workers: detectionWorkers.map((worker: Record<string, unknown>, workerIndex: number) =>
        workerIndex === index ? { ...worker, [field]: value } : worker,
      ),
    });
  const setDetectionWorkerVersion = (index: number, field: string, value: unknown) =>
    setSettings({
      ...settings,
      detection_workers: detectionWorkers.map((worker: Record<string, any>, workerIndex: number) =>
        workerIndex === index
          ? { ...worker, version: { ...(worker.version ?? {}), [field]: value || null } }
          : worker,
      ),
    });
  const setDetectionWorkerLicense = (index: number, field: string, value: unknown) =>
    setSettings({
      ...settings,
      detection_workers: detectionWorkers.map((worker: Record<string, any>, workerIndex: number) =>
        workerIndex === index
          ? { ...worker, license: { ...(worker.license ?? {}), [field]: value || null } }
          : worker,
      ),
    });
  const removeDetectionWorker = (index: number) => setSettings({
    ...settings,
    detection_workers: detectionWorkers.filter((_: unknown, workerIndex: number) => workerIndex !== index),
  });
  const finish = (value: Record<string, unknown>, nextMessage: string) => {
    setSettings(value);
    setSavedSignature(JSON.stringify(value));
    setMessage(nextMessage);
  };
  const save = () => {
    setSaving(true);
    void api
      .saveSettings(settings)
      .then((value) => finish(value, "Saved runtime and storage settings locally."))
      .catch((error: Error) => onError(error.message))
      .finally(() => setSaving(false));
  };
  const dirty = JSON.stringify(settings) !== savedSignature;
  return (
    <section className="settings-grid">
      {view === "workers" && <Panel title="Legacy HTTP models" eyebrow="Compatibility for existing external endpoints">
        <div className="worker-collection-actions">
          <p>Existing protocol v1 endpoints remain supported for historical bindings. Use Expert Model Plugins for every new native model installation.</p>
          <button onClick={() => setShowExpertWizard(true)}>Add HTTP compatibility model</button>
        </div>
        {!detectionWorkers.length && <Empty title="No legacy HTTP models configured" detail="Install a native Rust Expert Model Plugin for new Workflows, or add an endpoint only to preserve an existing HTTP Vision v1 deployment." />}
        {detectionWorkers.length ? <div className="detection-worker-settings">
          {detectionWorkers.map((worker: Record<string, any>, index: number) => {
            const evidence = worker.availability_evidence ?? {};
            const registrationReady = Boolean(
              evidence.health_passed
              && evidence.protocol_compatible
              && evidence.contracts_validated
              && evidence.sample_conversion_passed
              && evidence.weights_ready,
            );
            return <article key={String(worker.id)}>
            <div className="worker-setting-heading">
              <span><strong>{String(worker.display_name)}</strong><small>{String(worker.model_id)}</small></span>
              <div className="worker-setting-actions">
                <label className="checkbox-line" title={!registrationReady && !worker.enabled ? "Complete discovery, model identity, and a selected-image sample before enabling this Worker." : undefined}><input type="checkbox" checked={Boolean(worker.enabled)} disabled={!registrationReady && !worker.enabled} onChange={(event) => setDetectionWorker(index, "enabled", event.target.checked)} /><span>{t("Enabled")}</span></label>
                <button className="text-button" onClick={() => removeDetectionWorker(index)}>{t("Remove")}</button>
              </div>
            </div>
            <div className="form-grid">
              <label>{t("Display name")}<input value={String(worker.display_name ?? "")} onChange={(event) => setDetectionWorker(index, "display_name", event.target.value)} /></label>
              <label>Registry ID<input value={String(worker.id ?? "")} onChange={(event) => setDetectionWorker(index, "id", event.target.value)} /></label>
              <label>Model ID<input value={String(worker.model_id ?? "")} onChange={(event) => setDetectionWorker(index, "model_id", event.target.value)} /></label>
              <label>Worker URL<input type="url" value={String(worker.base_url ?? "")} onChange={(event) => setDetectionWorker(index, "base_url", event.target.value)} /></label>
              <label>Authentication reference<input value={String(worker.authentication_reference ?? "")} onChange={(event) => setDetectionWorker(index, "authentication_reference", event.target.value || null)} placeholder="env:ANNOTAGENT_WORKER_TOKEN" /></label>
              <label>{t("Capability")}<select value={String(worker.expected_capabilities?.[0] ?? "object_detection")} onChange={(event) => setDetectionWorker(index, "expected_capabilities", [event.target.value])}><option value="object_detection">{t("Object detection")}</option><option value="open_vocabulary_detection">{t("Open-vocabulary detection")}</option><option value="phrase_grounding">{t("Phrase grounding")}</option><option value="prompted_segmentation">{t("Prompted segmentation")}</option><option value="semantic_segmentation">{t("Semantic segmentation")}</option></select></label>
              <label>Score semantics<select value={String(worker.score_semantics ?? "unknown")} onChange={(event) => setDetectionWorker(index, "score_semantics", event.target.value)}><option value="calibrated_probability">Calibrated probability</option><option value="relative_confidence">Relative confidence</option><option value="ranking_score">Ranking score</option><option value="not_provided">{t("Not provided")}</option><option value="unknown">{t("Unknown")}</option></select></label>
              <label>Estimated cost / request<input inputMode="decimal" value={String(worker.cost_per_request ?? "0")} onChange={(event) => setDetectionWorker(index, "cost_per_request", event.target.value)} /></label>
              <label>Timeout seconds<input type="number" min="1" value={Number(worker.timeout_seconds ?? 120)} onChange={(event) => setDetectionWorker(index, "timeout_seconds", Number(event.target.value))} /></label>
            </div>
            <div className="worker-contract-summary">
              <small>Availability · {String(worker.availability ?? "unknown").replaceAll("_", " ")}</small>
              <small>Expected contract · {(worker.expected_capabilities ?? []).join(" · ")}</small>
              <small>Score · {String(worker.score_semantics ?? "unknown").replaceAll("_", " ")}</small>
              <small>Version · {String(worker.version?.model_version ?? "unversioned")}</small>
            </div>
            <label className="checkbox-line"><input type="checkbox" checked={Boolean(worker.requires_checkpoint_metadata)} onChange={(event) => setDetectionWorker(index, "requires_checkpoint_metadata", event.target.checked)} /><span>Require specialist checkpoint identity</span></label>
            {Boolean(worker.requires_checkpoint_metadata) && <details className="advanced-settings">
              <summary>Required model identity</summary>
              <div className="form-grid">
                <label>Architecture<input value={String(worker.version?.architecture ?? "")} onChange={(event) => setDetectionWorkerVersion(index, "architecture", event.target.value)} placeholder="rfdetr-small" /></label>
                <label>Model version<input value={String(worker.version?.model_version ?? "")} onChange={(event) => setDetectionWorkerVersion(index, "model_version", event.target.value)} placeholder="dataset-model-v1" /></label>
                <label>Checkpoint SHA-256<input value={String(worker.version?.checkpoint_sha256 ?? "")} onChange={(event) => setDetectionWorkerVersion(index, "checkpoint_sha256", event.target.value.trim())} placeholder="64 hexadecimal characters" /></label>
                <label>Training dataset version<input value={String(worker.version?.training_dataset_version ?? "")} onChange={(event) => setDetectionWorkerVersion(index, "training_dataset_version", event.target.value)} placeholder="dataset-v1" /></label>
                <label>Model label space<input value={Array.isArray(worker.label_space) ? worker.label_space.join(", ") : ""} onChange={(event) => setDetectionWorker(index, "label_space", event.target.value.split(",").map((label) => label.trim()).filter(Boolean))} placeholder="football, robot" /></label>
                <label>Checkpoint weight license<input value={String(worker.license?.weight_license ?? "")} onChange={(event) => setDetectionWorkerLicense(index, "weight_license", event.target.value)} placeholder="Exact checkpoint terms" /></label>
              </div>
              <small>All fields are required before enabling this specialist Worker. A filename such as best.pt is not a version identity.</small>
            </details>}
            <label className="checkbox-line"><input type="checkbox" checked={Boolean(worker.allow_remote)} onChange={(event) => setDetectionWorker(index, "allow_remote", event.target.checked)} /><span>Allow remote HTTPS Worker</span></label>
            <small>Loopback is the default trust boundary. Live capabilities are read from the Worker on the Models page; expected values here are validation constraints.</small>
          </article>})}
        </div> : null}
      </Panel>}
      {view === "workers" && showExpertWizard && <ExpertModelSetupWizard settings={settings} onSaved={finish} onClose={() => setShowExpertWizard(false)} onError={onError} />}
      {view === "storage" && <Panel title="Pricing & hard budgets" eyebrow="Exact decimal accounting">
        <div className="json-settings">
          <div>
            <h3>{t("Pricing")}</h3>
            {Object.entries(pricing).map(([name, value]) => (
              <label key={name}>
                {name}
                <input
                  value={String(value)}
                  onChange={(event) =>
                    setSettings({
                      ...settings,
                      pricing: { ...pricing, [name]: event.target.value },
                    })
                  }
                />
              </label>
            ))}
          </div>
          <div>
            <h3>Budget</h3>
            {Object.entries(budget).map(([name, value]) => (
              <label key={name}>
                {name}
                <input
                  value={String(value)}
                  onChange={(event) =>
                    setSettings({
                      ...settings,
                      budget: {
                        ...budget,
                        [name]:
                          name === "max_cost"
                            ? event.target.value
                            : Number(event.target.value),
                      },
                    })
                  }
                />
              </label>
            ))}
          </div>
        </div>
      </Panel>}
      <div className="settings-save" aria-live="polite">
        <span>
          {dirty
            ? t("Unsaved workspace settings")
            : message ||
            (settings.settings_persisted
              ? `Saved at ${settings.settings_path}`
              : "Save once to keep these settings across restarts.")}
        </span>
        {dirty && (
          <button className="primary" onClick={save} disabled={saving}>
            {saving ? t("Saving…") : t("Save settings")}
          </button>
        )}
      </div>
    </section>
  );
}

function Panel({
  title,
  eyebrow,
  children,
}: {
  title: string;
  eyebrow: string;
  children: React.ReactNode;
}) {
  return (
    <section className="panel">
      <span className="eyebrow">{eyebrow}</span>
      <h2>{title}</h2>
      {children}
    </section>
  );
}
function Metric({
  label,
  value,
  detail,
  accent,
  live,
}: {
  label: string;
  value: string | number;
  detail: string;
  accent?: boolean;
  live?: boolean;
}) {
  return (
    <article className={`metric ${accent ? "accent" : ""}`}>
      <span>
        {label}
        {live && <i className="live-dot" />}
      </span>
      <strong>{value}</strong>
      <small>{detail}</small>
    </article>
  );
}
function Fact({ label, value }: { label: string; value: string | number }) {
  return (
    <div className="detail-fact">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}
function Status({ status }: { status: string }) {
  const normalized = status.replaceAll(" ", "_").toLowerCase();
  const presentation =
    normalized === "ready" ||
    normalized === "available" ||
    normalized === "completed" ||
    normalized === "confirmed" ||
    normalized === "auto_accepted" ||
    normalized === "published" ||
    normalized === "valid"
      ? {
          tone: "auto-accepted",
          label:
            normalized === "available"
              ? "Available"
              : normalized === "published"
              ? "Published"
              : normalized === "ready"
                ? "Ready"
              : normalized === "valid"
                ? "Valid"
                : "Completed",
        }
      : normalized === "completed_with_review" || normalized === "needs_review" || normalized === "waiting_for_human"
        ? {
            tone: "needs-review",
            label:
              normalized === "waiting_for_human"
                ? "Waiting for human"
                : "Completed with review",
          }
        : normalized === "needs_weights" || normalized === "model_required" || normalized === "installed" || normalized === "license_acceptance_required"
          ? { tone: "needs-review", label: normalized === "needs_weights" || normalized === "model_required" ? "Model required" : normalized === "license_acceptance_required" ? "License required" : "Needs test" }
        : normalized === "ready_to_install"
          ? { tone: "needs-review", label: "Ready to install" }
        : normalized === "fixture"
          ? { tone: "draft", label: "Fixture" }
        : normalized === "unsupported_platform"
          ? { tone: "failed", label: "Unsupported" }
        : normalized === "update_available"
          ? { tone: "running", label: "Update available" }
        : normalized === "discovered" || normalized === "installing" || normalized === "starting" || normalized === "preparing" || normalized === "smoke_testing" || normalized === "downloading" || normalized === "verifying" || normalized === "importing"
          ? { tone: "running", label: normalized.replaceAll("_", " ").replace(/^./, (value) => value.toUpperCase()) }
        : normalized === "unhealthy" || normalized === "crashed" || normalized === "failed_smoke_test" || normalized === "incompatible_api" || normalized === "invalid_manifest" || normalized === "invalid_contract" || normalized === "invalid_checksum" || normalized === "contract_mismatch" || normalized === "missing_plugin" || normalized === "missing_model_bundle" || normalized === "plugin_unavailable" || normalized === "incompatible_plugin" || normalized === "corrupted"
          ? { tone: "failed", label: normalized.replaceAll("_", " ").replace(/^./, (value) => value.toUpperCase()) }
        : normalized === "configured" || normalized === "unverified" || normalized === "unknown"
          ? { tone: "draft", label: normalized === "configured" ? "Configured" : normalized === "unverified" ? "Unverified" : "Unknown" }
        : normalized === "disabled"
          ? { tone: "rejected", label: "Disabled" }
        : normalized === "unreachable" || normalized === "invalid_credential" || normalized === "incompatible_protocol" || normalized === "unavailable"
          ? { tone: "failed", label: normalized.replaceAll("_", " ").replace(/^./, (value) => value.toUpperCase()) }
        : normalized === "rate_limited"
          ? { tone: "needs-review", label: "Rate limited" }
        : normalized === "incomplete"
          ? { tone: "needs-review", label: "Incomplete" }
        : normalized === "configuration_issue"
          ? { tone: "failed", label: "Configuration issue" }
        : normalized === "partial"
          ? { tone: "needs-review", label: "Partial" }
          : normalized === "cancelled" ||
              normalized === "rejected" ||
              normalized === "archived"
            ? {
                tone: "rejected",
                label: normalized === "archived" ? "Archived" : "Cancelled",
              }
            : normalized === "failed" ||
                normalized === "budget_exceeded" ||
                normalized === "interrupted"
              ? {
                  tone: "failed",
                  label:
                    normalized === "interrupted"
                      ? "Interrupted"
                      : normalized === "budget_exceeded"
                        ? "Budget exceeded"
                        : "Failed",
                }
              : normalized === "running" || normalized === "paused"
                ? {
                    tone: "running",
                    label: normalized === "paused" ? "Paused" : "Running",
                  }
                : normalized === "succeeded"
                  ? { tone: "auto-accepted", label: "Succeeded" }
                : {
                    tone: "draft",
                    label: normalized === "pending" ? "Pending" : "Draft",
                  };
  return (
    <span className={`status status-${presentation.tone}`}>
      {t(presentation.label)}
    </span>
  );
}
function Empty({ title, detail }: { title: string; detail: string }) {
  return (
    <div className="empty" role="status">
      <img src="/brand/core/annotagent-mark.svg" alt="" aria-hidden="true" />
      <strong>{title}</strong>
      <small>{detail}</small>
    </div>
  );
}
function TagGroup({ title, values }: { title: string; values: string[] }) {
  return (
    <div>
      <h3>{title}</h3>
      <div className="tags">
        {values.length ? (
          values.map((value) => <span key={value}>{value}</span>)
        ) : (
          <small>{t("None")}</small>
        )}
      </div>
    </div>
  );
}
