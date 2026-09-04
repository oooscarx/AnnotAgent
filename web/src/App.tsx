import { useEffect, useRef, useState } from "react";
import { api, subscribeEvents } from "./api";
import { AnnotationCanvas } from "./components/AnnotationCanvas";
import { ImproveAutomationPanel } from "./components/GeometrySafetyPanel";
import {
  PROVIDER_PRESETS,
  isEnvironmentVariableName,
} from "./providerCatalog";
import { visualProfilesForSkills } from "./skills/visualProfiles";
import { annotationColor, annotationVisual, type LabelVisualMapping } from "./annotationVisuals";
import { deriveProjectRunView } from "./runState";
import { projectForReview, projectForRun, runsForContext } from "./workspaceContext";
import {
  parseWorkspaceRoute,
  projectBatchPath,
  projectReviewPath,
  projectRunPath,
  projectRunsPath,
  type SettingsSection,
  type WorkspaceRoute,
} from "./navigation";
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
  RunEvent,
  RunAnnotationInspection,
  RunDebugSummary,
  RunNodeArtifactInspection,
  RunResultSummary,
  SkillDetail,
  WorkflowCatalog,
  WorkflowDraft,
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
    label: "Detection",
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
    label: "Verification",
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
  const [reviewQueue, setReviewQueue] = useState(0);
  const [events, setEvents] = useState<RunEvent[]>([]);
  const [error, setError] = useState("");
  const [loaded, setLoaded] = useState(false);
  const [connection, setConnection] = useState<"connecting" | "connected" | "reconnecting">("connecting");
  const hasConnectedRef = useRef(false);
  const needsReconnectSyncRef = useRef(false);
  const [activeProjectId, setActiveProjectId] = useState(() =>
    window.localStorage.getItem("annotagent.activeProjectId") ?? "",
  );
  const pageTitleRef = useRef<HTMLHeadingElement>(null);

  const navigate = (path: string, replace = false) => {
    if (replace) window.history.replaceState({}, "", path);
    else window.history.pushState({}, "", path);
    setRoute(
      parseWorkspaceRoute(
        window.location.pathname,
        window.location.search,
        window.location.hash,
      ),
    );
  };

  useEffect(() => {
    const restore = () =>
      setRoute(
        parseWorkspaceRoute(
          window.location.pathname,
          window.location.search,
          window.location.hash,
        ),
      );
    window.addEventListener("popstate", restore);
    return () => window.removeEventListener("popstate", restore);
  }, []);
  useEffect(() => {
    const current = `${window.location.pathname}${window.location.search}`;
    if (route.canonicalPath !== current || window.location.hash)
      navigate(route.canonicalPath, true);
  }, [route.canonicalPath]);

  const refresh = () =>
    api
      .dashboard()
      .then((data) => {
        setProjects(data.projects);
        setRuns(data.runs);
        setModels(data.models);
        setReviewQueue(data.review_queue);
        setLoaded(true);
      })
      .catch((reason: Error) => {
        setLoaded(true);
        setError(reason.message);
      });

  useEffect(() => {
    void refresh();
  }, []);
  useEffect(
    () =>
      subscribeEvents(
        (event) => {
          setEvents((previous) => [...previous.slice(-149), event]);
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
            void refresh();
        },
        () => {
          needsReconnectSyncRef.current = true;
          setConnection("reconnecting");
        },
        () => {
          setConnection("connected");
          if (hasConnectedRef.current || needsReconnectSyncRef.current) void refresh();
          hasConnectedRef.current = true;
          needsReconnectSyncRef.current = false;
        },
      ),
    [],
  );
  useEffect(() => {
    pageTitleRef.current?.focus();
  }, [route.canonicalPath]);

  const routeProjectId = (() => {
    switch (route.kind) {
      case "project":
      case "build":
      case "export":
      case "projectRuns":
      case "projectRun":
      case "projectBatch":
      case "projectReview":
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
  const selectedProject = projects.find((project) => project.id === projectId);
  const isProjectWorkspace = Boolean(routeProjectId);
  const setProjectContext = (id: string) => {
    setActiveProjectId(id);
    if (id) window.localStorage.setItem("annotagent.activeProjectId", id);
    else window.localStorage.removeItem("annotagent.activeProjectId");
  };
  const openProject = (id: string) => {
    setProjectContext(id);
    navigate(id ? `/projects/${encodeURIComponent(id)}` : "/projects");
  };
  const switchProject = (id: string) => {
    setProjectContext(id);
    if (!id) {
      navigate("/projects");
      return;
    }
    if (route.kind === "export")
      navigate(`/projects/${encodeURIComponent(id)}/export`);
    else if (route.kind === "build")
      navigate(`/projects/${encodeURIComponent(id)}/build/${route.step}`);
    else if (
      route.kind === "projectRuns" ||
      route.kind === "projectRun" ||
      route.kind === "projectBatch"
    )
      navigate(projectRunsPath(id));
    else if (route.kind === "projectReview")
      navigate(projectReviewPath(id));
    else if (route.kind === "project") openProject(id);
  };
  useEffect(() => {
    const resolved = routeProjectId || routeRunProject?.id;
    if (resolved && resolved !== activeProjectId) setProjectContext(resolved);
  }, [routeProjectId, routeRunProject?.id]);
  useEffect(() => {
    if (
      route.kind === "runs" &&
      route.runId &&
      routeRunProject
    ) {
      navigate(
        projectRunPath(routeRunProject.id, route.runId, {
          imageId: route.imageId,
          nodeId: route.nodeId,
          artifactId: route.artifactId,
          view: route.view,
        }),
        true,
      );
    }
  }, [route.kind, routeRun?.id, routeRunProject?.id]);
  const page: WorkspacePage =
    route.kind === "projectRuns" ||
    route.kind === "projectRun" ||
    route.kind === "projectBatch"
      ? "runs"
      : route.kind === "projectReview"
        ? "review"
        : route.kind;

  return (
    <div className="app-shell">
      <a className="skip-link" href="#main-content">
        Skip to workspace
      </a>
      <aside className="sidebar aa-dark">
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
        <nav aria-label="Primary navigation">
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
                  : !isProjectWorkspace && page === item.page
              }
              href={item.href}
              onClick={() => navigate(item.href)}
            >
              {item.label}
            </Nav>
          ))}
        </nav>
        <div className="sidebar-foot">
          <span className={`live-dot ${connection}`} aria-hidden="true" /> SSE {connection}
          <small>
            {events.at(-1)?.kind.replaceAll("_", " ") ?? "waiting for events"}
          </small>
        </div>
      </aside>
      <main
        id="main-content"
        aria-busy={!loaded}
        className={page === "review" ? "review-main" : undefined}
      >
        <header className="topbar">
          <div>
            <span className="product-tagline">{PRODUCT_TAGLINE}</span>
            <h1 ref={pageTitleRef} tabIndex={-1}>{PAGE_TITLES[page]}</h1>
          </div>
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
            <span aria-hidden="true">Project context</span>
            <label className="sr-only" htmlFor="active-project">
              Active project
            </label>
            <select
              id="active-project"
              value={projectId}
              onChange={(event) => switchProject(event.target.value)}
            >
              <option value="">{NO_PROJECT_MESSAGE}</option>
              {projects.map((project) => (
                <option key={project.id} value={project.id}>
                  {project.name}
                </option>
              ))}
            </select>
          </div>}
        </header>
        {error && (
          <div className="error-banner" role="alert">
            <span className="error-message">
              <strong>AnnotAgent couldn’t complete that action.</strong>
              <span>{readableErrorMessage(error)}</span>
              <small>Saved workspace data remains on the server. Reloading recovers the latest persisted state; retry is a separate Agent action.</small>
            </span>
            <span className="error-actions">
              <button onClick={() => window.location.reload()}>
                Reload latest state
              </button>
              <button aria-label="Dismiss error" onClick={() => setError("")}>
                Dismiss
              </button>
            </span>
          </div>
        )}
        {!loaded && <div className="loading-banner" role="status">Loading workspace state…</div>}
        {loaded && route.kind === "home" && (
          <Dashboard
            projects={projects}
            runs={runs}
            reviewQueue={reviewQueue}
            onSelect={openProject}
            onNewProject={() => navigate("/projects?new=1")}
            onOpenRuns={() => navigate("/runs")}
            onOpenReview={() => navigate("/review")}
            onRefresh={refresh}
          />
        )}
        {loaded && route.kind === "projects" && (
          <ProjectsPage
            projects={projects}
            createOnOpen={route.create}
            onSelect={openProject}
            onCustomize={(id) => {
              setProjectContext(id);
              navigate(`/projects/${encodeURIComponent(id)}/build/pipeline`);
            }}
            onRefresh={refresh}
            onError={setError}
          />
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
        {loaded && route.kind === "build" && route.step === "pipeline" && (
          <WorkflowsPage
            projects={projects}
            runs={runs}
            activeProjectId={projectId}
            onActivate={(id) =>
              navigate(`/projects/${encodeURIComponent(id)}/build/pipeline`)
            }
            onRefresh={refresh}
            onNavigate={(step, draftId) =>
              navigate(`/projects/${encodeURIComponent(route.projectId)}/build/${step}${step === "test" && draftId ? `?draft=${encodeURIComponent(draftId)}` : ""}`)
            }
            onOpenProjects={() => navigate("/projects")}
            onOpenProject={() => openProject(route.projectId)}
            onOpenProviders={() => navigate("/settings")}
            onOpenModels={() => navigate("/settings/models")}
            onOpenPlugins={() => navigate("/settings/plugins")}
            onError={setError}
          />
        )}
        {loaded && route.kind === "build" && route.step !== "pipeline" && (
          <BuildWorkspace
            project={selectedProject}
            step={route.step}
            selectedDraftId={route.draftId}
            onNavigate={(step, draftId, replace) =>
              navigate(`/projects/${encodeURIComponent(route.projectId)}/build/${step}${step === "test" && draftId ? `?draft=${encodeURIComponent(draftId)}` : ""}`, replace)
            }
            onOpenRuns={() =>
              navigate(projectRunsPath(route.projectId))
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
            onNavigate={navigate}
            onError={setError}
          />
        )}
        {loaded && (route.kind === "runs" || route.kind === "projectRuns" || route.kind === "projectRun") && (
          <RunsPage
            runs={runs}
            projects={projects}
            activeProject={selectedProject}
            route={route}
            onNavigate={navigate}
            onRefresh={refresh}
            onError={setError}
          />
        )}
        {loaded && route.kind === "projectBatch" && (
          <BatchDetailWorkspace
            route={route}
            runs={runs}
            projects={projects}
            onNavigate={navigate}
            onRefresh={refresh}
            onError={setError}
          />
        )}
        {loaded && (route.kind === "review" || route.kind === "projectReview") && (
          <ReviewPage
            project={selectedProject}
            projects={projects}
            events={events}
            route={route}
            onNavigate={navigate}
            onError={setError}
          />
        )}
        {loaded && route.kind === "settings" && (
          <SettingsWorkspace
            section={route.section}
            models={models}
            onNavigate={(section) =>
              navigate(section === "providers" ? "/settings" : `/settings/${section}`)
            }
            onError={setError}
          />
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
    <nav className="breadcrumb" aria-label="Breadcrumb">
      <button className="text-button" onClick={onOpenProjects}>Projects</button>
      {project && (
        <>
          <span aria-hidden="true">/</span>
          <button className="text-button" onClick={onOpenProject}>{project.name}</button>
        </>
      )}
      <span aria-hidden="true">/</span>
      <strong>{current}</strong>
    </nav>
  );
}

const BUILD_SEQUENCE = ["data", "labels", "pipeline", "test"] as const;
type BuildStep = (typeof BUILD_SEQUENCE)[number];

function useBuildSummary(
  project: ProjectSummary | undefined,
  onError: (value: string) => void,
): ProjectWorkspaceSummary | undefined {
  const [summary, setSummary] = useState<ProjectWorkspaceSummary>();
  useEffect(() => {
    if (!project) {
      setSummary(undefined);
      return;
    }
    void api
      .projectSummary(project.id)
      .then(setSummary)
      .catch((error: Error) => onError(error.message));
  }, [project]);
  return summary?.project.id === project?.id ? summary : undefined;
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
  onNavigate,
  onOpenRuns,
  onOpenProjects,
  onOpenProject,
  onRefresh,
  onError,
}: {
  project?: ProjectSummary;
  step: "data" | "labels" | "test";
  selectedDraftId?: string;
  onNavigate: (step: BuildStep, draftId?: string, replace?: boolean) => void;
  onOpenRuns: () => void;
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
        <Empty title="Project unavailable" detail="Return to Projects and choose a valid Project." />
      ) : !summary ? (
        <div className="loading-banner" role="status">Loading Build readiness…</div>
      ) : !allowed ? (
        <BuildBlocker guidance={summary.guidance} onNavigate={onNavigate} />
      ) : step === "data" ? (
        <BuildData project={project} onRefresh={onRefresh} onError={onError} />
      ) : step === "labels" ? (
        <BuildLabels project={project} onRefresh={onRefresh} onError={onError} />
      ) : (
        <BuildTestPublish project={project} selectedDraftId={selectedDraftId} onSelectDraft={(draftId, replace) => onNavigate("test", draftId, replace)} onNavigate={onNavigate} onOpenRuns={onOpenRuns} onRefresh={onRefresh} onError={onError} />
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
  return <section className="build-blocker" aria-label="Build step blocked">
    <span aria-hidden="true">!</span>
    <div><span className="eyebrow">Complete the current step first</span><h2>{guidance.headline}</h2><p>{guidance.explanation}</p></div>
    {destination && <button className="primary" onClick={() => onNavigate(destination)}>{guidance.primary_action.label}</button>}
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
    <span>Changes in this step are saved to the Project as you complete them.</span>
    <div className="button-row">
      {previous && <button onClick={() => onNavigate(previous)}>← {name(previous)}</button>}
      {next && <button className={nextEnabled && nextPrimary ? "primary" : ""} disabled={!nextEnabled} title={!nextEnabled ? "Complete this step before continuing" : undefined} onClick={() => onNavigate(next)}>Continue to {name(next)} →</button>}
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
    <nav className="section-tabs build-steps" aria-label="Build steps">
      {BUILD_SEQUENCE.map((item, index) => {
        const journey = guidance && journeyForBuildStep(guidance, item);
        const complete = journey?.state === "complete" || (item === "test" && guidance?.journey.find((entry) => entry.id === "activation")?.state === "complete");
        const allowed = guidance ? buildStepAllowed(guidance, item) : item === step;
        return (
        <button
          key={item}
          className={`${step === item ? "active" : ""} ${complete ? "complete" : ""}`.trim()}
          aria-current={step === item ? "step" : undefined}
          disabled={!allowed}
          title={!allowed ? "Complete the earlier Build step first" : journey?.detail}
          onClick={() => onNavigate(item)}
        >
          <span>{complete ? "✓" : index + 1}</span>
          {item === "pipeline" ? "Automation" : item === "test" ? "Test & Activate" : item[0].toUpperCase() + item.slice(1)}
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
      <div className="build-step-heading"><span className="eyebrow">Step 1 · Data</span><h2>Add images to your Project</h2><p>Import workspace-local PNG or JPEG files. AnnotAgent validates every image, skips matching content, and keeps the Project copy under its dataset root.</p></div>
      <div className="metrics-grid build-metrics">
        <Metric label="Project images" value={images.length} detail="Ready for sample testing" />
        <Metric label="Latest import" value={result?.imported ?? 0} detail={`${result?.discovered ?? 0} supported files found`} />
        <Metric label="Needs attention" value={(result?.duplicates ?? 0) + (result?.corrupt.length ?? 0)} detail={`${result?.duplicates ?? 0} duplicate · ${result?.corrupt.length ?? 0} corrupt`} />
      </div>
      <Panel title="Add more images" eyebrow="Workspace import">
        <label>
          Image file or folder
          <input value={source} onChange={(event) => setSource(event.target.value)} placeholder="/workspace/dataset/images" />
        </label>
        <div className="button-row">
          <button className={imagesLoaded && images.length === 0 ? "primary" : ""} disabled={busy || !source.trim()} onClick={importImages}>
            {busy ? "Importing…" : "Add images"}
          </button>
          <small>Supported: PNG and JPEG · recursive folder discovery · 100 MP decode safety limit</small>
        </div>
        {result && <div className="import-outcome" aria-live="polite">
          <strong>{result.imported} images added</strong>
          <span>{result.discovered} discovered · {result.duplicates} duplicates skipped · {result.unsupported_files} unsupported files ignored</span>
          <small>Source: {result.source}</small>
          {result.corrupt.length > 0 && <details><summary>{result.corrupt.length} corrupt images were not imported</summary><ul>{result.corrupt.map((issue) => <li key={`${issue.name}:${issue.message}`}><strong>{issue.name}</strong> — {issue.message}</li>)}</ul></details>}
        </div>}
      </Panel>
      <Panel title="Project images" eyebrow={`${images.length} registered · ${project.dataset.root}`}>
        {images.length ? <div className="build-image-list">
          {images.map((image) => <article key={image.image_id}>
            <img src={image.url} alt="" />
            <span><strong>{image.name}</strong><small>{image.path} · {(image.size_bytes / 1024).toFixed(1)} KB</small></span>
            <button className="danger-text" disabled={busy} onClick={() => removeImage(image)} aria-label={`Remove ${image.name} from Project`}>Remove</button>
          </article>)}
        </div> : <Empty title="No images yet" detail="Add a supported image or folder to complete the Data step." />}
        <details className="advanced-settings"><summary>Dataset discovery settings</summary><Fact label="Discovery" value={project.dataset.recursive ? "Recursive" : "Top level"} /><TagGroup title="Include patterns" values={project.dataset.include} /></details>
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
      <div className="build-step-heading"><span className="eyebrow">Step 2 · Labels</span><h2>What do you want to annotate?</h2><p>Labels describe the meaning and output you want. Models and execution order belong to the next Automation step.</p></div>
      <div className="build-label-layout">
      <Panel title="Add a Label group" eyebrow="Annotation meaning">
        <div className="label-group-form">
        <label>What should this group be called?<input value={displayName} onChange={(event) => setDisplayName(event.target.value)} placeholder="Football" /></label>
        <label>What kind of annotation?<select value={kind} onChange={(event) => setKind(event.target.value)}>
          <option value="classification">Classification</option>
          <option value="bounding_box">Bounding box</option>
          <option value="keypoints">Keypoints</option>
          <option value="polygon">Polygon</option>
          <option value="semantic_mask">Semantic mask</option>
        </select></label>
        <label>Labels to use<input value={labels} onChange={(event) => setLabels(event.target.value)} placeholder="football, training ball" /></label>
        <div className="label-output-preview"><span>Output</span><strong>{outputName(kind)}</strong></div>
        <details className="advanced-settings"><summary>Attributes and internal settings</summary>
          <div className="form-grid">
            <label>Optional attribute<input value={attributeName} onChange={(event) => setAttributeName(event.target.value)} placeholder="occluded" /></label>
            <label>Attribute type<select value={attributeKind} onChange={(event) => setAttributeKind(event.target.value as typeof attributeKind)}>
              <option value="string">Text</option><option value="boolean">Boolean</option><option value="number">Number</option><option value="enum">Choice</option>
            </select></label>
          </div>
          <small>The internal task ID is generated from the display name and validated by Core. Raw Schema fields remain an Advanced concern.</small>
        </details>
        <button className={project.annotation_schema.length === 0 ? "primary form-submit-action" : "form-submit-action"} disabled={busy || !displayName.trim() || !labels.trim()} onClick={create}>{busy ? "Adding…" : "Add Label group"}</button>
        </div>
      </Panel>
      <Panel title="Current Labels" eyebrow={`${project.task_count} groups`}>
        <div className="label-definition-list">
          {project.annotation_schema.map((task) => (
            <article key={task.id}><span><strong>{kindName(task.kind)}</strong><small>{task.display_name}</small></span><dl><div><dt>Labels</dt><dd>{task.labels.join(", ") || "None"}</dd></div><div><dt>Output</dt><dd>{outputName(task.kind)}</dd></div></dl><details><summary>Advanced internal ID</summary><code>{task.id}</code></details></article>
          ))}
        </div>
        {project.annotation_schema.length === 0 && <Empty title="No Labels defined" detail="Create the first semantic Label group. Models and execution order belong in Pipeline." />}
      </Panel>
      </div>
    </>
  );
}

function BuildTestPublish({
  project,
  selectedDraftId,
  onSelectDraft,
  onNavigate,
  onOpenRuns,
  onRefresh,
  onError,
}: {
  project: ProjectSummary;
  selectedDraftId?: string;
  onSelectDraft: (draftId: string, replace?: boolean) => void;
  onNavigate: (step: BuildStep, draftId?: string, replace?: boolean) => void;
  onOpenRuns: () => void;
  onRefresh: () => Promise<void>;
  onError: (value: string) => void;
}) {
  const [drafts, setDrafts] = useState<WorkflowDraft[]>([]);
  const [draftId, setDraftId] = useState("");
  const [sampleCount, setSampleCount] = useState(3);
  const [report, setReport] = useState<WorkflowDryRunReport>();
  const [reportLoading, setReportLoading] = useState(false);
  const [staleReport, setStaleReport] = useState(false);
  const [restoredAt, setRestoredAt] = useState<string>();
  const [images, setImages] = useState<ImageItem[]>([]);
  const [activated, setActivated] = useState<{ workflow_id: string; version: number }>();
  const [busy, setBusy] = useState(false);
  const [startingRun, setStartingRun] = useState(false);
  const load = (selectFallback = true) => api.workflowDrafts(project.id).then((value) => {
    setDrafts(value.drafts);
    const available = value.drafts.filter((draft) => draft.status !== "archived");
    setDraftId((current) => {
      const requested = available.find((draft) => draft.id === selectedDraftId)?.id;
      const retained = available.find((draft) => draft.id === current)?.id;
      const restored = available.find(
        (draft) => draft.id === value.latest_current_sample_test_draft_id,
      )?.id;
      return requested ?? retained ?? (selectFallback ? restored ?? available[0]?.id ?? "" : "");
    });
  });
  useEffect(() => {
    void Promise.all([load(), api.images(project.id).then((value) => setImages(value.images))])
      .catch((error: Error) => onError(error.message));
  }, [project.id]);
  useEffect(() => {
    if (draftId && draftId !== selectedDraftId) onSelectDraft(draftId, true);
  }, [draftId, selectedDraftId]);
  useEffect(() => {
    let cancelled = false;
    setReport(undefined);
    setRestoredAt(undefined);
    setStaleReport(false);
    if (!draftId) {
      setReportLoading(false);
      return () => { cancelled = true; };
    }
    setReportLoading(true);
    void api.workflowSampleTest(draftId)
      .then(({ sample_test: sampleTest, current }) => {
        if (cancelled) return;
        if (sampleTest && current) {
          setReport(sampleTest.report);
          setRestoredAt(sampleTest.completed_at);
        } else {
          setStaleReport(Boolean(sampleTest));
        }
      })
      .catch((error: Error) => {
        if (!cancelled) onError(error.message);
      })
      .finally(() => {
        if (!cancelled) setReportLoading(false);
      });
    return () => { cancelled = true; };
  }, [draftId]);
  const test = () => {
    if (!draftId) return;
    setBusy(true);
    void api.dryRunWorkflow(draftId, Array.from({ length: sampleCount }, (_, index) => index))
      .then((value) => {
        setActivated(undefined);
        setReport(value);
        setRestoredAt(undefined);
        setStaleReport(false);
      })
      .then(() => load())
      .catch((error: Error) => onError(error.message))
      .finally(() => setBusy(false));
  };
  const publish = () => {
    if (!draftId || !report?.validation.valid || drafts.find((draft) => draft.id === draftId)?.status === "published") return;
    setBusy(true);
    void api.publishWorkflow(draftId)
      .then((version) => {
        setActivated(version);
        return Promise.all([onRefresh(), load(false)]);
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
  const currentDraft = drafts.find((draft) => draft.id === draftId);
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
      .then(async () => {
        await onRefresh();
        onOpenRuns();
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
    if (nextDraftId) onSelectDraft(nextDraftId, true);
  };
  const draftControls = <div className="sample-test-controls" aria-label="Sample Test controls">
    <label className="sample-test-field"><span>Automation Draft</span><select aria-label="Current Draft" value={draftId} onChange={(event) => chooseDraft(event.target.value)}><option value="">Choose Current Draft…</option>{drafts.filter((draft) => draft.status !== "archived").map((draft) => <option key={draft.id} value={draft.id}>{draft.name} · {draft.status === "published" ? "Activated" : draft.status.replaceAll("_", " ")}</option>)}</select></label>
    <label className="sample-test-field"><span>Sample images</span><input type="number" min="1" max="10" value={sampleCount} onChange={(event) => setSampleCount(Math.max(1, Math.min(10, Number(event.target.value))))} /></label>
    <button className={!report ? "primary" : ""} onClick={test} disabled={busy || reportLoading || !draftId || isActivated}>{busy ? "Testing…" : isActivated ? "Already activated" : report ? "Test again" : "Test samples"}</button>
  </div>;
  return (
    <>
      <div className="toolbar-panel sample-test-toolbar">
        <div className="sample-test-toolbar-copy"><span className="eyebrow">Step 4 · Test & Activate</span><h2>Test samples, then activate automation</h2><p>A Sample Test executes 1–10 real Project images in a sandbox and never writes formal annotations. Activation publishes the tested Draft as an immutable Version.</p></div>
        <button className="sample-test-back" onClick={() => onNavigate("pipeline")}>← Edit Automation</button>
        {draftControls}
      </div>
      {!report && <ol className="activation-lifecycle" aria-label="Automation activation lifecycle">
        <li className={draftId ? "complete" : "current"}><span>1</span><strong>{draftId ? "Unpublished changes" : "Choose a Draft"}</strong></li>
        <li className={draftId ? "current" : ""}><span>2</span><strong>Check setup</strong></li>
        <li><span>3</span><strong>Test samples</strong></li>
        <li><span>4</span><strong>Activate automation</strong></li>
      </ol>}
      {summary ? (
        <>
          <section className={`sample-test-hero ${report.validation.valid ? "ready" : "blocked"}`} aria-label="Dry Run result summary">
            <div className="sample-test-hero-copy">
              <span className="eyebrow">{isActivated ? "Activated evidence" : report.validation.valid ? "Ready to activate" : "Automation needs changes"}</span>
              <h2>Sample test complete</h2>
              <p>AnnotAgent tested real Project images in a sandbox. No formal Annotations were written.</p>
            </div>
            <dl className="sample-outcome-metrics">
              <div><dt>Images</dt><dd>{summary.image_count}</dd><small>tested</small></div>
              <div><dt>Results found</dt><dd>{resultCount}</dd><small>{summary.auto_accepted_count} ready to accept</small></div>
              <div><dt>Needs attention</dt><dd>{needsAttention}</dd><small>{summary.needs_review_count} review · {summary.failed_count} failed</small></div>
            </dl>
            <div className="sample-test-context">
              <span>{summary.empty_count} no-target result{summary.empty_count === 1 ? "" : "s"}</span>
              <span>{summary.fallback_count} fallback{summary.fallback_count === 1 ? "" : "s"}</span>
              <span>{summary.cache_hit_count} cache hit{summary.cache_hit_count === 1 ? "" : "s"}</span>
              <span>{formatSampleDuration(summary.duration_ms)}</span>
              <span>${summary.usage.estimated_cost} sample cost</span>
              {restoredAt && <span title={new Date(restoredAt).toLocaleString()}>Restored saved Sample Test</span>}
            </div>
            {isActivated ? <div className="activation-success" role="status"><span><strong>Automation activated</strong><small>This saved Sample Test belongs to the immutable active Version.</small></span><button className="primary" disabled={!publishedWorkflow || startingRun || Boolean(project.active_batch || project.active_run)} onClick={startFullRun}>{startingRun ? "Starting…" : project.active_batch || project.active_run ? "Run already active" : "Start full Run"}</button></div> : <>
              <div className="button-row">
                {!report.validation.valid || summary.failed_count > 0 ? <button className="primary" onClick={() => onNavigate("pipeline")}>Fix automation</button> : summary.needs_review_count > 0 ? <button className="primary" onClick={() => document.getElementById("uncertain-results")?.scrollIntoView({ behavior: "smooth", block: "start" })}>Review uncertain result</button> : <button className="primary" onClick={publish} disabled={busy || Boolean(activated)}>{busy ? "Activating…" : "Activate automation"}</button>}
                {report.validation.valid && summary.needs_review_count > 0 && <button onClick={publish} disabled={busy || Boolean(activated)}>{busy ? "Activating…" : "Activate with Review gate"}</button>}
              </div>
              {activated && <div className="activation-success" role="status"><span><strong>Automation activated</strong><small>Immutable Version v{activated.version} is ready for the full Dataset Run.</small></span><button className="primary" disabled={startingRun || Boolean(project.active_batch || project.active_run)} onClick={startFullRun}>{startingRun ? "Starting…" : project.active_batch || project.active_run ? "Run already active" : "Start full Run"}</button></div>}
            </>}
          </section>
          {fullRun && <section className="full-run-estimate" aria-label="Full Run Estimate">
            <div><span className="eyebrow">Full Run Estimate</span><h2>{fullRun.image_count} Project images</h2><p>Projected from this Sample Test; actual usage can vary with image content and Provider behavior.</p></div>
            <dl><div><dt>Estimated cost</dt><dd>${fullRun.estimated_cost}</dd></div><div><dt>Estimated duration</dt><dd>{formatSampleDuration(fullRun.duration_ms)}</dd></div><div><dt>Review workload</dt><dd>{fullRun.review_count_min === fullRun.review_count_max ? fullRun.review_count_min : `${fullRun.review_count_min}–${fullRun.review_count_max}`} results</dd></div></dl>
          </section>}
          <section className="sample-results-section" aria-labelledby="sample-results-title">
            <div className="section-heading"><div><span className="eyebrow">Results Gallery</span><h2 id="sample-results-title">What the automation found</h2></div><small>{summary.image_count} sandbox image{summary.image_count === 1 ? "" : "s"}</small></div>
            <div className="sample-results-gallery">{report.samples.map((sample) => <SampleResultCard key={`${sample.image_index}-${sample.image_name}`} sample={sample} image={images.find((item) => item.index === sample.image_index)} />)}</div>
          </section>
          <section className="sample-results-section uncertain-results" id="uncertain-results" aria-labelledby="uncertain-results-title">
            <div className="section-heading"><div><span className="eyebrow">Uncertain Results</span><h2 id="uncertain-results-title">What needs a human decision</h2></div><small>{uncertainSamples.length} image{uncertainSamples.length === 1 ? "" : "s"}</small></div>
            {uncertainSamples.length ? <div className="sample-results-gallery">{uncertainSamples.map((sample) => <SampleResultCard key={`uncertain-${sample.image_index}-${sample.image_name}`} sample={sample} image={images.find((item) => item.index === sample.image_index)} compact />)}</div> : <div className="positive-empty"><strong>No uncertain results in this sample</strong><span>The configured confidence and Review gates accepted every result.</span></div>}
          </section>
          <section className="sample-diagnostics" aria-label="Sample Test diagnostics">
            <div className="section-heading"><div><span className="eyebrow">Diagnostics</span><h2>Inspect only when you need to troubleshoot</h2></div></div>
            <details><summary>Pipeline Diagnostics</summary><div>{report.validation.issues.map((issue) => <div className="error-banner" key={`${issue.path}-${issue.code}`}><span>{issue.code}: {issue.message}</span></div>)}{!report.validation.issues.length && <p>No blocking static or execution issues.</p>}</div></details>
            <details><summary>Model Usage</summary><dl className="diagnostic-facts"><div><dt>Input tokens</dt><dd>{summary.usage.input_tokens.toLocaleString()}</dd></div><div><dt>Output tokens</dt><dd>{summary.usage.output_tokens.toLocaleString()}</dd></div><div><dt>Estimated cost</dt><dd>${summary.usage.estimated_cost}</dd></div></dl></details>
            <details><summary>Node Timings</summary>{report.samples.map((sample) => <div className="diagnostic-sample" key={`timing-${sample.image_index}`}><strong>{sample.image_name}</strong>{sample.nodes.map((node) => <span key={node.node_id}>{node.node_id}<small>{node.latency_ms} ms · {node.status}</small></span>)}</div>)}</details>
            <details><summary>Technical Artifacts</summary>{report.samples.map((sample) => <div className="diagnostic-sample" key={`artifacts-${sample.image_index}`}><strong>{sample.image_name}</strong>{sample.nodes.filter((node) => node.output_types.length).map((node) => <span key={node.node_id}>{node.node_id}<small>{node.output_types.join(", ")}</small></span>)}</div>)}</details>
          </section>
        </>
      ) : reportLoading ? <div className="loading-banner" role="status">Restoring the saved Sample Test…</div> : staleReport ? <Empty title="Sample Test is out of date" detail="This Draft changed after its saved Sample Test. Test the current Draft again before activation." /> : <Empty title="No Sample Test result" detail="Choose a Current Draft and test 1–10 images to see result counts, diagnostics, and trace." />}
      {!isActivated && <details className="advanced-settings"><summary>Discard this Draft</summary><p>Archiving removes this unpublished Draft from the active Build flow. Published Versions are never changed.</p><button onClick={discard} disabled={busy || !draftId}>Discard unpublished changes</button></details>}
    </>
  );
}

function formatSampleDuration(durationMs: number) {
  if (durationMs < 1_000) return `${durationMs} ms`;
  if (durationMs < 60_000) return `${(durationMs / 1_000).toFixed(durationMs < 10_000 ? 1 : 0)} sec`;
  return `${Math.ceil(durationMs / 60_000)} min`;
}

function SampleResultCard({
  sample,
  image,
  compact = false,
}: {
  sample: WorkflowDryRunReport["samples"][number];
  image?: ImageItem;
  compact?: boolean;
}) {
  const boxes = sample.outcomes.filter((outcome) => outcome.value?.kind === "bounding_box");
  const state = sample.failed ? "Failed" : sample.review_count ? "Needs review" : sample.empty ? "No target found" : "Ready";
  return <article className={`sample-result-card ${sample.failed ? "failed" : sample.review_count ? "review" : "ready"} ${compact ? "compact" : ""}`}>
    <figure className="sample-result-preview" style={{ aspectRatio: `${sample.width} / ${sample.height}` }}>
      {image ? <img src={image.url} alt={sample.image_name} /> : <div className="image-placeholder">Preview unavailable</div>}
      {boxes.map((outcome) => {
        const rect = outcome.value?.kind === "bounding_box" ? outcome.value.rect : undefined;
        return rect ? <span className="sample-result-box" key={outcome.id} style={{ left: `${rect[0] * 100}%`, top: `${rect[1] * 100}%`, width: `${rect[2] * 100}%`, height: `${rect[3] * 100}%` }}><b>{outcome.label}{outcome.confidence != null ? ` ${Math.round(outcome.confidence * 100)}%` : ""}</b></span> : null;
      })}
    </figure>
    <div className="sample-result-body"><div><strong>{sample.image_name}</strong><Status status={state} /></div><p>{sample.failed ? "A Pipeline step failed on this image." : sample.empty ? "No target found. This is a valid empty result." : `${sample.result_count} result${sample.result_count === 1 ? "" : "s"} · ${sample.auto_accepted_count} ready · ${sample.review_count} review`}</p>{sample.outcomes.length > 0 && <ul>{sample.outcomes.map((outcome) => <li key={`summary-${outcome.id}`}><span>{outcome.label}</span><small>{outcome.status.replaceAll("_", " ")}{outcome.confidence != null ? ` · ${Math.round(outcome.confidence * 100)}%` : ""}</small></li>)}</ul>}</div>
  </article>;
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
      <nav className="section-tabs" aria-label="Settings sections">
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
            {label}
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

function Dashboard({
  projects,
  runs,
  reviewQueue,
  onSelect,
  onNewProject,
  onOpenRuns,
  onOpenReview,
  onRefresh,
}: {
  projects: ProjectSummary[];
  runs: HistoryRun[];
  reviewQueue: number;
  onSelect: (id: string) => void;
  onNewProject: () => void;
  onOpenRuns: () => void;
  onOpenReview: () => void;
  onRefresh: () => void;
}) {
  const activeRuns = runs.filter(
    (run) =>
      run.controllable && (run.status === "running" || run.status === "paused"),
  ).length;
  const failures = runs.filter((run) =>
    ["failed", "interrupted", "budget_exceeded"].includes(run.status),
  );
  const tokens = runs.reduce(
    (sum, run) => sum + run.input_tokens + run.output_tokens,
    0,
  );
  const cost = runs.reduce((sum, run) => sum + Number(run.cost || 0), 0);
  return (
    <section className="page-stack">
      <div className="hero-panel aa-dark">
        <div>
          <span className="kicker">Guided annotation workspace</span>
          <h2>
            Move vision data from setup
            <br />
            {" "}
            <em>to reviewed output.</em>
          </h2>
          <p>
            Open a Project to import data, define Labels, build and test a
            Pipeline, run it, inspect its work, and review the result.
          </p>
        </div>
        <div className="hero-actions">
          <button
            className="primary"
            onClick={onNewProject}
          >
            New project
          </button>
          <button onClick={onRefresh}>Refresh state</button>
        </div>
      </div>
      <div className="metrics-grid platform-metrics">
        <Metric
          label="Projects"
          value={projects.length}
          detail={`${projects.reduce((sum, project) => sum + project.image_count, 0)} images registered`}
        />
        <Metric
          label="Active runs"
          value={activeRuns}
          detail={`${runs.length} total executions`}
          live={activeRuns > 0}
        />
        <Metric
          label="Review queue"
          value={reviewQueue}
          detail="Annotations requiring attention"
          accent={reviewQueue > 0}
        />
      </div>
      <p className="platform-usage-line" aria-label="Workspace model usage">Persisted model usage across {runs.length} Run{runs.length === 1 ? "" : "s"}: <strong>{tokens.toLocaleString()} tokens</strong> · <strong>${cost.toFixed(4)}</strong></p>
      <div className="platform-grid">
        <Panel title="Recent projects" eyebrow="Concrete annotation work">
          <ProjectList projects={projects.slice(0, 5)} onSelect={onSelect} />
        </Panel>
        <Panel title="Active runs" eyebrow="Work in progress">
          <button className="summary-link" onClick={onOpenRuns}>
            <strong>{activeRuns} active</strong>
            <small>Open progress, errors, cost, and artifacts</small>
          </button>
        </Panel>
        <Panel title="Needs review" eyebrow="Human decisions">
          <button className="summary-link" onClick={onOpenReview}>
            <strong>{reviewQueue} waiting</strong>
            <small>Accept, edit, reject, or remove results</small>
          </button>
        </Panel>
        <Panel title="Recent failures" eyebrow="Requires attention">
          {failures.length ? (
            <button className="summary-link" onClick={onOpenRuns}>
              <strong>{failures[0].workflow_name}</strong>
              <small>{failures[0].terminal_reason ?? failures[0].status}</small>
            </button>
          ) : (
            <Empty title="No recent failures" detail="Recent terminal runs are healthy." />
          )}
        </Panel>
      </div>
    </section>
  );
}

function ProjectsPage({
  projects,
  createOnOpen,
  onSelect,
  onCustomize,
  onRefresh,
  onError,
}: {
  projects: ProjectSummary[];
  createOnOpen?: boolean;
  onSelect: (id: string) => void;
  onCustomize: (id: string) => void;
  onRefresh: () => Promise<void>;
  onError: (value: string) => void;
}) {
  const [creating, setCreating] = useState(Boolean(createOnOpen));
  return (
    <section className="page-stack">
      <div className="toolbar-panel">
        <div>
          <span className="eyebrow">Project inventory</span>
          <h2>Datasets, schemas, Workflows, and bindings</h2>
          <p>
            A Project is concrete annotation work; Skills remain reusable
            extensions.
          </p>
        </div>
        <button className="primary" onClick={() => setCreating(true)}>
          New project
        </button>
      </div>
      <Panel title="All projects" eyebrow={`${projects.length} configured`}>
        <ProjectList projects={projects} onSelect={onSelect} />
      </Panel>
      {creating && (
        <CreateProject
          onClose={() => setCreating(false)}
          onCreated={(projectId, customize) => {
            setCreating(false);
            void onRefresh().then(() =>
              customize ? onCustomize(projectId) : onSelect(projectId),
            );
          }}
          onError={onError}
        />
      )}
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
          title="No projects yet"
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
  const [workflowKey, setWorkflowKey] = useState("");
  const [importSource, setImportSource] = useState("");
  const [importFormat, setImportFormat] = useState("native");
  const [importDryRun, setImportDryRun] = useState(true);
  const [importResult, setImportResult] = useState("");
  const [labelTaskId, setLabelTaskId] = useState("");
  const [newLabel, setNewLabel] = useState("");
  const [skillCatalog, setSkillCatalog] = useState<SkillDetail[]>([]);
  const [selectedSkillIds, setSelectedSkillIds] = useState<string[]>([]);
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
  useEffect(() => {
    setLabelTaskId(project?.annotation_schema[0]?.id ?? "");
    setNewLabel("");
    setSelectedSkillIds(project?.enabled_skills.map((skill) => skill.id) ?? []);
  }, [project?.id]);
  useEffect(() => {
    void api.skills().then(setSkillCatalog).catch((error: Error) => onError(error.message));
  }, []);
  if (!project)
    return (
      <section className="page-stack">
        <Empty
          title="No project opened"
          detail="Choose a Project from Projects or the active Project switcher."
        />
      </section>
    );
  if (!activeWorkspace)
    return (
      <section className="page-stack">
        <div className="loading-banner" role="status">Loading Project guidance…</div>
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
  const importAnnotations = () => {
    if (!importSource.trim()) return onError("Choose a workspace-local annotation file or directory.");
    setImportResult("Import running…");
    void api
      .importAnnotations(project.id, importFormat, importSource, importDryRun)
      .then((report) => {
        setImportResult(
          `${report.dry_run ? "Dry run" : "Imported"}: ${report.imported_count} accepted, ${report.skipped_count} skipped\n${[...report.warnings, ...report.issues.map((issue) => `${issue.record}: ${issue.message}`)].join("\n")}`,
        );
        if (!report.dry_run) void refreshWorkspace();
      })
      .catch((error: Error) => {
        setImportResult("");
        onError(error.message);
      });
  };
  const addLabel = () => {
    if (!labelTaskId || !newLabel.trim())
      return onError("Choose a task and enter a Label id.");
    void api
      .addProjectLabel(project.id, labelTaskId, newLabel.trim())
      .then(() => {
        setNewLabel("");
        void refreshWorkspace();
      })
      .catch((error: Error) => onError(error.message));
  };
  const saveSkills = () => {
    const resolved = new Set(selectedSkillIds);
    let changed = true;
    while (changed) {
      changed = false;
      for (const id of [...resolved]) {
        const skill = skillCatalog.find((item) => item.id === id);
        const legacyPack =
          skill?.kind === "pack" &&
          project.enabled_skills.length === 1 &&
          project.enabled_skills[0]?.id === id;
        if (legacyPack) continue;
        for (const requirement of skill?.capability_requirements ?? []) {
          const dependency = requirement.split("@")[0];
          if (!resolved.has(dependency)) {
            resolved.add(dependency);
            changed = true;
          }
        }
      }
    }
    const enabled = [...resolved].sort().map((id) => {
      const skill = skillCatalog.find((item) => item.id === id);
      return { id, version: skill?.version ?? "1" };
    });
    void api
      .setProjectSkills(project.id, enabled)
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
        <button className="active" aria-current="page">Overview</button>
        <button onClick={() => onOpenBuild("data")}>Build</button>
        <button onClick={() => onNavigate(projectRunsPath(project.id))}>Runs</button>
        <button onClick={onOpenReview}>Review</button>
        <button onClick={() => onNavigate(`/projects/${encodeURIComponent(project.id)}/export`)}>Export</button>
      </nav>
      <header className="project-context-header">
        <div>
          <span className="eyebrow">Project workspace</span>
          <h2>{project.name}</h2>
          <p>{project.description || "No Project description provided."}</p>
        </div>
        <div className="project-context-facts" aria-label="Project status">
          <span><b>{project.image_count}</b> Images</span>
          <span><b>{labelCount}</b> Labels</span>
          <span><b>{project.review_count}</b> Needs review</span>
        </div>
        <div className="project-context-status" aria-label="Project operational status">
          <span>Automation <b>{project.default_workflow_version?.name ?? "Not active"}</b></span>
          <span>Active run <b>{project.active_run?.status ?? project.active_batch?.status ?? "None"}</b></span>
          <span>Readiness <b>{guidance.stage.replaceAll("_", " ")}</b></span>
        </div>
      </header>

      <section className="guidance-hero" aria-labelledby="project-guidance-title">
        <div className="guidance-copy">
          <span className="eyebrow">Next step · {guidance.completed_steps} of {guidance.total_steps} complete</span>
          <h2 id="project-guidance-title">{guidance.headline}</h2>
          <p>{guidance.explanation}</p>
          <div className="guidance-progress" aria-label={`${guidance.completed_steps} of ${guidance.total_steps} journey steps complete`}>
            <i style={{ width: `${(guidance.completed_steps / guidance.total_steps) * 100}%` }} />
          </div>
        </div>
        <div className="guidance-actions">
          <button
            className="primary"
            disabled={starting || !guidance.primary_action.enabled}
            title={guidance.primary_action.disabled_reason}
            onClick={() => runGuidedAction(guidance.primary_action)}
          >
            {starting ? "Starting…" : guidance.primary_action.label}
          </button>
          {guidance.secondary_actions.slice(0, 2).map((action) => (
            <button key={`${action.kind}:${action.destination}`} disabled={!action.enabled} title={action.disabled_reason} onClick={() => runGuidedAction(action)}>{action.label}</button>
          ))}
          {project.available_workflow_versions.some((workflow) => workflow.status === "published") && <button onClick={onOpenWorkflows}>Improve automation</button>}
        </div>
        {guidance.blockers.length > 0 && <div className="guidance-blockers" aria-label="Project blockers">
          {guidance.blockers.map((blocker) => <article key={blocker.code}>
            <span aria-hidden="true">!</span>
            <div><strong>{blocker.title}</strong><small>{blocker.explanation}</small></div>
            {blocker.repair_action && blocker.repair_action.kind !== guidance.primary_action.kind && <button onClick={() => runGuidedAction(blocker.repair_action!)}>{blocker.repair_action.label}</button>}
          </article>)}
        </div>}
      </section>

      <section className="journey-panel" aria-labelledby="project-journey-title">
        <div className="section-heading"><div><span className="eyebrow">Project journey</span><h2 id="project-journey-title">From data to compatible export</h2></div><small>Server-owned state · updated {new Date(guidance.updated_at).toLocaleString()}</small></div>
        <ol className="journey-timeline">
          {guidance.journey.map((step, index) => <li key={step.id} className={step.state}>
            <button onClick={() => openJourneyStep(step)} disabled={!step.destination} aria-label={`${step.label}: ${step.detail}`}>
              <i aria-hidden="true">{step.state === "complete" ? "✓" : index + 1}</i>
              <span><strong>{step.label}</strong><small>{step.detail}</small></span>
              <b>{step.state.replaceAll("_", " ")}</b>
            </button>
          </li>)}
        </ol>
      </section>
      {(project.active_batch || project.active_run || project.last_run || selectedPublishedWorkflow) && <div className="run-state-grid" id="project-active-run">
        <Panel title="Active Run" eyebrow="Server-owned state">
          {project.active_batch ? (
            <>
              <Fact label="Batch" value={project.active_batch.id.slice(0, 8)} />
              <Status status={project.active_batch.status} />
              {project.active_batch_progress && (
                <Fact
                  label="Images"
                  value={`${project.active_batch_progress.completed_images}/${project.active_batch_progress.total_images}`}
                />
              )}
              <Fact
                label="Progress events"
                value={String(project.active_batch.event_sequence)}
              />
              <div className="button-row" aria-label="Active Batch controls">
                {visibleStatus === "running" && <button onClick={() => control("pause")}><img src="/brand/core/icons/pause.svg" alt="" aria-hidden="true" /> Pause</button>}
                {visibleStatus === "paused" && <button onClick={() => control("resume")}><img src="/brand/core/icons/resume.svg" alt="" aria-hidden="true" /> Resume</button>}
                <button className="danger" onClick={() => control("cancel")}><img src="/brand/core/icons/cancel.svg" alt="" aria-hidden="true" /> Cancel</button>
              </div>
            </>
          ) : project.active_run ? (
            <>
              <Fact label="Run" value={project.active_run.id.slice(0, 8)} />
              <Status status={project.active_run.status} />
              <div className="button-row" aria-label="Active Run controls">
                {visibleStatus === "running" && <button onClick={() => control("pause")}><img src="/brand/core/icons/pause.svg" alt="" aria-hidden="true" /> Pause</button>}
                {visibleStatus === "paused" && <button onClick={() => control("resume")}><img src="/brand/core/icons/resume.svg" alt="" aria-hidden="true" /> Resume</button>}
                <button className="danger" onClick={() => control("cancel")}><img src="/brand/core/icons/cancel.svg" alt="" aria-hidden="true" /> Cancel</button>
              </div>
            </>
          ) : (
            <div className="empty-run-state">
              <Empty
                title="No active Run"
                detail="Run every image with the selected immutable Workflow Version."
              />
              {selectedPublishedWorkflow && <button disabled={starting} onClick={startBatch}>{starting ? "Starting…" : "Start full Run"}</button>}
            </div>
          )}
        </Panel>
        <Panel title="Last Run" eyebrow="Terminal history">
          {project.last_run ? (
            <>
              <Fact label="Run" value={project.last_run.id.slice(0, 8)} />
              <Status status={project.last_run.status} />
              {project.last_run.terminal_reason && (
                <small className="run-reason">
                  {project.last_run.terminal_reason}
                </small>
              )}
            </>
          ) : (
            <Empty
              title="No completed Run"
              detail="Terminal history will appear here."
            />
          )}
        </Panel>
      </div>}
      {activeRun && (
        <div className="run-progress aa-dark" aria-live="polite">
          <div>
            <span className="live-dot" aria-hidden="true" />
            <strong>Run {activeRun.slice(0, 8)}</strong>
            <small>
              {lastTask ?? "restored active Run"} ·{" "}
              {runEvents.at(-1)?.kind.replaceAll("_", " ") ??
                visibleStatus.replaceAll("_", " ")}
            </small>
          </div>
          <div className="progress-track">
            <i
              style={{
                width: `${Math.min(100, Math.max(6, runEvents.length * 3))}%`,
              }}
            />
          </div>
          <pre>
            {usage
              ? JSON.stringify(usage.payload.data, null, 2)
              : activeSummary
                ? `${(activeSummary.input_tokens + activeSummary.output_tokens).toLocaleString()} tokens\n$${activeSummary.cost}`
                : "usage pending"}
          </pre>
        </div>
      )}
      <div className="project-support-grid">
        <Panel title="Recent activity" eyebrow="Latest dataset work">
          {projectRuns.length ? <div className="activity-list">
            {projectRuns.slice(0, 3).map((run) => <button key={run.id} onClick={() => onOpenRun(run.id)}><span><strong>{run.workflow_name}</strong><small>{new Date(run.updated_at).toLocaleString()}</small></span><Status status={run.status} /></button>)}
          </div> : <Empty title="No Runs yet" detail="Dataset activity will appear after the first active Automation Run." />}
        </Panel>
        <Panel title="Usage" eyebrow="Persisted across Project Runs">
          <div className="usage-summary"><span><b>{projectRuns.length}</b> Runs</span><span><b>{projectUsage.tokens.toLocaleString()}</b> Tokens</span><span><b>${projectUsage.cost.toFixed(4)}</b> Cost</span></div>
        </Panel>
      </div>

      <details className="advanced-project-details" id="project-advanced-details">
        <summary><span><strong>Advanced Project Details</strong><small>Schema, model bindings, Skills, versions, import, and image records</small></span><b aria-hidden="true">⌄</b></summary>
        <div className="project-overview-grid">
        <Panel title="Run configuration" eyebrow="Immutable Workflow selection">
          <label>
            Workflow Version for next Run
            <select
              value={workflowKey}
              disabled={Boolean(activeRun || project.active_batch)}
              onChange={(event) => setWorkflowKey(event.target.value)}
            >
              {project.available_workflow_versions.map((workflow) => (
                <option
                  key={`${workflow.workflow_id}:${workflow.version}`}
                  value={`${workflow.workflow_id}:${workflow.version}`}
                >
                  {workflow.name} · v{workflow.version}
                </option>
              ))}
            </select>
          </label>
          <small>The Guidance action starts the Dataset with this exact immutable Version.</small>
        </Panel>
        <Panel title="Dataset" eyebrow="Project-owned">
          <Fact label="Root" value={project.dataset.root} />
          <Fact label="Images" value={project.dataset.image_count} />
          <Fact
            label="Discovery"
            value={project.dataset.recursive ? "Recursive" : "Top level"}
          />
          <TagGroup title="Include patterns" values={project.dataset.include} />
        </Panel>
        <Panel
          title="Active Workflow"
          eyebrow={`${project.active_workflow.validation_status} · ${project.active_workflow.status}`}
        >
          <Fact label="Version" value={`v${project.active_workflow.version}`} />
          <Fact label="Nodes" value={project.active_workflow.nodes.length} />
          <Fact label="Source" value={project.active_workflow.source} />
          <button onClick={onOpenWorkflows}>View Workflow definition</button>
        </Panel>
        <Panel title="Enabled Skills" eyebrow="Domain extensions">
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
              title="No Skills enabled"
              detail="Stable schema and hash visuals remain available."
            />
          )}
          <details className="advanced-settings">
            <summary>Configure Capability and Domain Skills</summary>
            <div className="skill-picker">
              {skillCatalog.map((skill) => (
                <label className="checkbox-line" key={skill.id}>
                  <input
                    type="checkbox"
                    checked={selectedSkillIds.includes(skill.id)}
                    onChange={(event) =>
                      setSelectedSkillIds((current) =>
                        event.target.checked
                          ? [...new Set([...current, skill.id])]
                          : current.filter((id) => id !== skill.id),
                      )
                    }
                  />
                  <span>
                    <strong>{skill.display_name}</strong>
                    <small>{skill.kind} · {skill.id}@{skill.version}</small>
                  </span>
                </label>
              ))}
            </div>
            <small>Required Capability Skills are added automatically. Rust Registry validation blocks missing or incompatible versions.</small>
            <button className="form-submit-action" onClick={saveSkills}>Save Project Skills</button>
          </details>
        </Panel>
        <Panel title="Model Bindings" eyebrow="Node execution">
          {project.model_bindings.map((binding) => (
            <Fact
              key={binding.id}
              label={`${binding.id} · ${binding.role}`}
              value={`${binding.provider} / ${binding.model}`}
            />
          ))}
        </Panel>
        <Panel
          title="Annotation Schema"
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
                <span>{task.labels.join(", ") || "No labels"}</span>
              </article>
            ))}
          </div>
          <div className="schema-label-authoring">
            <label>
              Task
              <select
                aria-label="Label task"
                value={labelTaskId}
                onChange={(event) => setLabelTaskId(event.target.value)}
              >
                {project.annotation_schema.map((task) => (
                  <option key={task.id} value={task.id}>
                    {task.id} · {task.kind}
                  </option>
                ))}
              </select>
            </label>
            <label>
              New Label id
              <input
                value={newLabel}
                placeholder="vehicle"
                onChange={(event) => setNewLabel(event.target.value)}
              />
            </label>
            <button
              onClick={addLabel}
              disabled={!labelTaskId || !newLabel.trim()}
            >
              Add Label to Project Schema
            </button>
          </div>
        </Panel>
        <Panel title="Versions, Runs & Reviews" eyebrow="Project outputs">
          <Fact
            label="Workflow versions"
            value={project.available_workflow_versions.length}
          />
          <Fact label="Runs" value={projectRuns.length} />
          <Fact
            label="Runs awaiting review"
            value={
              projectRuns.filter(
                (run) => run.status === "completed_with_review",
              ).length
            }
          />
          <button onClick={() => onNavigate(`/projects/${encodeURIComponent(project.id)}/export`)}>Open Export workspace</button>
        </Panel>
        <Panel title="Annotation import" eyebrow="Dry-run first · compatibility report">
          <label>
            Format
            <select value={importFormat} onChange={(event) => setImportFormat(event.target.value)}>
              <option value="native">AnnotAgent Native</option>
              <option value="coco">COCO</option>
              <option value="labelme">LabelMe</option>
              <option value="yolo_detection">YOLO detection</option>
              <option value="yolo_segmentation">YOLO segmentation</option>
            </select>
          </label>
          <label>
            Workspace-local file or directory
            <input value={importSource} onChange={(event) => setImportSource(event.target.value)} placeholder="/workspace/project/import/annotations.json" />
          </label>
          <label className="checkbox-line">
            <input type="checkbox" checked={importDryRun} onChange={(event) => setImportDryRun(event.target.checked)} />
            Dry run without persistence
          </label>
          <button onClick={importAnnotations}>{importDryRun ? "Preview import" : "Import to Review"}</button>
          {importResult && <pre className="import-report" aria-live="polite">{importResult}</pre>}
        </Panel>
        </div>
        <ProjectAgentActivity projectId={project.id} onError={onError} />
      <Panel title="Dataset images" eyebrow={`${images.length} visible`}>
        <div className="image-grid">
          {images.map((image) => (
            <article key={image.index}>
              <img src={image.url} alt={image.name} />
              <div>
                <span>
                  <strong>{image.name}</strong>
                  <small>Image {image.index + 1}</small>
                </span>
                <Status status={visibleStatus} />
              </div>
            </article>
          ))}
          {images.length === 0 && (
            <Empty
              title="No images"
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
  onNavigate,
  onError,
}: {
  project?: ProjectSummary;
  onNavigate: (destination: string) => void;
  onError: (value: string) => void;
}) {
  const [readiness, setReadiness] = useState<ExportReadiness>();
  const [format, setFormat] = useState("");
  const [exporting, setExporting] = useState(false);
  const [result, setResult] = useState<ProjectExportResult>();
  const [copyStatus, setCopyStatus] = useState("");
  const activeReadiness = readiness?.project_id === project?.id ? readiness : undefined;
  const loadReadiness = () => {
    if (!project) return Promise.resolve();
    return api.exportReadiness(project.id).then((value) => {
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
    setReadiness(undefined);
    setResult(undefined);
    setCopyStatus("");
    void loadReadiness().catch((error: Error) => onError(error.message));
  }, [
    project?.id,
    project?.image_count,
    project?.review_count,
    project?.active_run?.updated_at,
    project?.active_batch?.event_sequence,
  ]);
  const executeExport = () => {
    if (!project || !format || !activeReadiness?.ready) return;
    setExporting(true);
    setCopyStatus("");
    void api
      .export(project.id, format)
      .then((value) => {
        setResult(value);
        return loadReadiness();
      })
      .catch((error: Error) => onError(error.message))
      .finally(() => setExporting(false));
  };
  const copyOutputPath = () => {
    if (!result) return;
    void navigator.clipboard
      .writeText(result.output_path)
      .then(() => setCopyStatus("Folder path copied"))
      .catch(() => setCopyStatus("Select the path above to copy it"));
  };
  if (!project)
    return <section className="page-stack"><Empty title="Project unavailable" detail="Return to Projects and choose a valid Project." /></section>;
  return (
    <section className="page-stack export-workspace">
      <ProjectBreadcrumb project={project} current="Export" onOpenProjects={() => onNavigate("/projects")} onOpenProject={() => onNavigate(`/projects/${encodeURIComponent(project.id)}`)} />
      <nav className="section-tabs" aria-label={`${project.name} workspace`}>
        <button onClick={() => onNavigate(`/projects/${encodeURIComponent(project.id)}`)}>Overview</button>
        <button onClick={() => onNavigate(`/projects/${encodeURIComponent(project.id)}/build/data`)}>Build</button>
        <button onClick={() => onNavigate(projectRunsPath(project.id))}>Runs</button>
        <button onClick={() => onNavigate(projectReviewPath(project.id))}>Review</button>
        <button className="active" aria-current="page">Export</button>
      </nav>
      {!activeReadiness ? (
        <div className="loading-banner" role="status">Checking dataset export readiness…</div>
      ) : (
        <>
          <header className={`export-hero ${activeReadiness.ready ? "ready" : "blocked"}`}>
            <div>
              <span className="eyebrow">Dataset delivery</span>
              <h2>{activeReadiness.ready ? "Your dataset is ready" : "Export needs attention"}</h2>
              <p>{activeReadiness.ready ? "All images have completed runs and every review decision is resolved. Choose a compatible format to create the dataset." : "Resolve the items below before creating a formal dataset export."}</p>
            </div>
            <dl className="export-readiness-metrics">
              <div><dt>Images</dt><dd>{activeReadiness.image_count}</dd><small>{activeReadiness.processed_image_count} processed</small></div>
              <div><dt>Accepted annotations</dt><dd>{activeReadiness.accepted_annotations}</dd><small>Included in export</small></div>
              <div><dt>Unresolved reviews</dt><dd>{activeReadiness.unresolved_reviews}</dd><small>{activeReadiness.unresolved_reviews ? "Blocking export" : "Queue is clear"}</small></div>
            </dl>
          </header>

          {activeReadiness.blocking_issues.length > 0 && <section className="export-blockers" aria-labelledby="export-blockers-title">
            <div className="section-heading"><div><span className="eyebrow">Blocking reviews and setup</span><h2 id="export-blockers-title">Complete these items</h2></div></div>
            {activeReadiness.blocking_issues.map((issue) => <article key={issue.code}>
              <span aria-hidden="true">!</span>
              <div><strong>{issue.title}</strong><small>{issue.explanation}</small></div>
              <button onClick={() => onNavigate(issue.repair_destination)}>Resolve</button>
            </article>)}
          </section>}

          <section className="export-format-section" aria-labelledby="export-format-title">
            <div className="section-heading"><div><span className="eyebrow">Schema compatibility</span><h2 id="export-format-title">Choose an export format</h2></div><small>The recommendation is calculated from the active Project Schema.</small></div>
            <div className="export-format-grid">
              {activeReadiness.formats.map((item) => <label className={`export-format-card ${format === item.format ? "selected" : ""} ${!item.supported ? "unsupported" : ""}`} key={item.format}>
                <input type="radio" name="export-format" value={item.format} checked={format === item.format} disabled={!item.supported} onChange={() => setFormat(item.format)} />
                <span>
                  <span className="export-format-title"><strong>{item.display_name}</strong>{item.recommended && <b>Recommended</b>}{!item.supported && <b>Incompatible</b>}</span>
                  <small>{item.summary}</small>
                  {item.unsupported_task_kinds.length > 0 && <small>Unsupported: {item.unsupported_task_kinds.join(", ")}</small>}
                  {item.warnings.map((warning) => <small key={warning}>{warning}</small>)}
                </span>
              </label>)}
            </div>
            <div className="export-actions">
              <button onClick={() => onNavigate(`/projects/${encodeURIComponent(project.id)}`)}>Back to Project</button>
              <button className="primary" disabled={!activeReadiness.ready || !format || exporting} onClick={executeExport}>
                {exporting ? "Exporting dataset…" : `Export ${activeReadiness.formats.find((item) => item.format === format)?.display_name ?? "dataset"} dataset`}
              </button>
            </div>
          </section>

          {result && <section className="export-success" aria-live="polite">
            <div className="export-success-heading"><span aria-hidden="true">✓</span><div><span className="eyebrow">Export complete</span><h2>Dataset exported successfully</h2><p>{result.report.exported_count} annotation{result.report.exported_count === 1 ? "" : "s"} exported · {result.report.skipped_count} skipped · {new Date(result.completed_at).toLocaleString()}</p></div></div>
            <div className="export-result-path"><span>Result folder</span><code>{result.output_path}</code><button onClick={copyOutputPath}>Copy folder path</button>{copyStatus && <small role="status">{copyStatus}</small>}</div>
            <details className="export-report"><summary>View export report</summary><dl>
              <div><dt>Format</dt><dd>{result.format}</dd></div>
              <div><dt>Exported</dt><dd>{result.report.exported_count}</dd></div>
              <div><dt>Skipped</dt><dd>{result.report.skipped_count}</dd></div>
              <div><dt>Files</dt><dd>{result.report.output_files.length}</dd></div>
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

function InlineProviderSetup({
  onOpenProviders,
  onOpenModels,
  onReady,
  onError,
}: {
  onOpenProviders: () => void;
  onOpenModels: () => void;
  onReady: () => Promise<void>;
  onError: (value: string) => void;
}) {
  const [presets, setPresets] = useState<ProviderPresetProfile[]>([]);
  const [presetId, setPresetId] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [modelId, setModelId] = useState("");
  const [credentialSource, setCredentialSource] = useState<
    "environment_variable" | "workspace_file" | "session_only"
  >("workspace_file");
  const [environmentVariable, setEnvironmentVariable] =
    useState("ANNOTAGENT_PROVIDER_API_KEY");
  const [secret, setSecret] = useState("");
  const [busy, setBusy] = useState<"connect" | "probe" | "">("");
  const [created, setCreated] = useState<{
    provider: ProviderProfile;
    model: RegistryModelProfile;
  }>();
  useEffect(() => {
    void api
      .providerPresets()
      .then(({ presets: values }) => {
        const available = values.filter(
          (preset) => preset.adapter === "open_ai_compatible",
        );
        setPresets(available);
        const first = available[0];
        if (first) {
          setPresetId(first.id);
          setDisplayName(first.display_name);
          setModelId(first.suggested_models[0] ?? "");
        }
      })
      .catch((error: Error) => onError(error.message));
  }, []);
  const choosePreset = (id: string) => {
    const preset = presets.find((candidate) => candidate.id === id);
    setPresetId(id);
    if (preset) {
      setDisplayName(preset.display_name);
      setModelId(preset.suggested_models[0] ?? "");
    }
  };
  const connect = async () => {
    const preset = presets.find((candidate) => candidate.id === presetId);
    if (!preset) return onError("Choose a Provider preset first.");
    if (!displayName.trim() || !modelId.trim())
      return onError("Enter a Provider name and exact model ID.");
    if (
      credentialSource === "environment_variable" &&
      !isEnvironmentVariableName(environmentVariable)
    )
      return onError(
        "Enter an environment variable name such as DASHSCOPE_API_KEY, not the API key itself. To paste a key directly, choose Local workspace file.",
      );
    if (credentialSource !== "environment_variable" && !secret)
      return onError("Enter the API key.");
    setBusy("connect");
    try {
      const provider = await api.createProvider({
        display_name: displayName.trim(),
        preset_id: preset.id,
        adapter: "open_ai_compatible",
        base_url: preset.base_url,
      });
      await api.saveProviderCredential(provider.id, {
        source: credentialSource,
        ...(credentialSource === "environment_variable"
          ? { environment_variable: environmentVariable.trim() }
          : { secret }),
      });
      const model = await api.createModelProfile({
        provider_id: provider.id,
        display_name: modelId.trim(),
        remote_model_id: modelId.trim(),
        input_modalities: ["text"],
        task_capabilities: ["text_generation"],
        protocol_features: {
          tool_calls: true,
          parallel_tool_calls: false,
          structured_output: true,
          json_schema: false,
          usage_reporting: true,
          streaming: false,
          reasoning_controls: false,
        },
      });
      await api.checkProvider(provider.id);
      setSecret("");
      setCreated({ provider, model });
    } catch (error) {
      onError((error as Error).message);
    } finally {
      setBusy("");
    }
  };
  const probe = async () => {
    if (!created) return;
    if (
      !window.confirm(
        "This sends one minimal generation request and may incur Provider charges. Continue?",
      )
    )
      return;
    setBusy("probe");
    try {
      await api.activeProbe(created.provider.id, created.model.id);
      await onReady();
    } catch (error) {
      onError((error as Error).message);
    } finally {
      setBusy("");
    }
  };
  return (
    <details className="inline-provider-setup" open>
      <summary>Provider setup required</summary>
      <p>
        Pipeline Builder needs an Available text model with Tool Calls and
        Structured Output. This setup keeps the current Draft and returns here
        after verification.
      </p>
      {!created ? (
        <div className="inline-provider-form">
          <label>
            Provider preset
            <select
              value={presetId}
              onChange={(event) => choosePreset(event.target.value)}
            >
              {presets.map((preset) => (
                <option key={preset.id} value={preset.id}>
                  {preset.display_name}
                </option>
              ))}
            </select>
          </label>
          <label>
            Connection name
            <input
              value={displayName}
              onChange={(event) => setDisplayName(event.target.value)}
            />
          </label>
          <label>
            Agent model
            <input
              value={modelId}
              onChange={(event) => setModelId(event.target.value)}
              placeholder="Exact Provider model ID"
            />
          </label>
          <label>
            Credential source
            <select
              value={credentialSource}
              onChange={(event) =>
                setCredentialSource(
                  event.target.value as
                    | "environment_variable"
                    | "workspace_file"
                    | "session_only",
                )
              }
            >
              <option value="workspace_file">Local workspace file</option>
              <option value="environment_variable">
                Server environment variable
              </option>
              <option value="session_only">This server session only</option>
            </select>
          </label>
          {credentialSource === "environment_variable" ? (
            <label>
              Environment variable name
              <input
                value={environmentVariable}
                onChange={(event) =>
                  setEnvironmentVariable(event.target.value)
                }
                placeholder="ANNOTAGENT_PROVIDER_API_KEY"
              />
            </label>
          ) : (
            <label>
              API key
              <input
                type="password"
                autoComplete="off"
                value={secret}
                onChange={(event) => setSecret(event.target.value)}
              />
            </label>
          )}
          <small>
            Local workspace file persists across restarts under the Git-ignored
            .annotagent/credentials directory. Environment variable mode accepts
            only a variable name set before the server starts. Session-only
            values disappear when the server stops. Credentials are never placed
            in browser storage or the OS keychain.
          </small>
          <button
            disabled={busy === "connect" || !presets.length}
            onClick={() => void connect()}
          >
            {busy === "connect"
              ? "Saving and checking…"
              : "Save and check connection"}
          </button>
        </div>
      ) : (
        <div className="inline-provider-verified">
          <Status status="configured" />
          <span>
            <strong>{created.model.display_name}</strong>
            <small>via {created.provider.display_name}</small>
          </span>
          <p>
            The non-billable Provider check passed. One explicit model test is
            required before this Profile becomes Available.
          </p>
          <button
            disabled={busy === "probe"}
            onClick={() => void probe()}
          >
            {busy === "probe" ? "Testing model…" : "Run billable model test"}
          </button>
        </div>
      )}
      <div className="button-row">
        <button onClick={onOpenProviders}>Open Provider settings</button>
        <button onClick={onOpenModels}>Open Model settings</button>
      </div>
    </details>
  );
}

function WorkflowsPage({
  projects,
  runs,
  activeProjectId,
  onActivate,
  onRefresh,
  onNavigate,
  onOpenProjects,
  onOpenProject,
  onOpenProviders,
  onOpenModels,
  onOpenPlugins,
  onError,
}: {
  projects: ProjectSummary[];
  runs: HistoryRun[];
  activeProjectId: string;
  onActivate: (id: string) => void;
  onRefresh: () => Promise<void>;
  onNavigate: (step: "data" | "labels" | "pipeline" | "test", draftId?: string) => void;
  onOpenProjects: () => void;
  onOpenProject: () => void;
  onOpenProviders: () => void;
  onOpenModels: () => void;
  onOpenPlugins: () => void;
  onError: (value: string) => void;
}) {
  const entries = projects.flatMap((project) =>
    project.available_workflow_versions.map((workflow) => ({
      project,
      workflow,
    })),
  );
  const [selectedPublishedKey, setSelectedPublishedKey] = useState("");
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
  const [templateId, setTemplateId] = useState("");
  const activeProject = projects.find((project) => project.id === activeProjectId);
  const buildSummary = useBuildSummary(activeProject, onError);
  const [targetTaskId, setTargetTaskId] = useState("");
  const [targetLabel, setTargetLabel] = useState("");
  const [busy, setBusy] = useState(false);
  const [advisorRunning, setAdvisorRunning] = useState(false);
  const advisorRequestActive = useRef(false);
  const persistedDrafts = useRef(new Map<string, string>());
  const autosaveTimer = useRef<number | undefined>(undefined);
  const [savedAt, setSavedAt] = useState<Date>();
  const [clock, setClock] = useState(() => Date.now());
  const refreshDrafts = () =>
    api
      .workflowDrafts(activeProjectId || undefined)
      .then((value) => {
        for (const item of value.drafts)
          persistedDrafts.current.set(item.id, JSON.stringify(item));
        setDrafts(value.drafts);
        setDraft(
          (current) =>
            value.drafts.find((item) => item.id === current?.id) ??
            value.drafts[0],
        );
      })
      .catch((error: Error) => onError(error.message));
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
    const [{ drafts: latestDrafts }, sample] = await Promise.all([
      api.workflowDrafts(activeProjectId),
      api.workflowSampleTest(session.draft_id).catch(() => ({ sample_test: null, current: false })),
    ]);
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
    const timer = window.setInterval(() => setClock(Date.now()), 1_000);
    return () => window.clearInterval(timer);
  }, []);
  useEffect(() => {
    if (!draft || draft.status === "published" || draft.status === "archived") return;
    const snapshot = JSON.stringify(draft);
    if (persistedDrafts.current.get(draft.id) === snapshot) return;
    if (autosaveTimer.current) window.clearTimeout(autosaveTimer.current);
    autosaveTimer.current = window.setTimeout(() => {
      void api
        .saveWorkflowDraft(draft)
        .then((saved) => {
          persistedDrafts.current.set(saved.id, JSON.stringify(saved));
          setSavedAt(new Date());
          return onRefresh();
        })
        .catch((error: Error) => onError(`Draft autosave failed: ${error.message}`));
    }, 800);
    return () => {
      if (autosaveTimer.current) window.clearTimeout(autosaveTimer.current);
    };
  }, [draft]);
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
      void api
        .agentSessions(activeProjectId)
        .then(({ sessions }) => {
          const latest = sessions.find(
            (session) =>
              session.kind === "pipeline_builder" &&
              ["running", "waiting_for_human"].includes(session.status),
          ) ?? sessions.find((session) => session.kind === "pipeline_builder");
          setActiveAgentSession(latest);
          setAdvisorRunning(latest?.status === "running");
          if (latest && latest.status !== "running")
            void recoverAdvisorProposal(latest).catch((error: Error) =>
              onError(`Agent result recovery: ${error.message}`),
            );
        })
        .catch((error: Error) => onError(`Agent recovery: ${error.message}`));
    } else {
      setCatalog(undefined);
      setActiveAgentSession(undefined);
      setAdvisorRunning(false);
    }
    setReport(undefined);
    setSelectedPublishedKey("");
    setTargetTaskId(activeProject?.annotation_schema[0]?.id ?? "");
    setTargetLabel(activeProject?.annotation_schema[0]?.labels[0] ?? "");
  }, [activeProjectId]);
  useEffect(() => {
    if (!advisorRunning || !activeProjectId) return;
    let stopped = false;
    const poll = () =>
      api.agentSessions(activeProjectId).then(({ sessions }) => {
        if (stopped) return;
        const latest = sessions.find((session) => session.kind === "pipeline_builder");
        if (latest) {
          if (
            advisorRequestActive.current &&
            latest.status !== "running"
          )
            return;
          setActiveAgentSession(latest);
          if (
            !advisorRequestActive.current &&
            latest.status !== "running"
          ) {
            setAdvisorRunning(false);
            void recoverAdvisorProposal(latest).catch((error: Error) =>
              onError(`Agent result recovery: ${error.message}`),
            );
          }
        }
      }).catch(() => undefined);
    void poll();
    const timer = window.setInterval(() => void poll(), 750);
    return () => {
      stopped = true;
      window.clearInterval(timer);
    };
  }, [advisorRunning, activeProjectId]);
  useEffect(() => {
    if (!draft) return;
    let cancelled = false;
    void api.workflowSampleTest(draft.id)
      .then(({ sample_test: sampleTest, current }) => {
        if (!cancelled) setReport(current ? sampleTest?.report : undefined);
      })
      .catch(() => {
        if (!cancelled) setReport(undefined);
      });
    return () => { cancelled = true; };
  }, [draft?.id, draft?.updated_at]);
  const finish = (promise: Promise<unknown>) => {
    setBusy(true);
    void promise
      .then(() => Promise.all([refreshDrafts(), onRefresh()]))
      .catch((error: Error) => onError(error.message))
      .finally(() => setBusy(false));
  };
  const create = (fromTemplate: boolean, selectedTemplate?: string) =>
    activeProjectId
      ? finish(
          api.createWorkflowDraft(
            activeProjectId,
            fromTemplate,
            selectedTemplate,
          ),
        )
      : onError("Select a Project before creating a Workflow.");
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
    setActiveAgentSession(undefined);
    setAdvisorProposalRecovered(false);
    setProposalDiff(undefined);
    setSelectedProposalChanges([]);
    const editableBase = draft && !["published", "archived"].includes(draft.status)
      ? Promise.resolve(draft)
      : api.createWorkflowDraft(activeProjectId, false).then((created) => {
          persistedDrafts.current.set(created.id, JSON.stringify(created));
          setDraft(created);
          return created;
        });
    void editableBase
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
        );
        setAdvisorProposal(proposal);
        setAdvisorProposalRecovered(false);
        setActiveAgentSession(proposal.agent_session);
        setShowProposalComparison(true);
        if (baseDraft.id !== proposal.draft.id) {
          const diff = await api.workflowDraftDiff(baseDraft.id, proposal.draft.id);
          setProposalDiff(diff);
          setSelectedProposalChanges(pipelineDiffChangeIds(diff));
        }
        await refreshDrafts();
      })
      .catch((error: Error) => onError(error.message))
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
      return;
    }
    void api.workflowDrafts(activeProjectId || undefined).then(({ drafts: latest }) => {
      const recovered = latest.find((candidate) => candidate.id === draftId);
      if (recovered) setDraft(recovered);
      else onError("The saved Agent Draft is no longer available in this Project.");
    }).catch((error: Error) => onError(error.message));
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
    if (!undoDraft) return;
    setBusy(true);
    void api
      .saveWorkflowDraft(undoDraft)
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
    return <section className="page-stack"><ProjectBreadcrumb project={activeProject} current="Build" onOpenProjects={onOpenProjects} onOpenProject={onOpenProject} /><BuildNavigation step="pipeline" onNavigate={onNavigate} /><div className="loading-banner" role="status">Loading Build readiness…</div></section>;
  if (buildSummary && !buildStepAllowed(buildSummary.guidance, "pipeline"))
    return <section className="page-stack"><ProjectBreadcrumb project={activeProject} current="Build" onOpenProjects={onOpenProjects} onOpenProject={onOpenProject} /><BuildNavigation step="pipeline" guidance={buildSummary.guidance} onNavigate={onNavigate} /><BuildBlocker guidance={buildSummary.guidance} onNavigate={onNavigate} /></section>;
  return (
    <section className="page-stack">
      <ProjectBreadcrumb project={activeProject} current="Build" onOpenProjects={onOpenProjects} onOpenProject={onOpenProject} />
      <BuildNavigation step="pipeline" guidance={buildSummary?.guidance} onNavigate={(step) => onNavigate(step, step === "test" ? draft?.id : undefined)} />
      <div className="toolbar-panel workflow-designer-header">
        <div>
          <span className="eyebrow">Step 3 · Automation</span>
          <h2>How AnnotAgent will label your data</h2>
          <p>
            Start from a registered recipe or Advisor suggestion, then edit the
            same autosaved Draft. Technical graph details remain available for
            expert inspection.
          </p>
        </div>
        <div className="button-row">
          <small className="save-indicator" aria-live="polite">
            Saved {Math.max(0, Math.floor((clock - (savedAt?.getTime() ?? new Date(draft?.updated_at ?? clock).getTime())) / 1000))} seconds ago
          </small>
          <button
            onClick={() => create(false)}
            disabled={busy || !activeProjectId}
          >
            New Draft
          </button>
        </div>
      </div>
      <div className="workflow-command-grid">
        <section className="workflow-command-card">
          <span className="eyebrow">Starting point</span>
          <h3>Template</h3>
          <select
            aria-label="Workflow template"
            value={templateId}
            onChange={(event) => setTemplateId(event.target.value)}
          >
            <option value="">Generic project template</option>
            {(catalog?.workflow_templates ?? []).map((template) => (
              <option key={template.id} value={template.id}>
                {template.name}
              </option>
            ))}
          </select>
          <button
            onClick={() => create(true, templateId || undefined)}
            disabled={busy || !activeProjectId}
          >
            From Template
          </button>
        </section>
        <section className="workflow-command-card workflow-advisor-recommendation">
          <span className="eyebrow">Pipeline Builder Agent</span>
          <h3>Build a recommended automation</h3>
          <p>{targetLabel ? `Set the boundaries for ${targetLabel}. The Agent may inspect, draft, validate, and Dry Run, but it cannot activate the result.` : "Choose a Label and bounded objective before starting the Agent."}</p>
          {registryLoading ? (
            <div className="loading-banner compact" role="status">
              Finding compatible Agent models…
            </div>
          ) : liveAgentModels.length ? (
            <fieldset className="agent-model-choice">
              <legend>Agent model</legend>
              <label>
                Model Profile
                <select
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
                        {model.display_name} via {provider?.display_name ?? "Provider"}
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
                    <span>Text generation</span>
                    <span>Structured output</span>
                    <span>Tool calls</span>
                    <span>
                      {agentProjectBinding
                        ? "Project choice"
                        : globalModelDefaults.pipeline_builder ===
                            selectedAgentModel.id
                          ? "Global default"
                          : "Compatible fallback"}
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
                />
                Lock this Project choice so the Agent cannot replace it
              </label>
            </fieldset>
          ) : (
            <InlineProviderSetup
              onOpenProviders={onOpenProviders}
              onOpenModels={onOpenModels}
              onReady={refreshModelChoices}
              onError={onError}
            />
          )}
          <fieldset className="agent-objective" aria-label="Pipeline Builder objective">
            <legend>Objective</legend>
            <label>Target task<select aria-label="Target task" value={targetTaskId} onChange={(event) => {
              const taskId = event.target.value;
              setTargetTaskId(taskId);
              setTargetLabel(activeProject?.annotation_schema.find((task) => task.id === taskId)?.labels[0] ?? "");
            }}>
              {(activeProject?.annotation_schema ?? []).map((task) => <option key={task.id} value={task.id}>{task.id} · {task.kind}</option>)}
            </select></label>
            <label>Target Label<select aria-label="Target Label" value={targetLabel} onChange={(event) => setTargetLabel(event.target.value)}>
              {(targetTask?.labels ?? []).map((label) => <option key={label} value={label}>{label}</option>)}
            </select></label>
            <label>Priority<select aria-label="Optimization priority" value={builderConstraints.priority} onChange={(event) => setBuilderConstraints((current) => ({ ...current, priority: event.target.value as OptimizationPriority }))}>
              <option value="balanced">Balanced</option>
              <option value="accurate">Accuracy first</option>
              <option value="fast">Speed first</option>
              <option value="low_cost">Lowest cost</option>
            </select></label>
            <label>Maximum cost per image<input aria-label="Maximum cost per image" inputMode="decimal" placeholder="No per-image limit" value={builderConstraints.max_cost_per_image ?? ""} onChange={(event) => setBuilderConstraints((current) => ({ ...current, max_cost_per_image: event.target.value || undefined }))} /></label>
            <label>Maximum latency (ms)<input aria-label="Maximum latency" type="number" min="1" placeholder="No latency limit" value={builderConstraints.max_expected_latency_ms ?? ""} onChange={(event) => setBuilderConstraints((current) => ({ ...current, max_expected_latency_ms: event.target.value ? Number(event.target.value) : undefined }))} /></label>
            <label>Desired Review workload<input aria-label="Desired review rate" type="number" min="0" max="100" value={Math.round((builderConstraints.target_review_rate ?? 0) * 100)} onChange={(event) => setBuilderConstraints((current) => ({ ...current, target_review_rate: Number(event.target.value) / 100 }))} /><small>Percent of decided candidates</small></label>
            <label className="checkbox-row"><input type="checkbox" checked={builderConstraints.allow_external_models} onChange={(event) => setBuilderConstraints((current) => ({ ...current, allow_external_models: event.target.checked }))} />Allow configured external APIs</label>
            <label className="checkbox-row"><input type="checkbox" checked={builderConstraints.allow_human_review} onChange={(event) => setBuilderConstraints((current) => ({ ...current, allow_human_review: event.target.checked }))} />Allow Human Review</label>
            <div className="agent-worker-summary"><span>Available local workers</span><strong>{activeProject?.model_bindings.filter((model) => model.scope === "workspace_worker" && model.availability_group === "ready").map((model) => model.id).join(", ") || "None ready"}</strong></div>
          </fieldset>
          <details className="project-model-choices">
            <summary>Project model choices</summary>
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
                    {choice.label}
                    <select
                      aria-label={choice.label}
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
                      <option value="">Use a node-specific choice</option>
                      {models.map((model) => {
                        const provider = registryProviders.find(
                          (candidate) => candidate.id === model.provider_id,
                        );
                        return (
                          <option key={model.id} value={model.id}>
                            {model.display_name} via {provider?.display_name ?? "Provider"}
                          </option>
                        );
                      })}
                    </select>
                    <small>
                      {models.length
                        ? `${models.length} compatible and Available`
                        : "No compatible Profile is Available"}
                    </small>
                  </label>
                );
              })}
            </div>
            <div className="button-row">
              <button onClick={onOpenModels}>Manage Model Profiles</button>
              <button onClick={onOpenProviders}>Manage Providers</button>
            </div>
          </details>
          <details className="advanced-settings"><summary>Agent limits and provider</summary><div className="workflow-advisor-fields">
            <p className="field-note">AnnotAgent uses the selected live Agent model. Test fixtures are never eligible for generated Pipelines.</p>
            <label>Maximum model calls / image<input type="number" min="1" max="16" value={builderConstraints.max_model_calls_per_image ?? ""} onChange={(event) => setBuilderConstraints((current) => ({ ...current, max_model_calls_per_image: event.target.value ? Number(event.target.value) : undefined }))} /></label>
            <label>Maximum Agent turns<input type="number" min="1" max="64" value={builderConstraints.maximum_agent_turns} onChange={(event) => setBuilderConstraints((current) => ({ ...current, maximum_agent_turns: Number(event.target.value) }))} /></label>
            <label>Maximum Tool Calls<input type="number" min="1" max="128" value={builderConstraints.maximum_tool_calls} onChange={(event) => setBuilderConstraints((current) => ({ ...current, maximum_tool_calls: Number(event.target.value) }))} /></label>
            <label>Maximum Dry Runs<input type="number" min="1" max="10" value={builderConstraints.maximum_dry_runs} onChange={(event) => setBuilderConstraints((current) => ({ ...current, maximum_dry_runs: Number(event.target.value) }))} /></label>
            <label>Maximum Agent cost<input inputMode="decimal" value={builderConstraints.maximum_agent_cost} onChange={(event) => setBuilderConstraints((current) => ({ ...current, maximum_agent_cost: event.target.value }))} /></label>
            <button onClick={suggest} disabled={busy || !activeProjectId}>Build complete Project automation</button>
          </div></details>
          <button className={activeAgentSession?.draft_id ? undefined : "primary"} onClick={suggestLabelPipeline} disabled={busy || advisorRunning || !activeProjectId || !targetTaskId || !targetLabel || !selectedAgentModelId}>{advisorRunning ? "Agent is working…" : "Ask AnnotAgent"}</button>
        </section>
        <section className="workflow-command-card workflow-version-actions">
          <span className="eyebrow">Current Automation</span>
          <h3>{immutable ? "Immutable Version" : "Autosaved Draft"}</h3>
          <p>{immutable ? "Clone this Version before making changes." : "Edits stay unpublished until you test and activate them in the next step."}</p>
          <div className="button-row">
            {!immutable && undoDraft && <button onClick={undoAgentApply} disabled={busy}>Undo Agent changes</button>}
            {!immutable && <button onClick={discardChanges} disabled={busy || !draft}>Discard</button>}
            {!immutable && <button onClick={() => onNavigate("test", draft?.id)} disabled={busy || !draft}>Open Test &amp; Activate</button>}
            {immutable && <button onClick={clonePublished} disabled={busy || !selected?.workflow.source.startsWith("published draft")}>Clone to Draft</button>}
            <button onClick={() => document.getElementById("improve-automation")?.scrollIntoView({ behavior: "smooth" })}>Improve from evidence</button>
            {!immutable && draft && <details className="action-menu"><summary>More</summary><div><button onClick={archive} disabled={busy}>Archive</button></div></details>}
          </div>
        </section>
      </div>
      {geometryBlockingIssues.length > 0 && <section className="geometry-safety-blocker" role="alert">
        <div>
          <span className="eyebrow">Publication blocked</span>
          <h2>Automatic acceptance is unsafe</h2>
          <p>The selected model score describes semantic or relative confidence, but the bounding boxes do not have valid geometry evidence for this Project.</p>
          <ul>{geometryBlockingIssues.map((issue) => <li key={`${issue.code}:${issue.path}`}><strong>{issue.code.replaceAll("_", " ")}</strong><span>{issue.message}</span></li>)}</ul>
        </div>
        <div className="geometry-repair-actions">
          <button className="primary" disabled={busy || !selected?.workflow.source.startsWith("published draft")} onClick={createSafeDraft}>Require human review</button>
          <button onClick={() => document.getElementById("improve-automation")?.scrollIntoView({ behavior: "smooth" })}>Add compatible refiner</button>
          <button onClick={() => {
            document.querySelector<HTMLDetailsElement>(".geometry-calibration-panel")?.setAttribute("open", "");
            document.getElementById("improve-automation")?.scrollIntoView({ behavior: "smooth" });
          }}>Run geometry calibration</button>
        </div>
      </section>}
      {(advisorRunning || (activeAgentSession && !advisorProposal)) && (
        <Panel title="Agent progress" eyebrow={advisorRunning ? "Live persisted Pipeline Builder session" : "Recovered persisted Pipeline Builder session"}>
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
            <div className="loading-banner" role="status">Starting the bounded Agent session…</div>
          )}
        </Panel>
      )}
      {advisorProposal && (
        <Panel
          title={advisorProposalRecovered ? "Saved Agent Result" : "Proposed Changes"}
          eyebrow={advisorProposalRecovered ? "Recovered from server · editable Draft · not activated" : "Advisor preview · Draft only · never activated automatically"}
        >
          <div className="advisor-proposal-grid">
            <div>
              <h3>Automation Recipe</h3>
              <ol className="advisor-recipe-list">
                {guidedWorkflowNodes(advisorProposal.draft.nodes).map((node) => <li key={node.id}>{workflowNodeTitle(node.node_type)} <small>{node.model_binding ?? "Core"}</small></li>)}
              </ol>
            </div>
            <div className="fact-grid">
              <Fact label="Model calls / image" value={advisorProposal.estimated_model_calls_per_image} />
              <Fact label="Estimated latency" value={advisorProposal.estimated_latency_ms ? `${advisorProposal.estimated_latency_ms} ms` : "Unresolved"} />
              <Fact label="Cost tier" value={advisorProposal.estimated_cost_tier} />
              <Fact label="Expected Review workload" value={advisorProposal.agent_dry_run ? `${advisorProposal.agent_dry_run.summary.needs_review_count} of ${advisorProposal.agent_dry_run.summary.image_count} samples` : "Test required"} />
              <Fact label={advisorProposalRecovered ? "Persistence" : "Compared with current"} value={advisorProposalRecovered ? "Saved on server" : draft ? `${advisorProposal.draft.nodes.length - draft.nodes.length >= 0 ? "+" : ""}${advisorProposal.draft.nodes.length - draft.nodes.length} nodes` : "No Current Draft"} />
            </div>
          </div>
          {showProposalComparison && proposalDiff && (
            <fieldset className="advisor-change-preview" aria-label="Draft Diff">
              <legend>Review changes</legend>
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
              {!pipelineDiffChangeIds(proposalDiff).length && <p>No changes from the Current Draft.</p>}
            </fieldset>
          )}
          <TagGroup title="Why" values={advisorProposal.rationale} />
          <TagGroup title="Unresolved bindings" values={advisorProposal.unresolved_model_bindings} />
          {!!advisorProposal.unresolved_model_bindings.length && <div className="unresolved-plugin-action"><span><strong>A required Expert Model capability is not Ready.</strong><small>Inspect compatible installed contracts, add legal checkpoint files, and run the isolated Rust process test. AnnotAgent will keep this as a blocked Draft until you retry.</small></span><button onClick={onOpenPlugins}>Install or finish model setup</button></div>}
          <TagGroup title="Warnings" values={advisorProposal.warnings} />
          <TagGroup title="Alternatives" values={advisorProposal.alternatives} />
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
          <div className="button-row">
            {advisorProposalRecovered ? <>
              <button className="primary" onClick={() => openAgentDraft(advisorProposal.draft.id)}>Open saved Draft</button>
              <button onClick={() => { setAdvisorProposal(undefined); setAdvisorProposalRecovered(false); }}>Dismiss result</button>
            </> : <>
              <button className="primary" onClick={() => applyProposalChanges()} disabled={!proposalDiff || !selectedProposalChanges.length || busy}>Apply selected</button>
              <button onClick={() => proposalDiff && applyProposalChanges(pipelineDiffChangeIds(proposalDiff))} disabled={!proposalDiff || !pipelineDiffChangeIds(proposalDiff).length || busy}>Apply all</button>
              <button onClick={() => setShowProposalComparison((value) => !value)}>{showProposalComparison ? "Hide comparison" : "Compare with current"}</button>
              <button onClick={() => { setAdvisorProposal(undefined); setAdvisorProposalRecovered(false); setProposalDiff(undefined); setSelectedProposalChanges([]); }}>Reject proposal</button>
            </>}
          </div>
        </Panel>
      )}
      {activeProject && <ImproveAutomationPanel
        project={activeProject}
        runs={runs}
        workflows={publishedEntries.map(({ workflow }) => workflow)}
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
        <summary>Version History</summary>
      <div className="toolbar-panel">
        <div>
          <span className="eyebrow">Version comparison</span>
          <p>Compare immutable node sets and content hashes.</p>
        </div>
        <div className="button-row">
          <select
            aria-label="Left Workflow Version"
            value={compareLeft}
            onChange={(event) => setCompareLeft(event.target.value)}
          >
            <option value="">Left version…</option>
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
            aria-label="Right Workflow Version"
            value={compareRight}
            onChange={(event) => setCompareRight(event.target.value)}
          >
            <option value="">Right version…</option>
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
          >
            Compare
          </button>
        </div>
        {comparison && (
          <small>
            {comparison.same_content ? "Same content" : "Different content"} · +
            {comparison.added_nodes.length} / −{comparison.removed_nodes.length}{" "}
            / changed {comparison.changed_nodes.length}
          </small>
        )}
      </div>
      </details>
      <div className="workflow-layout">
        <aside className="panel workflow-list">
          <span className="eyebrow">Current Draft</span>
          <h2>{draft ? draft.name : "No Current Draft"}</h2>
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
              title="No drafts"
              detail="Create a blank Draft, use a template, or ask the registry-bound Advisor."
            />
          )}
          {drafts.length > 1 && (
            <details className="draft-history">
              <summary>Historical Drafts ({drafts.filter((item) => item.id !== draft?.id).length})</summary>
              {drafts.filter((item) => item.id !== draft?.id).map((item) => (
                <button key={item.id} onClick={() => { setDraft(item); setReport(undefined); }}>
                  <span><strong>{item.name}</strong><small>{item.updated_at}</small></span>
                  <Status status={item.status} />
                </button>
              ))}
            </details>
          )}
          <span className="eyebrow workflow-published-title">
            Default Published Version
          </span>
          {entries.filter(({ project, workflow }) => project.id === activeProjectId && workflow.is_default).map(({ project, workflow }) => (
            <button
              key={`${project.id}-${workflow.workflow_id}-${workflow.version}`}
              onClick={() => {
                onActivate(project.id);
                setSelectedPublishedKey(
                  `${workflow.workflow_id}:${workflow.version}`,
                );
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
        <div>
          {draft ? (
            <Panel
              title={draft.name}
              eyebrow={`${draft.status} · autosaved Automation Draft`}
            >
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
              <div className="natural-workflow-recipe">
                <span className="eyebrow">Automation Recipe</span>
                <ol>{guidedWorkflowNodes(draft.nodes).map((node) => <li key={`recipe-${node.id}`}><strong>{workflowNodeTitle(node.node_type)}</strong><small>{node.model_binding ? `Model · ${node.model_binding}` : "Reliable built-in step"}</small></li>)}</ol>
                {!draft.nodes.length && <Empty title="No Automation steps" detail="Start from a template or preview an AnnotAgent recommendation." />}
              </div>
              <details className="advanced-graph">
                <summary>View technical graph</summary>
                <p>These technical nodes and connections edit the same autosaved Draft shown above.</p>
                <div className="button-row"><button onClick={addNode} disabled={immutable}>Add node</button><button onClick={addEdge} disabled={immutable || draft.nodes.length < 2}>Add connection</button></div>
              <div className="workflow-nodes editable-workflow">
                {draft.nodes.map((node, index) => (
                  <article key={node.id}>
                    <span className="node-index">
                      {String(index + 1).padStart(2, "0")}
                    </span>
                    <div>
                      <label>
                        Node ID
                        <input
                          value={node.id}
                          disabled={immutable}
                          onChange={(event) =>
                            updateNode(index, { id: event.target.value })
                          }
                        />
                      </label>
                      <div className="form-grid">
                        <label>
                          Node type
                          <select
                            value={node.node_type}
                            disabled={immutable}
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
                        <label>
                          Model binding
                          <select
                            value={node.model_binding ?? ""}
                            disabled={immutable}
                            onChange={(event) =>
                              updateNode(index, {
                                model_binding: event.target.value || undefined,
                              })
                            }
                          >
                            <option value="">No model</option>
                            {workflowCatalogModelOptions(catalog).map((model) => (
                              <option key={model.id} value={model.id}>
                                {model.display_name}
                              </option>
                            ))}
                          </select>
                        </label>
                        <label>
                          Fallback node
                          <input
                            value={node.fallback ?? ""}
                            disabled={immutable}
                            placeholder="none"
                            onChange={(event) =>
                              updateNode(index, {
                                fallback: event.target.value || undefined,
                              })
                            }
                          />
                        </label>
                        <label>
                          Retries
                          <input
                            type="number"
                            min="0"
                            value={node.max_retries}
                            disabled={immutable}
                            onChange={(event) =>
                              updateNode(index, {
                                max_retries: Number(event.target.value),
                              })
                            }
                          />
                        </label>
                      </div>
                      <label>
                        Depends on
                        <input
                          value={node.depends_on.join(", ")}
                          disabled={immutable}
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
                        <label>
                          Validators
                          <input
                            value={node.validators.join(", ")}
                            disabled={immutable}
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
                        <label>
                          Refiners
                          <input
                            value={node.refiners.join(", ")}
                            disabled={immutable}
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
                          disabled={immutable}
                          onChange={(event) =>
                            updateNode(index, {
                              review_gate: event.target.checked,
                            })
                          }
                        />
                        Human review gate
                      </label>
                      <label>
                        Parameters (JSON)
                        <textarea
                          value={JSON.stringify(node.parameters, null, 2)}
                          disabled={immutable}
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
                        <span>
                          Inputs ·{" "}
                          {node.inputs
                            ?.map((port) => `${port.id}:${port.artifact_type}`)
                            .join(", ") || "none"}
                        </span>
                        <span>
                          Outputs ·{" "}
                          {node.outputs
                            ?.map((port) => `${port.id}:${port.artifact_type}`)
                            .join(", ") || "none"}
                        </span>
                      </div>
                      <button
                        className="danger"
                        onClick={() => removeNode(index)}
                        disabled={immutable}
                      >
                        Delete node
                      </button>
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
                        <label>
                          From node
                          <input
                            value={edge.from_node}
                            disabled={immutable}
                            onChange={(event) =>
                              updateEdge(index, {
                                from_node: event.target.value,
                              })
                            }
                          />
                        </label>
                        <label>
                          From port
                          <input
                            value={edge.from_port}
                            disabled={immutable}
                            onChange={(event) =>
                              updateEdge(index, {
                                from_port: event.target.value,
                              })
                            }
                          />
                        </label>
                        <label>
                          To node
                          <input
                            value={edge.to_node}
                            disabled={immutable}
                            onChange={(event) =>
                              updateEdge(index, { to_node: event.target.value })
                            }
                          />
                        </label>
                        <label>
                          To port
                          <input
                            value={edge.to_port}
                            disabled={immutable}
                            onChange={(event) =>
                              updateEdge(index, { to_port: event.target.value })
                            }
                          />
                        </label>
                        <label>
                          Gate route
                          <input
                            value={edge.route ?? ""}
                            disabled={immutable}
                            onChange={(event) =>
                              updateEdge(index, {
                                route: event.target.value || undefined,
                              })
                            }
                          />
                        </label>
                      </div>
                      <button
                        className="danger"
                        onClick={() => removeEdge(index)}
                        disabled={immutable}
                      >
                        Delete connection
                      </button>
                    </div>
                  </article>
                ))}
              </div>
              <section className="runtime-policy-summary" aria-label="Runtime Policies">
                <span className="eyebrow">Runtime behavior · not graph nodes</span>
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
                      ? "Dry Run passed"
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
              title="Select a Workflow"
              detail="Choose a Project and create or select a Draft."
            />
          )}
        </div>
      </div>
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
  immutable,
  onChange,
}: {
  draft: WorkflowDraft;
  immutable: boolean;
  onChange: (draft: WorkflowDraft) => void;
}) {
  const [raw, setRaw] = useState(() => JSON.stringify(draft.label_pipeline, null, 2));
  const [error, setError] = useState("");
  useEffect(() => {
    setRaw(JSON.stringify(draft.label_pipeline, null, 2));
    setError("");
  }, [draft.id, draft.label_pipeline]);
  const apply = () => {
    try {
      const next = JSON.parse(raw) as NonNullable<WorkflowDraft["label_pipeline"]>;
      if (!Array.isArray(next.shared_stages) || !Array.isArray(next.label_pipelines))
        throw new Error("Technical graph must contain shared_stages and label_pipelines arrays.");
      onChange({ ...draft, label_pipeline: next });
      setError("");
    } catch (reason) {
      setError((reason as Error).message);
    }
  };
  return <details className="advanced-graph">
    <summary>View technical graph</summary>
    <p>This is the same autosaved Workflow Definition shown by the guided Recipe. Applying valid JSON changes the current Draft; it never publishes.</p>
    <textarea aria-label="Technical graph JSON" value={raw} readOnly={immutable} onChange={(event) => setRaw(event.target.value)} />
    {error && <small className="field-error" role="alert">{error}</small>}
    {!immutable && <button onClick={apply}>Apply technical graph to Draft</button>}
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
            : source.source === "step" && stepIds.has(source.step_id),
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
          <span className="eyebrow">Shared Stages</span>
          <h3>Runs once per image, then serves every Label Pipeline</h3>
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
          <span className="eyebrow">Label Pipelines</span>
          <h3>One execution method per semantic Label</h3>
        </div>
        <div className="button-row">
          <select
            aria-label="Edited Label Pipeline"
            value={selected?.id ?? ""}
            onChange={(event) => setPipelineId(event.target.value)}
          >
            {composition.label_pipelines.map((pipeline) => (
              <option key={pipeline.id} value={pipeline.id}>
                {pipeline.target_task_id} / {pipeline.target_label}
              </option>
            ))}
          </select>
          <details className="recipe-edit-menu"><summary>Edit automation</summary><div>
          <select
            aria-label="Node Catalog"
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
          <button onClick={addCatalogNode} disabled={immutable || !selected}>
            Add step
          </button>
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
          >
            Add detection + crop
          </button>
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
          >
            Use VLM detection + crop
          </button>
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
      <span className="pipeline-step-kind">{shared ? "shared" : "Label pipeline"}</span>
      <strong>{pipelineStepTitle(step, targetLabel)}</strong>
      <small>{pipelineStepDescription(step, targetLabel)}</small>
      <div className="pipeline-card-summary">
        <span>Runs with <strong>{step.model_binding?.model_id ?? "AnnotAgent Core"}</strong></span>
        {mergedCount > 1 && <span>Guided action <strong>{mergedCount} coordinated operations</strong></span>}
        <Status status={immutable ? "published" : "valid"} />
      </div>
      <button
        aria-label={immutable ? "Inspect node" : "Configure node"}
        onClick={onConfigure}
      >
        {immutable ? "Inspect" : "Configure"}
      </button>
      {onRemove && step.kind !== "commit" && (
        <button className="danger" onClick={onRemove} disabled={immutable}>
          Remove step
        </button>
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
          <div><span className="eyebrow">Pipeline step</span><h2 id="node-drawer-title">{pipelineStepTitle(step)}</h2></div>
          <button ref={closeRef} onClick={onClose} aria-label="Close node configuration">Close</button>
        </header>
        <Fact label="Status" value={immutable ? "Published · read only" : "Draft · editable"} />
        {step.model_binding && (
          <label>Model<select value={step.model_binding.model_id} disabled={immutable} onChange={(event) => onChange({ ...step, model_binding: { ...step.model_binding!, model_id: event.target.value } })}>
            {workflowCatalogModelOptions(catalog).filter((model) => model.capabilities.includes(step.model_binding!.capability)).map((model) => <option key={model.id} value={model.id}>{model.display_name}</option>)}
          </select></label>
        )}
        {Array.isArray(step.parameters.labels) && (
          <label>Labels<input value={step.parameters.labels.join(", ")} disabled={immutable} onChange={(event) => onChange({ ...step, parameters: { ...step.parameters, labels: event.target.value.split(",").map((label) => label.trim()).filter(Boolean) } })} /></label>
        )}
        {isDetection && typeof step.parameters.object_description === "string" && (
          <label>What should the model find?<textarea value={step.parameters.object_description} disabled={immutable} onChange={(event) => onChange({ ...step, parameters: { ...step.parameters, object_description: event.target.value } })} /></label>
        )}
        {step.node_type === "core.confidence_gate" && <label>Confidence threshold<input type="number" min="0" max="1" step="0.05" value={Number(step.parameters.threshold ?? 0)} disabled={immutable} onChange={(event) => updateNumber("threshold", Number(event.target.value))} /></label>}
        {step.node_type === "core.filter" && <label>Minimum confidence<input type="number" min="0" max="1" step="0.05" value={Number(step.parameters.minimum_confidence ?? 0)} disabled={immutable} onChange={(event) => updateNumber("minimum_confidence", Number(event.target.value))} /></label>}
        {step.node_type === "core.crop" && <label>Crop padding<input type="number" min="0" max="0.5" step="0.01" value={Number(step.parameters.padding ?? 0)} disabled={immutable} onChange={(event) => updateNumber("padding", Number(event.target.value))} /></label>}
        {isDetection && (
          <fieldset className="grounding-assist-fieldset">
            <legend>Positioning assistance</legend>
            <label className="checkbox-row"><input type="checkbox" checked={Boolean(groundingAssist.enabled)} disabled={immutable} onChange={(event) => updateGroundingAssist({ enabled: event.target.checked })} />Use a positioning grid to improve coordinate accuracy</label>
            {Boolean(groundingAssist.enabled) && <div className="form-grid">
              <label>Grid rows<input type="number" min="2" max="16" value={Number(groundingAssist.rows ?? 10)} disabled={immutable} onChange={(event) => updateGroundingAssist({ rows: Number(event.target.value) })} /></label>
              <label>Grid columns<input type="number" min="2" max="16" value={Number(groundingAssist.columns ?? 10)} disabled={immutable} onChange={(event) => updateGroundingAssist({ columns: Number(event.target.value) })} /></label>
            </div>}
            <small>The original image remains the source of truth; the grid is sent only as a second calibration view.</small>
          </fieldset>
        )}
        <details><summary>Expert details</summary>
          <code>{step.node_type} · {step.id}</code>
          <label>Input<input readOnly value={Object.entries(step.inputs).map(([name, source]) => `${name}: ${source.source === "image" ? "Image" : `${source.step_id}.${source.port}`}`).join(" + ") || "None"} /></label>
          <label>Output<input readOnly value={Object.entries(step.outputs).map(([name, type]) => `${name}: ${type}`).join(", ") || "Terminal"} /></label>
          <label>Fallback node<input value={step.fallback ?? ""} disabled={immutable} placeholder="No fallback" onChange={(event) => onChange({ ...step, fallback: event.target.value || undefined })} /></label>
          <label>Raw parameters and class mapping<textarea value={parameters} disabled={immutable} onChange={(event) => {
            setParameters(event.target.value);
            try { onChange({ ...step, parameters: JSON.parse(event.target.value) as Record<string, unknown> }); } catch { /* Keep editing until JSON is valid. */ }
          }} /></label>
          <Fact label="Kind" value={step.kind} />
          <Fact label="Retries" value={step.retry_policy.max_attempts} />
          <Fact label="Validators" value={step.validators.join(", ") || "None"} />
          <Fact label="Refiners" value={step.refiners.join(", ") || "None"} />
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

function pipelineModelBinding(nodeType: string, catalog?: WorkflowCatalog) {
  const capability = nodeType === "vlm_detection.detect"
    ? "vision_language"
    : nodeType === "capability.segment"
    ? "prompted_segmentation"
    : nodeType.includes("classify")
    ? "classification"
    : nodeType.includes("detect")
      ? "object_detection"
      : undefined;
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
        <span className="eyebrow">Visual preview</span>
        {imageUrl ? (
          <div className="artifact-image-stage">
            <img src={imageUrl} alt="Original Pipeline input" />
            <svg viewBox="0 0 100 100" preserveAspectRatio="none" aria-label="Artifact bounding boxes">
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
          <Fact label="Status" value={node.status} />
          <Fact label="Latency" value={`${node.latency_ms} ms`} />
          <Fact label="Attempts" value={node.attempts} />
          <Fact label="Cache" value={node.cache_hit ? "hit" : "miss"} />
        </div>
        {node.error && (
          <p className="run-reason">
            <code>{node.error.code}</code> {node.error.summary}
          </p>
        )}
        <details open>
          <summary>Configuration</summary>
          <pre>{JSON.stringify(node.configuration, null, 2)}</pre>
        </details>
        <details>
          <summary>Inputs · {node.inputs.length}</summary>
          <pre>{JSON.stringify(node.inputs, null, 2)}</pre>
        </details>
        <details open>
          <summary>Outputs · {node.outputs.length}</summary>
          <pre>{JSON.stringify(node.outputs, null, 2)}</pre>
        </details>
        {replay && replay.replayed_from === node.node_id && (
          <div className="validation-report valid">
            <strong>Sandbox Replay completed</strong>
            <small>Re-executed: {replay.reexecuted_nodes.join(", ")}</small>
            <small>
              Preserved upstream: {replay.preserved_upstream_nodes.join(", ")}
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
        <span className="eyebrow">Local model runtime</span>
        <h2>Expert Model Plugins</h2>
        <p>Install an isolated Rust runtime, then pair it with a verified, versioned Model Bundle. Files, licenses, Contracts, and smoke-test evidence stay visible.</p>
        <div className="plugin-trust-line" aria-label="Plugin runtime properties"><span>Isolated Rust process</span><span>Verified Model Bundles</span><span>Immutable versions</span></div>
      </div>
      <dl className="plugin-registry-summary" aria-label="Plugin Registry summary">
        <div><dt>Ready models</dt><dd>{readyModels}</dd><small>selectable now</small></div>
        <div><dt>Finish setup</dt><dd>{setupInstallations}</dd><small>installed packages</small></div>
        <div className={attentionInstallations ? "attention" : ""}><dt>Needs attention</dt><dd>{attentionInstallations}</dd><small>failed checks</small></div>
      </dl>
    </header>

    <section className="plugin-page-guidance" aria-label="Plugin setup overview">
      <article className="plugin-setup-roadmap">
        <div><span className="eyebrow">How setup works</span><strong>From local package to selectable model</strong></div>
        <ol>
          <li><span>1</span><small>Install package</small></li>
          <li><span>2</span><small>Install model</small></li>
          <li><span>3</span><small>Smoke test</small></li>
          <li><span>4</span><small>Use in Workflow</small></li>
        </ol>
      </article>
      <article className="plugin-agent-policy" aria-label="Agent plugin permissions">
        <span className="plugin-policy-mark" aria-hidden="true">A</span>
        <div><strong>Agent discovery is read only</strong><p>Pipeline Builder can recommend compatible Ready models. Installation, licenses, model assets, and executables always remain manual actions.</p></div>
      </article>
    </section>

    <details className="plugin-install-wizard" open={!installations.length}>
      <summary><span className="plugin-install-summary"><i aria-hidden="true">+</i><span><strong>Install plugin runtime</strong><small>Advanced setup for a local .annotplugin package. Model assets are installed separately.</small></span></span><b>Advanced</b></summary>
      <div className="plugin-install-body">
        <ol className="plugin-install-steps" aria-label="Installation steps">
          {['Select package', 'Verify package', 'Review permissions', 'Install runtime'].map((step, index) => <li className={verified && index < 2 ? "complete" : ""} key={step}><span>{index + 1}</span>{step}</li>)}
        </ol>
        <div className="plugin-package-picker">
          <label htmlFor="expert-plugin-package">Plugin package</label>
          <FilePicker id="expert-plugin-package" label="Plugin package" accept=".annotplugin" file={packageFile} chooseLabel="Choose plugin package" emptyLabel="No .annotplugin selected" onSelect={(file) => { setPackageFile(file); setVerified(undefined); setMessage(""); }} />
          <button onClick={inspectPackage} disabled={!packageFile || Boolean(busy)}>{busy === "inspect" ? "Verifying…" : "Verify package"}</button>
        </div>
        {verified && <div className="plugin-review-grid">
          <section>
            <span className="eyebrow">Package identity</span>
            <h3>{verified.manifest.display_name} <small>v{verified.manifest.version}</small></h3>
            <p>{verified.manifest.description}</p>
            <dl className="registry-facts">
              <div><dt>Publisher</dt><dd>{verified.manifest.publisher}</dd></div>
              <div><dt>Package hash</dt><dd title={verified.package_sha256}>{verified.package_sha256.slice(0, 12)}…</dd></div>
              <div><dt>Plugin API</dt><dd>{verified.manifest.plugin_api}</dd></div>
              <div><dt>Runtime</dt><dd>Native Rust process</dd></div>
              <div><dt>Targets</dt><dd>{verified.manifest.compatibility.targets.join(", ")}</dd></div>
              <div><dt>Implementation</dt><dd>{verified.manifest.implementation_status.replaceAll("_", " ")}</dd></div>
              <div><dt>Publisher signature</dt><dd>{verified.signature_trusted ? "Trusted" : verified.signature.replaceAll("_", " ")}</dd></div>
            </dl>
          </section>
          <fieldset className="plugin-review-checks">
            <legend>Required human review</legend>
            <div className="plugin-permission-summary">
              <span>Network <strong>{verified.manifest.permissions.network.replaceAll("_", " ")}</strong></span>
              <span>Provider secrets <strong>{verified.manifest.permissions.provider_secrets ? "Requested" : "Denied"}</strong></span>
              <span>Project files <strong>{verified.manifest.permissions.project_files ? "Requested" : "Denied"}</strong></span>
              <span>Subprocesses <strong>{verified.manifest.permissions.subprocesses ? "Requested" : "Denied"}</strong></span>
            </div>
            <label className="checkbox-line"><input type="checkbox" checked={permissionsReviewed} onChange={(event) => setPermissionsReviewed(event.target.checked)} /><span>I reviewed the publisher, target, runtime resources, and requested permissions.</span></label>
            <label className="checkbox-line"><input type="checkbox" checked={codeLicenseAccepted} onChange={(event) => setCodeLicenseAccepted(event.target.checked)} /><span>I accept the code license: <strong>{verified.manifest.license.code}</strong>.</span></label>
            {verified.manifest.weights.required && <label className="checkbox-line"><input type="checkbox" checked={weightLicenseAccepted} onChange={(event) => setWeightLicenseAccepted(event.target.checked)} /><span>I accept the weight license: <strong>{verified.manifest.license.weights}</strong>.</span></label>}
            {!verified.web_installable && <p className="setup-requirement" role="status">{verified.install_guidance}</p>}
            <button className="primary" onClick={installPackage} disabled={Boolean(busy) || !verified.web_installable || !permissionsReviewed || !codeLicenseAccepted || (verified.manifest.weights.required && !weightLicenseAccepted)} title={!verified.web_installable ? verified.install_guidance : undefined}>{busy === "install" ? "Installing…" : "Install trusted package"}</button>
          </fieldset>
        </div>}
      </div>
    </details>

    <details className="plugin-install-wizard model-bundle-import">
      <summary><span className="plugin-install-summary"><i aria-hidden="true">⇧</i><span><strong>Import .annotmodel</strong><small>Advanced local import for an already prepared data-only Model Bundle.</small></span></span><b>Advanced</b></summary>
      <div className="plugin-install-body">
        <div className="plugin-package-picker">
          <label htmlFor="expert-model-bundle">Model Bundle</label>
          <FilePicker id="expert-model-bundle" label="Model Bundle" accept=".annotmodel" file={bundleFile} chooseLabel="Choose model bundle" emptyLabel="No .annotmodel selected" onSelect={(file) => { setBundleFile(file); setVerifiedBundle(undefined); setMessage(""); }} />
          <button onClick={inspectBundle} disabled={!bundleFile || Boolean(busy)}>{busy === "inspect-bundle" ? "Verifying…" : "Verify Bundle"}</button>
        </div>
        {verifiedBundle && <div className="bundle-import-review">
          <div><span className="eyebrow">Verified package</span><h3>{verifiedBundle.manifest.display_name}</h3><p>{verifiedBundle.manifest.source.upstream_project} · {verifiedBundle.manifest.architecture} · {verifiedBundle.file_count} files</p><code>{verifiedBundle.bundle_sha256}</code></div>
          <div><strong>{verifiedBundle.manifest.license.name}</strong><small>{verifiedBundle.manifest.license.redistribution} redistribution · {verifiedBundle.manifest.license.commercial_use} commercial use</small>{verifiedBundle.manifest.license.requires_acceptance && <label className="checkbox-line"><input type="checkbox" checked={bundleImportLicenseAccepted} onChange={(event) => setBundleImportLicenseAccepted(event.target.checked)} /><span>I accept this exact license digest.</span></label>}<button className="primary" onClick={importBundle} disabled={Boolean(busy) || !bundleImportLicenseAccepted}>{busy === "import-bundle" ? "Importing and testing…" : "Import verified Bundle"}</button></div>
        </div>}
      </div>
    </details>

    {setupInstallation && <div className="modal-backdrop model-setup-backdrop">
      <section ref={setupDialogRef} className="model-setup-wizard" role="dialog" aria-modal="true" aria-labelledby="model-setup-title" aria-describedby="model-setup-description">
        <header><div><span className="eyebrow">Model Setup</span><h3 id="model-setup-title">{setupInstallation.manifest.display_name}</h3><p id="model-setup-description">Only a verified Bundle that passes the exact Rust Plugin smoke test becomes selectable.</p></div><button ref={setupCloseButtonRef} type="button" className="icon-button" aria-label="Close model setup" onClick={closeModelSetup}>×</button></header>
        <div className="model-setup-scroll">
          <ol className="model-setup-progress" aria-label="Model installation progress">
            {MODEL_SETUP_STEPS.map((step, index) => <li className={index < setupStep ? "complete" : index === setupStep ? "current" : ""} key={step}><span>{index < setupStep ? "✓" : index + 1}</span><small>{step}</small></li>)}
          </ol>
          {setupStep === 0 && <div className="model-setup-content"><span className="eyebrow">Select model</span>{setupInventory?.available.length ? <div className="compatible-model-list">{setupInventory.available.map((entry) => <label className={setupEntryIdentity === catalogBundleIdentity(entry) ? "selected" : ""} key={catalogBundleIdentity(entry)}><input type="radio" name="setup-model" checked={setupEntryIdentity === catalogBundleIdentity(entry)} onChange={() => setSetupEntryIdentity(catalogBundleIdentity(entry))} /><span><strong>{entry.display_name}</strong><small>{entry.description}</small><small>{formatPluginBytes(entry.bundle_size_bytes)} · {entry.license_summary.name}</small></span><Status status={entry.fixture ? "Fixture" : "Ready to install"} /></label>)}</div> : setupInventory?.setup_blockers?.length ? <div className="model-setup-blocker" role="status"><span className="model-setup-blocker-label">Update required</span><strong>Plugin runtime update required</strong><p>{setupInventory.setup_blockers[0].message}</p><small>Model files were not downloaded again. The installed Bundle and previous Plugin version remain unchanged.</small></div> : <Empty title="No verified bundle is available for this platform" detail="The Plugin remains installed, but AnnotAgent will not suggest raw ONNX downloads or an unverified model." />}</div>}
          {setupStep === 1 && setupEntry && <div className="model-setup-content"><span className="eyebrow">Review source</span><dl className="bundle-review-facts"><div><dt>Model</dt><dd>{setupEntry.display_name}</dd></div><div><dt>Model family</dt><dd>{setupEntry.model_family ?? "Declared by the verified Bundle"}</dd></div><div><dt>Capability</dt><dd>{setupEntry.capabilities.map((value) => value.replaceAll("_", " ")).join(", ")}</dd></div><div><dt>Publisher</dt><dd>{setupEntry.publisher.display_name}{setupEntry.publisher.verified ? " · verified" : " · unverified"}</dd></div><div><dt>Curated Catalog</dt><dd>{setupEntry.catalog_id}</dd></div><div><dt>Bundle digest</dt><dd>{setupEntry.bundle_sha256}</dd></div><div><dt>Download size</dt><dd>{formatPluginBytes(setupEntry.bundle_size_bytes)}</dd></div><div><dt>Installed size</dt><dd>{formatPluginBytes(setupEntry.installed_size_bytes ?? setupEntry.platform_requirements[0]?.minimum_disk_bytes ?? setupEntry.bundle_size_bytes)}</dd></div><div><dt>Delivery</dt><dd>{setupEntry.fixture ? "Built-in deterministic local Catalog" : setupEntry.bundle_url}</dd></div><div><dt>Release status</dt><dd>{setupEntry.fixture ? "Fixture only · not publishable" : "Real model · production eligible"}</dd></div></dl></div>}
          {setupStep === 2 && setupEntry && <div className="model-setup-content"><span className="eyebrow">Review license</span><div className="license-review-card"><h4>{setupEntry.license_summary.name}</h4><p>Redistribution: {setupEntry.license_summary.redistribution.replaceAll("_", " ")} · Commercial use: {setupEntry.license_summary.commercial_use.replaceAll("_", " ")}</p><code>{setupEntry.license_summary.license_digest}</code>{setupEntry.license_summary.license_url && <a href={setupEntry.license_summary.license_url} target="_blank" rel="noreferrer">Read license source</a>}{setupEntry.license_summary.requires_acceptance && <label className="checkbox-line"><input type="checkbox" checked={setupLicenseAccepted} onChange={(event) => setSetupLicenseAccepted(event.target.checked)} /><span>I accept this exact model license and digest.</span></label>}</div></div>}
          {setupStep === 3 && setupEntry && <div className="model-setup-content"><span className="eyebrow">Check compatibility</span><div className="compatibility-checks"><span className="passed"><b>✓</b><strong>Plugin</strong><small>{setupInstallation.manifest.id}@{setupInstallation.manifest.version}</small></span><span className="passed"><b>✓</b><strong>Model binding</strong><small>{setupEntry.compatible_plugins.map((item) => `${item.model_id} · ${item.required_file_roles.join(" + ")}`).join(", ")}</small></span><span className={setupEntry.platform_requirements.length ? "passed" : "blocked"}><b>{setupEntry.platform_requirements.length ? "✓" : "—"}</b><strong>Platform</strong><small>{setupEntry.platform_requirements.map((item) => item.target).join(", ") || "No supported platform"}</small></span><span className={setupInventory?.plugin_runtime_status === "incompatible" ? "blocked" : "passed"}><b>{setupInventory?.plugin_runtime_status === "incompatible" ? "—" : "✓"}</b><strong>Execution provider</strong><small>{setupEntry.platform_requirements.flatMap((item) => item.execution_providers).join(", ").toUpperCase()} · Rust {setupInventory?.plugin_runtime_status.replaceAll("_", " ")}</small></span></div></div>}
          {setupStep === 4 && <div className="model-setup-content install-stage"><span className="eyebrow">Installation evidence</span><div className="model-install-live"><div><h4>{setupOperation?.status === "failed" ? "Setup stopped safely" : setupOperation ? MODEL_INSTALL_STAGES[setupInstallStageIndex]?.label : "Ready to retry"}</h4><p>{setupOperation?.detail ?? "Review the failure below, then retry from the verified Catalog entry."}</p></div>{setupOperation?.status === "running" && <Status status="Running" />}</div>{setupDownloadPercent !== undefined && setupOperation?.status === "running" && <div className="model-install-meter" aria-label={`Model download ${setupDownloadPercent}%`}><span style={{ width: `${setupDownloadPercent}%` }} /></div>}<ol className="model-install-stage-list" aria-label="Real model installation stages">{MODEL_INSTALL_STAGES.map((stage, index) => <li className={setupOperation?.status === "failed" && index === setupInstallStageIndex ? "failed" : index < setupInstallStageIndex || setupOperation?.status === "succeeded" ? "complete" : index === setupInstallStageIndex ? "current" : "pending"} key={stage.id}><i aria-hidden="true">{index < setupInstallStageIndex || setupOperation?.status === "succeeded" ? "✓" : index === setupInstallStageIndex && setupOperation?.status === "failed" ? "!" : index + 1}</i><span>{stage.label}</span></li>)}</ol></div>}
          {setupStep === 5 && <div className="model-setup-content install-stage ready"><span className="eyebrow">Installation evidence</span><h4>{setupEntry?.fixture ? "Fixture Model Instance Ready" : "Real Model Instance Ready"}</h4><p>{setupEntry?.fixture ? "The Fixture passed its deterministic Rust Plugin test but remains ineligible for Published Workflows." : "The exact Bundle, real ONNX graphs, Rust Plugin, bbox-prompt sample inference, mask validation, and immutable Model Profile are verified. This model is now selectable by Workflow Drafts."}</p>{setupOperation?.model_instance_ids.length ? <code>{setupOperation.model_instance_ids.join(", ")}</code> : null}</div>}
          {setupFailure && <div className="model-setup-error" role="alert"><strong>Setup stopped at {setupOperation ? MODEL_INSTALL_STAGES[setupInstallStageIndex]?.label : MODEL_SETUP_STEPS[setupStep]}</strong><span>{setupFailure}</span><small>{setupOperation?.suggested_action ?? "Review the selected Catalog entry and Plugin compatibility, then retry. Existing verified assets were preserved."}</small></div>}
        </div>
        <footer>{setupStep > 0 && setupStep < 4 && <button onClick={() => { setSetupStep((value) => value - 1); setSetupError(""); }} disabled={Boolean(busy)}>Back</button>}<span />{setupStep < 3 && <button className="primary" onClick={() => setSetupStep((value) => value + 1)} disabled={!setupEntry || (setupStep === 2 && setupEntry.license_summary.requires_acceptance && !setupLicenseAccepted)}>Continue</button>}{setupStep === 3 && <button className="primary" onClick={installSelectedBundle} disabled={Boolean(busy) || !setupEntry}>{busy === "install-model-bundle" ? "Starting installation…" : "Install model"}</button>}{setupStep === 5 && <button className="primary" onClick={closeModelSetup}>Done</button>}{setupFailure && setupStep === 4 && <button className="primary" onClick={() => { setSetupStep(3); setSetupOperationId(""); setSetupError(""); }}>Review and retry</button>}</footer>
      </section>
    </div>}

    {legacyInstallation && legacySetup && <section className="legacy-bundle-creator" aria-label="Create local model bundle">
      <header><div><span className="eyebrow">Legacy migration</span><h3>Create local model bundle</h3><p>AnnotAgent copies the existing files into a data-only Bundle, verifies every hash and ONNX tensor Contract, then runs the exact Rust Plugin smoke test. The originals are never deleted.</p></div><button className="icon-button" aria-label="Close local bundle creator" onClick={() => setLegacySetup(undefined)}>×</button></header>
      <div className="legacy-bundle-form">
        <fieldset><legend>Bundle identity</legend><label>Display name<input value={legacyDraft.display_name} onChange={(event) => setLegacyDraft((current) => ({ ...current, display_name: event.target.value }))} /></label><label>Bundle version<input value={legacyDraft.bundle_version} onChange={(event) => setLegacyDraft((current) => ({ ...current, bundle_version: event.target.value }))} placeholder="1.0.0" /></label><label>Model<input value={legacySetup.modelId} readOnly /></label></fieldset>
        <fieldset><legend>Upstream source</legend><label>Upstream project<input value={legacyDraft.upstream_project} onChange={(event) => setLegacyDraft((current) => ({ ...current, upstream_project: event.target.value }))} placeholder="Project or organization" /></label><label>Upstream model ID<input value={legacyDraft.upstream_model_id} onChange={(event) => setLegacyDraft((current) => ({ ...current, upstream_model_id: event.target.value }))} /></label><label>Upstream version<input value={legacyDraft.upstream_version} onChange={(event) => setLegacyDraft((current) => ({ ...current, upstream_version: event.target.value }))} /></label><label>Source URL<input type="url" value={legacyDraft.source_url} onChange={(event) => setLegacyDraft((current) => ({ ...current, source_url: event.target.value }))} placeholder="https://…" /></label></fieldset>
        <fieldset><legend>Export provenance</legend><label>Exporter name<input value={legacyDraft.exporter_name} onChange={(event) => setLegacyDraft((current) => ({ ...current, exporter_name: event.target.value }))} /></label><label>Exporter version<input value={legacyDraft.exporter_version} onChange={(event) => setLegacyDraft((current) => ({ ...current, exporter_version: event.target.value }))} /></label><label>ONNX opset<input type="number" min="1" max="21" value={legacyDraft.opset} onChange={(event) => setLegacyDraft((current) => ({ ...current, opset: event.target.value }))} /></label></fieldset>
        <fieldset><legend>Model license</legend><label>License name<input value={legacyDraft.license_name} onChange={(event) => setLegacyDraft((current) => ({ ...current, license_name: event.target.value }))} /></label><label>License URL<input type="url" value={legacyDraft.license_url} onChange={(event) => setLegacyDraft((current) => ({ ...current, license_url: event.target.value }))} /></label><label>Redistribution<select value={legacyDraft.redistribution} onChange={(event) => setLegacyDraft((current) => ({ ...current, redistribution: event.target.value as typeof current.redistribution }))}><option value="allowed">Allowed</option><option value="restricted">Restricted</option><option value="unknown">Unknown</option><option value="prohibited">Prohibited</option></select></label><label>Commercial use<select value={legacyDraft.commercial_use} onChange={(event) => setLegacyDraft((current) => ({ ...current, commercial_use: event.target.value as typeof current.commercial_use }))}><option value="allowed">Allowed</option><option value="restricted">Restricted</option><option value="unknown">Unknown</option></select></label><label className="wide">Exact license text<textarea value={legacyDraft.license_text} onChange={(event) => setLegacyDraft((current) => ({ ...current, license_text: event.target.value }))} rows={7} /></label>{legacyDraft.redistribution === "prohibited" && <p className="wide">A redistribution-prohibited asset cannot be packaged as a publishable local Bundle. Keep the legacy files unchanged and resolve the license terms first.</p>}</fieldset>
        <fieldset><legend>ONNX Model Contract</legend><div className="wide legacy-contract-file"><label htmlFor="legacy-contract-json">Contract JSON file</label><FilePicker id="legacy-contract-json" label="Contract JSON file" accept=".json,application/json" file={legacyContractFile} chooseLabel="Choose Contract JSON" emptyLabel="No Contract JSON selected" onSelect={(file) => { setLegacyContractFile(file); if (!file) { setLegacyDraft((current) => ({ ...current, contract_document: "" })); return; } void file.text().then((contract_document) => setLegacyDraft((current) => ({ ...current, contract_document }))); }} /></div><p className="wide">The Contract must use schema version 1 and exactly declare these file roles: <code>{legacyInstallation.manifest.models.find((model) => model.id === legacySetup.modelId)?.required_file_roles.join(", ")}</code>.</p>{legacyDraft.contract_document && <pre className="wide">{legacyDraft.contract_document.slice(0, 800)}{legacyDraft.contract_document.length > 800 ? "…" : ""}</pre>}</fieldset>
      </div>
      <label className="checkbox-line legacy-license-acceptance"><input type="checkbox" checked={legacyDraft.license_accepted} onChange={(event) => setLegacyDraft((current) => ({ ...current, license_accepted: event.target.checked }))} /><span>I supplied and accept the exact license above. I understand this local migration is not publisher-verified.</span></label>
      {legacyError && <div className="model-setup-error" role="alert"><strong>Local Bundle creation stopped</strong><span>{legacyError}</span><small>The legacy model files were not changed or removed.</small></div>}
      <footer><button onClick={() => setLegacySetup(undefined)} disabled={Boolean(busy)}>Cancel</button><button className="primary" onClick={createLegacyBundle} disabled={Boolean(busy) || legacyDraft.redistribution === "prohibited" || !legacyDraft.display_name.trim() || !legacyDraft.upstream_project.trim() || !legacyDraft.upstream_model_id.trim() || !legacyDraft.license_name.trim() || !legacyDraft.license_text.trim() || !legacyDraft.contract_document.trim() || !legacyDraft.license_accepted}>{busy === "create-legacy-bundle" ? "Hashing, packing, and testing…" : "Create and test local Bundle"}</button></footer>
    </section>}

    {message && <p className="registry-message" role="status" aria-live="polite">{message}</p>}
    {loading && <div className="loading-banner" role="status">Loading the local Plugin Registry…</div>}
    {!loading && !installations.length && <Empty title="No Expert Model Plugins installed" detail="Install a verified .annotplugin package above. No model becomes selectable until its real process test passes." />}

    {PLUGIN_STATUS_GROUPS.map((group) => {
      const groupItems = installations.filter((installation) => {
        return group.statuses.includes(hasWorkflowReadyModel(installation) ? "ready" : installation.status);
      });
      if (!groupItems.length) return null;
      return <section className="plugin-status-group" key={group.title} aria-labelledby={`plugin-group-${group.title.replaceAll(" ", "-")}`}>
        <header><div><h3 id={`plugin-group-${group.title.replaceAll(" ", "-")}`}>{group.title}</h3><p>{group.description}</p></div><span>{groupItems.length}</span></header>
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
              ? { tone: "ready", eyebrow: "Ready for Workflows", title: `${workflowReadyInstances.length} verified model${workflowReadyInstances.length === 1 ? "" : "s"} available`, detail: "Plugin, Bundle, Contract, and sample inference evidence are registered." }
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
                <div><dt>Capabilities</dt><dd>{installation.manifest.models.flatMap((model) => model.capabilities).map((value) => value.replaceAll("_", " ")).join(", ")}</dd></div>
                <div><dt>Runtime</dt><dd>Rust native process</dd></div>
                <div><dt>Runtime models</dt><dd>{installation.manifest.models.map((model) => model.display_name).join(", ")}</dd></div>
                <div><dt>Device</dt><dd>{installation.manifest.compatibility.accelerators.join(", ") || "CPU"}</dd></div>
                <div><dt>Installed Bundles</dt><dd>{inventory.installed.length}</dd></div>
                <div><dt>Used by</dt><dd>{installation.references.length} Published Workflow reference{installation.references.length === 1 ? "" : "s"}</dd></div>
              </dl>
              <div className="registry-card-actions">
                {!workflowReadyInstances.length && <button className="primary" onClick={() => openModelSetup(installation)} disabled={Boolean(busy) || installation.status === "unsupported_platform"}>{latestInstallOperation?.status === "running" ? "View installation" : latestInstallOperation?.status === "failed" ? "Review failed setup" : setupBlocker ? "Review required update" : "Install compatible model"}</button>}
                <button onClick={() => perform(`${identity}:toggle`, () => api.setExpertPluginEnabled(installation.manifest.id, installation.manifest.version, !installation.enabled), installation.enabled ? "Plugin disabled." : "Plugin enabled; test evidence is preserved.")} disabled={Boolean(busy) || installation.status === "unsupported_platform"}>{installation.enabled ? "Disable" : "Enable"}</button>
                <button className="danger-button" onClick={() => { if (window.confirm(`Uninstall ${identity}? Installed Model Bundles remain in the shared model store.`)) void perform(`${identity}:uninstall`, () => api.uninstallExpertPlugin(installation.manifest.id, installation.manifest.version), "Plugin version uninstalled."); }} disabled={Boolean(busy) || installation.references.length > 0} title={installation.references.length ? "Published Workflow references protect this exact version" : undefined}>Uninstall</button>
              </div>
              <details className="registry-card-section" open>
                <summary><span>Runtime</span><small>{inventory.plugin_runtime_status.replaceAll("_", " ")}</small></summary>
                <dl className="plugin-detail-list"><div><dt>Process</dt><dd>Isolated native Rust</dd></div><div><dt>Protocol</dt><dd>{installation.manifest.runtime.protocol}</dd></div><div><dt>Plugin version</dt><dd>{installation.manifest.version}</dd></div><div><dt>Package SHA-256</dt><dd>{installation.package_sha256}</dd></div></dl>
              </details>
              <details className="registry-card-section" open={!inventory.installed.length}>
                <summary><span>Compatible Models</span><small>{inventory.available.length} available</small></summary>
                {inventory.available.length ? <div className="compatible-model-list compact">{inventory.available.map((entry) => { const installedInstance = pluginInstances.find((instance) => instance.model_bundle_id === entry.bundle_id && instance.model_bundle_version === entry.bundle_version); const entryOperation = installOperations.find((operation) => operation.plugin_id === installation.manifest.id && operation.plugin_version === installation.manifest.version && operation.bundle_id === entry.bundle_id && operation.bundle_version === entry.bundle_version); return <div key={catalogBundleIdentity(entry)}><span><strong>{entry.display_name}</strong><small>{entry.model_family ?? entry.bundle_id} · {entry.publisher.display_name} · {formatPluginBytes(entry.bundle_size_bytes)} · {entry.license_summary.name}</small></span><Status status={entry.fixture ? "Fixture" : installedInstance?.status === "ready" ? "Ready" : entryOperation?.status === "running" ? "Installing" : "Ready to install"} /><button onClick={() => openModelSetup(installation, entry)}>{entryOperation?.status === "running" ? "View progress" : installedInstance?.status === "ready" ? "View evidence" : entryOperation?.status === "failed" ? "Review failure" : "Install model"}</button></div>; })}</div> : setupBlocker ? <div className="bundle-empty-state warning"><strong>Plugin runtime update required</strong><p>{setupBlocker.message}</p><button onClick={() => openModelSetup(installation)}>Review required update</button></div> : <div className="bundle-empty-state"><strong>No verified bundle is available for this platform</strong><p>Unpublished SAM 2 and unverified checkpoints stay in Labs; AnnotAgent will not turn them into a selectable model.</p></div>}
              </details>
              <details className="registry-card-section">
                <summary><span>Installed Models</span><small>{pluginInstances.length} instances</small></summary>
                {inventory.installed.length ? <div className="installed-bundle-list">{inventory.installed.map((bundle) => { const matching = pluginInstances.filter((instance) => instance.model_bundle_id === bundle.manifest.id && instance.model_bundle_version === bundle.manifest.version); const instanceReady = matching.some((instance) => instance.status === "ready"); return <div key={bundleIdentity(bundle)}><header><span><strong>{bundle.manifest.display_name}</strong><small>{bundleIdentity(bundle)} · {bundle.manifest.variant}</small></span><Status status={bundle.manifest.fixture && instanceReady ? "Fixture" : instanceReady ? "Ready" : bundle.status.replaceAll("_", " ")} /></header><code>{bundle.bundle_sha256}</code>{bundle.manifest.fixture && <p>Offline contract test only · not selectable for Published Workflows</p>}{matching.map((instance) => <p key={instance.id}>{instance.execution_provider.toUpperCase()} · {instance.status.replaceAll("_", " ")} · profile revision {instance.model_profile_revision}</p>)}</div>; })}</div> : <div className={`bundle-empty-state${setupBlocker ? " warning" : ""}`}><strong>{setupBlocker ? "Installed model cannot bind to this Plugin version" : "No compatible model installed"}</strong><p>{setupBlocker ? "The existing Bundle is preserved. Update the immutable Plugin runtime before creating a Model Instance." : "This Plugin cannot run until a verified model is installed."}</p><button className="primary" onClick={() => openModelSetup(installation)}>{setupBlocker ? "Review required update" : "Install compatible model"}</button></div>}
              </details>
              <details className="registry-card-section">
                <summary><span>Model Setup</span><small>{workflowReadyInstances.length ? "Ready" : fixtureReadyInstances.length ? "Fixture only" : "Action required"}</small></summary>
                {pluginInstances.map((instance) => <div className="model-instance-evidence" key={instance.id}><span><strong>{instance.model_bundle_id}</strong><small>{instance.contract_inspection.valid ? "ONNX Contract verified" : instance.contract_inspection.errors.join(" · ")}</small></span><Status status={instance.status.replaceAll("_", " ")} />{instance.status !== "ready" && <button onClick={() => perform(`${instance.id}:smoke`, () => api.testModelInstance(instance.id), "Fixed Bundle smoke test completed.")} disabled={Boolean(busy)}>{busy === `${instance.id}:smoke` ? "Testing…" : "Run Smoke Test"}</button>}</div>)}
                {!pluginInstances.length && <p>{setupBlocker ? <>No verified Model Instance exists for this Plugin version. Review and install the required immutable runtime update first.</> : <>No verified Model Instance exists. Choose <strong>Install compatible model</strong> to review an available Bundle.</>}</p>}
              </details>
              <details className="registry-card-section"><summary><span>References</span><small>{installation.references.length} protected</small></summary>{installation.references.length ? <ul className="plugin-reference-list">{installation.references.map((reference) => <li key={`${reference.kind}:${reference.location}`}><strong>{reference.kind.replaceAll("_", " ")}</strong><span>{reference.location}</span></li>)}</ul> : <p>No Published Workflow currently protects this Plugin version. Bundle references are tracked independently by exact digest.</p>}</details>
              {installation.weights.length > 0 && <details className="registry-card-section legacy-provisioning"><summary><span>Legacy manual provisioning</span><small>Not recommended</small></summary><div className="legacy-model-warning"><strong>LegacyUnbundledModel</strong><p>These files predate Model Bundles. Their hashes are preserved, but they have no Bundle source, license document, Contract, or reproducible smoke-test identity and are not treated as trusted assets.</p>{installation.weights.map((weight) => <code key={`${weight.model_id}:${weight.component_id}`}>{weight.component_id} · {weight.original_filename} · {weight.checkpoint_sha256}</code>)}<p>Create a local Bundle only after supplying source, license, and a complete Model Contract. A failed conversion never removes these files.</p>{Array.from(new Set(installation.weights.map((weight) => weight.model_id))).map((modelId) => <button key={modelId} onClick={() => openLegacyBundleSetup(installation, modelId)}>Create local model bundle</button>)}</div></details>}
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
          `Imported Provider and Model Profile. ${result.migration.bindings_created} Project binding${result.migration.bindings_created === 1 ? "" : "s"} created; ${result.migration.bindings_preserved} existing choice${result.migration.bindings_preserved === 1 ? " was" : "s were"} preserved.`,
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
          <span className="eyebrow">Reusable connections</span>
          <h2>Providers</h2>
          <p>Configure each API connection once. Credentials are write-only and never returned to this page.</p>
        </div>
        <button className="primary" onClick={() => setAdding((value) => !value)}>
          {adding ? "Cancel" : "Add provider"}
        </button>
      </div>
      {notice && <div className="positive-empty" role="status"><strong>{notice}</strong></div>}
      {adding && (
        <Panel title="New Provider" eyebrow="Connection profile">
          <div className="form-grid">
            <label>Preset<select value={presetId} onChange={(event) => choosePreset(event.target.value)}>{presets.map((preset) => <option key={preset.id} value={preset.id}>{preset.display_name}</option>)}</select></label>
            <label>Display name<input value={displayName} onChange={(event) => setDisplayName(event.target.value)} /></label>
            <label>Adapter<select value="open_ai_compatible" disabled><option value="open_ai_compatible">OpenAI compatible</option></select></label>
            <label>Base URL<input type="url" value={baseUrl} onChange={(event) => setBaseUrl(event.target.value)} /></label>
          </div>
          <div className="button-row"><button className="primary" disabled={busy === "create" || !displayName.trim() || !baseUrl.trim()} onClick={create}>{busy === "create" ? "Saving…" : "Save Provider"}</button></div>
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
        <Empty title="No Providers configured" detail="Connect an OpenAI-compatible API before asking AnnotAgent to build a Pipeline." />
      )}
      {legacyImport && !legacyImport.already_applied && (
        <details className="legacy-registry-import">
          <summary>
            <span><strong>Legacy compatibility</strong><small>Optional import available from older workspace settings</small></span>
            <b>Optional</b>
          </summary>
          <div className="legacy-registry-import-body">
            <p>Import older compatibility settings into the Provider Registry. Current Providers continue to work if you leave this untouched.</p>
            <dl>
              <div><dt>Provider</dt><dd>{legacyImport.provider_display_name}</dd></div>
              <div><dt>Model Profile</dt><dd>{legacyImport.model_display_name}</dd></div>
              <div><dt>Project bindings</dt><dd>{legacyImport.project_binding_count}</dd></div>
            </dl>
            <div className="legacy-registry-import-footer">
              <small>The credential remains a {legacyImport.credential_source?.replaceAll("_", " ") ?? "non-secret configuration"} reference. No secret or Run history is moved.</small>
              <button disabled={Boolean(busy)} onClick={importLegacy}>
                {busy === "legacy-import" ? "Importing…" : "Review and import"}
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
        setMessage(`Discovered ${result.models.length} model ID${result.models.length === 1 ? "" : "s"}.`);
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
        <div><dt>Endpoint</dt><dd title={provider.base_url}>{provider.endpoint_summary}</dd></div>
        <div><dt>Credential</dt><dd>{provider.credential_configured ? `${provider.credential_source?.replaceAll("_", " ")} configured` : "Missing"}</dd></div>
        <div><dt>Models</dt><dd>{provider.model_count}</dd></div>
        <div><dt>Last checked</dt><dd>{provider.health.checked_at ? new Date(provider.health.checked_at).toLocaleString() : "Never"}</dd></div>
      </dl>
      {provider.health.safe_message && <p className="registry-safe-message">{provider.health.safe_message}</p>}
      <div className="registry-card-actions">
        <button disabled={Boolean(busy) || !provider.enabled} onClick={() => run("check", () => api.checkProvider(provider.id), "Connection check succeeded without a generation request.")}>{busy === "check" ? "Checking…" : "Check connection"}</button>
        <button disabled={Boolean(busy) || !provider.enabled} onClick={discover}>{busy === "discover" ? "Discovering…" : "Discover models"}</button>
        <button disabled={Boolean(busy)} onClick={() => run("toggle", () => api.updateProvider(provider.id, { enabled: !provider.enabled }), provider.enabled ? "Provider disabled." : "Provider enabled; run a connection check.")}>{provider.enabled ? "Disable" : "Enable"}</button>
      </div>
      <details className="registry-card-section">
        <summary>Edit connection</summary>
        <div className="form-grid">
          <label>Display name<input value={editDisplayName} onChange={(event) => setEditDisplayName(event.target.value)} /></label>
          <label>Base URL<input type="url" value={editBaseUrl} onChange={(event) => setEditBaseUrl(event.target.value)} /></label>
        </div>
        <div className="button-row"><button disabled={Boolean(busy) || !editDisplayName.trim() || !editBaseUrl.trim()} onClick={() => run("edit", () => api.updateProvider(provider.id, { display_name: editDisplayName, base_url: editBaseUrl }), "Provider connection updated.")}>{busy === "edit" ? "Saving…" : "Save connection"}</button></div>
        <small>Changing an endpoint is blocked while Model Profiles reference this Provider; create a new Provider and rebind instead.</small>
      </details>
      <details className="registry-card-section">
        <summary>{provider.credential_configured ? "Rotate or remove credential" : "Add credential"}</summary>
        <>
          <div className="credential-editor">
            <div className="credential-field">
              <label htmlFor={`${credentialFieldId}-storage`}>Storage</label>
              <select id={`${credentialFieldId}-storage`} aria-describedby={`${credentialFieldId}-storage-help`} value={credentialSource} onChange={(event) => setCredentialSource(event.target.value as typeof credentialSource)}>
                <option value="workspace_file">Local workspace file</option>
                <option value="environment_variable">Environment variable</option>
                <option value="session_only">This server session only</option>
                <option value="system_keyring">System credential store</option>
              </select>
              <p className="credential-field-help" id={`${credentialFieldId}-storage-help`}>{credentialStorageHelp}</p>
            </div>
            {credentialSource === "environment_variable" ? <div className="credential-field">
              <label htmlFor={`${credentialFieldId}-variable`}>Variable name</label>
              <input id={`${credentialFieldId}-variable`} aria-describedby={`${credentialFieldId}-variable-help`} value={environmentVariable} onChange={(event) => setEnvironmentVariable(event.target.value)} placeholder="DASHSCOPE_API_KEY" />
              <p className="credential-field-help" id={`${credentialFieldId}-variable-help`}>Enter only a variable name, such as <code>DASHSCOPE_API_KEY</code>. Do not paste the API key into this field.</p>
            </div> : <div className="credential-field">
              <label htmlFor={`${credentialFieldId}-secret`}>API key</label>
              <input id={`${credentialFieldId}-secret`} aria-describedby={`${credentialFieldId}-secret-help`} type="password" autoComplete="new-password" value={secret} onChange={(event) => setSecret(event.target.value)} placeholder={provider.credential_configured ? "Enter a replacement key" : "Paste API key"} />
              <p className="credential-field-help" id={`${credentialFieldId}-secret-help`}>For security, an existing key is never shown here. Saving replaces the current key.</p>
            </div>}
          </div>
          <div className="credential-actions"><button className="primary" disabled={busy === "credential" || (credentialSource === "environment_variable" ? !environmentVariable.trim() : !secret.trim())} onClick={saveCredential}>{busy === "credential" ? "Saving…" : provider.credential_configured ? "Rotate credential" : "Save credential"}</button><button disabled={!provider.credential_configured || Boolean(busy)} onClick={() => run("remove-credential", () => api.deleteProviderCredential(provider.id), "Credential reference removed.")}>Remove credential</button>{provider.credential_source === "legacy_workspace_file" && <button disabled={Boolean(busy)} onClick={() => run("migrate", () => api.migrateProviderCredential(provider.id, false), "Credential copied to the system credential store. The legacy source was preserved.")}>Migrate legacy credential</button>}</div>
        </>
      </details>
      <details className="registry-card-section">
        <summary>Billable model test</summary>
        <p>This is separate from Check connection and sends a real generation request.</p>
        {models.length ? <div className="button-row"><select aria-label="Model Profile for active probe" value={selectedModel} onChange={(event) => setSelectedModel(event.target.value)}>{models.map((model) => <option key={model.id} value={model.id}>{model.display_name} · r{model.revision}</option>)}</select><button disabled={Boolean(busy) || !provider.enabled} onClick={probe}>{busy === "probe" ? "Testing…" : "Run billable test"}</button></div> : <button onClick={onOpenModels}>Add a Model Profile</button>}
      </details>
      {discovery && <details className="registry-discovery" open><summary>Discovered model IDs · {discovery.models.length}</summary><p>{discovery.warning}</p><div className="discovered-model-list">{discovery.models.slice(0, 100).map((model) => <code key={model.remote_model_id}>{model.remote_model_id}</code>)}</div><button onClick={onOpenModels}>Create a verified Model Profile</button></details>}
      {message && <small className="registry-message" role="status">{message}</small>}
      <details className="advanced-settings"><summary>Advanced and destructive actions</summary><div className="button-row"><button className="danger-button" disabled={Boolean(busy)} onClick={remove}>Delete Provider</button></div><small>Deletion is blocked when Models, Drafts, published Workflows, Runs, or bindings reference this Provider.</small></details>
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
      <div className="toolbar-panel"><div><span className="eyebrow">Reusable capability contracts</span><h2>Models</h2><p>Model Profiles bind a Provider model ID to explicit modalities, protocol features, capabilities, pricing, and an immutable revision.</p></div><button className="primary" disabled={!providers.length} onClick={() => adding ? resetEditor() : setAdding(true)}>{adding ? "Cancel" : "Add model"}</button></div>
      {!providers.length && <div className="guided-callout"><strong>Provider setup required</strong><p>Add a Provider before creating a Model Profile.</p><button onClick={onOpenProviders}>Connect a Provider</button></div>}
      <Panel title="Default model choices" eyebrow="Reusable workspace defaults">
        <p>
          Projects may override these choices. Published Workflows still freeze
          the final Model Profile revision.
        </p>
        <div className="registry-default-models">
          <label>
            Default Pipeline Builder model
            <select
              aria-label="Default Pipeline Builder model"
              value={globalDefaults.pipeline_builder ?? ""}
              disabled={busy === "defaults"}
              onChange={(event) =>
                saveGlobalDefault("pipeline_builder", event.target.value)
              }
            >
              <option value="">No global default</option>
              {defaultChoices.pipeline_builder.map((model) => (
                <option key={model.id} value={model.id}>
                  {defaultOption(model)}
                </option>
              ))}
            </select>
            <small>Text · Structured Output · Tool Calls · Available</small>
          </label>
          <label>
            Default Vision Language model
            <select
              aria-label="Default Vision Language model"
              value={globalDefaults.vision_language ?? ""}
              disabled={busy === "defaults"}
              onChange={(event) =>
                saveGlobalDefault("vision_language", event.target.value)
              }
            >
              <option value="">No global default</option>
              {defaultChoices.vision_language.map((model) => (
                <option key={model.id} value={model.id}>
                  {defaultOption(model)}
                </option>
              ))}
            </select>
            <small>Image · Vision Language · Available</small>
          </label>
          <label>
            Default Text model
            <select
              aria-label="Default Text model"
              value={globalDefaults.text_generation ?? ""}
              disabled={busy === "defaults"}
              onChange={(event) =>
                saveGlobalDefault("text_generation", event.target.value)
              }
            >
              <option value="">No global default</option>
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
      {adding && <Panel title={editingId ? "Edit Model Profile" : "New Model Profile"} eyebrow="Manual capability declaration">
        <div className="registry-model-editor">
          <section className="registry-form-section">
            <header><strong>Model identity</strong><small>Choose the connection and enter the exact model identifier exposed by that Provider.</small></header>
            <div className="registry-model-identity">
              <label><span>Provider</span><select value={providerId} onChange={(event) => setProviderId(event.target.value)}>{providers.map((provider) => <option value={provider.id} key={provider.id}>{provider.display_name}</option>)}</select></label>
              <label><span>Display name</span><input value={displayName} onChange={(event) => setDisplayName(event.target.value)} /></label>
              <label><span>Remote model ID</span><input value={remoteModelId} onChange={(event) => setRemoteModelId(event.target.value)} placeholder="Exact Provider model ID" /></label>
            </div>
          </section>
          <div className="registry-option-sections">
            <fieldset className="registry-check-group"><legend>Input modalities</legend>{(["text", "image", "video"] as InputModality[]).map((value) => <label className="checkbox-line" key={value}><input type="checkbox" checked={modalities.includes(value)} onChange={() => toggle(value, modalities, setModalities)} /><span>{value}</span></label>)}</fieldset>
            <fieldset className="registry-check-group"><legend>Protocol features</legend><label className="checkbox-line"><input type="checkbox" checked={toolCalls} onChange={(event) => setToolCalls(event.target.checked)} /><span>Tool calls</span></label><label className="checkbox-line"><input type="checkbox" checked={structuredOutput} onChange={(event) => setStructuredOutput(event.target.checked)} /><span>Structured output</span></label><label className="checkbox-line"><input type="checkbox" checked={jsonSchema} onChange={(event) => setJsonSchema(event.target.checked)} /><span>JSON Schema</span></label></fieldset>
            <fieldset className="registry-check-group registry-capability-group"><legend>Task capabilities</legend>{REGISTRY_MODEL_CAPABILITIES.map((capability) => <label className="checkbox-line" key={capability.id}><input type="checkbox" checked={capabilities.includes(capability.id)} onChange={() => toggle(capability.id, capabilities, setCapabilities)} /><span>{capability.label}</span></label>)}</fieldset>
          </div>
          <section className="registry-form-section">
            <header><strong>Pricing</strong><small>Optional USD estimates used for Run previews and persisted usage summaries.</small></header>
            <div className="registry-pricing-grid">
              <label><span>Input / 1M tokens</span><input aria-label="Input / 1M tokens (USD)" inputMode="decimal" value={inputPrice} onChange={(event) => setInputPrice(event.target.value)} placeholder="Unknown" /><small>USD</small></label>
              <label><span>Output / 1M tokens</span><input aria-label="Output / 1M tokens (USD)" inputMode="decimal" value={outputPrice} onChange={(event) => setOutputPrice(event.target.value)} placeholder="Unknown" /><small>USD</small></label>
              <label><span>Per request</span><input aria-label="Per request (USD)" inputMode="decimal" value={requestPrice} onChange={(event) => setRequestPrice(event.target.value)} placeholder="Unknown" /><small>USD</small></label>
            </div>
          </section>
          <footer className="registry-model-editor-footer">
            <p>Manual capabilities remain unverified until an explicit active probe succeeds.</p>
            <button className="primary" disabled={busy === "save" || !providerId || !displayName.trim() || !remoteModelId.trim() || !modalities.length || !capabilities.length} onClick={save}>{busy === "save" ? "Saving…" : editingId ? "Save as next revision if needed" : "Save Model Profile"}</button>
          </footer>
        </div>
      </Panel>}
      <div className="registry-filter-bar"><label>Provider<select value={providerFilter} onChange={(event) => setProviderFilter(event.target.value)}><option value="all">All Providers</option>{providers.map((provider) => <option key={provider.id} value={provider.id}>{provider.display_name}</option>)}</select></label><label>Capability<select value={capabilityFilter} onChange={(event) => setCapabilityFilter(event.target.value)}><option value="all">All capabilities</option>{REGISTRY_MODEL_CAPABILITIES.map((capability) => <option key={capability.id} value={capability.id}>{capability.label}</option>)}</select></label><label>Health<select value={statusFilter} onChange={(event) => setStatusFilter(event.target.value)}><option value="all">All statuses</option><option value="available">Available</option><option value="unverified">Unverified</option><option value="disabled">Disabled</option><option value="unavailable">Unavailable</option></select></label><label>Input modality<select value={modalityFilter} onChange={(event) => setModalityFilter(event.target.value)}><option value="all">All modalities</option><option value="text">Text</option><option value="image">Image</option><option value="video">Video</option></select></label><label>Enabled<select value={enabledFilter} onChange={(event) => setEnabledFilter(event.target.value)}><option value="all">Enabled and disabled</option><option value="enabled">Enabled</option><option value="disabled">Disabled</option></select></label><label>Pricing<select value={costFilter} onChange={(event) => setCostFilter(event.target.value)}><option value="all">Any pricing status</option><option value="configured">Configured</option><option value="unknown">Unknown</option></select></label></div>
      {filtered.length ? <div className="registry-card-grid">{filtered.map((model) => {
        const provider = providers.find((candidate) => candidate.id === model.provider_id);
        return <article className="registry-model-card" key={model.id}><header><span><strong>{model.display_name}</strong><small>{provider?.display_name ?? "Missing Provider"} · revision {model.revision}</small></span><Status status={model.status} /></header><code>{model.remote_model_id}</code><div className="tag-group">{model.input_modalities.map((value) => <span key={value}>{value} input</span>)}{model.task_capabilities.map((value) => <span key={value}>{value.replaceAll("_", " ")}</span>)}</div><dl className="registry-facts"><div><dt>Protocol</dt><dd>{[model.protocol_features.tool_calls && "tools", model.protocol_features.structured_output && "structured", model.protocol_features.json_schema && "JSON Schema"].filter(Boolean).join(" · ") || "basic"}</dd></div><div><dt>Capability source</dt><dd>{model.capability_source.replaceAll("_", " ")}</dd></div><div><dt>Pricing</dt><dd>{model.pricing.source === "unknown" ? "Unknown" : `${model.pricing.currency} · ${model.pricing.source.replaceAll("_", " ")}`}</dd></div><div><dt>Binding lock</dt><dd>{model.locked ? "Locked" : "Editable"}</dd></div></dl><ModelQualityContracts modelId={model.id} onError={onError} /><div className="registry-card-actions"><button disabled={busy === model.id} onClick={() => edit(model)}>Edit</button><button disabled={busy === model.id || !provider?.enabled} onClick={() => probe(model)}>{busy === model.id ? "Working…" : "Run billable test"}</button><button disabled={busy === model.id} onClick={() => change(model, { enabled: !model.enabled })}>{model.enabled ? "Disable" : "Enable"}</button><button disabled={busy === model.id} onClick={() => change(model, { locked: !model.locked })}>{model.locked ? "Unlock" : "Lock"}</button></div><details className="advanced-settings"><summary>Revision and destructive actions</summary><pre>{JSON.stringify({ limits: model.limits, generation_defaults: model.generation_defaults, pricing: model.pricing }, null, 2)}</pre><button className="danger-button" disabled={busy === model.id} onClick={() => { if (window.confirm(`Delete ${model.display_name}? Referenced profiles cannot be deleted.`)) { setBusy(model.id); void api.deleteModelProfile(model.id).then(refresh).catch((error: Error) => onError(error.message)).finally(() => setBusy("")); } }}>Delete Model Profile</button></details></article>;
      })}</div> : <Empty title="No matching Model Profiles" detail="Change the filters or add a manually declared model." />}
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
    <summary><span><strong>Score and box quality</strong><small>Operation-scoped safety contract</small></span><b>{contracts?.length ?? "View"}</b></summary>
    {loading && <p>Loading quality contracts…</p>}
    {contracts?.map((contract) => <article key={`${contract.operation}:${contract.capability}`}>
      <header><strong>{contract.operation.replaceAll("_", " ")}</strong><small>Model revision {contract.model_profile_revision}</small></header>
      <dl>
        <div><dt>Geometry output</dt><dd>{geometrySemanticsLabel(contract.output_geometry)}</dd></div>
        <div><dt>Score meaning</dt><dd>{scoreSemanticsLabel(contract.score_semantics)}</dd></div>
        <div><dt>Automatic acceptance</dt><dd>{contract.auto_accept_eligibility === "never_from_score_alone" ? "Never from score alone" : contract.auto_accept_eligibility.replaceAll("_", " ")}</dd></div>
        <div><dt>Evidence</dt><dd>{contract.evidence_source.replaceAll("_", " ")}</dd></div>
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
  return <section className="registry-page"><div className="toolbar-panel"><div><span className="eyebrow">Read-only migration compatibility</span><h2>Legacy HTTP models</h2><p>Existing versioned HTTP Vision Protocol bindings remain inspectable. New native expert models should be installed as isolated Rust packages.</p></div><button onClick={onOpenSettings}>Open compatibility settings</button></div>{workers.length ? <div className="registry-card-grid">{workers.map((worker) => <article className="registry-model-card" key={worker.id}><header><span><strong>{worker.id}</strong><small>{worker.model} · {worker.role}</small></span><Status status={worker.health_status} /></header><code>{worker.endpoint ?? "No endpoint"}</code><div className="tag-group">{worker.capabilities?.map((capability) => <span key={capability}>{capability.replaceAll("_", " ")}</span>)}</div><div className="worker-contract-summary">{worker.score_semantics && <small>Confidence {worker.score_semantics.replaceAll("_", " ")}</small>}{worker.label_space?.length ? <small>Label space · {worker.label_space.join(" · ")}</small> : null}{worker.checkpoint_sha256 && <small>Checkpoint · {worker.checkpoint_sha256.slice(0, 12)}…</small>}{worker.architecture && <small>Architecture · {worker.architecture}</small>}{worker.cost_per_request !== undefined && <small>Estimated cost · ${worker.cost_per_request} / request</small>}</div><p>{worker.health_detail}</p><button disabled={testing === worker.id} onClick={() => test(worker)}>{testing === worker.id ? "Discovering…" : "Refresh discovery"}</button>{results[worker.id] && <div className="registry-safe-message" role="status"><strong>{results[worker.id].passed ? "Discovery passed" : `Stopped at ${results[worker.id].failed_stage ?? "discovery"}`}</strong><span>{results[worker.id].capabilities?.capabilities.join(" · ") || results[worker.id].error}</span><span>{results[worker.id].evidence?.detail}</span></div>}</article>)}</div> : <Empty title="No legacy HTTP models configured" detail="Install a native Rust Expert Model Plugin for new Workflows." />}</section>;
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
  return <section className="registry-page"><div className="toolbar-panel"><div><span className="eyebrow">Recorded Registry operations</span><h2>Usage</h2><p>Active model probes are listed separately from normal Run usage because each probe requires explicit billable confirmation.</p></div></div><div className="metrics-grid"><Metric label="Active probes" value={usage.length} detail="explicitly confirmed" /><Metric label="Probe tokens" value={totals.tokens.toLocaleString()} detail="reported by Providers" /><Metric label="Estimated probe cost" value={`$${totals.cost.toFixed(6)}`} detail="configured pricing snapshots" /></div>{usage.length ? <div className="registry-usage-list">{usage.map((record) => { const model = models.find((candidate) => candidate.id === record.model_profile_id); return <article key={record.id}><span><strong>{model?.display_name ?? record.model_profile_id}</strong><small>{new Date(record.created_at).toLocaleString()} · revision {record.model_profile_revision}</small></span><span>{record.total_tokens ?? "Unknown"} tokens</span><span>{record.currency} {record.cost}</span><Status status={record.succeeded ? "succeeded" : "failed"} /></article>; })}</div> : <Empty title="No active probe usage" detail="Passive connection checks do not generate usage records." />}</section>;
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
        <Fact label="Validation" value={workflow.validation_status} />
        <Fact label="Default" value={workflow.is_default ? "Yes" : "No"} />
        <Fact label="Source" value={workflow.source} />
        <Fact
          label="Enabled Skills"
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
                <span>Fallback · {node.fallback || "none"}</span>
                <span>
                  Human review ·{" "}
                  {node.human_review_gate ? "gate enabled" : "not configured"}
                </span>
              </div>
              <TagGroup title="Validators" values={node.validators} />
              <TagGroup title="Refiners" values={node.refiners} />
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
          <h2>Models</h2>
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
                  <header><div><strong id={`model-group-${group.id}`}>{group.title}</strong><small>{group.detail}</small></div><b>{bindings.length}</b></header>
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
                        {testingModel === binding.id ? "Discovering…" : "Refresh discovery"}
                      </button>
                    </div>}
                    {testResults[binding.id] && <div className="worker-discovery" role="status">
                      <strong>{testResults[binding.id].passed ? "Discovery passed" : `Stopped at ${testResults[binding.id].failed_stage ?? "discovery"}`}</strong>
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
                        : "Custom model IDs"}
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

function RunsPage({
  runs,
  projects,
  activeProject: scopeProject,
  route,
  onNavigate,
  onRefresh,
  onError,
}: {
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
  useEffect(() => {
    void api.batches().then((value) => setBatches(value.batches)).catch((error: Error) => onError(error.message));
  }, [runs.length, runs[0]?.updated_at]);
  const detailRoute =
    route.kind === "runs" || route.kind === "projectRun" ? route : undefined;
  const routeRunId = detailRoute?.runId;
  const run = runs.find((item) => item.id === routeRunId);
  const runOwner = run
    ? projects.find((item) => item.project_id === run.project_id)
    : undefined;
  useEffect(() => {
    if (
      route.kind === "projectRun" &&
      runOwner &&
      route.projectId !== runOwner.id
    )
      onNavigate(
        projectRunPath(runOwner.id, route.runId, {
          imageId: route.imageId,
          nodeId: route.nodeId,
          artifactId: route.artifactId,
          view: route.view,
        }),
        true,
      );
  }, [route.kind, routeRunId, runOwner?.id]);
  if (detailRoute && routeRunId && run)
    return (
      <RunDetailWorkspace
        run={run}
        project={runOwner}
        route={detailRoute}
        onNavigate={onNavigate}
        onRefresh={onRefresh}
        onError={onError}
      />
    );
  const projectRuns = runsForContext(runs, scopeProject);
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
    const params = new URLSearchParams();
    if (status !== "all") params.set("status", status);
    onNavigate(
      projectId
        ? projectRunsPath(projectId, status)
        : `/runs${params.size ? `?${params.toString()}` : ""}`,
    );
  };
  return (
    <section className="page-stack">
      {scopeProject && <ProjectBreadcrumb
        project={scopeProject}
        current="Runs"
        onOpenProjects={() => onNavigate("/projects")}
        onOpenProject={() => onNavigate(`/projects/${encodeURIComponent(scopeProject.id)}`)}
      />}
      <div className="toolbar-panel"><div><span className="eyebrow">Immutable execution history</span><h2>Runs</h2><p>Open a Run to inspect its exact Pipeline Version, progress, image, node Artifacts, errors, usage, and Replay.</p></div></div>
      <Panel title="Run history" eyebrow={`${visibleExecutions.length} executions visible · ${runs.length} image Runs recorded`}>
        <div className="list-filters">
          <label>Project
            <select
              aria-label="Project filter"
              value={scopeProject?.id ?? ""}
              onChange={(event) => setListFilters(event.target.value, statusFilter)}
            >
              <option value="">All projects</option>
              {projects.map((project) => <option key={project.id} value={project.id}>{project.name}</option>)}
            </select>
          </label>
          <label>Status
            <select aria-label="Status filter" value={statusFilter} onChange={(event) => setListFilters(scopeProject?.id ?? "", event.target.value)}>
              <option value="all">All statuses</option>
              {availableStatuses.map((status) => <option key={status} value={status}>{status.replaceAll("_", " ")}</option>)}
            </select>
          </label>
        </div>
        <div className="runs-table">
          {visibleExecutions.map((execution) => execution.kind === "batch"
            ? <BatchRunGroup key={execution.batch.id} batch={execution.batch} runs={runs} project={projects.find((project) => project.id === execution.batch.project_id)} onNavigate={onNavigate} />
            : <RunHistoryRow key={execution.run.id} run={execution.run} projectId={projects.find((project) => project.project_id === execution.run.project_id)?.id} onNavigate={onNavigate} />)}
          {visibleExecutions.length === 0 && <Empty title="No matching runs" detail="Change the explicit Project or status filter to see more Run history." />}
        </div>
      </Panel>
      {routeRunId && !run && <Empty title="Run not found" detail="The linked Run is not available in this workspace." />}
    </section>
  );
}

function BatchRunGroup({
  batch,
  runs,
  project,
  onNavigate,
}: {
  batch: DatasetBatchSummary;
  runs: HistoryRun[];
  project?: ProjectSummary;
  onNavigate: (path: string) => void;
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
  return <details className="batch-run-group">
    <summary className="batch-run-row">
      <span className="event-rail" />
      <div><strong>{project?.name ?? batch.project_id}</strong><small>Dataset Run · {workflowName}@v{workflowVersion}</small><code>{batch.progress.completed_images}/{batch.progress.total_images} images completed · Batch {batch.id.slice(0, 8)}</code></div>
      <div className="run-usage"><span>{usage.total_tokens.toLocaleString()} tokens</span><span>${usage.cost}</span></div>
      <Status status={batch.status} />
      <span className="row-arrow" aria-hidden="true">⌄</span>
    </summary>
    <div className="batch-run-children">
      <div className="batch-run-explanation"><strong>One Dataset Run</strong><span>AnnotAgent created {batch.progress.total_images} image Runs so each image keeps its own Artifacts, errors, Replay and Review history.</span></div>
      <button className="text-button" onClick={() => onNavigate(projectBatchPath(batch.project_id, batch.id))}>Open Dataset Run detail →</button>
      {childRuns.map((run, index) => <RunHistoryRow key={run.id} run={run} projectId={project?.id} onNavigate={onNavigate} childLabel={`Image ${index + 1} of ${batch.progress.total_images}`} />)}
      {childRuns.length === 0 && <Empty title="No image Runs recorded" detail="This Dataset Run stopped before an image Run was created." />}
    </div>
  </details>;
}

function BatchDetailWorkspace({
  route,
  runs,
  projects,
  onNavigate,
  onRefresh,
  onError,
}: {
  route: Extract<WorkspaceRoute, { kind: "projectBatch" }>;
  runs: HistoryRun[];
  projects: ProjectSummary[];
  onNavigate: (path: string, replace?: boolean) => void;
  onRefresh: () => Promise<void>;
  onError: (value: string) => void;
}) {
  const [batches, setBatches] = useState<DatasetBatchSummary[]>();
  const [busy, setBusy] = useState(false);
  const load = () =>
    api
      .batches()
      .then((value) => setBatches(value.batches));
  useEffect(() => {
    setBatches(undefined);
    void load().catch((error: Error) => onError(error.message));
  }, [route.batchId, runs.length, runs[0]?.updated_at]);
  const batch = batches?.find((candidate) => candidate.id === route.batchId);
  const owner = batch
    ? projects.find((project) => project.id === batch.project_id)
    : projects.find((project) => project.id === route.projectId);
  useEffect(() => {
    if (batch && batch.project_id !== route.projectId)
      onNavigate(projectBatchPath(batch.project_id, batch.id), true);
  }, [batch?.id, batch?.project_id, route.projectId]);
  if (!batches)
    return <div className="loading-banner" role="status">Loading Dataset Run…</div>;
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
          title="Dataset Run not found"
          detail="The linked Batch is not available in this workspace."
        />
      </section>
    );
  if (batch.project_id !== route.projectId)
    return <div className="loading-banner" role="status">Opening the owning Project…</div>;
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
      >
        ← Run history
      </button>
      <div className="toolbar-panel run-detail-header">
        <div>
          <span className="eyebrow">Dataset Run · {batch.id.slice(0, 8)}</span>
          <h2>{workflowName}@v{workflowVersion}</h2>
          <div className="context-line">
            <Status status={batch.status} />
            <span>{progress.completed_images}/{progress.total_images} images completed</span>
            <span>{batch.max_concurrency} concurrent</span>
          </div>
        </div>
        <div className="button-row">
          {batch.status === "running" && <button disabled={busy} onClick={() => control("pause")}>Pause</button>}
          {batch.status === "paused" && <button disabled={busy} onClick={() => control("resume")}>Resume</button>}
          {(batch.status === "running" || batch.status === "paused" || batch.status === "pending") && <button className="danger" disabled={busy} onClick={() => control("cancel")}>Cancel</button>}
        </div>
      </div>
      <dl className="run-result-metrics" aria-label="Dataset Run progress">
        <div><dt>Total</dt><dd>{progress.total_images}</dd><small>images</small></div>
        <div><dt>Completed</dt><dd>{progress.completed_images}</dd><small>ready</small></div>
        <div><dt>Running</dt><dd>{progress.running_images}</dd><small>in progress</small></div>
        <div><dt>Review</dt><dd>{progress.review_images}</dd><small>needs attention</small></div>
        <div><dt>Failed</dt><dd>{progress.failed_images}</dd><small>images</small></div>
        <div><dt>Pending</dt><dd>{progress.pending_images}</dd><small>queued</small></div>
      </dl>
      <Panel title="Image Runs" eyebrow={`${childRuns.length} of ${progress.total_images} created`}>
        <div className="runs-table">
          {childRuns.map((run, index) => (
            <RunHistoryRow
              key={run.id}
              run={run}
              projectId={owner?.id}
              onNavigate={onNavigate}
              childLabel={`Image ${index + 1} of ${progress.total_images}`}
            />
          ))}
          {childRuns.length === 0 && (
            <Empty
              title="No image Runs recorded"
              detail="This Dataset Run has not created an image Run yet."
            />
          )}
        </div>
      </Panel>
    </section>
  );
}

function RunHistoryRow({
  run,
  projectId,
  onNavigate,
  childLabel,
}: {
  run: HistoryRun;
  projectId?: string;
  onNavigate: (path: string) => void;
  childLabel?: string;
}) {
  return <button className={`run-row${childLabel ? " batch-child-run" : ""}`} onClick={() => onNavigate(projectId ? projectRunPath(projectId, run.id) : `/runs/${encodeURIComponent(run.id)}`)}>
    <span className="event-rail" />
    <div><strong>{childLabel ?? run.project_name}</strong><small>{run.workflow_name}@v{run.workflow_version}</small><code>{run.model_identity} · {run.artifact_count} Artifacts</code>{run.terminal_reason && <small className="run-reason">{run.terminal_reason}</small>}</div>
    <div className="run-usage"><span>{(run.input_tokens + run.output_tokens).toLocaleString()} tokens</span><span>${run.cost}</span></div>
    <Status status={run.status} />
    <span className="row-arrow" aria-hidden="true">→</span>
  </button>;
}

function RunDetailWorkspace({
  run,
  project,
  route,
  onNavigate,
  onRefresh,
  onError,
}: {
  run: HistoryRun;
  project?: ProjectSummary;
  route: Extract<WorkspaceRoute, { kind: "runs" | "projectRun" }>;
  onNavigate: (path: string, replace?: boolean) => void;
  onRefresh: () => Promise<void>;
  onError: (value: string) => void;
}) {
  const view = route.view ?? "results";
  const [inspection, setInspection] = useState<RunNodeArtifactInspection>();
  const [annotationInspection, setAnnotationInspection] = useState<RunAnnotationInspection>();
  const [resultSummary, setResultSummary] = useState<RunResultSummary>();
  const [debugSummary, setDebugSummary] = useState<RunDebugSummary>();
  const [replay, setReplay] = useState<NodeReplayReport>();
  const [images, setImages] = useState<ImageItem[]>([]);
  const [runReview, setRunReview] = useState<ReviewItem>();
  const [search, setSearch] = useState("");
  const [busy, setBusy] = useState(false);
  const runPath = (context: {
    imageId?: string;
    nodeId?: string;
    artifactId?: string;
    view?: "results" | "debug";
  } = {}) =>
    project
      ? projectRunPath(project.id, run.id, context)
      : `/runs/${encodeURIComponent(run.id)}${(() => {
          const params = new URLSearchParams();
          if (context.view === "debug" || context.nodeId || context.artifactId)
            params.set("view", "debug");
          if (context.imageId) params.set("image", context.imageId);
          if (context.nodeId) params.set("node", context.nodeId);
          if (context.artifactId) params.set("artifact", context.artifactId);
          return params.size ? `?${params.toString()}` : "";
        })()}`;
  useEffect(() => {
    setInspection(undefined);
    setAnnotationInspection(undefined);
    setResultSummary(undefined);
    setDebugSummary(undefined);
    setReplay(undefined);
    if (run.checkpoint_present)
      void api.pipelineArtifacts(run.id).then(setInspection).catch((error: Error) => onError(error.message));
    void api.runResultSummary(run.id).then(setResultSummary).catch((error: Error) => onError(error.message));
    void api.runAnnotations(run.id).then(setAnnotationInspection).catch((error: Error) => onError(error.message));
    if (project)
      void api.images(project.id).then((value) => setImages(value.images)).catch((error: Error) => onError(error.message));
    void api.reviews().then((value) => setRunReview(
      value.reviews.find(
        (review) => review.run_id === run.id && review.annotation.value.kind === "bounding_box",
      ) ?? value.reviews.find((review) => review.run_id === run.id),
    )).catch((error: Error) => onError(error.message));
  }, [run.id, project?.id]);
  useEffect(() => {
    if (view !== "debug") return;
    void api.runDebugSummary(run.id).then(setDebugSummary).catch((error: Error) => onError(error.message));
    if (!route.nodeId && inspection?.nodes[0]) {
      const imageIndex = inspection.image_index ?? annotationInspection?.image_index;
      onNavigate(
        runPath({
          view: "debug",
          imageId: imageIndex === undefined ? undefined : String(imageIndex),
          nodeId: inspection.nodes[0].node_id,
        }),
        true,
      );
    }
  }, [view, run.id, route.nodeId, inspection, annotationInspection?.image_index]);
  const selectedNode = inspection?.nodes.find((node) => node.node_id === route.nodeId) ?? inspection?.nodes[0];
  const selectedArtifacts = selectedNode
    ? selectedNode.outputs.filter(
        (artifact, index) =>
          !route.artifactId || pipelineArtifactIdentity(artifact, index) === route.artifactId,
      )
    : [];
  const runImageIndex = inspection?.image_index ?? annotationInspection?.image_index;
  const selectedImageIndex = Number(route.imageId ?? runImageIndex ?? 0);
  const visibleImages = images.filter((image) => image.name.toLowerCase().includes(search.toLowerCase()));
  const runAnnotations = annotationInspection?.annotations ?? [];
  const selectedPreviewArtifacts = selectedNode
    ? [...selectedNode.inputs, ...(selectedArtifacts.length ? selectedArtifacts : selectedNode.outputs)]
    : [];
  const resultArtifacts = inspection?.nodes.flatMap((node) => node.outputs) ?? [];
  const previewArtifacts = view === "results" ? resultArtifacts : selectedPreviewArtifacts;
  const previewProjectId = inspection?.project_id ?? annotationInspection?.project_id ?? project?.id;
  const canPreview = Boolean(
    previewProjectId && runImageIndex !== undefined && (inspection || runAnnotations.length),
  );
  const setContext = (context: { image?: number; node?: string; artifact?: string }) => {
    onNavigate(
      runPath({
        view: "debug",
        imageId: String(context.image ?? selectedImageIndex),
        nodeId: context.node ?? selectedNode?.node_id,
        artifactId: context.artifact,
      }),
    );
  };
  const setView = (next: "results" | "debug") => {
    onNavigate(
      runPath({
        view: next,
        imageId:
          runImageIndex === undefined ? undefined : String(selectedImageIndex),
        nodeId: next === "debug" ? selectedNode?.node_id : undefined,
      }),
    );
  };
  const control = (action: "pause" | "resume" | "cancel") => {
    setBusy(true);
    void api.control(run.id, action).then(onRefresh).catch((error: Error) => onError(error.message)).finally(() => setBusy(false));
  };
  const replayNode = () => {
    if (!selectedNode) return;
    setBusy(true);
    void api.replayNode(run.id, selectedNode.node_id).then((value) => { setReplay(value); setInspection(value.inspection); }).catch((error: Error) => onError(error.message)).finally(() => setBusy(false));
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
  const setResultImage = (image: number) =>
    onNavigate(runPath({ imageId: String(image) }));
  const resultHeadline = run.status === "running"
    ? "Run in progress"
    : run.status === "paused"
      ? "Run paused"
      : run.status === "completed" || run.status === "completed_with_review"
        ? "Run completed"
        : run.status === "awaiting_review"
          ? "Results need review"
          : `Run ${run.status.replaceAll("_", " ")}`;
  return (
    <section className="page-stack run-detail-page">
      <ProjectBreadcrumb
        project={project}
        current={`Run ${run.id.slice(0, 8)}`}
        onOpenProjects={() => onNavigate("/projects")}
        onOpenProject={project ? () => onNavigate(`/projects/${encodeURIComponent(project.id)}`) : undefined}
      />
      <button className="text-button run-back" onClick={() => onNavigate(project ? projectRunsPath(project.id) : "/runs")}>← Run history</button>
      <nav className="run-view-tabs" aria-label="Run workspace view">
        <button className={view === "results" ? "active" : ""} aria-current={view === "results" ? "page" : undefined} onClick={() => setView("results")}>Results</button>
        <button className={view === "debug" ? "active" : ""} aria-current={view === "debug" ? "page" : undefined} onClick={() => setView("debug")}>Debug</button>
      </nav>
      <div className="toolbar-panel run-detail-header">
        {view === "results" ? <div><span className="eyebrow">{run.project_name} · {run.workflow_name}@v{run.workflow_version}</span><h2>{resultHeadline}</h2><div className="context-line"><Status status={run.status} /><span>{formatSampleDuration(resultSummary?.duration_ms ?? duration)}</span><span>${resultSummary?.usage.estimated_cost ?? run.cost}</span></div></div> : <div><span className="eyebrow">Debug · Run {run.id.slice(0, 8)}</span><h2>{run.workflow_name}@v{run.workflow_version}</h2><div className="context-line"><Status status={run.status} /><span>{nodeProgress}</span><span>{run.artifact_count} Artifacts</span><span>{(run.input_tokens + run.output_tokens).toLocaleString()} tokens</span><span>${run.cost}</span></div></div>}
        <div className="button-row">
          {project && <button onClick={() => onNavigate(`/projects/${encodeURIComponent(project.id)}/build/pipeline`)}>Improve automation</button>}
          {runReview && <button onClick={() => onNavigate(project ? projectReviewPath(project.id, runReview.id) : `/review/${encodeURIComponent(runReview.id)}`)}>Review {resultSummary?.needs_review_count || 1} result</button>}
          {!runReview && Boolean(resultSummary?.needs_review_count) && project && <button onClick={() => onNavigate(projectReviewPath(project.id))}>Open Review inbox</button>}
          {run.status === "running" && <button disabled={busy} onClick={() => control("pause")}>Pause</button>}
          {run.status === "paused" && <button disabled={busy} onClick={() => control("resume")}>Resume</button>}
          {run.controllable && <button className="danger" disabled={busy} onClick={() => control("cancel")}>Cancel</button>}
        </div>
      </div>
      {view === "results" ? <>
        <dl className="run-result-metrics" aria-label="Run result summary">
          <div><dt>Images</dt><dd>{resultSummary?.image_count ?? 1}</dd><small>processed</small></div>
          <div><dt>Accepted</dt><dd>{resultSummary?.ready_count ?? 0}</dd><small>{resultSummary?.result_count ?? runAnnotations.length} detections</small></div>
          <div><dt>Needs review</dt><dd>{resultSummary?.needs_review_count ?? (runReview ? 1 : 0)}</dd><small>human decision</small></div>
          <div><dt>Fallbacks</dt><dd>{resultSummary?.fallback_count ?? run.fallback_nodes.length}</dd><small>open-vocabulary</small></div>
          <div><dt>Cache hits</dt><dd>{resultSummary?.cache_hit_count ?? 0}</dd><small>model calls reused</small></div>
          <div><dt>Failed</dt><dd>{resultSummary?.failed_count ?? 0}</dd><small>{resultSummary?.no_target_count ?? 0} no-target</small></div>
        </dl>
        <div className="run-results-workspace">
          <aside className="panel run-image-browser"><span className="eyebrow">Images</span><input aria-label="Search run images" value={search} onChange={(event) => setSearch(event.target.value)} placeholder="Search images" /><select aria-label="Image status filter" defaultValue="all"><option value="all">All statuses</option><option value={run.status}>{run.status.replaceAll("_", " ")}</option></select>
            <div>{visibleImages.filter((image) => runImageIndex === undefined || image.index === runImageIndex).map((image) => <button key={image.index} className={image.index === selectedImageIndex ? "active" : ""} onClick={() => setResultImage(image.index)}><img src={image.url} alt="" /><span><strong>{image.name}</strong><small>{resultSummary?.failed_count ? "Failed" : resultSummary?.no_target_count ? "No target found" : resultSummary?.needs_review_count ? "Needs review" : "Ready"}</small></span></button>)}</div>
          </aside>
          <main className="panel run-visual-workspace run-result-preview"><span className="eyebrow">Result Preview</span>{resultSummary?.labels.length ? <div className="run-result-labels" aria-label="Result labels">{resultSummary.labels.map((item) => <span key={item.label}>{item.label}<b>{item.count}</b></span>)}</div> : null}{canPreview && (resultSummary?.result_count ?? runAnnotations.length) > 0 ? <RunArtifactCanvas projectId={previewProjectId!} project={project} artifacts={previewArtifacts} annotations={runAnnotations} imageIndex={selectedImageIndex} /> : resultSummary ? <Empty title={resultSummary.no_target_count ? "No target found" : resultSummary.failed_count ? "No result produced" : "No visual result"} detail={resultSummary.no_target_count ? "The automation completed successfully and found no matching target in this image." : resultSummary.failed_count ? "Open Debug to inspect the failed step and available repair action." : "This result has no bounding-box or Crop preview."} /> : <Empty title="Loading results" detail="Reading persisted Annotations and result Artifacts." />}</main>
          <aside className="panel run-needs-attention"><span className="eyebrow">Needs Attention</span>{runReview ? <><h3>{resultSummary?.needs_review_count || 1} result needs a decision</h3><p>{runReview.review_reason.replaceAll("_", " ")}</p><button className="primary" onClick={() => onNavigate(project ? projectReviewPath(project.id, runReview.id) : `/review/${encodeURIComponent(runReview.id)}`)}>Review result</button></> : resultSummary?.needs_review_count && project ? <><h3>{resultSummary.needs_review_count} result needs a decision</h3><p>Open the Project Review inbox to inspect the uncertain result.</p><button className="primary" onClick={() => onNavigate(projectReviewPath(project.id))}>Open Review inbox</button></> : resultSummary?.failed_count ? <><h3>Run needs repair</h3><p>{run.terminal_reason ?? "A Pipeline step did not produce a usable result."}</p><button className="primary" onClick={() => setView("debug")}>Open Debug</button></> : <div className="positive-empty"><strong>No results need attention</strong><span>{resultSummary?.no_target_count ? "The empty result is valid." : "All results passed the configured gates."}</span></div>}</aside>
        </div>
      </> : <>
        <div className="debug-summary-strip" aria-label="Run debug summary"><span>{debugSummary?.succeeded_node_count ?? completedNodes ?? 0}/{debugSummary?.node_count ?? inspection?.nodes.length ?? 0} steps complete</span><span>{debugSummary?.failed_node_count ?? 0} failed</span><span>{debugSummary?.issues.length ?? 0} issues</span><span>{formatSampleDuration(debugSummary?.duration_ms ?? duration)}</span></div>
        <div className="run-workspace">
          <aside className="panel run-image-browser"><span className="eyebrow">Images</span><input aria-label="Search run images" value={search} onChange={(event) => setSearch(event.target.value)} placeholder="Search images" /><select aria-label="Image status filter" defaultValue="all"><option value="all">All statuses</option><option value={run.status}>{run.status}</option></select>
            <div>{visibleImages.filter((image) => runImageIndex === undefined || image.index === runImageIndex).map((image) => <button key={image.index} className={image.index === selectedImageIndex ? "active" : ""} onClick={() => setContext({ image: image.index })}><img src={image.url} alt="" /><span><strong>{image.name}</strong><small>{run.status}</small></span></button>)}</div>
          </aside>
          <main className="panel run-visual-workspace"><span className="eyebrow">Artifact Preview</span>{canPreview ? <RunArtifactCanvas projectId={previewProjectId!} project={project} artifacts={previewArtifacts} annotations={runAnnotations} imageIndex={selectedImageIndex} /> : <Empty title="No visual Artifact" detail={run.checkpoint_present ? "Loading the persisted checkpoint and annotations." : "This Run has no visual Artifact to preview."} />}</main>
          <aside className="panel run-node-timeline"><span className="eyebrow">Pipeline Steps</span>{inspection?.nodes.map((node, index) => <button key={node.node_id} className={node.node_id === selectedNode?.node_id ? "active" : ""} onClick={() => setContext({ node: node.node_id })}><span>{index + 1}</span><span><strong title={node.operation}>{node.operation}</strong><small title={`${node.status} · ${node.latency_ms} ms`}>{node.status} · {node.latency_ms} ms</small></span>{node.error && <i title={node.error.summary}>!</i>}</button>)}{!inspection && <small>No node trace available.</small>}</aside>
        </div>
      {selectedNode && (
        <section className="panel run-node-inspector" aria-label="Node inspector">
          <header className="run-node-inspector-header">
            <div>
              <span className="eyebrow">Node inspector</span>
              <h2>{selectedNode.operation}</h2>
              <code>Node ID · {selectedNode.node_id}</code>
            </div>
            <button disabled={busy} onClick={replayNode}>Replay from this node</button>
          </header>
          <div className="run-node-metrics" aria-label="Node execution summary">
            <article><span>Status</span><Status status={selectedNode.status} /></article>
            <article><span>Duration</span><strong>{selectedNode.latency_ms.toLocaleString()} ms</strong></article>
            <article><span>Model usage</span><strong>{selectedNode.usage.input_tokens + selectedNode.usage.output_tokens} tokens</strong><small>${selectedNode.usage.cost}</small></article>
          </div>
          <section className="run-node-artifacts" aria-labelledby="node-output-artifacts">
            <header><div><span className="eyebrow">Artifacts</span><h3 id="node-output-artifacts">Node outputs</h3></div><b>{selectedNode.outputs.length}</b></header>
            <div className="artifact-choice" aria-label="Node output Artifacts">{selectedNode.outputs.map((artifact, index) => { const id = pipelineArtifactIdentity(artifact, index); return <button key={id} className={route.artifactId === id ? "active" : ""} onClick={() => setContext({ artifact: id })}><span>{artifact.kind.replaceAll("_", " ")}</span><code>{id.slice(0, 8)}</code></button>; })}</div>
            {selectedNode.outputs.length === 0 && <p className="node-payload-empty">This node did not produce an Artifact.</p>}
          </section>
          <EvidenceDecisionCard metadata={selectedNode.metadata ?? {}} route={selectedNode.route} />
          {selectedNode.error && <div className="run-repair-card"><div><strong>{selectedNode.error.code}</strong><p>{selectedNode.error.summary}</p></div><div className="button-row">{selectedNode.error.retryable && <button className="primary" disabled={busy} onClick={replayNode}>Replay failed step</button>}{project && <button onClick={() => onNavigate(`/projects/${encodeURIComponent(project.id)}/build/pipeline`)}>Fix automation</button>}</div></div>}
          <div className="node-payload-sections">
            {selectedNode.metadata?.model_asset != null && <NodePayloadSection title="Model identity" description="Immutable Plugin, Bundle, files, Contract, Model Instance, revision, and execution provider frozen by this Workflow Version" badge="Frozen" value={selectedNode.metadata.model_asset} open />}
            <NodePayloadSection title="Input" description="Artifacts received from upstream nodes" badge={selectedNode.inputs.length} value={selectedNode.inputs} />
            <NodePayloadSection title="Output" description="Artifacts emitted by this node" badge={selectedNode.outputs.length} value={selectedNode.outputs} open />
            <NodePayloadSection title="Configuration" description="Resolved runtime configuration" badge="JSON" value={selectedNode.configuration} />
            <NodePayloadSection title="Provider request" description="Recorded provider context; credentials and image bytes are redacted" badge={run.provider} value={{ provider: run.provider, model: run.model, operation: selectedNode.operation, parameters: selectedNode.configuration.parameters }} />
            {selectedNode.error && <NodePayloadSection title="Raw error" description="Structured Runtime failure" badge={selectedNode.error.code} value={selectedNode.error} />}
          </div>
          {replay?.replayed_from === selectedNode.node_id && <div className="validation-report valid"><strong>Sandbox Replay completed</strong><small>Preserved upstream: {replay.preserved_upstream_nodes.join(", ") || "None"}</small><small>Re-executed: {replay.reexecuted_nodes.join(", ")}</small></div>}
        </section>
      )}
      </>}
    </section>
  );
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
    <section className={`evidence-decision-card decision-${report.decision}`} aria-label="Evidence decision">
      <header>
        <div><span className="eyebrow">Evidence decision</span><h3>{report.decision}</h3></div>
        <span>{report.candidate_count} candidate{report.candidate_count === 1 ? "" : "s"}</span>
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
          ? <p className="node-payload-empty">No {title.toLowerCase()} data recorded.</p>
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

function sourceModelLabel(modelId: string): string {
  if (modelId.toLowerCase().includes("rfdetr")) return "RF-DETR";
  if (modelId.toLowerCase().includes("locate")) return "LocateAnything";
  return modelId;
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
    ? `${sourceModelLabel(source.source_model_id)} · score not provided`
    : `${sourceModelLabel(source.source_model_id)} · ${scoreSemanticsLabel(source.score.semantics)} ${source.score.value.toFixed(2)}`;
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

function RunArtifactCanvas({ projectId, project, artifacts, annotations, imageIndex }: { projectId: string; project?: ProjectSummary; artifacts: PipelineArtifact[]; annotations: Annotation[]; imageIndex: number }) {
  const imageUrl = `/api/projects/${projectId}/images/${imageIndex}/content`;
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
    <div className="run-artifact-canvas" role="region" aria-label="Run result annotation viewer" onKeyDown={(event) => { if (event.target instanceof HTMLInputElement && event.target.type === "range") return; if (event.key === "ArrowRight" || event.key === "ArrowDown") { event.preventDefault(); selectOffset(1); } if (event.key === "ArrowLeft" || event.key === "ArrowUp") { event.preventDefault(); selectOffset(-1); } }}>
      <div className="preview-toggle">
        <button className={mode === "original" ? "active" : ""} onClick={() => setMode("original")}>Original</button>
        <button className={mode === "result" ? "active" : ""} onClick={() => setMode("result")}>Result</button>
        <button className={mode === "compare" ? "active" : ""} onClick={() => setMode("compare")}>Compare</button>
        <button className={mode === "crops" ? "active" : ""} disabled={!crops.length} onClick={() => setMode("crops")}>Crop ({crops.length})</button>
        <label className="preview-zoom-control">
          <span>Zoom</span>
          <input aria-label="Preview zoom" type="range" min="1" max="3" step="0.25" value={zoom} onChange={(event) => setZoom(Number(event.target.value))} />
          <output aria-live="polite">{Math.round(zoom * 100)}%</output>
        </label>
      </div>
      {(legend.length > 0 || masks.length > 0) && <div className="bbox-legend" aria-label="Annotation color legend">{legend.map((item) => <span key={item.label}><i style={{ background: item.color }} />{item.label}</span>)}{masks.length > 0 && <span><i className="mask-overlay-swatch" />Mask overlay · {masks.length}</span>}</div>}
      {detections.length > 0 && <ul className="canvas-annotation-list" aria-label="Run result annotations">{detections.map((item) => <li key={item.id}><button aria-pressed={item.id === selectedId} onClick={() => setSelectedId(item.id)}><i aria-hidden="true" style={{ borderColor: item.color }} /><span><strong>{item.label}</strong><small>{artifactMarkSummary(item)}</small></span></button></li>)}</ul>}
      {selectedMark && <section className="geometry-quality-facts" aria-label="Semantic and geometry quality">
        <article><span>{scoreSemanticsLabel(selectedMark.scoreSemantics)}</span><strong>{selectedMark.confidence === undefined ? "Not provided" : selectedMark.confidence.toFixed(2)}</strong><small>This score describes model belief, not box geometry.</small></article>
        <article><span>Box quality</span><strong>{geometrySemanticsLabel(selectedMark.geometrySemantics)}</strong><small>A model score is not box IoU or tightness.</small></article>
        <article className={selectedMark.calibrationStatus === "passed" || selectedMark.geometrySemantics === "human_verified" ? "verified" : "needs-check"}><span>Geometry verification</span><strong>{selectedMark.geometrySemantics === "human_verified" ? "Verified by a reviewer" : selectedMark.calibrationStatus === "passed" ? "Project calibration passed" : "Not performed"}</strong><small>{selectedMark.geometryReportId ? `Quality report ${selectedMark.geometryReportId.slice(0, 8)}` : `${(selectedMark.calibrationStatus ?? "uncalibrated").replaceAll("_", " ")} · review or measured evidence required`}</small></article>
        {selectedMark.geometryIssues?.length ? <article className="needs-check"><span>Geometry issues</span><strong>{selectedMark.geometryIssues.map((issue) => issue.replaceAll("_", " ")).join(", ")}</strong></article> : null}
      </section>}
      {selectedMark?.evidence.length ? <section className="evidence-inspector" aria-label="Detection evidence inspector">
        <header><span className="eyebrow">Evidence inspector</span><strong>{artifactMarkSummary(selectedMark)}</strong></header>
        <div>{uniqueEvidence(selectedMark.evidence).map((item) => <article key={evidenceIdentity(item)}>
          <span><strong>{sourceModelLabel(item.source_model_id)}</strong><small>{item.source_capability.replaceAll("_", " ")}</small></span>
          <span><strong>{item.score.value == null ? "Score not provided" : item.score.value.toFixed(2)}</strong><small>{scoreSemanticsLabel(item.score.semantics)}</small></span>
          <code>[{item.bbox.map((value) => value.toFixed(3)).join(", ")}]</code>
          {(item.query_id || item.model_label) && <small>{item.query_id ? `Query · ${item.query_id}` : ""}{item.query_id && item.model_label ? " · " : ""}{item.model_label ? `Model label · ${item.model_label}` : ""}</small>}
        </article>)}</div>
      </section> : null}
      {mode === "original" ? imageStage(false, "Original Run input") : mode === "result" ? imageStage(true, "Run result") : mode === "compare" ? <div className="run-result-compare"><section><span>Original</span>{imageStage(false, "Original Run input")}</section><section><span>Result</span>{imageStage(true, "Run result")}</section></div> : (
        <div className="crop-preview-list enlarged">{crops.map((crop, index) => <button className={crop.parentId === selectedId ? "selected" : ""} key={crop.id} onClick={() => setSelectedId(crop.parentId ?? crop.id)}><svg style={{ transform: `scale(${zoom})` }} viewBox={`${crop.x * 100} ${crop.y * 100} ${crop.width * 100} ${crop.height * 100}`} aria-label={`Crop ${index + 1}: ${crop.label}`}><image href={imageUrl} x="0" y="0" width="100" height="100" /></svg><span><strong>{crop.label}</strong>{crop.confidence !== undefined && <small>{Math.round(crop.confidence * 100)}%</small>}<small>Parent: {crop.parentArtifact?.slice(0, 8) ?? crop.parentId ?? "Unknown"}</small><small>Source: {crop.sourceNode ?? "Unknown"}</small></span></button>)}</div>
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
  events,
  route,
  onNavigate,
  onError,
}: {
  project?: ProjectSummary;
  projects: ProjectSummary[];
  events: RunEvent[];
  route: Extract<WorkspaceRoute, { kind: "review" | "projectReview" }>;
  onNavigate: (path: string, replace?: boolean) => void;
  onError: (value: string) => void;
}) {
  const [reviews, setReviews] = useState<ReviewItem[]>([]);
  const [queueLoaded, setQueueLoaded] = useState(false);
  const [progress, setProgress] = useState<ReviewQueueProgress>({
    reviewed_count: 0,
    total_count: 0,
    remaining_count: 0,
  });
  const [queueNavigation, setQueueNavigation] = useState<ReviewNavigation>();
  const [selectedId, setSelectedId] = useState(route.reviewItemId ?? "");
  const [draft, setDraft] = useState<Annotation>();
  const [past, setPast] = useState<Annotation[]>([]);
  const [future, setFuture] = useState<Annotation[]>([]);
  const [isNew, setIsNew] = useState(false);
  const [editing, setEditing] = useState(false);
  const [decisionBusy, setDecisionBusy] = useState(false);
  const [rejectOpen, setRejectOpen] = useState(false);
  const [rejectReason, setRejectReason] = useState("wrong_object");
  const [completedProject, setCompletedProject] = useState<ProjectSummary>();
  const [compareMode, setCompareMode] = useState<"after" | "before" | "split">("after");
  const [inspectorCollapsed, setInspectorCollapsed] = useState(() =>
    window.localStorage.getItem("annotagent.reviewInspectorCollapsed") === "true",
  );
  const [attributesText, setAttributesText] = useState("{}");
  const [reason, setReason] = useState("");
  const [skillReasonOptions, setSkillReasonOptions] = useState<
    { value: string; label: string; skillId: string }[]
  >([]);
  const [correctionSkillId, setCorrectionSkillId] = useState("");
  const [note, setNote] = useState("");
  const [images, setImages] = useState<ImageItem[]>([]);
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
  const selected =
    routeReview &&
    (!scopedProject || routeReview.project_id === scopedProject.project_id)
      ? routeReview
      : visibleReviews.find((review) => review.id === selectedId) ??
        visibleReviews[0];
  const reviewProject = projectForReview(projects, selected) ?? scopedProject;
  const reviewHref = (reviewId: string, item?: ReviewItem) => {
    const owner = projectForReview(
      projects,
      item ?? reviews.find((review) => review.id === reviewId),
    );
    if (owner) return projectReviewPath(owner.id, reviewId);
    if (route.projectId) return projectReviewPath(route.projectId, reviewId);
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
        projectReviewPath(routeReviewProject.id, route.reviewItemId),
        true,
      );
  }, [route.kind, route.reviewItemId, route.projectId, routeReviewProject?.id]);
  const refresh = () => {
    setQueueLoaded(false);
    return api
      .reviews(route.projectId)
      .then((value) => {
        setReviews(value.reviews);
        setProgress(value.progress);
        const first = value.reviews[0];
        if (
          route.reviewItemId &&
          !value.reviews.some((review) => review.id === selectedId) &&
          first
        )
          onNavigate(reviewHref(first.id, first), true);
      })
      .catch((error: Error) => onError(error.message))
      .finally(() => setQueueLoaded(true));
  };
  useEffect(() => {
    if (!route.reviewItemId) return;
    void api
      .review(route.reviewItemId)
      .then((review) =>
        setReviews((items) =>
          items.some((item) => item.id === review.id) ? items : [review, ...items],
        ),
      )
      .catch((error: Error) => onError(error.message));
  }, [route.reviewItemId]);
  useEffect(() => {
    void refresh();
  }, [route.projectId]);
  useEffect(() => {
    if (!selected?.id) {
      setQueueNavigation(undefined);
      return;
    }
    void api
      .reviewNext(selected.id, route.projectId)
      .then((value) => {
        setQueueNavigation(value);
        setProgress(value.progress);
      })
      .catch((error: Error) => onError(error.message));
  }, [selected?.id, route.projectId]);
  useEffect(() => {
    void api
      .skills()
      .then((skills) => {
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
      .catch((error: Error) => onError(error.message));
  }, [reviewProject?.id, selected?.source_skill_id]);
  useEffect(() => {
    if (reviewProject)
      void api
        .images(reviewProject.id)
        .then((value) => setImages(value.images))
        .catch((error: Error) => onError(error.message));
    else setImages([]);
  }, [reviewProject?.id]);
  useEffect(() => {
    setDraft(selected?.annotation);
    setAttributesText(JSON.stringify(selected?.annotation.attributes ?? {}, null, 2));
    setPast([]);
    setFuture([]);
    setIsNew(false);
    setEditing(false);
    setRejectOpen(false);
  }, [selected?.id]);
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
      confidence: evidence.score.value ?? undefined,
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
    if (!note.trim()) setNote(`Used the ${sourceModelLabel(evidence.source_model_id)} source box.`);
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
      if (isNew) {
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
  const save = () => void persistDraft();
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
      provenance: {},
      created_at: new Date().toISOString(),
    };
    if (draft) setPast((items) => [...items, structuredClone(draft)]);
    setFuture([]);
    setDraft(annotation);
    setAttributesText("{}");
    setIsNew(true);
  };
  const hasUnsavedAnnotationChanges = Boolean(
    isNew ||
    (draft && selected && (
      JSON.stringify(draft) !== JSON.stringify(selected.annotation) ||
      attributesText !== JSON.stringify(selected.annotation.attributes ?? {}, null, 2)
    )),
  );
  const moveQueueSelection = (item?: ReviewItem) => {
    if (!item) return;
    setSelectedId(item.id);
    onNavigate(reviewHref(item.id, item));
  };
  const decideAndAdvance = async (
    decision: "accept" | "reject",
    reasonCode: string,
  ) => {
    if (!selected || !reviewProject) {
      onError("Select the Review item's Project before recording a decision.");
      return;
    }
    if (isNew) {
      onError("Create the new annotation before deciding the original result.");
      return;
    }
    setDecisionBusy(true);
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
      const queue = await api.reviews(route.projectId);
      setReviews(queue.reviews);
      setProgress(queue.progress);
      if (outcome.next_review) {
        setSelectedId(outcome.next_review.id);
        onNavigate(reviewHref(outcome.next_review.id, outcome.next_review), true);
      } else {
        setSelectedId("");
        onNavigate(projectReviewPath(reviewProject.id), true);
      }
    } catch (error) {
      onError((error as Error).message);
    } finally {
      setDecisionBusy(false);
    }
  };
  useEffect(() => {
    const keyboard = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
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
        setInspectorVisibility(false);
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
    <section className={`review-layout${inspectorCollapsed ? " inspector-collapsed" : ""}`}>
      <aside className="review-queue panel">
        <span className="eyebrow">Human attention</span>
        <h2>
          Review queue <b>{queueLoaded ? progress.remaining_count : "…"}</b>
        </h2>
        <label className="review-project-filter">
          Project
          <select
            aria-label="Project filter"
            value={scopedProject?.id ?? ""}
            onChange={(event) =>
              onNavigate(event.target.value
                ? projectReviewPath(event.target.value)
                : "/review")
            }
          >
            <option value="">All projects</option>
            {projects.map((candidate) => (
              <option key={candidate.id} value={candidate.id}>{candidate.name}</option>
            ))}
          </select>
        </label>
        <div className="queue-items" aria-label="Annotations requiring review">
          {visibleReviews.map((review) => (
            <button
              key={review.id}
              aria-pressed={selected?.id === review.id}
              className={selected?.id === review.id ? "active" : ""}
              onClick={() => {
                setSelectedId(review.id);
                onNavigate(reviewHref(review.id, review));
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
                  {!project && `${review.project_name} · `}
                  Image {review.image_index === undefined ? "?" : review.image_index + 1} ·{" "}
                  {review.annotation.confidence === undefined ? "No confidence" : `${Math.round(review.annotation.confidence * 100)}%`}
                </small>
              </span>
            </button>
          ))}
        </div>
        {visibleReviews.length === 0 && (
          <Empty
            title="Queue is clear"
            detail="Low confidence or conflicting evidence will route candidates here."
          />
        )}
      </aside>
      <div className="review-center">
        <div className="review-progress-header" aria-label="Review progress" role="status">
          <div>
            <span className="eyebrow">Inbox progress</span>
            <strong>{queueLoaded ? `${progress.reviewed_count} of ${progress.total_count} results reviewed` : "Loading review progress…"}</strong>
            <small>{queueLoaded ? `${progress.remaining_count} remaining${progress.current_position ? ` · item ${progress.current_position}` : ""}` : "Reading the persisted Project queue"}</small>
          </div>
          <div className="review-progress-navigation" aria-label="Review queue navigation">
            <button aria-label="Previous review result" disabled={!queueNavigation?.previous_review} onClick={() => moveQueueSelection(queueNavigation?.previous_review)}>←</button>
            <button aria-label="Next review result" disabled={!queueNavigation?.next_review} onClick={() => moveQueueSelection(queueNavigation?.next_review)}>→</button>
          </div>
        </div>
        {selected && <div className="review-edit-toolbar" aria-label="Annotation editing controls">
          <button className={editing ? "active" : ""} aria-pressed={editing} onClick={() => { setEditing((value) => !value); setInspectorVisibility(false); }}>Edit <kbd>E</kbd></button>
          {editing && availableShapeKinds.length > 0 && (
            <details className="review-add-menu">
              <summary aria-label="Add annotation">
                <span className="review-add-icon" aria-hidden="true" />
                <span className="review-add-label">Add</span>
                <span className="review-add-caret" aria-hidden="true" />
              </summary>
              <div role="menu" aria-label="Annotation types">
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
            <div className="review-history-tools" aria-label="Edit history">
              <button
                onClick={undo}
                disabled={!past.length}
                aria-label="Undo annotation edit"
                title="Undo (⌘Z)"
              ><span aria-hidden="true">↶</span></button>
              <button
                onClick={redo}
                disabled={!future.length}
                aria-label="Redo annotation edit"
                title="Redo (⇧⌘Z)"
              ><span aria-hidden="true">↷</span></button>
            </div>
          )}
          <div className="review-view-controls" aria-label="Canvas view controls">
            <select
              aria-label="Canvas view"
              value={compareMode}
              onChange={(event) => setCompareMode(event.target.value as typeof compareMode)}
            >
              <option value="after">Result</option>
              <option value="before">Original</option>
              <option value="split">Compare</option>
            </select>
            <button
              className="details-toggle"
              onClick={() => setInspectorVisibility(!inspectorCollapsed)}
              aria-label={inspectorCollapsed ? "Show details" : "Hide details"}
              aria-expanded={!inspectorCollapsed}
            >
              Details <span aria-hidden="true">{inspectorCollapsed ? "›" : "‹"}</span>
            </button>
          </div>
        </div>}
        {selected ? <div
          className={`review-canvas-stage${compareMode === "split" ? " review-canvas-compare" : ""}`}
        >
          {(compareMode === "before" || compareMode === "split") && (
            <div>{compareMode === "split" && <small>Original</small>}<AnnotationCanvas
              imageUrl={images[selected?.image_index ?? 0]?.url}
              annotations={selected ? [selected.annotation] : []}
              selectedId={selected?.annotation.id}
              visualContext={visualContext}
              onSelect={() => undefined}
              onChange={() => undefined}
            /></div>
          )}
          {(compareMode === "after" || compareMode === "split") && (
            <div>{compareMode === "split" && <small>Result</small>}<AnnotationCanvas
              imageUrl={images[selected?.image_index ?? 0]?.url}
              annotations={draft ? [draft] : []}
              selectedId={draft?.id}
              visualContext={visualContext}
              onSelect={() => undefined}
              onEditStart={editing ? beginEdit : undefined}
              onChange={editing ? setDraft : () => undefined}
            /></div>
          )}
        </div> : queueLoaded ? <section className="review-complete panel">
          <span className="eyebrow">Inbox complete</span>
          <h2>{progress.total_count > 0 ? "Review complete" : "Nothing needs review"}</h2>
          <p>{progress.total_count > 0 ? `All ${progress.total_count} queued results have a human decision.` : "Uncertain results will appear here when an Automation routes them to Human Review."}</p>
          {(completedProject ?? scopedProject) && progress.total_count > 0 && <button className="primary" onClick={() => onNavigate(`/projects/${encodeURIComponent((completedProject ?? scopedProject)!.id)}/export`)}>Continue to export</button>}
        </section> : <section className="review-complete panel" aria-busy="true">
          <span className="eyebrow">Review inbox</span>
          <h2>Loading review results…</h2>
          <p>Reading the persisted queue and human decisions.</p>
        </section>}
        <div className="review-footer-stack">
          {rejectOpen && selected && <section className="review-reject-panel" role="dialog" aria-labelledby="reject-review-title">
            <div>
              <span className="eyebrow">Reject result</span>
              <h3 id="reject-review-title">Why is this result incorrect?</h3>
              <p>A reason is required before the result leaves the Inbox.</p>
            </div>
            <label>
              Reason
              <select aria-label="Reject reason" value={rejectReason} onChange={(event) => setRejectReason(event.target.value)}>
                <optgroup label="Common reasons">
                  {GENERIC_REVIEW_REASONS.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}
                </optgroup>
                {skillReasonOptions.length > 0 && <optgroup label="Enabled Skill reasons">
                  {skillReasonOptions.map((option) => <option key={`${option.skillId}:${option.value}`} value={option.value}>{option.label}</option>)}
                </optgroup>}
              </select>
            </label>
            <label>
              Note {rejectReason === "other" ? "(required)" : "(optional)"}
              <textarea value={note} onChange={(event) => setNote(event.target.value)} placeholder="Add useful context for this decision" />
            </label>
            <div className="button-row">
              <button onClick={() => setRejectOpen(false)}>Cancel</button>
              <button className="danger" disabled={decisionBusy || (rejectReason === "other" && !note.trim())} onClick={() => void decideAndAdvance("reject", rejectReason)}>{decisionBusy ? "Rejecting…" : "Reject & next"}</button>
            </div>
          </section>}
          {draft && selected && !rejectOpen && (
            <div className="review-action-bar" aria-label="Review decision controls">
              <span className="review-shortcuts" aria-label="Keyboard shortcuts"><kbd>A</kbd> accept <kbd>R</kbd> reject <kbd>Space</kbd> original/result</span>
              {editing && hasUnsavedAnnotationChanges && (
                <button onClick={save}>{isNew ? "Create annotation" : "Save changes"}</button>
              )}
              <button onClick={() => setRejectOpen(true)} disabled={decisionBusy}>Reject & next</button>
              <button className="primary" disabled={decisionBusy || isNew} onClick={() => void decideAndAdvance("accept", hasUnsavedAnnotationChanges ? reason : "accepted_as_is")} aria-label="Accept and next">{decisionBusy ? "Saving decision…" : "Accept & next"}</button>
            </div>
          )}
        </div>
      </div>
      {!inspectorCollapsed && <aside className="inspector panel review-inspector">
        <div className="review-inspector-header">
          <div>
            <span className="eyebrow">Review details</span>
            <h2>{draft?.label ?? "No selection"}</h2>
          </div>
        </div>
        {draft && selected && (
          <>
            <div className="review-reason-summary">
              <span className="eyebrow">Why this needs review</span>
              <h3>{selected.review_explanation?.title ?? "Needs review"}</h3>
              <p>{reviewReasonExplanation(selected)}</p>
              {selected.review_explanation?.details.length ? <ul>{selected.review_explanation.details.map((detail) => <li key={detail}>{detail}</li>)}</ul> : null}
            </div>
            <dl className="review-essential-facts">
              <div><dt>{scoreSemanticsLabel(reviewScoreSemantics)}</dt><dd>{(selected.confidence ?? draft.confidence) === undefined ? "Not provided" : `${Math.round((selected.confidence ?? draft.confidence ?? 0) * 100)}%`}</dd></div>
              <div><dt>Box quality</dt><dd>{geometrySemanticsLabel(reviewGeometrySemantics)}</dd></div>
              <div><dt>Geometry verification</dt><dd>{reviewGeometrySemantics === "human_verified" ? "Human verified" : reviewCalibrationStatus === "passed" ? "Project calibration passed" : "Needs geometry check"}</dd></div>
              <div><dt>Source Run</dt><dd>{selected.run_id.slice(0, 8)}</dd></div>
              <div><dt>Automation Version</dt><dd>{selected.workflow_id ? `${selected.workflow_id}@v${selected.workflow_version}` : `v${selected.workflow_version}`}</dd></div>
              <div><dt>Source Step</dt><dd>{selected.source_node ?? "Unknown"}</dd></div>
            </dl>
            {selected.detection_evidence?.length ? <section className="review-evidence" aria-label="Source model evidence">
              <header><span className="eyebrow">Source evidence</span><strong>{uniqueEvidence(selected.detection_evidence).length} detector result{uniqueEvidence(selected.detection_evidence).length === 1 ? "" : "s"}</strong></header>
              <div>{uniqueEvidence(selected.detection_evidence).map((evidence) => <article key={evidenceIdentity(evidence)}>
                <span><strong>{sourceModelLabel(evidence.source_model_id)}</strong><small>{evidence.source_capability.replaceAll("_", " ")}</small></span>
                <span><strong>{evidence.score.value == null ? "Score not provided" : evidence.score.value.toFixed(2)}</strong><small>{scoreSemanticsLabel(evidence.score.semantics)}</small></span>
                <code>[{evidence.bbox.map((value) => value.toFixed(3)).join(", ")}]</code>
                <button onClick={() => useEvidenceBox(evidence)}>Use {sourceModelLabel(evidence.source_model_id)} box</button>
              </article>)}</div>
              {uniqueEvidence(selected.detection_evidence).length > 1 && <button onClick={() => setEditing(true)}>Merge manually</button>}
            </section> : null}
            <button onClick={() => onNavigate(reviewProject ? projectRunPath(reviewProject.id, selected.run_id, { nodeId: selected.source_node, artifactId: selected.source_artifact_id, view: "debug" }) : `/runs/${encodeURIComponent(selected.run_id)}`)}>Open run context</button>
            {reviewProject && <button onClick={() => onNavigate(`/projects/${encodeURIComponent(reviewProject.id)}/build/pipeline`)}>Improve automation</button>}
            {editing && <section className="review-edit-details" aria-label="Annotation edit details">
              <div><span className="eyebrow">Manual correction</span><strong>Edit result</strong></div>
              <label>
                Label
                <input value={draft.label ?? ""} onChange={(event) => edit({ ...draft, label: event.target.value })} />
              </label>
              <label>
                Correction reason
                <select aria-label="Correction reason" value={reason} onChange={(event) => setReason(event.target.value)}>
                  {GENERIC_REVIEW_REASONS.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}
                  {skillReasonOptions.map((option) => <option key={`${option.skillId}:${option.value}`} value={option.value}>{option.label}</option>)}
                </select>
              </label>
              <label>
                Reviewer note
                <textarea value={note} onChange={(event) => setNote(event.target.value)} placeholder="What changed, and why?" />
              </label>
              {hasUnsavedAnnotationChanges && <div className="correction-impact" role="status">
                <strong>Correction impact</strong>
                <span>This correction will be saved as geometry-quality evidence for calibration and future Automation improvements.</span>
              </div>
              }
            </section>}
            <details className="review-execution-details">
              <summary>Execution details</summary>
              <div className="fact-grid">
                <Fact label="Refinement" value={selected.refinement_chain?.map((refiner) => refiner === "sam_prompted_refiner" ? "SAM 2.1 multi-prompt" : refiner === "ball_foreground_refiner" ? "Local foreground fallback (no SAM)" : refiner).join(" → ") || "None recorded"} />
                <Fact label="Validation issue" value={selected.validation_issues.join(", ") || "None"} />
                <Fact label="Task" value={draft.task_id} />
                <Fact label="Status" value={draft.review_status} />
              </div>
              {editing && <label>
                Attributes (JSON)
                <textarea aria-label="Annotation attributes JSON" value={attributesText} onChange={(event) => setAttributesText(event.target.value)} />
              </label>}
              <button onClick={() => api.revisions(draft.id).then((value) => alert(JSON.stringify(value.revisions, null, 2)))}>View revision history</button>
              <Trace events={events.filter((event) => event.run_id === selected.run_id)} />
            </details>
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
        <span className="eyebrow">Visible execution events</span>
        <h3>Agent trace</h3>
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
                  `${event.task_id ?? "run"} · ${new Date(event.occurred_at).toLocaleTimeString()}`}
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
    drafting: "Building the Draft",
    validating: "Validating the Draft",
    dry_running: "Testing sample images",
    revising: "Revising from evidence",
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
  if (session.outcome) return labels[session.outcome];
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
  const reservedCalls = session.reserved_finalization_calls ?? session.builder_budget?.reserved_finalization_calls ?? 0;
  const progress = Math.min(100, Math.round((totalCalls / Math.max(1, maximumCalls)) * 100));
  const needsSetup = ["provider_setup_required", "blocked_draft_ready"].includes(session.outcome ?? "");
  const retryable = ["failed", "budget_exceeded"].includes(session.status) || needsSetup;
  return (
    <div className="agent-session-trace" aria-label={`${session.kind} Agent trace`}>
      <div className="context-line">
        <strong>Pipeline Builder</strong>
        <Status status={session.status} />
        <span>{stage}</span>
        <span>{totalCalls} tool calls</span>
        <span>{session.usage.input_tokens + session.usage.output_tokens} tokens</span>
        <span>${session.usage.cost}</span>
        {onCancel && (
          <button className="danger" disabled={!cancellable} onClick={onCancel}>
            Cancel Agent
          </button>
        )}
      </div>
      <div className="agent-progress" role="progressbar" aria-label="Tool budget used" aria-valuemin={0} aria-valuemax={100} aria-valuenow={progress}>
        <span style={{ width: `${progress}%` }} />
      </div>
      <div className="fact-grid">
        <Fact label="Current stage" value={stage} />
        <Fact label="Model turns" value={session.model_turns ?? session.model_calls.length} />
        <Fact label="Tool budget" value={`${remainingCalls} remaining · ${reservedCalls} reserved`} />
        <Fact label="Phase calls" value={session.phase_tool_calls ?? "Not recorded"} />
        <Fact label="Cache reuse" value={session.cache_hits ?? 0} />
        <Fact label="Duplicates blocked" value={session.duplicate_tool_calls ?? 0} />
        <Fact
          label="Provider"
          value={session.model_selection?.provider_display_name ?? "Not recorded"}
        />
        <Fact
          label="Agent model"
          value={session.model_selection?.model_display_name ?? "Offline deterministic"}
        />
        <Fact
          label="Model choice"
          value={
            session.model_selection
              ? `${session.model_selection.binding_source.replaceAll("_", " ")}${session.model_selection.locked ? " · locked" : ""}`
              : "Offline mode"
          }
        />
        <Fact
          label="Validation issues"
          value={validation?.issues.length ?? "Not recorded"}
        />
        <Fact
          label="Dry Run"
          value={dryRun ? `${dryRun.summary.image_count} image · ${dryRun.summary.failed_count} failed` : "Not run"}
        />
        <Fact label="Stop reason" value={session.stop_reason ?? "Running"} />
        <Fact
          label="Human action"
          value={session.pending_human_action ?? "None"}
        />
        {session.builder_constraints && <Fact label="Priority" value={session.builder_constraints.priority.replaceAll("_", " ")} />}
      </div>
      {session.status !== "running" && (
        <section className={`agent-outcome-card ${needsSetup ? "setup" : ""}`} aria-label="Pipeline Builder outcome">
          <div>
            <span className="eyebrow">Outcome</span>
            <h4>{agentOutcomeLabel(session)}</h4>
            <p>{session.next_action ?? readableErrorMessage(session.stop_reason ?? "Open the saved session for details.")}</p>
          </div>
          <div className="agent-outcome-facts">
            {session.draft_id && <span><small>Draft</small><strong>{session.draft_id.slice(0, 8)}</strong></span>}
            <span><small>Stop reason</small><strong>{session.builder_stop_reason?.replaceAll("_", " ") ?? session.stop_reason ?? "Completed"}</strong></span>
            {!!session.unresolved_bindings?.length && <span><small>Unresolved</small><strong>{session.unresolved_bindings.length} model binding{session.unresolved_bindings.length === 1 ? "" : "s"}</strong></span>}
          </div>
          <div className="button-row">
            {session.draft_id && onOpenDraft && <button className="primary" onClick={() => onOpenDraft(session.draft_id!)}>{session.outcome === "draft_ready_for_human_review" ? "Review Draft" : needsSetup ? "Open blocked Draft" : "Open Draft"}</button>}
            {needsSetup && onConfigureProvider && <button onClick={onConfigureProvider}>Configure Provider</button>}
            {needsSetup && onConfigureModel && <button onClick={onConfigureModel}>Configure Model</button>}
            {retryable && onRetry && <button onClick={onRetry}>Retry from current Draft</button>}
          </div>
        </section>
      )}
      <details className="agent-tool-trace" open={session.status === "running"}>
        <summary>Tool actions ({session.steps.length})</summary>
      <ol className="agent-action-list">
        {session.steps.map((step) => (
          <li key={step.call_id}>
            <strong>{step.sequence}. {step.tool_name.replaceAll("_", " ")}</strong>
            <small>{step.success ? "Completed" : "Failed"}</small>
            <details>
              <summary>Observable inputs and result</summary>
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
                  {call.succeeded ? "Succeeded" : "Failed"} · {call.duration_ms} ms
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
      <Panel title="Agent activity" eyebrow="Advisor and recovery sessions">
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
          <Empty title="No Agent sessions" detail="Deterministic Workflow execution remains the fast path." />
        )}
      </Panel>
      <Panel title="Correction Memory" eyebrow="Project-scoped structured evidence">
        {memory.length ? (
          <div className="catalog-list">
            {memory.map((record) => (
              <article key={record.id}>
                <span className="catalog-monogram">M</span>
                <span>
                  <strong>{record.reason_code.replaceAll("_", " ")}</strong>
                  <small>{record.skill_id} · {record.task_id} · {record.predicted_label ?? "any Label"}</small>
                  <small>This evidence can raise recovery risk for the same Project, Skill, task and Label.</small>
                </span>
              </article>
            ))}
          </div>
        ) : (
          <Empty title="No correction evidence" detail="Human corrections will appear here after Review." />
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
            <div><span className="eyebrow">{group.detail}</span><h2>{group.title}</h2></div>
            {items.map((skill) => (
              <Panel key={skill.id} title={`${skill.display_name} · v${skill.version}`} eyebrow={`${skill.kind} · ${skill.id}`}>
                <p className="lede">{skill.description}</p>
                <div className="skill-columns">
                  <TagGroup title="Provided Nodes" values={skill.nodes} />
                  <TagGroup title="Registered tools" values={skill.tools} />
                  <TagGroup title="Capabilities" values={skill.capabilities} />
                  <TagGroup title="Capability requirements" values={skill.capability_requirements} />
                  <TagGroup title="Validators" values={skill.validators} />
                  <TagGroup title="Refiners" values={skill.refiners} />
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
  const [projectId, setProjectId] = useState("");
  const [images, setImages] = useState<ImageItem[]>([]);
  const [imageIndex, setImageIndex] = useState(0);
  const [query, setQuery] = useState("football");
  const [busy, setBusy] = useState("");

  useEffect(() => {
    if (step === 5 && projects.length === 0) {
      void api.dashboard().then((value) => {
        const available = value.projects.filter((project) => project.image_count > 0);
        setProjects(available);
        if (available[0]) setProjectId(available[0].id);
      }).catch((error: Error) => onError(error.message));
    }
  }, [step]);
  useEffect(() => {
    if (!projectId) {
      setImages([]);
      return;
    }
    void api.images(projectId).then((value) => {
      setImages(value.images);
      setImageIndex(value.images[0]?.index ?? 0);
    }).catch((error: Error) => onError(error.message));
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
    {step === 1 && <div className="wizard-step"><div className="choice-grid expert-methods" role="radiogroup" aria-label="Expert Model integration method">{([
      ["preset", "Use preset", "Start with a known capability contract"],
      ["http", "Generic HTTP Worker", "Connect any Vision Protocol v1 service"],
    ] as const).map(([value, label, detail]) => <label className={method === value ? "selected" : ""} key={value}><input type="radio" name="expert-method" checked={method === value} onChange={() => { setMethod(value); if (value === "http") choosePreset("custom"); }} /><span><strong>{label}</strong><small>{detail}</small></span></label>)}</div>{method === "preset" && <label>Preset<select value={preset} onChange={(event) => choosePreset(event.target.value)}>{EXPERT_WORKER_PRESETS.map(([value, label, detail]) => <option key={value} value={value}>{label} — {detail}</option>)}</select></label>}</div>}
    {step === 2 && <div className="wizard-step"><div className="form-grid"><label>Endpoint<input type="url" value={String(worker.base_url ?? "")} onChange={(event) => setField("base_url", event.target.value)} /></label><label>Timeout seconds<input type="number" min="1" value={Number(worker.timeout_seconds ?? 120)} onChange={(event) => setField("timeout_seconds", Number(event.target.value))} /></label><label>Authentication reference<input value={String(worker.authentication_reference ?? "")} onChange={(event) => setField("authentication_reference", event.target.value || null)} placeholder="env:ANNOTAGENT_SAM_TOKEN" /></label><label className="checkbox-line"><input type="checkbox" checked={Boolean(worker.allow_remote)} onChange={(event) => setField("allow_remote", event.target.checked)} /><span>Allow remote HTTPS Worker</span></label></div><div className="wizard-summary"><strong>Trust boundary</strong><span>Loopback is allowed by default. Remote endpoints require HTTPS and explicit permission. Authentication is a reference; no secret is written to Settings.</span></div></div>}
    {step === 3 && <div className="wizard-step"><div className={`expert-test-banner ${discovery?.passed ? "passed" : "failed"}`} role="status"><strong>{discovery?.passed ? "Discovery passed" : `Discovery stopped at ${discovery?.failed_stage ?? "an unknown stage"}`}</strong><span>{discovery?.error ?? discovery?.evidence?.detail ?? "The Worker returned all required protocol resources."}</span></div><div className="expert-check-grid"><Fact label="Health" value={discovery?.health?.status ?? "Not available"} /><Fact label="Protocol" value={discovery?.evidence?.protocol_compatible ? "Compatible" : "Not verified"} /><Fact label="Models" value={discovery?.models?.models.length ?? 0} /><Fact label="Contracts" value={discovery?.evidence?.contracts_validated ? "Valid" : "Not verified"} /></div>{discovery?.capabilities && <div className="tag-group">{discovery.capabilities.capabilities.map((capability) => <span key={capability}>{capability.replaceAll("_", " ")}</span>)}</div>}<details className="advanced-settings"><summary>Raw discovery response</summary><pre>{JSON.stringify(discovery, null, 2)}</pre></details></div>}
    {step === 4 && <div className="wizard-step"><div className="form-grid"><label>Display name<input value={String(worker.display_name ?? "")} onChange={(event) => setField("display_name", event.target.value)} /></label><label>Model ID<input value={String(worker.model_id ?? "")} onChange={(event) => setField("model_id", event.target.value)} /></label><label>Architecture<input value={String(worker.version?.architecture ?? "")} onChange={(event) => setVersion("architecture", event.target.value)} /></label><label>Version<input value={String(worker.version?.model_version ?? "")} onChange={(event) => setVersion("model_version", event.target.value)} /></label><label>Checkpoint SHA-256<input value={String(worker.version?.checkpoint_sha256 ?? "")} onChange={(event) => setVersion("checkpoint_sha256", event.target.value.trim())} placeholder="64 hexadecimal characters" /></label><label>Training dataset version<input value={String(worker.version?.training_dataset_version ?? "")} onChange={(event) => setVersion("training_dataset_version", event.target.value)} /></label><label>Label space<input value={Array.isArray(worker.label_space) ? worker.label_space.join(", ") : ""} onChange={(event) => setField("label_space", event.target.value.split(",").map((value) => value.trim()).filter(Boolean))} placeholder="football, robot" /></label><label>Checkpoint license<input value={String(worker.license?.weight_license ?? "")} onChange={(event) => setLicense("weight_license", event.target.value)} /></label></div><div className="expert-test-banner missing"><strong>Missing weights until identity is complete</strong><span>A filename is not a checkpoint identity. SAM and specialist models remain unavailable without a version, SHA-256, and concrete weight license.</span></div></div>}
    {step === 5 && <div className="wizard-step"><div className="form-grid"><label>Project<select value={projectId} onChange={(event) => setProjectId(event.target.value)}><option value="">Choose a Project with images</option>{projects.map((project) => <option key={project.id} value={project.id}>{project.name}</option>)}</select></label><label>Sample image<select value={imageIndex} onChange={(event) => setImageIndex(Number(event.target.value))}>{images.map((image) => <option key={image.index} value={image.index}>{image.name}</option>)}</select></label><label>Text query<input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="football" /></label></div>{projectId && images.length > 0 && <div className="expert-sample-layout"><img src={`/api/projects/${projectId}/images/${imageIndex}/content`} alt="Selected Worker sample input" /><div><button className="primary" disabled={busy === "sample"} onClick={() => void runSample()}>{busy === "sample" ? "Running sample…" : "Run sample test"}</button><small>Prompted segmentation uses a visible centered sample box. Detection Workers use the selected image and query.</small></div></div>}{sample && <div className="expert-sample-result"><div className={`expert-test-banner ${sample.passed ? "passed" : "failed"}`} role="status"><strong>{sample.passed ? "Sample conversion passed" : "Sample conversion failed"}</strong><span>{sample.error ?? sample.evidence.detail}</span></div>{sample.input?.image_url && <img src={sample.input.image_url} alt="Worker sample result source" />}<div className="expert-check-grid"><Fact label="Artifacts" value={Array.isArray(sample.converted_artifacts) ? sample.converted_artifacts.length : 0} /><Fact label="Duration" value={`${sample.duration_ms} ms`} /><Fact label="Score semantics" value={sample.score_semantics?.replaceAll("_", " ") ?? "Unknown"} /><Fact label="Geometry" value={sample.geometry_semantics?.replaceAll("_", " ") ?? "Unknown"} /></div><details className="advanced-settings"><summary>Converted Artifact and coordinates</summary><pre>{JSON.stringify({ raw_output_summary: sample.raw_output_summary, converted_artifacts: sample.converted_artifacts, coordinates: sample.coordinates, warnings: sample.warnings }, null, 2)}</pre></details></div>}</div>}
    {step === 6 && <div className="wizard-step"><div className={`expert-test-banner ${canRegister ? "passed" : "missing"}`}><strong>{canRegister ? "Ready to register" : "Registration is blocked"}</strong><span>{canRegister ? "Health, protocol, contracts, model identity, weights, and sample conversion all have active evidence." : sample?.evidence.detail ?? "Run a successful selected-image sample after discovery and identity setup."}</span></div><div className="expert-checklist">{[["Health", readyEvidence?.health_passed], ["Protocol", readyEvidence?.protocol_compatible], ["Contracts", readyEvidence?.contracts_validated], ["Weights", readyEvidence?.weights_ready], ["Sample conversion", readyEvidence?.sample_conversion_passed]].map(([label, passed]) => <span key={String(label)} className={passed ? "complete" : "blocked"}><b>{passed ? "✓" : "—"}</b>{label}</span>)}</div></div>}
    <div className="wizard-actions"><button disabled={Boolean(busy)} onClick={step === 1 ? onClose : () => setStep((value) => value - 1)}>{step === 1 ? "Cancel" : "Back"}</button>{step === 1 ? <button className="primary" onClick={() => setStep(2)}>Continue</button> : step === 2 ? <button className="primary" disabled={busy === "discovery" || !String(worker.base_url ?? "").trim()} onClick={() => void discover()}>{busy === "discovery" ? "Discovering…" : "Save and discover"}</button> : step === 3 ? <button className="primary" onClick={() => setStep(4)}>Configure identity</button> : step === 4 ? <button className="primary" disabled={busy === "identity"} onClick={() => void saveIdentity()}>{busy === "identity" ? "Saving…" : "Save identity and test"}</button> : step === 5 ? <button className="primary" disabled={!sample?.passed} onClick={() => setStep(6)}>Review registration</button> : <button className="primary" disabled={!canRegister || busy === "register"} onClick={() => void register()}>{busy === "register" ? "Registering…" : "Register Expert Model"}</button>}</div>
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
                <label className="checkbox-line" title={!registrationReady && !worker.enabled ? "Complete discovery, model identity, and a selected-image sample before enabling this Worker." : undefined}><input type="checkbox" checked={Boolean(worker.enabled)} disabled={!registrationReady && !worker.enabled} onChange={(event) => setDetectionWorker(index, "enabled", event.target.checked)} /><span>Enabled</span></label>
                <button className="text-button" onClick={() => removeDetectionWorker(index)}>Remove</button>
              </div>
            </div>
            <div className="form-grid">
              <label>Display name<input value={String(worker.display_name ?? "")} onChange={(event) => setDetectionWorker(index, "display_name", event.target.value)} /></label>
              <label>Registry ID<input value={String(worker.id ?? "")} onChange={(event) => setDetectionWorker(index, "id", event.target.value)} /></label>
              <label>Model ID<input value={String(worker.model_id ?? "")} onChange={(event) => setDetectionWorker(index, "model_id", event.target.value)} /></label>
              <label>Worker URL<input type="url" value={String(worker.base_url ?? "")} onChange={(event) => setDetectionWorker(index, "base_url", event.target.value)} /></label>
              <label>Authentication reference<input value={String(worker.authentication_reference ?? "")} onChange={(event) => setDetectionWorker(index, "authentication_reference", event.target.value || null)} placeholder="env:ANNOTAGENT_WORKER_TOKEN" /></label>
              <label>Capability<select value={String(worker.expected_capabilities?.[0] ?? "object_detection")} onChange={(event) => setDetectionWorker(index, "expected_capabilities", [event.target.value])}><option value="object_detection">Object detection</option><option value="open_vocabulary_detection">Open-vocabulary detection</option><option value="phrase_grounding">Phrase grounding</option><option value="prompted_segmentation">Prompted segmentation</option><option value="semantic_segmentation">Semantic segmentation</option></select></label>
              <label>Score semantics<select value={String(worker.score_semantics ?? "unknown")} onChange={(event) => setDetectionWorker(index, "score_semantics", event.target.value)}><option value="calibrated_probability">Calibrated probability</option><option value="relative_confidence">Relative confidence</option><option value="ranking_score">Ranking score</option><option value="not_provided">Not provided</option><option value="unknown">Unknown</option></select></label>
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
            <h3>Pricing</h3>
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
            ? "Unsaved workspace settings"
            : message ||
            (settings.settings_persisted
              ? `Saved at ${settings.settings_path}`
              : "Save once to keep these settings across restarts.")}
        </span>
        {dirty && (
          <button className="primary" onClick={save} disabled={saving}>
            {saving ? "Saving…" : "Save settings"}
          </button>
        )}
      </div>
    </section>
  );
}

type GuidedIntent = "classification" | "detection" | "segmentation" | "custom";
type GuidedPriority = "faster" | "balanced" | "accuracy";

function guidedId(value: string, fallback: string): string {
  const normalized = value
    .normalize("NFKD")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 48);
  return normalized || fallback;
}

function guidedProjectYaml({
  name,
  taskDisplayName,
  taskId,
  labelId,
  kind,
  priority,
}: {
  name: string;
  taskDisplayName: string;
  taskId: string;
  labelId: string;
  kind: string;
  priority: GuidedPriority;
}): string {
  const parallel = priority === "faster" ? 4 : priority === "accuracy" ? 1 : 2;
  const autoAccept = priority === "faster" ? 0.82 : priority === "accuracy" ? 0.94 : 0.9;
  const formats =
    kind === "bounding_box"
      ? "[native, coco, yolo]"
      : kind === "semantic_mask"
        ? "[native, coco, yolo_segmentation]"
        : "[native]";
  return `version: 1
project:
  name: ${JSON.stringify(name)}
  language: en
dataset:
  root: images
runtime:
  max_parallel_images: ${parallel}
tasks:
  - id: ${taskId}
    display_name: ${JSON.stringify(taskDisplayName)}
    kind: ${kind}
    labels: [${JSON.stringify(labelId)}]
    required: true
review:
  auto_accept_confidence: ${autoAccept}
  force_review_below: 0.5
export:
  formats: ${formats}
`;
}

function CreateProject({
  onClose,
  onCreated,
  onError,
}: {
  onClose: () => void;
  onCreated: (projectId: string, customize: boolean) => void;
  onError: (value: string) => void;
}) {
  const [step, setStep] = useState(1);
  const [intent, setIntent] = useState<GuidedIntent>("detection");
  const [projectName, setProjectName] = useState("Football annotations");
  const [labelName, setLabelName] = useState("Football");
  const [customKind, setCustomKind] = useState("bounding_box");
  const [workspaceId, setWorkspaceId] = useState("");
  const [taskId, setTaskId] = useState("");
  const [labelId, setLabelId] = useState("");
  const [dataSource, setDataSource] = useState("");
  const [priority, setPriority] = useState<GuidedPriority>("balanced");
  const [maximumCost, setMaximumCost] = useState("");
  const [targetReviewRate, setTargetReviewRate] = useState("10");
  const [offlineOnly, setOfflineOnly] = useState(false);
  const [modelRegistry, setModelRegistry] = useState<ModelBinding[]>([]);
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState("");
  useEffect(() => {
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, []);
  useEffect(() => {
    void api.models()
      .then((value) => setModelRegistry(value.models))
      .catch((error: Error) => onError(error.message));
  }, []);
  const resolvedWorkspaceId = workspaceId || guidedId(projectName, "vision-project");
  const resolvedLabelId = labelId || guidedId(labelName, "target");
  const resolvedTaskId = taskId || `${resolvedLabelId}-${
    intent === "classification" ? "class" : intent === "segmentation" ? "regions" : "objects"
  }`;
  const kind =
    intent === "classification"
      ? "classification"
      : intent === "segmentation"
        ? "semantic_mask"
        : intent === "detection"
          ? "bounding_box"
          : customKind;
  const specialistModel = modelRegistry.find((model) =>
    model.enabled && model.capabilities?.includes("object_detection") &&
    (model.label_space?.length ?? 0) > 0 &&
    model.label_space?.some((label) => label.toLowerCase() === resolvedLabelId.toLowerCase()),
  );
  const openVocabularyModel = modelRegistry.find((model) =>
    model.enabled && model.capabilities?.includes("open_vocabulary_detection"),
  );
  const finish = async (customize: boolean) => {
    if (!projectName.trim() || !labelName.trim()) return;
    setBusy(true);
    setProgress("Creating the Project…");
    try {
      await api.createProject(
        resolvedWorkspaceId,
        guidedProjectYaml({
          name: projectName.trim(),
          taskDisplayName: labelName.trim(),
          taskId: resolvedTaskId,
          labelId: resolvedLabelId,
          kind,
          priority,
        }),
      );
      const warnings: string[] = [];
      if (dataSource.trim()) {
        setProgress("Importing images…");
        try {
          const report = await api.importImages(resolvedWorkspaceId, dataSource.trim());
          setProgress(`Imported ${report.imported}; skipped ${report.duplicates} duplicates.`);
        } catch (error) {
          warnings.push(`Images were not imported: ${(error as Error).message}`);
        }
      }
      setProgress("Preparing the recommended Automation Draft…");
      try {
        const [compatible, defaults] = await Promise.all([
          api.compatibleModelProfiles({
            input_modalities: ["text"],
            capabilities: ["text_generation"],
            tool_calls: true,
            structured_output: true,
          }),
          api.agentModelBindings(),
        ]);
        const agentModel = compatible.models.find(
          (model) => model.id === defaults.pipeline_builder,
        ) ?? compatible.models[0];
        if (!agentModel) {
          throw new Error(
            "Configure an Available text-generation Model Profile with Tool calls and Structured output before requesting a recommendation.",
          );
        }
        await api.suggestWorkflow(
          resolvedWorkspaceId,
          "llm",
          { task_id: resolvedTaskId, label: resolvedLabelId },
          {
            max_cost_per_image: maximumCost.trim() || undefined,
            max_latency_ms: priority === "faster" ? 1_000 : priority === "accuracy" ? 10_000 : 4_000,
            minimum_accuracy: priority === "faster" ? 0.75 : priority === "accuracy" ? 0.92 : 0.85,
            require_review_gate: Number(targetReviewRate) > 0,
          },
          DEFAULT_PIPELINE_BUILDER_CONSTRAINTS,
          agentModel.id,
        );
      } catch (error) {
        warnings.push(`The Project was created, but its recommendation needs attention: ${(error as Error).message}`);
      }
      onCreated(resolvedWorkspaceId, customize);
      if (warnings.length) onError(warnings.join(" "));
    } catch (error) {
      onError((error as Error).message);
    } finally {
      setBusy(false);
      setProgress("");
    }
  };
  const nextDisabled =
    step === 1 && (!projectName.trim() || !labelName.trim());
  return (
    <div className="modal-backdrop">
      <div className="modal guided-project-wizard" role="dialog" aria-modal="true" aria-label="Create Project">
        <header>
          <span className="eyebrow">New Project · Step {step} of 4</span>
          <h2 id="create-project-title">{
            step === 1 ? "What do you want to annotate?" :
            step === 2 ? "Add data" :
            step === 3 ? "Choose a priority" :
            "Recommended automation"
          }</h2>
          <div className="wizard-progress" aria-label={`Step ${step} of 4`}>
            {[1, 2, 3, 4].map((item) => <i key={item} className={item <= step ? "complete" : ""} />)}
          </div>
        </header>

        {step === 1 && <div className="wizard-step">
          <div className="choice-grid" role="radiogroup" aria-label="Annotation intent">
            {([
              ["classification", "Classify images", "Assign one or more labels to each image"],
              ["detection", "Find objects", "Locate each object with a bounding box"],
              ["segmentation", "Segment regions", "Trace regions with semantic masks"],
              ["custom", "Custom", "Choose an annotation output explicitly"],
            ] as const).map(([value, title, detail]) => <label key={value} className={intent === value ? "selected" : ""}>
              <input type="radio" name="intent" value={value} checked={intent === value} onChange={() => setIntent(value)} />
              <span><strong>{title}</strong><small>{detail}</small></span>
            </label>)}
          </div>
          <div className="form-grid">
            <label>Project name<input autoFocus value={projectName} onChange={(event) => setProjectName(event.target.value)} placeholder="Football annotations" /></label>
            <label>{intent === "classification" ? "Class name" : intent === "segmentation" ? "Region name" : "Object name"}<input value={labelName} onChange={(event) => setLabelName(event.target.value)} placeholder="Football" /></label>
            {intent === "custom" && <label>Output<select value={customKind} onChange={(event) => setCustomKind(event.target.value)}><option value="classification">Classification</option><option value="bounding_box">Bounding boxes</option><option value="semantic_mask">Semantic masks</option><option value="polygon">Polygons</option><option value="keypoints">Keypoints</option></select></label>}
            {intent !== "custom" && <div className="wizard-fact"><span>Output</span><strong>{kind.replaceAll("_", " ")}</strong></div>}
          </div>
          <details className="advanced-settings"><summary>Advanced IDs</summary><div className="form-grid">
            <label>Workspace ID<input value={resolvedWorkspaceId} onChange={(event) => setWorkspaceId(event.target.value)} /></label>
            <label>Task ID<input value={resolvedTaskId} onChange={(event) => setTaskId(event.target.value)} /></label>
            <label>Label ID<input value={resolvedLabelId} onChange={(event) => setLabelId(event.target.value)} /></label>
          </div><small>AnnotAgent generates stable IDs. Change them only for an existing integration.</small></details>
        </div>}

        {step === 2 && <div className="wizard-step">
          <label>Image file or folder<input autoFocus value={dataSource} onChange={(event) => setDataSource(event.target.value)} placeholder="/workspace/dataset/images" /></label>
          <div className="wizard-summary"><strong>{dataSource.trim() ? "Ready to scan this source" : "You can add data later"}</strong><span>PNG and JPEG · recursive folder discovery · content duplicates skipped</span><small>Decode errors and actual imported/duplicate counts are reported by the real import operation when you finish setup.</small></div>
        </div>}

        {step === 3 && <div className="wizard-step">
          <div className="choice-grid priority-grid" role="radiogroup" aria-label="Automation priority">
            {([
              ["faster", "Faster", "More parallel work and a lower acceptance threshold"],
              ["balanced", "Balanced", "Recommended trade-off for a first Project"],
              ["accuracy", "Higher accuracy", "More conservative automatic acceptance"],
            ] as const).map(([value, title, detail]) => <label key={value} className={priority === value ? "selected" : ""}>
              <input type="radio" name="priority" value={value} checked={priority === value} onChange={() => setPriority(value)} />
              <span><strong>{title}</strong><small>{detail}</small></span>
            </label>)}
          </div>
          <details className="advanced-settings"><summary>Cost, review, and local constraints</summary><div className="form-grid">
            <label>Maximum expected cost<input value={maximumCost} onChange={(event) => setMaximumCost(event.target.value)} placeholder="Optional" /></label>
            <label>Target human review rate (%)<input type="number" min="0" max="100" value={targetReviewRate} onChange={(event) => setTargetReviewRate(event.target.value)} /></label>
            <div className="wizard-fact"><span>Registered detection models</span><strong>{modelRegistry.filter((model) => model.enabled && model.role === "detection").length || "None enabled"}</strong></div>
            <label className="check-row"><input type="checkbox" checked={offlineOnly} onChange={(event) => setOfflineOnly(event.target.checked)} /> Offline only</label>
          </div></details>
        </div>}

        {step === 4 && <div className="wizard-step">
          <div className="recommendation-card">
            <span className="status status-auto-accepted">Recommended</span>
            <h3>{kind === "classification" ? `Classify each image as ${labelName}` : kind === "semantic_mask" ? `Segment ${labelName} regions` : specialistModel ? "Use your trained detector first" : "Find objects by description"}</h3>
            <ol>
              {kind === "bounding_box" && specialistModel ? <>
                <li>Use <strong>{specialistModel.model}</strong> for repeated {labelName} labeling.</li>
                <li>{openVocabularyModel ? <>Ask <strong>{openVocabularyModel.model}</strong> only when the specialist result is uncertain.</> : "Route uncertain detector results to Review until an open-vocabulary fallback is configured."}</li>
              </> : kind === "bounding_box" && openVocabularyModel ? <>
                <li>Use <strong>{openVocabularyModel.model}</strong> to find {labelName} from a text description.</li>
                <li>No training data is required.</li>
              </> : kind === "bounding_box" ? <>
                <li>Use a registered open-vocabulary detector to find {labelName} from its description.</li>
                <li>No training data is required. Configure the Worker before a live Run.</li>
              </> : <li>Bind a compatible <strong>Registry Model Profile</strong> in Automation before publishing.</li>}
              {kind === "bounding_box" && <li>Keep the detector output as editable bounding boxes.</li>}
              <li>Automatically accept high-confidence results.</li>
              <li>Send uncertain results to Review.</li>
            </ol>
            <div className="recommendation-estimate"><span><b>{priority === "faster" ? "Low" : priority === "accuracy" ? "Higher" : "Medium"}</b> latency</span><span><b>Low</b> setup effort</span><span><b>{targetReviewRate || "10"}%</b> target review</span></div>
          </div>
          <div className="inline-model-connection">
            <div><span className="eyebrow">Registry-first execution</span><strong>Bind in Automation</strong></div>
            <p>The wizard creates only the Project Schema and an editable Draft. Choose a reusable Model Profile on the Automation page, Dry Run it, then publish an immutable Workflow Version.</p>
            <small>{offlineOnly ? "Offline only is recorded as a design constraint; connect a local Vision Worker before building the Pipeline." : "Provider credentials are configured once under Settings → Providers and are never copied into the Project."}</small>
          </div>
          <details className="advanced-settings"><summary>Generated Project definition</summary><pre>{guidedProjectYaml({ name: projectName.trim(), taskDisplayName: labelName.trim(), taskId: resolvedTaskId, labelId: resolvedLabelId, kind, priority })}</pre></details>
        </div>}

        {progress && <div className="wizard-running" role="status">{progress}</div>}
        <div className="wizard-actions">
          <button onClick={step === 1 ? onClose : () => setStep((value) => value - 1)} disabled={busy}>{step === 1 ? "Cancel" : "Back"}</button>
          {step < 4 ? <button className="primary" disabled={nextDisabled} onClick={() => setStep((value) => value + 1)}>Continue</button> : <>
            <button disabled={busy} onClick={() => void finish(true)}>Customize</button>
            <button className="primary" disabled={busy || nextDisabled} onClick={() => void finish(false)}>{busy ? "Creating…" : "Use recommendation"}</button>
          </>}
        </div>
      </div>
    </div>
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
      {presentation.label}
    </span>
  );
}
function NotFoundPage({
  invalidPath,
  onNavigate,
}: {
  invalidPath: string;
  onNavigate: (path: string) => void;
}) {
  return (
    <section className="page-stack">
      <div className="toolbar-panel">
        <div>
          <span className="eyebrow">404 · Workspace route</span>
          <h2>This page does not exist</h2>
          <p>
            AnnotAgent kept the requested address visible instead of silently
            sending you somewhere unrelated.
          </p>
          <code>{invalidPath}</code>
        </div>
        <div className="button-row">
          <button onClick={() => window.history.back()}>Go back</button>
          <button className="primary" onClick={() => onNavigate("/projects")}>
            Open Projects
          </button>
        </div>
      </div>
    </section>
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
          <small>None</small>
        )}
      </div>
    </div>
  );
}
