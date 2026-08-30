import { useEffect, useRef, useState } from "react";
import { api, subscribeEvents } from "./api";
import { AnnotationCanvas } from "./components/AnnotationCanvas";
import {
  CUSTOM_MODEL,
  PROVIDER_PRESETS,
  applyProviderPreset,
  getProviderPreset,
  inferConfiguredProviderPreset,
  inferProviderPreset,
  isCatalogModel,
} from "./providerCatalog";
import { visualProfilesForSkills } from "./skills/visualProfiles";
import { annotationColor, annotationVisual, type LabelVisualMapping } from "./annotationVisuals";
import { deriveProjectRunView } from "./runState";
import { projectForReview, projectForRun, runsForContext } from "./workspaceContext";
import {
  parseWorkspaceRoute,
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
  DetectionWorkerTestResult,
  HistoryRun,
  ImageItem,
  ModelBinding,
  NodeReplayReport,
  PipelineArtifact,
  PipelineArtifactType,
  PipelineSource,
  PipelineStep,
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
} from "./types";

const PAGE_TITLES: Record<ProductPage | "project" | "build" | "export", string> = {
  home: "Home",
  projects: "Projects",
  project: "Project",
  build: "Build",
  export: "Export",
  runs: "Runs",
  review: "Review",
  settings: "Settings",
};

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

  const routeProjectId =
    route.kind === "project" || route.kind === "build" || route.kind === "export"
      ? route.projectId
      : "";
  const routeRun = route.kind === "runs" && route.runId
    ? runs.find((run) => run.id === route.runId)
    : undefined;
  const routeRunProject = projectForRun(projects, routeRun);
  const routeScopeProjectId =
    route.kind === "runs" || route.kind === "review" ? route.projectId ?? "" : "";
  const projectId = routeProjectId || routeRunProject?.id || routeScopeProjectId;
  const selectedProject = projects.find((project) => project.id === projectId);
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
    if (route.kind === "export")
      navigate(id ? `/projects/${encodeURIComponent(id)}/export` : "/projects");
    else if (route.kind === "build")
      navigate(id ? `/projects/${encodeURIComponent(id)}/build/${route.step}` : "/projects");
    else if (route.kind === "project") openProject(id);
  };
  useEffect(() => {
    const resolved = routeProjectId || routeRunProject?.id;
    if (resolved && resolved !== activeProjectId) setProjectContext(resolved);
  }, [routeProjectId, routeRunProject?.id]);
  const page =
    route.kind === "home" ||
    route.kind === "projects" ||
    route.kind === "runs" ||
    route.kind === "review" ||
    route.kind === "settings"
      ? route.kind
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
                    route.kind === "export"
                  : page === item.page
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
          {(route.kind === "project" || route.kind === "build" || route.kind === "export") && <div className="project-switch">
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
              <span>{error}</span>
              <small>Saved workspace data remains on the server. Retry reloads the latest state before you continue.</small>
            </span>
            <span className="error-actions">
              <button onClick={() => window.location.reload()}>
                Retry from latest state
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
            onOpenRun={(runId) => navigate(`/runs/${encodeURIComponent(runId)}`)}
            onOpenReview={() =>
              navigate(`/review?project_id=${encodeURIComponent(route.projectId)}`)
            }
            onNavigate={navigate}
            onError={setError}
          />
        )}
        {loaded && route.kind === "build" && route.step === "pipeline" && (
          <WorkflowsPage
            projects={projects}
            activeProjectId={projectId}
            onActivate={(id) =>
              navigate(`/projects/${encodeURIComponent(id)}/build/pipeline`)
            }
            onRefresh={refresh}
            onNavigate={(step) =>
              navigate(`/projects/${encodeURIComponent(route.projectId)}/build/${step}`)
            }
            onOpenProjects={() => navigate("/projects")}
            onOpenProject={() => openProject(route.projectId)}
            onError={setError}
          />
        )}
        {loaded && route.kind === "build" && route.step !== "pipeline" && (
          <BuildWorkspace
            project={selectedProject}
            step={route.step}
            onNavigate={(step) =>
              navigate(`/projects/${encodeURIComponent(route.projectId)}/build/${step}`)
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
        {loaded && route.kind === "runs" && (
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
        {loaded && route.kind === "review" && (
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
              navigate(section === "general" ? "/settings" : `/settings/${section}`)
            }
            onError={setError}
          />
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
  onNavigate,
  onOpenProjects,
  onOpenProject,
  onRefresh,
  onError,
}: {
  project?: ProjectSummary;
  step: "data" | "labels" | "test";
  onNavigate: (step: BuildStep) => void;
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
        <BuildTestPublish project={project} onNavigate={onNavigate} onRefresh={onRefresh} onError={onError} />
      )}
      {project && summary && allowed && <BuildFooter
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
  const [source, setSource] = useState("");
  const [result, setResult] = useState<Awaited<ReturnType<typeof api.importImages>>>();
  const [busy, setBusy] = useState(false);
  const load = () =>
    api.images(project.id).then((value) => setImages(value.images));
  useEffect(() => {
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
      .removeImage(project.id, image.index)
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
          <button className={images.length === 0 ? "primary" : ""} disabled={busy || !source.trim()} onClick={importImages}>
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
          {images.map((image) => <article key={image.index}>
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
        <button className={project.annotation_schema.length === 0 ? "primary" : ""} disabled={busy || !displayName.trim() || !labels.trim()} onClick={create}>{busy ? "Adding…" : "Add Label group"}</button>
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
  onNavigate,
  onRefresh,
  onError,
}: {
  project: ProjectSummary;
  onNavigate: (step: BuildStep) => void;
  onRefresh: () => Promise<void>;
  onError: (value: string) => void;
}) {
  const [drafts, setDrafts] = useState<WorkflowDraft[]>([]);
  const [draftId, setDraftId] = useState("");
  const [sampleCount, setSampleCount] = useState(3);
  const [report, setReport] = useState<WorkflowDryRunReport>();
  const [images, setImages] = useState<ImageItem[]>([]);
  const [activated, setActivated] = useState<{ workflow_id: string; version: number }>();
  const [busy, setBusy] = useState(false);
  const load = () => api.workflowDrafts(project.id).then((value) => {
    setDrafts(value.drafts);
    const editable = value.drafts.filter((draft) => !["published", "archived"].includes(draft.status));
    setDraftId((current) => editable.some((draft) => draft.id === current) ? current : (editable[0]?.id ?? ""));
  });
  useEffect(() => {
    void Promise.all([load(), api.images(project.id).then((value) => setImages(value.images))])
      .catch((error: Error) => onError(error.message));
  }, [project.id]);
  const test = () => {
    if (!draftId) return;
    setBusy(true);
    void api.dryRunWorkflow(draftId, Array.from({ length: sampleCount }, (_, index) => index))
      .then((value) => { setActivated(undefined); setReport(value); })
      .then(load)
      .catch((error: Error) => onError(error.message))
      .finally(() => setBusy(false));
  };
  const publish = () => {
    if (!draftId || !report?.validation.valid) return;
    setBusy(true);
    void api.publishWorkflow(draftId)
      .then((version) => {
        setActivated(version);
        setDraftId("");
        return onRefresh();
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
  const draftControls = <>
    <select aria-label="Current Draft" value={draftId} onChange={(event) => { setDraftId(event.target.value); setReport(undefined); setActivated(undefined); }}><option value="">Choose Current Draft…</option>{drafts.filter((draft) => !["published", "archived"].includes(draft.status)).map((draft) => <option key={draft.id} value={draft.id}>{draft.name}</option>)}</select>
    <label>Images<input type="number" min="1" max="10" value={sampleCount} onChange={(event) => setSampleCount(Math.max(1, Math.min(10, Number(event.target.value))))} /></label>
    <button className={!report ? "primary" : ""} onClick={test} disabled={busy || !draftId}>{busy ? "Testing…" : "Test samples"}</button>
  </>;
  return (
    <>
      <div className="toolbar-panel">
        <div><span className="eyebrow">Step 4 · Test & Activate</span><h2>Test samples, then activate automation</h2><p>A Sample Test executes 1–10 real Project images in a sandbox and never writes formal annotations. Activation publishes the tested Draft as an immutable Version.</p></div>
        {!report ? <div className="button-row sample-test-controls">{draftControls}</div> : <details className="retest-controls"><summary>Test another sample</summary><div className="sample-test-controls">{draftControls}</div></details>}
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
              <span className="eyebrow">{report.validation.valid ? "Ready to activate" : "Automation needs changes"}</span>
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
              <span>{formatSampleDuration(summary.duration_ms)}</span>
              <span>${summary.usage.estimated_cost} sample cost</span>
            </div>
            <div className="button-row">
              {!report.validation.valid || summary.failed_count > 0 ? <button className="primary" onClick={() => onNavigate("pipeline")}>Fix automation</button> : summary.needs_review_count > 0 ? <button className="primary" onClick={() => document.getElementById("uncertain-results")?.scrollIntoView({ behavior: "smooth", block: "start" })}>Review uncertain result</button> : <button className="primary" onClick={publish} disabled={busy || Boolean(activated)}>{busy ? "Activating…" : "Activate automation"}</button>}
              {report.validation.valid && summary.needs_review_count > 0 && <button onClick={publish} disabled={busy || Boolean(activated)}>{busy ? "Activating…" : "Activate with Review gate"}</button>}
            </div>
            {activated && <div className="activation-success" role="status"><strong>Automation activated</strong><span>Immutable Version v{activated.version} is ready for the full Dataset Run.</span></div>}
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
      ) : <Empty title="No Sample Test result" detail="Choose a Current Draft and test 1–10 images to see result counts, diagnostics, and trace." />}
      <details className="advanced-settings"><summary>Discard this Draft</summary><p>Archiving removes this unpublished Draft from the active Build flow. Published Versions are never changed.</p><button onClick={discard} disabled={busy || !draftId}>Discard unpublished changes</button></details>
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
            ["general", "Provider & budgets"],
            ["models", "Models"],
            ["capabilities", "Capabilities"],
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
      {section === "general" && <SettingsPage onError={onError} />}
      {section === "models" && (
        <ModelsPage
          models={models}
          onConfigure={() => onNavigate("general")}
          onError={onError}
        />
      )}
      {section === "capabilities" && <SkillsPage onError={onError} />}
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
  const projectRuns = runs.filter((run) => run.project_name === project.name);
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
  const guidance = activeWorkspace.guidance;
  const startBatch = () => {
    setStarting(true);
    void api
      .startBatch(
        project.id,
        undefined,
        undefined,
        selectedWorkflow.source.startsWith("published draft")
          ? {
              workflow_id: selectedWorkflow.workflow_id,
              version: Number(selectedWorkflow.version),
            }
          : undefined,
      )
      .then(refreshWorkspace)
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
        <button onClick={() => onNavigate(`/runs?project_id=${encodeURIComponent(project.id)}`)}>Runs</button>
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
      {(project.active_batch || project.active_run || project.last_run) && <div className="run-state-grid" id="project-active-run">
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
            <Empty
              title="No active Run"
              detail="Start is available because the backend has no Pending, Running, or Paused Run for this Project."
            />
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
            <button onClick={saveSkills}>Save Project Skills</button>
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
        <button onClick={() => onNavigate(`/runs?project_id=${encodeURIComponent(project.id)}`)}>Runs</button>
        <button onClick={() => onNavigate(`/review?project_id=${encodeURIComponent(project.id)}`)}>Review</button>
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

function WorkflowsPage({
  projects,
  activeProjectId,
  onActivate,
  onRefresh,
  onNavigate,
  onOpenProjects,
  onOpenProject,
  onError,
}: {
  projects: ProjectSummary[];
  activeProjectId: string;
  onActivate: (id: string) => void;
  onRefresh: () => Promise<void>;
  onNavigate: (step: "data" | "labels" | "pipeline" | "test") => void;
  onOpenProjects: () => void;
  onOpenProject: () => void;
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
  const [showProposalComparison, setShowProposalComparison] = useState(true);
  const [compareLeft, setCompareLeft] = useState("");
  const [compareRight, setCompareRight] = useState("");
  const [advisorKind, setAdvisorKind] = useState<"mock" | "llm">("mock");
  const [templateId, setTemplateId] = useState("");
  const activeProject = projects.find((project) => project.id === activeProjectId);
  const buildSummary = useBuildSummary(activeProject, onError);
  const [targetTaskId, setTargetTaskId] = useState("");
  const [targetLabel, setTargetLabel] = useState("");
  const [busy, setBusy] = useState(false);
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
    void refreshDrafts();
    if (activeProjectId) {
      void api
        .workflowCatalog(activeProjectId)
        .then(setCatalog)
        .catch((error: Error) => onError(error.message));
    } else {
      setCatalog(undefined);
    }
    setReport(undefined);
    setSelectedPublishedKey("");
    setTargetTaskId(activeProject?.annotation_schema[0]?.id ?? "");
    setTargetLabel(activeProject?.annotation_schema[0]?.labels[0] ?? "");
  }, [activeProjectId]);
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
  const runAdvisor = (target?: { task_id: string; label: string }) => {
    if (!activeProjectId)
      return onError("Select a Project before suggesting a Pipeline.");
    setBusy(true);
    void api
      .suggestWorkflow(activeProjectId, advisorKind, target)
      .then((proposal) => {
        setAdvisorProposal(proposal);
        setShowProposalComparison(true);
        return refreshDrafts();
      })
      .catch((error: Error) => onError(error.message))
      .finally(() => setBusy(false));
  };
  const suggest = () => runAdvisor();
  const suggestLabelPipeline = () =>
    targetTaskId && targetLabel
      ? runAdvisor({ task_id: targetTaskId, label: targetLabel })
      : onError("Choose a Project task and target Label first.");
  const targetTask = activeProject?.annotation_schema.find(
    (task) => task.id === targetTaskId,
  );
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
          node_type: "vision_language",
          kind: "vision_language_model",
          depends_on: [],
          inputs: [],
          outputs: [],
          model_binding: "default-vision",
          validators: [],
          refiners: [],
          max_retries: 1,
          review_gate: false,
          parameters: {},
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
  if (activeProject && !buildSummary)
    return <section className="page-stack"><ProjectBreadcrumb project={activeProject} current="Build" onOpenProjects={onOpenProjects} onOpenProject={onOpenProject} /><BuildNavigation step="pipeline" onNavigate={onNavigate} /><div className="loading-banner" role="status">Loading Build readiness…</div></section>;
  if (buildSummary && !buildStepAllowed(buildSummary.guidance, "pipeline"))
    return <section className="page-stack"><ProjectBreadcrumb project={activeProject} current="Build" onOpenProjects={onOpenProjects} onOpenProject={onOpenProject} /><BuildNavigation step="pipeline" guidance={buildSummary.guidance} onNavigate={onNavigate} /><BuildBlocker guidance={buildSummary.guidance} onNavigate={onNavigate} /></section>;
  return (
    <section className="page-stack">
      <ProjectBreadcrumb project={activeProject} current="Build" onOpenProjects={onOpenProjects} onOpenProject={onOpenProject} />
      <BuildNavigation step="pipeline" guidance={buildSummary?.guidance} onNavigate={onNavigate} />
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
          <span className="eyebrow">Contextual suggestion</span>
          <h3>AnnotAgent recommendation</h3>
          <p>{targetLabel ? `Build a registered Automation Recipe for ${targetLabel}, using only Models, Skills, Validators, and Refiners available to this Project.` : "Choose a Label to preview a registry-bounded Automation Recipe."}</p>
          <button className="primary" onClick={suggestLabelPipeline} disabled={busy || !activeProjectId || !targetTaskId || !targetLabel}>{busy ? "Preparing preview…" : "Preview recommendation"}</button>
          <details className="advanced-settings"><summary>Recommendation scope</summary><div className="workflow-advisor-fields">
            <select aria-label="Target task" value={targetTaskId} onChange={(event) => {
              const taskId = event.target.value;
              setTargetTaskId(taskId);
              setTargetLabel(activeProject?.annotation_schema.find((task) => task.id === taskId)?.labels[0] ?? "");
            }}>
              {(activeProject?.annotation_schema ?? []).map((task) => <option key={task.id} value={task.id}>{task.id} · {task.kind}</option>)}
            </select>
            <select aria-label="Target Label" value={targetLabel} onChange={(event) => setTargetLabel(event.target.value)}>
              {(targetTask?.labels ?? []).map((label) => <option key={label} value={label}>{label}</option>)}
            </select>
            <select aria-label="Workflow Advisor" value={advisorKind} onChange={(event) => setAdvisorKind(event.target.value as "mock" | "llm")}>
              <option value="mock">Mock Advisor · offline</option>
              <option value="llm">Workspace LLM Advisor</option>
            </select>
            <button onClick={suggest} disabled={busy || !activeProjectId}>Preview complete Workflow</button>
          </div></details>
        </section>
        <section className="workflow-command-card workflow-version-actions">
          <span className="eyebrow">Current Automation</span>
          <h3>{immutable ? "Immutable Version" : "Autosaved Draft"}</h3>
          <p>{immutable ? "Clone this Version before making changes." : "Edits stay unpublished until you test and activate them in the next step."}</p>
          <div className="button-row">
            {!immutable && <button onClick={discardChanges} disabled={busy || !draft}>Discard</button>}
            {!immutable && <button onClick={() => onNavigate("test")} disabled={busy || !draft}>Open Test &amp; Activate</button>}
            {immutable && <button onClick={clonePublished} disabled={busy || !selected?.workflow.source.startsWith("published draft")}>Clone to Draft</button>}
            {!immutable && draft && <details className="action-menu"><summary>More</summary><div><button onClick={archive} disabled={busy}>Archive</button></div></details>}
          </div>
        </section>
      </div>
      {advisorProposal && (
        <Panel title="Proposed Changes" eyebrow="Advisor preview · Draft only · never activated automatically">
          <div className="advisor-proposal-grid">
            <div>
              <h3>Automation Recipe</h3>
              <ol className="advisor-recipe-list">
                {advisorProposal.draft.nodes.map((node) => <li key={node.id}>{workflowNodeTitle(node.node_type)} <small>{node.model_binding ?? "Core"}</small></li>)}
              </ol>
            </div>
            <div className="fact-grid">
              <Fact label="Model calls / image" value={advisorProposal.estimated_model_calls_per_image} />
              <Fact label="Estimated latency" value={advisorProposal.estimated_latency_ms ? `${advisorProposal.estimated_latency_ms} ms` : "Unresolved"} />
              <Fact label="Cost tier" value={advisorProposal.estimated_cost_tier} />
              <Fact label="Compared with current" value={draft ? `${advisorProposal.draft.nodes.length - draft.nodes.length >= 0 ? "+" : ""}${advisorProposal.draft.nodes.length - draft.nodes.length} nodes` : "No Current Draft"} />
            </div>
          </div>
          {showProposalComparison && <div className="advisor-change-preview" aria-label="Advisor change preview">
            <span className="eyebrow">Compared with Current Draft</span>
            {(draft?.nodes ?? []).filter((node) => !advisorProposal.draft.nodes.some((candidate) => candidate.id === node.id)).map((node) => <div className="removed" key={`removed-${node.id}`}>− {workflowNodeTitle(node.node_type)}</div>)}
            {advisorProposal.draft.nodes.map((node) => <div className={draft?.nodes.some((candidate) => candidate.id === node.id) ? "unchanged" : "added"} key={`proposal-${node.id}`}>{draft?.nodes.some((candidate) => candidate.id === node.id) ? "  " : "+ "}{workflowNodeTitle(node.node_type)}</div>)}
          </div>}
          <TagGroup title="Why" values={advisorProposal.rationale} />
          <TagGroup title="Unresolved bindings" values={advisorProposal.unresolved_model_bindings} />
          <TagGroup title="Warnings" values={advisorProposal.warnings} />
          <TagGroup title="Alternatives" values={advisorProposal.alternatives} />
          {advisorProposal.agent_session && (
            <AgentSessionTrace
              session={advisorProposal.agent_session}
              validation={advisorProposal.agent_validation}
              dryRun={advisorProposal.agent_dry_run}
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
            <button className="primary" onClick={() => { setDraft(advisorProposal.draft); setAdvisorProposal(undefined); }}>Apply to Draft</button>
            <button onClick={() => setShowProposalComparison((value) => !value)}>{showProposalComparison ? "Hide comparison" : "Compare with current"}</button>
            <button onClick={() => setAdvisorProposal(undefined)}>Dismiss proposal</button>
          </div>
        </Panel>
      )}
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
                <ol>{draft.nodes.map((node) => <li key={`recipe-${node.id}`}><strong>{workflowNodeTitle(node.node_type)}</strong><small>{node.model_binding ? `Model · ${node.model_binding}` : "Deterministic Core step"}</small></li>)}</ol>
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
                            {(catalog?.model_registry ?? []).map((model) => (
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
              <div className="workflow-nodes">
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
      {buildSummary && <BuildFooter previous="labels" next="test" nextEnabled={buildStepAllowed(buildSummary.guidance, "test")} nextPrimary={false} onNavigate={onNavigate} />}
    </section>
  );
}

function workflowNodeTitle(nodeType: string): string {
  const known: Record<string, string> = {
    "core.image_input": "Read each image",
    "vlm_detection.detect": "Find object candidates",
    "yolo_detection.detect": "Detect objects with YOLO",
    "classification.classify": "Classify subjects",
    "core.filter": "Keep matching detections",
    "core.crop": "Crop each candidate",
    "core.map_label": "Map model classes to Labels",
    "core.attach_result": "Attach verification results",
    "core.attach_attribute": "Attach attributes",
    "core.confidence_gate": "Apply the auto-accept rule",
    "core.human_review": "Send uncertain results to Review",
    "core.commit": "Save annotations",
    "core.artifact_cache": "Keep replayable artifacts",
  };
  return known[nodeType] ?? nodeType.split(".").at(-1)?.replaceAll("_", " ") ?? nodeType;
}

function pipelineStepTitle(step: PipelineStep, targetLabel?: string): string {
  const label = targetLabel || (Array.isArray(step.parameters.labels) ? step.parameters.labels.join(", ") : "targets");
  if (step.node_type.includes("detect")) return `Find ${label} candidates`;
  if (step.node_type === "core.filter") return `Keep detections labeled ${label}`;
  if (step.node_type === "core.crop") return "Crop each candidate";
  if (step.node_type.includes("classify")) return `Verify each crop as ${label}`;
  if (step.node_type === "core.confidence_gate") return "Automatically accept confident results";
  if (step.kind === "human_review") return "Send uncertain results to Review";
  if (step.kind === "commit") return "Save the annotation";
  return workflowNodeTitle(step.node_type);
}

function pipelineStepDescription(step: PipelineStep, targetLabel?: string): string {
  if (step.node_type === "core.crop")
    return `${Math.round(Number(step.parameters.padding ?? 0) * 100)}% padding around the source detection`;
  if (step.node_type === "core.confidence_gate")
    return `Accept confidence ≥ ${Number(step.parameters.threshold ?? 0).toFixed(2)}; route the rest to Review`;
  if (step.node_type === "core.filter")
    return `Class filter · minimum confidence ${Number(step.parameters.minimum_confidence ?? 0).toFixed(2)}`;
  if (step.model_binding)
    return `Uses ${step.model_binding.model_id} for ${targetLabel || step.model_binding.capability}`;
  if (step.kind === "commit") return "Produces an editable Project annotation";
  return "Deterministic Core processing";
}

function pipelineInputSummary(step: PipelineStep): string {
  const values = Object.values(step.inputs).map((source) =>
    source.source === "image" ? "Image" : source.artifact_type.replaceAll("_", " "),
  );
  return values.join(" + ") || "None";
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
        catalogNode === "core.crop"
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
      const cache: PipelineStep = {
        id: `${prefix}.crop_cache`,
        node_type: "core.artifact_cache",
        kind: "export",
        inputs: {
          crops: {
            source: "step",
            step_id: crop.id,
            port: "crops",
            artifact_type: "crop_set",
          },
        },
        outputs: {},
        parameters: { purpose: "bbox_and_crop_preview" },
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
                  ...pipeline.steps.filter(
                    (step) =>
                      step.id !== gate.id &&
                      step.id !== commit.id &&
                      step.node_type !== "core.crop" &&
                      step.node_type !== "core.artifact_cache",
                  ),
                  crop,
                  cache,
                  gate,
                  commit,
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
        mock_label: selected.target_label,
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
    const cache: PipelineStep = {
      id: `${selected.id}.crop_cache`,
      node_type: "core.artifact_cache",
      kind: "export",
      inputs: {
        crops: {
          source: "step",
          step_id: crop.id,
          port: "crops",
          artifact_type: "crop_set",
        },
      },
      outputs: {},
      parameters: { purpose: "bbox_and_crop_preview" },
      validators: [],
      refiners: [],
      retry_policy: { max_attempts: 1 },
      review_gate: { required: false, allow_manual_override: false },
      resources: {},
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
                    step.node_type !== "core.crop" &&
                    step.node_type !== "core.artifact_cache" &&
                    step.node_type !== "classification.classify" &&
                    step.node_type !== "core.attach_result",
                ),
                crop,
                cache,
                gate,
                commit,
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
            {stage.steps.map((step, stepIndex) => (
              <PipelineStepCard
                key={step.id}
                step={step}
                catalog={catalog}
                immutable={immutable}
                shared
                targetLabel={(stageConsumers(stage).length ? stageConsumers(stage) : composition.label_pipelines).map((pipeline) => pipeline.target_label).join(", ")}
                onConfigure={() => setDrawer({ scope: "shared", stageIndex, stepIndex })}
                onChange={(next) => updateSharedStep(stageIndex, stepIndex, next)}
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
                [
                  "core.crop",
                  "core.filter",
                  "core.map_label",
                  "core.attach_result",
                  "core.attach_attribute",
                  "core.confidence_gate",
                  "classification.classify",
                  "vlm_detection.detect",
                  "yolo_detection.detect",
                ].includes(node.id),
              )
              .map((node) => (
                <option key={node.id} value={node.id}>
                  {node.display_name}
                </option>
              ))}
          </select>
          <button onClick={addCatalogNode} disabled={immutable || !selected}>
            Add Catalog Node
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
            Apply Detect &amp; Crop template
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
            Apply VLM Detect &amp; Crop
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
            {pipeline.steps.map((step, stepIndex) => (
              <PipelineStepCard
                key={step.id}
                step={step}
                catalog={catalog}
                immutable={immutable}
                targetLabel={pipeline.target_label}
                onConfigure={() => setDrawer({ scope: "label", pipelineId: pipeline.id, stepIndex })}
                onChange={(next) =>
                  updatePipelineStep(pipeline.id, stepIndex, next)
                }
                onRemove={() => removePipelineStep(pipeline.id, stepIndex)}
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
  catalog,
  immutable,
  shared = false,
  targetLabel,
  onChange,
  onRemove,
  onConfigure,
}: {
  step: PipelineStep;
  catalog?: WorkflowCatalog;
  immutable: boolean;
  shared?: boolean;
  targetLabel?: string;
  onChange: (step: PipelineStep) => void;
  onRemove?: () => void;
  onConfigure: () => void;
}) {
  const parameterNumber = (name: string) =>
    typeof step.parameters[name] === "number"
      ? String(step.parameters[name])
      : "";
  return (
    <article className="pipeline-step-card">
      <span className="pipeline-step-kind">{shared ? "shared" : step.kind}</span>
      <strong>{pipelineStepTitle(step, targetLabel)}</strong>
      <small>{pipelineStepDescription(step, targetLabel)}</small>
      <div className="pipeline-card-summary">
        <span>Model <strong>{step.model_binding?.model_id ?? "Core"}</strong></span>
        <span>Input <strong>{pipelineInputSummary(step)}</strong></span>
        <span>Output <strong>{Object.values(step.outputs).join(", ").replaceAll("_", " ") || "Annotation"}</strong></span>
        <span>Threshold <strong>{String(step.parameters.threshold ?? step.parameters.minimum_confidence ?? "—")}</strong></span>
        <Status status={immutable ? "published" : "valid"} />
      </div>
      <button onClick={onConfigure}>{immutable ? "Inspect node" : "Configure node"}</button>
      {Object.entries(step.inputs).map(([inputName, source]) => (
        <div className="form-grid" key={inputName}>
          <label>
            {inputName} source node
            <input
              value={source.source === "image" ? "core.image_input" : source.step_id}
              disabled={immutable || source.source === "image"}
              onChange={(event) =>
                source.source !== "image" &&
                onChange({
                  ...step,
                  inputs: {
                    ...step.inputs,
                    [inputName]: { ...source, step_id: event.target.value },
                  },
                })
              }
            />
          </label>
          <label>
            Source port
            <input
              value={source.source === "image" ? "image" : source.port}
              disabled={immutable || source.source === "image"}
              onChange={(event) =>
                source.source !== "image" &&
                onChange({
                  ...step,
                  inputs: {
                    ...step.inputs,
                    [inputName]: { ...source, port: event.target.value },
                  },
                })
              }
            />
          </label>
        </div>
      ))}
      {step.model_binding && (
        <label>
          Model binding
          <select
            value={step.model_binding.model_id}
            disabled={immutable}
            onChange={(event) =>
              onChange({
                ...step,
                model_binding: {
                  ...step.model_binding!,
                  model_id: event.target.value,
                },
              })
            }
          >
            {(catalog?.model_registry ?? [])
              .filter((model) =>
                model.capabilities.includes(step.model_binding!.capability),
              )
              .map((model) => (
                <option key={model.id} value={model.id}>
                  {model.display_name}
                </option>
              ))}
          </select>
        </label>
      )}
      {step.node_type === "core.confidence_gate" && (
        <label>
          Confidence threshold
          <input
            type="number"
            min="0"
            max="1"
            step="0.05"
            value={parameterNumber("threshold")}
            disabled={immutable}
            onChange={(event) =>
              onChange({
                ...step,
                parameters: {
                  ...step.parameters,
                  threshold: Number(event.target.value),
                },
              })
            }
          />
        </label>
      )}
      {step.node_type === "core.crop" && (
        <label>
          Crop padding
          <input
            type="number"
            min="0"
            max="0.5"
            step="0.01"
            value={parameterNumber("padding")}
            disabled={immutable}
            onChange={(event) =>
              onChange({
                ...step,
                parameters: { ...step.parameters, padding: Number(event.target.value) },
              })
            }
          />
        </label>
      )}
      {step.node_type === "core.filter" && (
        <label>
          Minimum confidence
          <input
            type="number"
            min="0"
            max="1"
            step="0.05"
            value={parameterNumber("minimum_confidence")}
            disabled={immutable}
            onChange={(event) =>
              onChange({
                ...step,
                parameters: {
                  ...step.parameters,
                  minimum_confidence: Number(event.target.value),
                },
              })
            }
          />
        </label>
      )}
      <label>
        Fallback node
        <input
          value={step.fallback ?? ""}
          placeholder="none"
          disabled={immutable}
          onChange={(event) =>
            onChange({ ...step, fallback: event.target.value || undefined })
          }
        />
      </label>
      <label>
        Parameters / class mapping (JSON)
        <textarea
          value={JSON.stringify(step.parameters, null, 2)}
          disabled={immutable}
          onChange={(event) => {
            try {
              onChange({
                ...step,
                parameters: JSON.parse(event.target.value) as Record<string, unknown>,
              });
            } catch {
              // Keep the last valid structured configuration while JSON is incomplete.
            }
          }}
        />
      </label>
      {onRemove && step.kind !== "commit" && (
        <button className="danger" onClick={onRemove} disabled={immutable}>
          Remove node
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
  return (
    <div className="drawer-backdrop" role="presentation" onMouseDown={(event) => event.target === event.currentTarget && onClose()}>
      <aside className="node-drawer" role="dialog" aria-modal="true" aria-labelledby="node-drawer-title">
        <header>
          <div><span className="eyebrow">Pipeline node</span><h2 id="node-drawer-title">{step.node_type}</h2><code>{step.id}</code></div>
          <button ref={closeRef} onClick={onClose} aria-label="Close node configuration">Close</button>
        </header>
        <Fact label="Status" value={immutable ? "Published · read only" : "Draft · editable"} />
        <label>Input<input readOnly value={Object.entries(step.inputs).map(([name, source]) => `${name}: ${source.source === "image" ? "Image" : `${source.step_id}.${source.port}`}`).join(" + ") || "None"} /></label>
        <label>Output<input readOnly value={Object.entries(step.outputs).map(([name, type]) => `${name}: ${type}`).join(", ") || "Terminal"} /></label>
        {step.model_binding && (
          <label>Model binding<select value={step.model_binding.model_id} disabled={immutable} onChange={(event) => onChange({ ...step, model_binding: { ...step.model_binding!, model_id: event.target.value } })}>
            {(catalog?.model_registry ?? []).filter((model) => model.capabilities.includes(step.model_binding!.capability)).map((model) => <option key={model.id} value={model.id}>{model.display_name}</option>)}
          </select></label>
        )}
        {step.node_type === "core.confidence_gate" && <label>Confidence threshold<input type="number" min="0" max="1" step="0.05" value={Number(step.parameters.threshold ?? 0)} disabled={immutable} onChange={(event) => updateNumber("threshold", Number(event.target.value))} /></label>}
        {step.node_type === "core.filter" && <label>Minimum confidence<input type="number" min="0" max="1" step="0.05" value={Number(step.parameters.minimum_confidence ?? 0)} disabled={immutable} onChange={(event) => updateNumber("minimum_confidence", Number(event.target.value))} /></label>}
        {step.node_type === "core.crop" && <label>Crop padding<input type="number" min="0" max="0.5" step="0.01" value={Number(step.parameters.padding ?? 0)} disabled={immutable} onChange={(event) => updateNumber("padding", Number(event.target.value))} /></label>}
        <label>Fallback<input value={step.fallback ?? ""} disabled={immutable} placeholder="No fallback" onChange={(event) => onChange({ ...step, fallback: event.target.value || undefined })} /></label>
        <label>Parameters and class mapping<textarea value={parameters} disabled={immutable} onChange={(event) => {
          setParameters(event.target.value);
          try { onChange({ ...step, parameters: JSON.parse(event.target.value) as Record<string, unknown> }); } catch { /* Keep editing until JSON is valid. */ }
        }} /></label>
        <details><summary>Advanced execution</summary>
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
  if (nodeType === "classification.classify")
    return { port: "classifications", type: "classification_set" };
  if (nodeType === "core.attach_result" || nodeType === "core.attach_attribute")
    return { port: "candidates", type: "annotation_candidate_set" };
  if (nodeType === "core.confidence_gate")
    return { port: "candidates", type: "annotation_candidate_set" };
  return { port: "detections", type: "detection_set" };
}

export function pipelineNodeKind(nodeType: string): NonNullable<PipelineStep["kind"]> {
  if (nodeType.includes("classify") || nodeType.includes("detect"))
    return "vision_model";
  if (nodeType === "core.attach_result") return "candidate_merge";
  if (nodeType === "core.confidence_gate") return "gate";
  return "transform";
}

export function pipelineNodeParameters(nodeType: string, label: string) {
  if (nodeType === "core.crop") return { padding: 0.05 };
  if (nodeType === "core.filter")
    return { labels: [label], minimum_confidence: 0.5 };
  if (nodeType === "core.map_label") return { class_mapping: {} };
  if (nodeType === "core.confidence_gate") return { threshold: 0.9 };
  if (nodeType === "classification.classify")
    return { labels: [label], mock_label: label };
  if (nodeType === "vlm_detection.detect")
    return {
      labels: [label],
      object_description: `Locate every visible ${label} and return a tight normalized bounding box.`,
      max_detections: 20,
    };
  return {};
}

function pipelineModelBinding(nodeType: string, catalog?: WorkflowCatalog) {
  const capability = nodeType === "vlm_detection.detect"
    ? "vision_language"
    : nodeType.includes("classify")
    ? "classification"
    : nodeType.includes("detect")
      ? "object_detection"
      : undefined;
  if (!capability) return undefined;
  const model = catalog?.model_registry.find((candidate) =>
    candidate.capabilities.includes(capability),
  );
  return {
    model_id:
      model?.id ??
      (capability === "classification"
        ? "mock-classifier"
        : capability === "vision_language"
          ? "default-vision"
          : "mock-detector"),
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

export function artifactRects(artifacts: PipelineArtifact[]): ArtifactRect[] {
  return artifacts.flatMap((artifact) => {
    if (artifact.kind !== "detection_set") return [];
    const detections = artifact.artifact.detections;
    if (!Array.isArray(detections)) return [];
    return detections.flatMap((detection) => {
      if (!detection || typeof detection !== "object") return [];
      const record = detection as Record<string, unknown>;
      const rect = record.bbox ?? record.rect;
      return parseArtifactRect(rect) ? [parseArtifactRect(rect)!] : [];
    });
  });
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
  return (
    <section className="page-stack">
      <div className="toolbar-panel">
        <div>
          <span className="eyebrow">Provider catalog and bindings</span>
          <h2>Models</h2>
          <p>
            Credentials stay in a private workspace file; Workflows refer to
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
              {catalogModels.map((binding) => (
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
                    {binding.model_version && <small>Version · {binding.model_version}</small>}
                    {binding.license_summary && <small>Weights · {binding.license_summary}</small>}
                    {binding.scope === "workspace_worker" && <div className="worker-actions">
                      <button
                        onClick={() => testWorker(binding.id)}
                        disabled={!binding.enabled || testingModel === binding.id}
                        title={binding.enabled ? "Read live health and capabilities from the Worker" : "Enable this Worker in Provider & budgets first"}
                      >
                        {testingModel === binding.id ? "Testing…" : "Test Worker"}
                      </button>
                      <button
                        disabled
                        title={testResults[binding.id]
                          ? testResults[binding.id].capabilities.supports_visual_prompt
                            ? "Worker reports support, but visual-prompt Workflow editing is not implemented in this Alpha"
                            : "Worker reports supports_visual_prompt=false; this Alpha accepts text queries only"
                          : "Test Worker to discover visual prompt capability"}
                      >Visual prompt</button>
                    </div>}
                    {testResults[binding.id] && <div className="worker-discovery" role="status">
                      <strong>Live · {testResults[binding.id].health.status}</strong>
                      <small>{testResults[binding.id].capabilities.capabilities.join(" · ")}</small>
                      <small>Confidence {testResults[binding.id].capabilities.score_semantics.replaceAll("_", " ")}</small>
                      {!testResults[binding.id].capabilities.supports_visual_prompt && <small>Visual prompt unavailable in this Worker</small>}
                    </div>}
                  </div>
                </article>
              ))}
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
            {PROVIDER_PRESETS.filter((preset) => !preset.offline).map(
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
  route: Extract<WorkspaceRoute, { kind: "runs" }>;
  onNavigate: (path: string, replace?: boolean) => void;
  onRefresh: () => Promise<void>;
  onError: (value: string) => void;
}) {
  const run = runs.find((item) => item.id === route.runId);
  if (route.runId && run)
    return (
      <RunDetailWorkspace
        run={run}
        project={projects.find((item) => item.name === run.project_name)}
        route={route}
        onNavigate={onNavigate}
        onRefresh={onRefresh}
        onError={onError}
      />
    );
  const projectRuns = runsForContext(runs, scopeProject);
  const statusFilter = route.status ?? "all";
  const visibleRuns = runsForContext(runs, scopeProject, statusFilter);
  const setListFilters = (projectId: string, status: string) => {
    const params = new URLSearchParams();
    if (projectId) params.set("project_id", projectId);
    if (status !== "all") params.set("status", status);
    onNavigate(`/runs${params.size ? `?${params.toString()}` : ""}`);
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
      <Panel title="Run history" eyebrow={`${visibleRuns.length} visible · ${runs.length} recorded`}>
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
              {[...new Set(projectRuns.map((item) => item.status))].map((status) => <option key={status} value={status}>{status.replaceAll("_", " ")}</option>)}
            </select>
          </label>
        </div>
        <div className="runs-table">
          {visibleRuns.map((item) => (
            <button key={item.id} className="run-row" onClick={() => onNavigate(`/runs/${item.id}`)}>
              <span className="event-rail" />
              <div><strong>{item.project_name}</strong><small>{item.workflow_name}@v{item.workflow_version}</small><code>{item.model_identity} · {item.artifact_count} Artifacts</code>{item.terminal_reason && <small className="run-reason">{item.terminal_reason}</small>}</div>
              <div className="run-usage"><span>{(item.input_tokens + item.output_tokens).toLocaleString()} tokens</span><span>${item.cost}</span></div>
              <Status status={item.status} />
              <span className="row-arrow" aria-hidden="true">→</span>
            </button>
          ))}
          {visibleRuns.length === 0 && <Empty title="No matching runs" detail="Change the explicit Project or status filter to see more Run history." />}
        </div>
      </Panel>
      {route.runId && !run && <Empty title="Run not found" detail="The linked Run is not available in this workspace." />}
    </section>
  );
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
  route: Extract<WorkspaceRoute, { kind: "runs" }>;
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
      const params = new URLSearchParams({ view: "debug" });
      const imageIndex = inspection.image_index ?? annotationInspection?.image_index;
      if (imageIndex !== undefined) params.set("image", String(imageIndex));
      params.set("node", inspection.nodes[0].node_id);
      onNavigate(`/runs/${run.id}?${params.toString()}`, true);
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
    const params = new URLSearchParams();
    params.set("view", "debug");
    params.set("image", String(context.image ?? selectedImageIndex));
    if (context.node ?? selectedNode?.node_id) params.set("node", context.node ?? selectedNode!.node_id);
    if (context.artifact) params.set("artifact", context.artifact);
    onNavigate(`/runs/${run.id}?${params.toString()}`);
  };
  const setView = (next: "results" | "debug") => {
    const params = new URLSearchParams();
    if (next === "debug") params.set("view", "debug");
    if (runImageIndex !== undefined) params.set("image", String(selectedImageIndex));
    if (next === "debug" && selectedNode) params.set("node", selectedNode.node_id);
    const query = params.size ? `?${params.toString()}` : "";
    onNavigate(`/runs/${run.id}${query}`);
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
  const setResultImage = (image: number) => onNavigate(`/runs/${run.id}?image=${image}`);
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
      <button className="text-button run-back" onClick={() => onNavigate("/runs")}>← Run history</button>
      <nav className="run-view-tabs" aria-label="Run workspace view">
        <button className={view === "results" ? "active" : ""} aria-current={view === "results" ? "page" : undefined} onClick={() => setView("results")}>Results</button>
        <button className={view === "debug" ? "active" : ""} aria-current={view === "debug" ? "page" : undefined} onClick={() => setView("debug")}>Debug</button>
      </nav>
      <div className="toolbar-panel run-detail-header">
        {view === "results" ? <div><span className="eyebrow">{run.project_name} · {run.workflow_name}@v{run.workflow_version}</span><h2>{resultHeadline}</h2><div className="context-line"><Status status={run.status} /><span>{formatSampleDuration(resultSummary?.duration_ms ?? duration)}</span><span>${resultSummary?.usage.estimated_cost ?? run.cost}</span></div></div> : <div><span className="eyebrow">Debug · Run {run.id.slice(0, 8)}</span><h2>{run.workflow_name}@v{run.workflow_version}</h2><div className="context-line"><Status status={run.status} /><span>{nodeProgress}</span><span>{run.artifact_count} Artifacts</span><span>{(run.input_tokens + run.output_tokens).toLocaleString()} tokens</span><span>${run.cost}</span></div></div>}
        <div className="button-row">
          {runReview && <button onClick={() => onNavigate(`/review/${runReview.id}`)}>Review {resultSummary?.needs_review_count || 1} result</button>}
          {!runReview && Boolean(resultSummary?.needs_review_count) && project && <button onClick={() => onNavigate(`/review?project_id=${encodeURIComponent(project.id)}`)}>Open Review inbox</button>}
          {run.status === "running" && <button disabled={busy} onClick={() => control("pause")}>Pause</button>}
          {run.status === "paused" && <button disabled={busy} onClick={() => control("resume")}>Resume</button>}
          {run.controllable && <button className="danger" disabled={busy} onClick={() => control("cancel")}>Cancel</button>}
        </div>
      </div>
      {view === "results" ? <>
        <dl className="run-result-metrics" aria-label="Run result summary">
          <div><dt>Images</dt><dd>{resultSummary?.image_count ?? 1}</dd><small>processed</small></div>
          <div><dt>Results found</dt><dd>{resultSummary?.result_count ?? runAnnotations.length}</dd><small>{resultSummary?.ready_count ?? 0} ready</small></div>
          <div><dt>Needs review</dt><dd>{resultSummary?.needs_review_count ?? (runReview ? 1 : 0)}</dd><small>{resultSummary?.no_target_count ?? 0} no-target · {resultSummary?.failed_count ?? 0} failed</small></div>
        </dl>
        <div className="run-results-workspace">
          <aside className="panel run-image-browser"><span className="eyebrow">Images</span><input aria-label="Search run images" value={search} onChange={(event) => setSearch(event.target.value)} placeholder="Search images" /><select aria-label="Image status filter" defaultValue="all"><option value="all">All statuses</option><option value={run.status}>{run.status.replaceAll("_", " ")}</option></select>
            <div>{visibleImages.filter((image) => runImageIndex === undefined || image.index === runImageIndex).map((image) => <button key={image.index} className={image.index === selectedImageIndex ? "active" : ""} onClick={() => setResultImage(image.index)}><img src={image.url} alt="" /><span><strong>{image.name}</strong><small>{resultSummary?.failed_count ? "Failed" : resultSummary?.no_target_count ? "No target found" : resultSummary?.needs_review_count ? "Needs review" : "Ready"}</small></span></button>)}</div>
          </aside>
          <main className="panel run-visual-workspace run-result-preview"><span className="eyebrow">Result Preview</span>{resultSummary?.labels.length ? <div className="run-result-labels" aria-label="Result labels">{resultSummary.labels.map((item) => <span key={item.label}>{item.label}<b>{item.count}</b></span>)}</div> : null}{canPreview && (resultSummary?.result_count ?? runAnnotations.length) > 0 ? <RunArtifactCanvas projectId={previewProjectId!} project={project} artifacts={previewArtifacts} annotations={runAnnotations} imageIndex={selectedImageIndex} /> : resultSummary ? <Empty title={resultSummary.no_target_count ? "No target found" : resultSummary.failed_count ? "No result produced" : "No visual result"} detail={resultSummary.no_target_count ? "The automation completed successfully and found no matching target in this image." : resultSummary.failed_count ? "Open Debug to inspect the failed step and available repair action." : "This result has no bounding-box or Crop preview."} /> : <Empty title="Loading results" detail="Reading persisted Annotations and result Artifacts." />}</main>
          <aside className="panel run-needs-attention"><span className="eyebrow">Needs Attention</span>{runReview ? <><h3>{resultSummary?.needs_review_count || 1} result needs a decision</h3><p>{runReview.review_reason.replaceAll("_", " ")}</p><button className="primary" onClick={() => onNavigate(`/review/${runReview.id}`)}>Review result</button></> : resultSummary?.needs_review_count && project ? <><h3>{resultSummary.needs_review_count} result needs a decision</h3><p>Open the Project Review inbox to inspect the uncertain result.</p><button className="primary" onClick={() => onNavigate(`/review?project_id=${encodeURIComponent(project.id)}`)}>Open Review inbox</button></> : resultSummary?.failed_count ? <><h3>Run needs repair</h3><p>{run.terminal_reason ?? "A Pipeline step did not produce a usable result."}</p><button className="primary" onClick={() => setView("debug")}>Open Debug</button></> : <div className="positive-empty"><strong>No results need attention</strong><span>{resultSummary?.no_target_count ? "The empty result is valid." : "All results passed the configured gates."}</span></div>}</aside>
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
          {selectedNode.error && <div className="run-repair-card"><div><strong>{selectedNode.error.code}</strong><p>{selectedNode.error.summary}</p></div><div className="button-row">{selectedNode.error.retryable && <button className="primary" disabled={busy} onClick={replayNode}>Replay failed step</button>}{project && <button onClick={() => onNavigate(`/projects/${encodeURIComponent(project.id)}/build/pipeline`)}>Fix automation</button>}</div></div>}
          <div className="node-payload-sections">
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
};

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

export function artifactDetectionMarks(
  artifacts: PipelineArtifact[],
  project?: ProjectSummary,
): ArtifactMark[] {
  return artifacts.flatMap((artifact) => {
    if (artifact.kind !== "detection_set") return [];
    const detections = artifact.artifact.detections;
    const reference = artifact.artifact.reference as Record<string, unknown> | undefined;
    if (!Array.isArray(detections)) return [];
    return detections.flatMap((value, index) => {
      if (!value || typeof value !== "object") return [];
      const detection = value as Record<string, unknown>;
      const rect = parseArtifactRect(detection.bbox ?? detection.rect);
      if (!rect) return [];
      const label = typeof detection.project_label === "string"
        ? detection.project_label
        : typeof detection.model_label === "string"
          ? detection.model_label
          : typeof detection.label === "string"
            ? detection.label
            : typeof detection.class_id === "string"
              ? detection.class_id
              : "detection";
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
  return marks.filter(
    (mark, index) => !marks.slice(0, index).some((existing) => sameMark(existing, mark)),
  );
}

function RunArtifactCanvas({ projectId, project, artifacts, annotations, imageIndex }: { projectId: string; project?: ProjectSummary; artifacts: PipelineArtifact[]; annotations: Annotation[]; imageIndex: number }) {
  const imageUrl = `/api/projects/${projectId}/images/${imageIndex}/content`;
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
  const imageStage = (showResults: boolean, label: string) => <div className="canvas-pan"><div className="artifact-image-stage" style={{ transform: `scale(${zoom})` }}><img src={imageUrl} alt={label} />{showResults && <svg viewBox="0 0 100 100" preserveAspectRatio="none" aria-label="Annotation overlay">{detections.map((rect) => <g key={rect.id} role="button" tabIndex={0} aria-label={`${rect.label} ${rect.confidence === undefined ? "" : `${Math.round(rect.confidence * 100)} percent`}`} className={rect.id === selectedId ? "selected" : ""} onClick={() => setSelectedId(rect.id)}><rect style={{ stroke: rect.color }} x={rect.x * 100} y={rect.y * 100} width={rect.width * 100} height={rect.height * 100} /><text x={rect.x * 100} y={Math.max(3, rect.y * 100 - 1)}>{rect.label} {rect.confidence === undefined ? "" : `${Math.round(rect.confidence * 100)}%`}</text></g>)}</svg>}</div></div>;
  return (
    <div className="run-artifact-canvas" tabIndex={0} onKeyDown={(event) => { if (event.key === "ArrowRight" || event.key === "ArrowDown") { event.preventDefault(); selectOffset(1); } if (event.key === "ArrowLeft" || event.key === "ArrowUp") { event.preventDefault(); selectOffset(-1); } }}>
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
      {legend.length > 0 && <div className="bbox-legend" aria-label="Annotation color legend">{legend.map((item) => <span key={item.label}><i style={{ background: item.color }} />{item.label}</span>)}</div>}
      {detections.length > 0 && <ul className="canvas-annotation-list" aria-label="Run result annotations">{detections.map((item) => <li key={item.id}><button aria-pressed={item.id === selectedId} onClick={() => setSelectedId(item.id)}><i aria-hidden="true" style={{ borderColor: item.color }} /><span><strong>{item.label}</strong><small>Bounding box{item.confidence === undefined ? "" : ` · ${Math.round(item.confidence * 100)}%`}</small></span></button></li>)}</ul>}
      {mode === "original" ? imageStage(false, "Original Run input") : mode === "result" ? imageStage(true, "Run result") : mode === "compare" ? <div className="run-result-compare"><section><span>Original</span>{imageStage(false, "Original Run input")}</section><section><span>Result</span>{imageStage(true, "Run result")}</section></div> : (
        <div className="crop-preview-list enlarged">{crops.map((crop, index) => <button className={crop.parentId === selectedId ? "selected" : ""} key={crop.id} onClick={() => setSelectedId(crop.parentId ?? crop.id)}><svg style={{ transform: `scale(${zoom})` }} viewBox={`${crop.x * 100} ${crop.y * 100} ${crop.width * 100} ${crop.height * 100}`} aria-label={`Crop ${index + 1}: ${crop.label}`}><image href={imageUrl} x="0" y="0" width="100" height="100" /></svg><span><strong>{crop.label}</strong>{crop.confidence !== undefined && <small>{Math.round(crop.confidence * 100)}%</small>}<small>Parent: {crop.parentArtifact?.slice(0, 8) ?? crop.parentId ?? "Unknown"}</small><small>Source: {crop.sourceNode ?? "Unknown"}</small></span></button>)}</div>
      )}
    </div>
  );
}

const GENERIC_REVIEW_REASONS = [
  { value: "not_target", label: "Not the target" },
  { value: "wrong_box", label: "Wrong box" },
  { value: "duplicate", label: "Duplicate" },
  { value: "wrong_label", label: "Wrong label" },
  { value: "other", label: "Other" },
] as const;

function reviewReasonExplanation(item: ReviewItem) {
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
  route: Extract<WorkspaceRoute, { kind: "review" }>;
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
  const [rejectReason, setRejectReason] = useState("not_target");
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
  const visibleReviews = scopedProject
    ? reviews.filter(
        (review) =>
          review.project_id === scopedProject.id ||
          (!review.project_id && review.project_name === scopedProject.name),
      )
    : reviews;
  const selected =
    routeReview ?? visibleReviews.find((review) => review.id === selectedId) ?? visibleReviews[0];
  const reviewProject = projectForReview(projects, selected) ?? scopedProject;
  const reviewHref = (reviewId: string) =>
    `/review/${reviewId}${route.projectId ? `?project_id=${encodeURIComponent(route.projectId)}` : ""}`;
  const refresh = () => {
    setQueueLoaded(false);
    return api
      .reviews(route.projectId)
      .then((value) => {
        setReviews(value.reviews);
        setProgress(value.progress);
        const first = value.reviews[0];
        if (!value.reviews.some((review) => review.id === selectedId) && first)
          onNavigate(reviewHref(first.id), true);
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
        setReason("manual_edit");
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
    onNavigate(reviewHref(item.id));
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
        onNavigate(reviewHref(outcome.next_review.id), true);
      } else {
        setSelectedId("");
        onNavigate(`/review?project_id=${encodeURIComponent(reviewProject.id)}`, true);
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
                ? `/review?project_id=${encodeURIComponent(event.target.value)}`
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
                onNavigate(reviewHref(review.id));
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
                  {Math.round((review.annotation.confidence ?? 0) * 100)}%
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
              <p>{reviewReasonExplanation(selected)}</p>
            </div>
            <dl className="review-essential-facts">
              <div><dt>Confidence</dt><dd>{Math.round((selected.confidence ?? draft.confidence ?? 0) * 100)}%</dd></div>
              <div><dt>Source Run</dt><dd>{selected.run_id.slice(0, 8)}</dd></div>
              <div><dt>Automation Version</dt><dd>{selected.workflow_id ? `${selected.workflow_id}@v${selected.workflow_version}` : `v${selected.workflow_version}`}</dd></div>
              <div><dt>Source Step</dt><dd>{selected.source_node ?? "Unknown"}</dd></div>
            </dl>
            <button onClick={() => onNavigate(`/runs/${selected.run_id}?node=${encodeURIComponent(selected.source_node ?? "")}${selected.source_artifact_id ? `&artifact=${encodeURIComponent(selected.source_artifact_id)}` : ""}`)}>Open run context</button>
            {editing && <section className="review-edit-details" aria-label="Annotation edit details">
              <div><span className="eyebrow">Manual correction</span><strong>Edit result</strong></div>
              <label>
                Label
                <input value={draft.label ?? ""} onChange={(event) => edit({ ...draft, label: event.target.value })} />
              </label>
              <label>
                Correction reason
                <select aria-label="Correction reason" value={reason} onChange={(event) => setReason(event.target.value)}>
                  <option value="manual_edit">Manual edit</option>
                  {GENERIC_REVIEW_REASONS.filter((option) => option.value === "wrong_box" || option.value === "wrong_label" || option.value === "other").map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}
                  {skillReasonOptions.map((option) => <option key={`${option.skillId}:${option.value}`} value={option.value}>{option.label}</option>)}
                </select>
              </label>
              <label>
                Reviewer note
                <textarea value={note} onChange={(event) => setNote(event.target.value)} placeholder="What changed, and why?" />
              </label>
              {hasUnsavedAnnotationChanges && <div className="correction-impact" role="status">
                <strong>Correction impact</strong>
                <span>This correction will make similar candidates more likely to be reviewed.</span>
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

function AgentSessionTrace({
  session,
  validation,
  dryRun,
  onCancel,
}: {
  session: AgentSession;
  validation?: WorkflowDryRunReport["validation"];
  dryRun?: WorkflowDryRunReport;
  onCancel?: () => void;
}) {
  const cancellable = ["running", "waiting_for_human"].includes(session.status);
  return (
    <div className="agent-session-trace" aria-label={`${session.kind} Agent trace`}>
      <div className="context-line">
        <strong>{session.kind.replaceAll("_", " ")}</strong>
        <Status status={session.status} />
        <span>{session.usage.tool_calls} tool calls</span>
        <span>{session.usage.input_tokens + session.usage.output_tokens} tokens</span>
        <span>${session.usage.cost}</span>
        {onCancel && (
          <button className="danger" disabled={!cancellable} onClick={onCancel}>
            Cancel Agent
          </button>
        )}
      </div>
      <div className="fact-grid">
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
      </div>
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

function SettingsPage({ onError }: { onError: (value: string) => void }) {
  const [settings, setSettings] = useState<Record<string, any>>();
  const [savedSignature, setSavedSignature] = useState("");
  const [presetId, setPresetId] = useState("mock");
  const [key, setKey] = useState("");
  const [message, setMessage] = useState("");
  const [saving, setSaving] = useState(false);
  const credentialPresetRef = useRef("custom");
  useEffect(() => {
    void api
      .settings()
      .then((value) => {
        setSettings(value);
        setSavedSignature(JSON.stringify(value));
        setPresetId(inferProviderPreset(value).id);
        credentialPresetRef.current = inferConfiguredProviderPreset(value).id;
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
  const provider = settings.provider ?? {};
  const pricing = settings.pricing ?? {};
  const budget = settings.budget ?? {};
  const detectionWorkers = Array.isArray(settings.detection_workers)
    ? settings.detection_workers
    : [];
  const preset = getProviderPreset(presetId);
  const providerChanged =
    !preset.offline && credentialPresetRef.current !== preset.id;
  const customModel =
    !preset.custom &&
    !preset.offline &&
    !isCatalogModel(preset, provider.model);
  const setProvider = (field: string, value: unknown) =>
    setSettings({ ...settings, provider: { ...provider, [field]: value } });
  const setDetectionWorker = (index: number, field: string, value: unknown) =>
    setSettings({
      ...settings,
      detection_workers: detectionWorkers.map((worker: Record<string, unknown>, workerIndex: number) =>
        workerIndex === index ? { ...worker, [field]: value } : worker,
      ),
    });
  const chooseProvider = (id: string) => {
    setPresetId(id);
    setSettings(applyProviderPreset(settings, id));
    setKey("");
    setMessage("");
  };
  const finish = (
    value: Record<string, unknown>,
    nextMessage: string,
    updateCredential = false,
  ) => {
    setSettings(value);
    setSavedSignature(JSON.stringify(value));
    setKey("");
    setMessage(nextMessage);
    if (updateCredential && !preset.offline)
      credentialPresetRef.current = preset.id;
  };
  const save = () => {
    setSaving(true);
    const clearMismatchedKey =
      providerChanged && settings.api_key_persisted && !key;
    void api
      .saveSettings({
        ...settings,
        api_key: key || undefined,
        clear_saved_api_key: clearMismatchedKey || undefined,
      })
      .then((value) =>
        finish(
          value,
          clearMismatchedKey
            ? `Saved ${preset.shortLabel}. The previous provider key was removed; add a ${preset.shortLabel} key before running.`
            : `Saved ${preset.shortLabel} locally. Future runs will use this workspace model binding.`,
          true,
        ),
      )
      .catch((error: Error) => onError(error.message))
      .finally(() => setSaving(false));
  };
  const clearKey = () => {
    setSaving(true);
    void api
      .saveSettings({ ...settings, clear_saved_api_key: true })
      .then((value) =>
        finish(value, "Saved API key removed from the workspace.", true),
      )
      .catch((error: Error) => onError(error.message))
      .finally(() => setSaving(false));
  };
  const dirty = Boolean(key) || JSON.stringify(settings) !== savedSignature;
  return (
    <section className="settings-grid">
      <Panel title="Vision model provider" eyebrow="Workspace default binding">
        <label>
          Provider
          <select
            value={presetId}
            onChange={(event) => chooseProvider(event.target.value)}
          >
            {PROVIDER_PRESETS.map((candidate) => (
              <option key={candidate.id} value={candidate.id}>
                {candidate.label}
              </option>
            ))}
          </select>
        </label>
        <div className={`provider-summary ${preset.offline ? "offline" : ""}`}>
          <span className="provider-monogram" aria-hidden="true">
            {preset.shortLabel.slice(0, 2).toUpperCase()}
          </span>
          <span>
            <strong>{preset.shortLabel}</strong>
            <small>{preset.description}</small>
          </span>
          {preset.docsUrl && (
            <a href={preset.docsUrl} target="_blank" rel="noreferrer">
              Provider docs ↗
            </a>
          )}
        </div>
        {!preset.offline && (
          <>
            {preset.custom ? (
              <div className="form-grid">
                <label>
                  Endpoint
                  <input
                    type="url"
                    value={provider.endpoint ?? ""}
                    onChange={(event) =>
                      setProvider("endpoint", event.target.value)
                    }
                    placeholder="https://provider.example/v1"
                  />
                </label>
                <label>
                  Model
                  <input
                    value={provider.model ?? ""}
                    onChange={(event) =>
                      setProvider("model", event.target.value)
                    }
                    placeholder="vision-model-id"
                  />
                </label>
              </div>
            ) : (
              <>
                <label>
                  Vision model
                  <select
                    value={customModel ? CUSTOM_MODEL : (provider.model ?? "")}
                    onChange={(event) =>
                      setProvider(
                        "model",
                        event.target.value === CUSTOM_MODEL
                          ? ""
                          : event.target.value,
                      )
                    }
                  >
                    {preset.models.map((model) => (
                      <option key={model.id} value={model.id}>
                        {model.label} — {model.hint}
                      </option>
                    ))}
                    <option value={CUSTOM_MODEL}>Another model ID…</option>
                  </select>
                </label>
                {customModel && (
                  <label>
                    Custom model ID
                    <input
                      autoFocus
                      value={provider.model ?? ""}
                      onChange={(event) =>
                        setProvider("model", event.target.value)
                      }
                      placeholder="Enter the exact model ID"
                    />
                  </label>
                )}
              </>
            )}
            {providerChanged && settings.api_key_persisted && !key && (
              <div className="credential-notice" role="status">
                The saved key belongs to the previous provider. Paste your{" "}
                {preset.shortLabel} key now, or saving will safely remove the
                old key.
              </div>
            )}
            <label>
              {preset.shortLabel} API key
              <input
                type="password"
                autoComplete="new-password"
                value={key}
                onChange={(event) => setKey(event.target.value)}
                placeholder={
                  settings.api_key_persisted && !providerChanged
                    ? "Stored in a private workspace file · paste to replace"
                    : `Paste your ${preset.shortLabel} key once`
                }
              />
            </label>
            <div className="button-row">
              <button
                onClick={clearKey}
                disabled={saving || !settings.api_key_persisted}
              >
                Clear saved key
              </button>
              <small>
                {settings.api_key_persisted && !providerChanged
                  ? "Workspace-local 0600 file · never returned by the API"
                  : `Environment fallback: ${provider.api_key_env ?? "ANNOTAGENT_API_KEY"}`}
              </small>
            </div>
            <details className="advanced-settings">
              <summary>Advanced settings</summary>
              <div className="form-grid">
                {!preset.custom && (
                  <label>
                    Endpoint
                    <input readOnly value={provider.endpoint ?? ""} />
                  </label>
                )}
                <label>
                  API key environment
                  <input
                    value={provider.api_key_env ?? ""}
                    onChange={(event) =>
                      setProvider("api_key_env", event.target.value)
                    }
                  />
                </label>
                <label>
                  Temperature
                  <input
                    type="number"
                    min="0"
                    max="2"
                    step="0.05"
                    value={provider.temperature ?? 0.1}
                    onChange={(event) =>
                      setProvider("temperature", Number(event.target.value))
                    }
                  />
                </label>
                <label>
                  Timeout seconds
                  <input
                    type="number"
                    min="1"
                    value={provider.request_timeout_seconds ?? 120}
                    onChange={(event) =>
                      setProvider(
                        "request_timeout_seconds",
                        Number(event.target.value),
                      )
                    }
                  />
                </label>
                <label>
                  Max output tokens
                  <input
                    type="number"
                    min="1"
                    value={provider.max_output_tokens ?? 4096}
                    onChange={(event) =>
                      setProvider(
                        "max_output_tokens",
                        Number(event.target.value),
                      )
                    }
                  />
                </label>
                <label>
                  Retries
                  <input
                    type="number"
                    min="0"
                    value={provider.max_retries ?? 2}
                    onChange={(event) =>
                      setProvider("max_retries", Number(event.target.value))
                    }
                  />
                </label>
              </div>
              <small>
                Protocol: OpenAI Chat Completions · image input + function tools
              </small>
            </details>
          </>
        )}
        {preset.offline && (
          <div className="offline-note">
            Ready to run immediately. Mock keeps your real provider
            configuration and saved key untouched.
          </div>
        )}
        {settings.credential_store_error && (
          <div className="error-banner" role="alert">
            <span>
              Local credential storage unavailable:{" "}
              {String(settings.credential_store_error)}
            </span>
          </div>
        )}
      </Panel>
      <Panel title="Detection Workers" eyebrow="Optional local model processes">
        {detectionWorkers.length ? <div className="detection-worker-settings">
          {detectionWorkers.map((worker: Record<string, any>, index: number) => <article key={String(worker.id)}>
            <div className="worker-setting-heading">
              <span><strong>{String(worker.display_name)}</strong><small>{String(worker.model_id)}</small></span>
              <label className="checkbox-line"><input type="checkbox" checked={Boolean(worker.enabled)} onChange={(event) => setDetectionWorker(index, "enabled", event.target.checked)} /><span>Enabled</span></label>
            </div>
            <label>Worker URL<input type="url" value={String(worker.base_url ?? "")} onChange={(event) => setDetectionWorker(index, "base_url", event.target.value)} /></label>
            <div className="worker-contract-summary">
              <small>Expected contract · {(worker.expected_capabilities ?? []).join(" · ")}</small>
              <small>Score · {String(worker.score_semantics ?? "unknown").replaceAll("_", " ")}</small>
              <small>Version · {String(worker.version?.model_version ?? "unversioned")}</small>
            </div>
            <label className="checkbox-line"><input type="checkbox" checked={Boolean(worker.allow_remote)} onChange={(event) => setDetectionWorker(index, "allow_remote", event.target.checked)} /><span>Allow remote HTTPS Worker</span></label>
            <small>Loopback is the default trust boundary. Live capabilities are read from the Worker on the Models page; expected values here are validation constraints.</small>
          </article>)}
        </div> : <Empty title="No Detection Workers" detail="This Alpha ships an optional local LocateAnything profile in new workspaces." />}
      </Panel>
      <Panel title="Pricing & hard budgets" eyebrow="Exact decimal accounting">
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
      </Panel>
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
          <button
            className="primary"
            onClick={save}
            disabled={
              saving ||
              (!preset.offline && (!provider.endpoint || !provider.model))
            }
          >
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
  const [localModels, setLocalModels] = useState("");
  const [settings, setSettings] = useState<Record<string, any>>();
  const [providerId, setProviderId] = useState("mock");
  const [customModel, setCustomModel] = useState("");
  const [apiKey, setApiKey] = useState("");
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
    void api.settings()
      .then((value) => {
        setSettings(value);
        setProviderId(inferProviderPreset(value).id);
      })
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
  const preset = getProviderPreset(offlineOnly ? "mock" : providerId);
  const provider = settings?.provider ?? {};
  const selectedModel = provider.model === CUSTOM_MODEL ? customModel.trim() : provider.model;
  const modelConnected =
    preset.offline ||
    (Boolean(selectedModel) && (settings?.api_key_persisted || Boolean(apiKey.trim())));
  const chooseProvider = (id: string) => {
    setProviderId(id);
    setSettings((current) => {
      const next = applyProviderPreset(current ?? {}, id);
      const providerPreset = getProviderPreset(id);
      return providerPreset.offline
        ? next
        : {
            ...next,
            provider: {
              ...next.provider,
              model: providerPreset.models[0]?.id ?? next.provider?.model,
            },
          };
    });
  };
  const finish = async (customize: boolean) => {
    if (!settings || !projectName.trim() || !labelName.trim()) return;
    setBusy(true);
    setProgress("Saving model connection…");
    try {
      let configured = offlineOnly ? applyProviderPreset(settings, "mock") : settings;
      if (!preset.offline) {
        configured = {
          ...configured,
          provider: {
            ...configured.provider,
            model: selectedModel || preset.models[0]?.id,
          },
          ...(apiKey.trim() ? { api_key: apiKey.trim() } : {}),
        };
      }
      await api.saveSettings(configured);
      setProgress("Creating the Project…");
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
        await api.suggestWorkflow(
          resolvedWorkspaceId,
          "mock",
          { task_id: resolvedTaskId, label: resolvedLabelId },
          {
            max_cost_per_image: maximumCost.trim() || undefined,
            max_latency_ms: priority === "faster" ? 1_000 : priority === "accuracy" ? 10_000 : 4_000,
            minimum_accuracy: priority === "faster" ? 0.75 : priority === "accuracy" ? 0.92 : 0.85,
            require_review_gate: Number(targetReviewRate) > 0,
          },
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
    (step === 1 && (!projectName.trim() || !labelName.trim())) ||
    (step === 4 && (!settings || !modelConnected));
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
            <label>Available local models<input value={localModels} onChange={(event) => setLocalModels(event.target.value)} placeholder="Optional model IDs" /></label>
            <label className="check-row"><input type="checkbox" checked={offlineOnly} onChange={(event) => setOfflineOnly(event.target.checked)} /> Offline only</label>
          </div></details>
        </div>}

        {step === 4 && <div className="wizard-step">
          <div className="recommendation-card">
            <span className="status status-auto-accepted">Recommended</span>
            <h3>{kind === "classification" ? `Classify each image as ${labelName}` : kind === "semantic_mask" ? `Segment ${labelName} regions` : `Find ${labelName} candidates`}</h3>
            <ol>
              <li>Use <strong>{preset.offline ? "the deterministic Mock model" : selectedModel || preset.models[0]?.label}</strong> through the registered model binding.</li>
              {kind === "bounding_box" && <li>Keep the detector output as editable bounding boxes.</li>}
              <li>Automatically accept high-confidence results.</li>
              <li>Send uncertain results to Review.</li>
            </ol>
            <div className="recommendation-estimate"><span><b>{priority === "faster" ? "Low" : priority === "accuracy" ? "Higher" : "Medium"}</b> latency</span><span><b>Low</b> setup effort</span><span><b>{targetReviewRate || "10"}%</b> target review</span></div>
          </div>
          <div className="inline-model-connection">
            <div><span className="eyebrow">Model connection</span><strong>{modelConnected ? "Ready" : "Connection required"}</strong></div>
            <label>Provider<select value={offlineOnly ? "mock" : providerId} disabled={offlineOnly} onChange={(event) => chooseProvider(event.target.value)}>{PROVIDER_PRESETS.map((item) => <option key={item.id} value={item.id}>{item.label}</option>)}</select></label>
            {!preset.offline && <>
              <label>Vision model<select value={provider.model ?? ""} onChange={(event) => setSettings((current) => ({ ...current, provider: { ...current?.provider, model: event.target.value } }))}>{preset.models.map((model) => <option key={model.id} value={model.id}>{model.label}</option>)}<option value={CUSTOM_MODEL}>Another model ID…</option></select></label>
              {provider.model === CUSTOM_MODEL && <label>Model ID<input value={customModel} onChange={(event) => setCustomModel(event.target.value)} placeholder="provider/model-name" /></label>}
              {!settings?.api_key_persisted && <label>API key<input type="password" autoComplete="off" value={apiKey} onChange={(event) => setApiKey(event.target.value)} placeholder="Stored in the workspace-private credential file" /></label>}
            </>}
            {!modelConnected && <small role="alert">Enter a key, select Mock, or choose Offline only before using the recommendation.</small>}
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
    normalized === "completed" ||
    normalized === "confirmed" ||
    normalized === "auto_accepted" ||
    normalized === "published" ||
    normalized === "valid"
      ? {
          tone: "auto-accepted",
          label:
            normalized === "published"
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
