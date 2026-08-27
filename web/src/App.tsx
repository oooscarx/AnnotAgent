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
import { deriveProjectRunView } from "./runState";
import {
  NO_PROJECT_MESSAGE,
  PRIMARY_NAVIGATION,
  PRODUCT_NAME,
  PRODUCT_TAGLINE,
  activeSkills,
  type ProductPage,
} from "./productIdentity";
import type {
  Annotation,
  EnabledSkill,
  HistoryRun,
  ImageItem,
  ModelBinding,
  NodeReplayReport,
  PipelineArtifact,
  PipelineArtifactType,
  PipelineSource,
  PipelineStep,
  ProjectSummary,
  ReviewItem,
  RunEvent,
  RunNodeArtifactInspection,
  SkillDetail,
  WorkflowCatalog,
  WorkflowDraft,
  WorkflowDryRunReport,
  WorkflowVersion,
  WorkflowVersionComparison,
} from "./types";

const PAGE_TITLES: Record<ProductPage, string> = {
  dashboard: "Platform overview",
  projects: "Projects",
  project: "Project",
  workflows: "Workflows",
  models: "Models",
  skills: "Skills",
  runs: "Runs",
  review: "Review",
  settings: "Settings",
};

export function App() {
  const [page, setPage] = useState<ProductPage>("dashboard");
  const [projects, setProjects] = useState<ProjectSummary[]>([]);
  const [runs, setRuns] = useState<HistoryRun[]>([]);
  const [models, setModels] = useState<ModelBinding[]>([]);
  const [installedSkills, setInstalledSkills] = useState<EnabledSkill[]>([]);
  const [reviewQueue, setReviewQueue] = useState(0);
  const [projectId, setProjectId] = useState("");
  const [events, setEvents] = useState<RunEvent[]>([]);
  const [error, setError] = useState("");

  const refresh = () =>
    api
      .dashboard()
      .then((data) => {
        setProjects(data.projects);
        setRuns(data.runs);
        setModels(data.models);
        setInstalledSkills(data.installed_skills);
        setReviewQueue(data.review_queue);
      })
      .catch((reason: Error) => setError(reason.message));

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
          void refresh();
        },
      ),
    [],
  );

  const openProject = (id: string) => {
    setProjectId(id);
    setPage(id ? "project" : "projects");
  };
  const selectedProject = projects.find((project) => project.id === projectId);

  return (
    <div className="app-shell">
      <a className="skip-link" href="#main-content">
        Skip to workspace
      </a>
      <aside className="sidebar aa-dark">
        <a
          className="brand"
          href="#dashboard"
          aria-label={`${PRODUCT_NAME} dashboard`}
          onClick={() => setPage("dashboard")}
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
                  ? page === "projects" || page === "project"
                  : page === item.page
              }
              onClick={() => setPage(item.page)}
            >
              {item.label}
            </Nav>
          ))}
        </nav>
        <div className="sidebar-foot">
          <span className="live-dot" aria-hidden="true" /> SSE connected
          <small>
            {events.at(-1)?.kind.replaceAll("_", " ") ?? "waiting for events"}
          </small>
        </div>
      </aside>
      <main
        id="main-content"
        className={page === "review" ? "review-main" : undefined}
      >
        <header className="topbar">
          <div>
            <span className="product-tagline">{PRODUCT_TAGLINE}</span>
            <h1>{PAGE_TITLES[page]}</h1>
          </div>
          <div className="project-switch">
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
            <span aria-hidden="true">Active project</span>
            <label className="sr-only" htmlFor="active-project">
              Active project
            </label>
            <select
              id="active-project"
              value={projectId}
              onChange={(event) => openProject(event.target.value)}
            >
              <option value="">{NO_PROJECT_MESSAGE}</option>
              {projects.map((project) => (
                <option key={project.id} value={project.id}>
                  {project.name}
                </option>
              ))}
            </select>
          </div>
        </header>
        {error && (
          <div className="error-banner" role="alert">
            <span>{error}</span>
            <button aria-label="Dismiss error" onClick={() => setError("")}>
              Dismiss
            </button>
          </div>
        )}
        {page === "dashboard" && (
          <Dashboard
            projects={projects}
            runs={runs}
            models={models}
            skills={installedSkills}
            reviewQueue={reviewQueue}
            onSelect={openProject}
            onRefresh={refresh}
          />
        )}
        {page === "projects" && (
          <ProjectsPage
            projects={projects}
            onSelect={openProject}
            onRefresh={refresh}
            onError={setError}
          />
        )}
        {page === "project" && (
          <ProjectPage
            project={selectedProject}
            runs={runs}
            events={events}
            onRefresh={refresh}
            onOpenWorkflows={() => setPage("workflows")}
            onError={setError}
          />
        )}
        {page === "workflows" && (
          <WorkflowsPage
            projects={projects}
            activeProjectId={projectId}
            onActivate={setProjectId}
            onRefresh={refresh}
            onError={setError}
          />
        )}
        {page === "models" && (
          <ModelsPage models={models} onConfigure={() => setPage("settings")} />
        )}
        {page === "skills" && <SkillsPage onError={setError} />}
        {page === "runs" && <RunsPage runs={runs} />}
        {page === "review" && (
          <ReviewPage
            project={selectedProject}
            projects={projects}
            events={events}
            onError={setError}
          />
        )}
        {page === "settings" && <SettingsPage onError={setError} />}
      </main>
    </div>
  );
}

function Nav({
  icon,
  active,
  onClick,
  children,
}: {
  icon: string;
  active: boolean;
  onClick: () => void;
  children: string;
}) {
  return (
    <button
      className={active ? "active" : ""}
      aria-current={active ? "page" : undefined}
      onClick={onClick}
    >
      <img src={`/brand/core/icons/${icon}.svg`} alt="" aria-hidden="true" />
      {children}
    </button>
  );
}

function Dashboard({
  projects,
  runs,
  models,
  skills,
  reviewQueue,
  onSelect,
  onRefresh,
}: {
  projects: ProjectSummary[];
  runs: HistoryRun[];
  models: ModelBinding[];
  skills: EnabledSkill[];
  reviewQueue: number;
  onSelect: (id: string) => void;
  onRefresh: () => void;
}) {
  const activeRuns = runs.filter(
    (run) =>
      run.controllable && (run.status === "running" || run.status === "paused"),
  ).length;
  const publishedWorkflows = projects
    .flatMap((project) => project.workflows)
    .filter((workflow) => workflow.status === "published").length;
  const tokens = runs.reduce(
    (sum, run) => sum + run.input_tokens + run.output_tokens,
    0,
  );
  const cost = runs.reduce((sum, run) => sum + Number(run.cost || 0), 0);
  return (
    <section className="page-stack">
      <div className="hero-panel aa-dark">
        <div>
          <span className="kicker">AnnotAgent workflow platform</span>
          <h2>
            Compose annotation work
            <br />
            <em>that stays auditable.</em>
          </h2>
          <p>
            Projects bind datasets, typed workflows, vision models, reusable
            Skills, deterministic validation, and human review.
          </p>
        </div>
        <div className="hero-actions">
          <button
            className="primary"
            onClick={() => onSelect(projects[0]?.id ?? "")}
          >
            Open a project
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
          label="Published workflows"
          value={publishedWorkflows}
          detail="Validated compatibility versions"
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
        <Metric
          label="Tokens"
          value={tokens.toLocaleString()}
          detail="Recorded input + output"
        />
        <Metric
          label="Cost"
          value={`$${cost.toFixed(4)}`}
          detail="Exact persisted run totals"
        />
        <Metric
          label="Installed skills"
          value={skills.length}
          detail="Registered domain extensions"
        />
        <Metric
          label="Configured models"
          value={models.length}
          detail="Workspace model bindings"
        />
      </div>
      <div className="platform-grid">
        <Panel title="Recent projects" eyebrow="Concrete annotation work">
          <ProjectList projects={projects.slice(0, 5)} onSelect={onSelect} />
        </Panel>
        <Panel title="Installed Skills" eyebrow="Reusable domain capability">
          {skills.length ? (
            <div className="catalog-summary">
              <strong>
                {skills.length} registered extension
                {skills.length === 1 ? "" : "s"}
              </strong>
              <small>
                Open Skills to inspect domain capabilities and templates.
              </small>
            </div>
          ) : (
            <Empty
              title="No Skills installed"
              detail="Install a registered extension before creating a runnable Project."
            />
          )}
        </Panel>
        <Panel title="Configured models" eyebrow="Workspace bindings">
          {models.length ? (
            <div className="catalog-list">
              {models.map((binding) => (
                <article key={binding.id}>
                  <span className="catalog-monogram">AI</span>
                  <span>
                    <strong>{binding.model}</strong>
                    <small>
                      {binding.provider} · {binding.scope.replaceAll("_", " ")}
                    </small>
                  </span>
                </article>
              ))}
            </div>
          ) : (
            <Empty
              title="No model configured"
              detail="Configure a provider in Settings."
            />
          )}
        </Panel>
      </div>
    </section>
  );
}

function ProjectsPage({
  projects,
  onSelect,
  onRefresh,
  onError,
}: {
  projects: ProjectSummary[];
  onSelect: (id: string) => void;
  onRefresh: () => void;
  onError: (value: string) => void;
}) {
  const [creating, setCreating] = useState(false);
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
          onCreated={() => {
            setCreating(false);
            onRefresh();
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
            status={
              project.active_batch?.status ??
              project.active_run?.status ??
              project.last_run?.status ??
              project.active_workflow.status
            }
          />
          <b>→</b>
        </button>
      ))}
    </div>
  );
}

function ProjectPage({
  project,
  runs,
  events,
  onRefresh,
  onOpenWorkflows,
  onError,
}: {
  project?: ProjectSummary;
  runs: HistoryRun[];
  events: RunEvent[];
  onRefresh: () => void;
  onOpenWorkflows: () => void;
  onError: (value: string) => void;
}) {
  const [images, setImages] = useState<ImageItem[]>([]);
  const [starting, setStarting] = useState(false);
  const [workflowKey, setWorkflowKey] = useState("");
  const [importSource, setImportSource] = useState("");
  const [importFormat, setImportFormat] = useState("native");
  const [importDryRun, setImportDryRun] = useState(true);
  const [importResult, setImportResult] = useState("");
  const [labelTaskId, setLabelTaskId] = useState("");
  const [newLabel, setNewLabel] = useState("");
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
  }, [project?.id]);
  if (!project)
    return (
      <section className="page-stack">
        <Empty
          title="No project opened"
          detail="Choose a Project from Projects or the active Project switcher."
        />
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
  const start = () => {
    setStarting(true);
    void api
      .startRun(
        project.id,
        undefined,
        crypto.randomUUID(),
        selectedWorkflow.source.startsWith("published draft")
          ? {
              workflow_id: selectedWorkflow.workflow_id,
              version: Number(selectedWorkflow.version),
            }
          : undefined,
      )
      .then(onRefresh)
      .catch((error: Error) => onError(error.message))
      .finally(() => setStarting(false));
  };
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
      .then(onRefresh)
      .catch((error: Error) => onError(error.message))
      .finally(() => setStarting(false));
  };
  const control = (action: "pause" | "resume" | "cancel") =>
    activeRun &&
    api
      .control(activeRun, action)
      .then(onRefresh)
      .catch((error: Error) => onError(error.message));
  const importAnnotations = () => {
    if (!importSource.trim()) return onError("Choose a workspace-local annotation file or directory.");
    setImportResult("Import running…");
    void api
      .importAnnotations(project.id, importFormat, importSource, importDryRun)
      .then((report) => {
        setImportResult(
          `${report.dry_run ? "Dry run" : "Imported"}: ${report.imported_count} accepted, ${report.skipped_count} skipped\n${[...report.warnings, ...report.issues.map((issue) => `${issue.record}: ${issue.message}`)].join("\n")}`,
        );
        if (!report.dry_run) onRefresh();
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
        onRefresh();
      })
      .catch((error: Error) => onError(error.message));
  };
  return (
    <section className="page-stack">
      <div className="toolbar-panel project-heading">
        <div>
          <span className="eyebrow">Project · {project.id}</span>
          <h2>{project.name}</h2>
          <p>{project.description || "No Project description provided."}</p>
          <div className="context-line">
            <span>
              Workflow: {project.active_workflow.name}@v
              {project.active_workflow.version}
            </span>
            <span>
              Skills:{" "}
              {project.enabled_skills
                .map((skill) => skill.display_name)
                .join(", ") || "None"}
            </span>
          </div>
          <label>
            Workflow Version for next Run
            <select
              value={workflowKey}
              disabled={Boolean(activeRun)}
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
        </div>
        <div className="button-row" aria-label="Run controls">
          <button
            className="primary"
            disabled={restoredRun.startDisabled || starting}
            title={
              activeRun ? "This Project already has an active Run" : undefined
            }
            onClick={start}
          >
            {starting ? "Starting…" : "Start image run"}
          </button>
          <button
            disabled={restoredRun.startDisabled || starting}
            title={
              activeRun
                ? "This Project already has an active Run or Dataset Batch"
                : undefined
            }
            onClick={startBatch}
          >
            {starting ? "Starting…" : "Start dataset batch"}
          </button>
          <button
            disabled={!activeRun || visibleStatus !== "running"}
            onClick={() => control("pause")}
          >
            <img src="/brand/core/icons/pause.svg" alt="" aria-hidden="true" />
            Pause
          </button>
          <button
            disabled={!activeRun || visibleStatus !== "paused"}
            onClick={() => control("resume")}
          >
            <img src="/brand/core/icons/resume.svg" alt="" aria-hidden="true" />
            Resume
          </button>
          <button
            className="danger"
            disabled={!activeRun}
            onClick={() => control("cancel")}
          >
            <img src="/brand/core/icons/cancel.svg" alt="" aria-hidden="true" />
            Cancel
          </button>
        </div>
      </div>
      <div className="run-state-grid">
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
            </>
          ) : project.active_run ? (
            <>
              <Fact label="Run" value={project.active_run.id.slice(0, 8)} />
              <Status status={project.active_run.status} />
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
      </div>
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
      <div className="project-overview-grid">
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
        <Panel
          title="Versions, Runs, Reviews & Exports"
          eyebrow="Project outputs"
        >
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
          <TagGroup title="Export formats" values={project.export_formats} />
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
    </section>
  );
}

function WorkflowsPage({
  projects,
  activeProjectId,
  onActivate,
  onRefresh,
  onError,
}: {
  projects: ProjectSummary[];
  activeProjectId: string;
  onActivate: (id: string) => void;
  onRefresh: () => Promise<void>;
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
  const [compareLeft, setCompareLeft] = useState("");
  const [compareRight, setCompareRight] = useState("");
  const [advisorKind, setAdvisorKind] = useState<"mock" | "llm">("mock");
  const [templateId, setTemplateId] = useState("");
  const activeProject = projects.find((project) => project.id === activeProjectId);
  const [targetTaskId, setTargetTaskId] = useState("");
  const [targetLabel, setTargetLabel] = useState("");
  const [inspectableRuns, setInspectableRuns] = useState<HistoryRun[]>([]);
  const [inspectRunId, setInspectRunId] = useState("");
  const [inspection, setInspection] = useState<RunNodeArtifactInspection>();
  const [inspectedNodeId, setInspectedNodeId] = useState("");
  const [replay, setReplay] = useState<NodeReplayReport>();
  const [busy, setBusy] = useState(false);
  const refreshDrafts = () =>
    api
      .workflowDrafts(activeProjectId || undefined)
      .then((value) => {
        setDrafts(value.drafts);
        setDraft(
          (current) =>
            value.drafts.find((item) => item.id === current?.id) ??
            value.drafts[0],
        );
      })
      .catch((error: Error) => onError(error.message));
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
    setInspection(undefined);
    setReplay(undefined);
    void api
      .runs()
      .then((value) => {
        const projectRuns = value.runs.filter(
          (run) => run.project_name === activeProject?.name && run.checkpoint_present,
        );
        setInspectableRuns(projectRuns);
        setInspectRunId(projectRuns[0]?.id ?? "");
      })
      .catch((error: Error) => onError(error.message));
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
  const suggest = () =>
    activeProjectId
      ? finish(api.suggestWorkflow(activeProjectId, advisorKind))
      : onError("Select a Project before suggesting a Workflow.");
  const suggestLabelPipeline = () =>
    activeProjectId && targetTaskId && targetLabel
      ? finish(
          api.suggestWorkflow(activeProjectId, advisorKind, {
            task_id: targetTaskId,
            label: targetLabel,
          }),
        )
      : onError("Choose a Project task and target Label first.");
  const targetTask = activeProject?.annotation_schema.find(
    (task) => task.id === targetTaskId,
  );
  const inspectRun = () => {
    if (!inspectRunId) return;
    setBusy(true);
    void api
      .pipelineArtifacts(inspectRunId)
      .then((value) => {
        setInspection(value);
        setInspectedNodeId(value.nodes[0]?.node_id ?? "");
        setReplay(undefined);
      })
      .catch((error: Error) => onError(error.message))
      .finally(() => setBusy(false));
  };
  const replayNode = () => {
    if (!inspection || !inspectedNodeId) return;
    setBusy(true);
    void api
      .replayNode(inspection.run_id, inspectedNodeId)
      .then((value) => {
        setReplay(value);
        setInspection(value.inspection);
      })
      .catch((error: Error) => onError(error.message))
      .finally(() => setBusy(false));
  };
  const save = () => draft && finish(api.saveWorkflowDraft(draft));
  const dryRun = () =>
    draft &&
    (setBusy(true),
    void api
      .dryRunWorkflow(draft.id)
      .then(setReport)
      .then(refreshDrafts)
      .catch((error: Error) => onError(error.message))
      .finally(() => setBusy(false)));
  const publish = () => draft && finish(api.publishWorkflow(draft.id));
  const archive = () => draft && finish(api.archiveWorkflowDraft(draft.id));
  const clonePublished = () =>
    selected &&
    finish(
      api.cloneWorkflowVersion(
        selected.workflow.workflow_id,
        Number(selected.workflow.version),
      ),
    );
  const publishedEntries = entries.filter(({ workflow }) =>
    workflow.source.startsWith("published draft"),
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
  return (
    <section className="page-stack">
      <div className="toolbar-panel">
        <div>
          <span className="eyebrow">Draft → validated → immutable version</span>
          <h2>Workflow Designer</h2>
          <p>
            Drafts only reference registered resources. Dry Run executes sample
            images in an isolated sandbox without creating annotations. Skill
            templates appear only when the active Project enables that Skill.
          </p>
        </div>
        <div className="button-row">
          <button
            onClick={() => create(false)}
            disabled={busy || !activeProjectId}
          >
            New Draft
          </button>
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
          <button
            onClick={suggest}
            disabled={busy || !activeProjectId}
            title={
              !activeProjectId
                ? "Select a Project first"
                : "Create a safe registry-bound suggestion"
            }
          >
            Suggest with Advisor
          </button>
          <select
            aria-label="Target task"
            value={targetTaskId}
            onChange={(event) => {
              const taskId = event.target.value;
              setTargetTaskId(taskId);
              setTargetLabel(
                activeProject?.annotation_schema.find((task) => task.id === taskId)
                  ?.labels[0] ?? "",
              );
            }}
          >
            {(activeProject?.annotation_schema ?? []).map((task) => (
              <option key={task.id} value={task.id}>
                {task.id} · {task.kind}
              </option>
            ))}
          </select>
          <select
            aria-label="Target Label"
            value={targetLabel}
            onChange={(event) => setTargetLabel(event.target.value)}
          >
            {(targetTask?.labels ?? []).map((label) => (
              <option key={label} value={label}>
                {label}
              </option>
            ))}
          </select>
          <button
            onClick={suggestLabelPipeline}
            disabled={busy || !activeProjectId || !targetTaskId || !targetLabel}
            title="Create an editable registry-bounded Label Pipeline Draft"
          >
            Suggest Label Pipeline
          </button>
          <select
            aria-label="Workflow Advisor"
            value={advisorKind}
            onChange={(event) =>
              setAdvisorKind(event.target.value as "mock" | "llm")
            }
          >
            <option value="mock">Mock Advisor · offline</option>
            <option value="llm">Workspace LLM Advisor</option>
          </select>
          <button
            onClick={save}
            disabled={busy || !draft || immutable}
            title={
              immutable
                ? "Published and archived drafts are immutable"
                : "Persist the current draft"
            }
          >
            Save Draft
          </button>
          <button
            onClick={dryRun}
            disabled={busy || !draft}
            title={
              !draft
                ? "Select or create a draft"
                : "Validate graph, nodes, and model capabilities"
            }
          >
            Dry Run
          </button>
          <button
            className="primary"
            onClick={publish}
            disabled={busy || !draft || immutable}
            title={
              immutable
                ? "This version is already immutable"
                : "Publishing is blocked if Dry Run finds issues"
            }
          >
            Publish
          </button>
          <button onClick={archive} disabled={busy || !draft || immutable}>
            Archive
          </button>
          <button
            onClick={clonePublished}
            disabled={
              busy || !selected?.workflow.source.startsWith("published draft")
            }
          >
            Clone Version to Draft
          </button>
        </div>
      </div>
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
      <div className="workflow-layout">
        <aside className="panel workflow-list">
          <span className="eyebrow">Workflow Drafts</span>
          <h2>{drafts.length} drafts</h2>
          {drafts.map((item) => (
            <button
              key={item.id}
              className={draft?.id === item.id ? "active" : ""}
              onClick={() => {
                setDraft(item);
                setReport(undefined);
              }}
            >
              <span>
                <strong>{item.name}</strong>
                <small>
                  {projects.find((project) => project.id === item.project_id)
                    ?.name ?? item.project_id}
                </small>
              </span>
              <Status status={item.status} />
            </button>
          ))}
          {drafts.length === 0 && (
            <Empty
              title="No drafts"
              detail="Create a blank Draft, use a template, or ask the registry-bound Advisor."
            />
          )}
          <span className="eyebrow workflow-published-title">
            Published Workflow Versions
          </span>
          {entries.map(({ project, workflow }) => (
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
              eyebrow={`${draft.status} · ${draft.id.slice(0, 8)}`}
            >
              <div className="button-row">
                <button
                  onClick={addNode}
                  disabled={immutable || Boolean(draft.label_pipeline)}
                  title={
                    draft.label_pipeline
                      ? "Use the Label Pipeline Node Catalog below"
                      : undefined
                  }
                >
                  Add node
                </button>
                <button
                  onClick={addEdge}
                  disabled={
                    immutable || Boolean(draft.label_pipeline) || draft.nodes.length < 2
                  }
                >
                  Add connection
                </button>
              </div>
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
      <Panel title="Node Artifact Inspector" eyebrow="Persisted checkpoint · exact Replay">
        <div className="button-row">
          <select
            aria-label="Inspectable Run"
            value={inspectRunId}
            onChange={(event) => setInspectRunId(event.target.value)}
          >
            <option value="">Choose a completed Pipeline Run…</option>
            {inspectableRuns.map((run) => (
              <option key={run.id} value={run.id}>
                {run.id.slice(0, 8)} · {run.workflow_name}@v{run.workflow_version}
              </option>
            ))}
          </select>
          <button onClick={inspectRun} disabled={busy || !inspectRunId}>
            Load Artifacts
          </button>
          <select
            aria-label="Inspected node"
            value={inspectedNodeId}
            disabled={!inspection}
            onChange={(event) => setInspectedNodeId(event.target.value)}
          >
            {(inspection?.nodes ?? []).map((node) => (
              <option key={node.node_id} value={node.node_id}>
                {node.node_id} · {node.status}
              </option>
            ))}
          </select>
          <button
            onClick={replayNode}
            disabled={busy || !inspection || !inspectedNodeId}
            title="Replay this node and descendants while preserving upstream checkpoint outputs"
          >
            Replay from node
          </button>
        </div>
        {inspection ? (
          <PipelineArtifactInspector
            inspection={inspection}
            nodeId={inspectedNodeId}
            replay={replay}
          />
        ) : (
          <Empty
            title="No checkpoint loaded"
            detail="Run a published Label Pipeline, then load its per-node typed Artifacts here."
          />
        )}
      </Panel>
    </section>
  );
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
  if (!composition) return null;
  const selected =
    composition.label_pipelines.find((pipeline) => pipeline.id === pipelineId) ??
    composition.label_pipelines[0];
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
    const detection = [...selected.steps]
      .reverse()
      .find((step) => Object.values(step.outputs).includes("detection_set"));
    const gate = selected.steps.find((step) => step.node_type === "core.confidence_gate");
    const commit = selected.steps.find((step) => step.kind === "commit");
    if (!detection || !gate || !commit) return;
    const prefix = selected.id;
    const crop: PipelineStep = {
      id: `${prefix}.crop`,
      node_type: "core.crop",
      kind: "transform",
      inputs: {
        image: { source: "image" },
        detections: {
          source: "step",
          step_id: detection.id,
          port: Object.keys(detection.outputs)[0],
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
        object_description:
          "A round soccer ball used in RoboCup, usually white with red, blue, or black panel markings, on or near the green playing field. It may be small in the image. Exclude white shoes, penalty marks, line intersections, and robot body parts.",
        instruction:
          "Scan the complete field and foreground. In these B-Human images, box each visible soccer ball tightly even when it occupies only a small region.",
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
          <h3>Execute once per image and configuration</h3>
        </div>
        <small>{composition.shared_stages.length} shared stage(s)</small>
      </div>
      {composition.shared_stages.map((stage, stageIndex) => (
        <section className="pipeline-lane shared" key={stage.id}>
          <header>
            <strong>{stage.name}</strong>
            <code>{stage.id}</code>
          </header>
          <div className="pipeline-step-row">
            {stage.steps.map((step, stepIndex) => (
              <PipelineStepCard
                key={step.id}
                step={step}
                catalog={catalog}
                immutable={immutable}
                shared
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
              !selected?.steps.some((step) =>
                Object.values(step.outputs).includes("detection_set"),
              )
            }
            title="Internal graph: detector → filter → Crop → classifier → Attach Result"
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
            Apply VLM Football Detect &amp; Crop
          </button>
        </div>
      </div>
      {composition.label_pipelines.map((pipeline) => (
        <section className="pipeline-lane" key={pipeline.id}>
          <header>
            <strong>{pipeline.target_label}</strong>
            <span>{pipeline.target_task_id}</span>
            <code>{pipeline.id}</code>
          </header>
          <div className="pipeline-step-row">
            {pipeline.steps.map((step, stepIndex) => (
              <PipelineStepCard
                key={step.id}
                step={step}
                catalog={catalog}
                immutable={immutable}
                onChange={(next) =>
                  updatePipelineStep(pipeline.id, stepIndex, next)
                }
                onRemove={() => removePipelineStep(pipeline.id, stepIndex)}
              />
            ))}
          </div>
        </section>
      ))}
    </div>
  );
}

function PipelineStepCard({
  step,
  catalog,
  immutable,
  shared = false,
  onChange,
  onRemove,
}: {
  step: PipelineStep;
  catalog?: WorkflowCatalog;
  immutable: boolean;
  shared?: boolean;
  onChange: (step: PipelineStep) => void;
  onRemove?: () => void;
}) {
  const parameterNumber = (name: string) =>
    typeof step.parameters[name] === "number"
      ? String(step.parameters[name])
      : "";
  return (
    <article className="pipeline-step-card">
      <span className="pipeline-step-kind">{shared ? "shared" : step.kind}</span>
      <strong>{step.node_type}</strong>
      <code>{step.id}</code>
      <small>
        {Object.values(step.inputs)
          .map((source) =>
            source.source === "image" ? "Image" : `${source.step_id}.${source.port}`,
          )
          .join(" + ") || "No input"}
        {" → "}
        {Object.values(step.outputs).join(", ") || "terminal"}
      </small>
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
      const rect = (detection as Record<string, unknown>).rect;
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
}: {
  models: ModelBinding[];
  onConfigure: () => void;
}) {
  return (
    <section className="page-stack">
      <div className="toolbar-panel">
        <div>
          <span className="eyebrow">Provider catalog and bindings</span>
          <h2>Models</h2>
          <p>
            Credentials stay in the system keychain; Workflows refer to stable
            binding IDs.
          </p>
        </div>
        <button className="primary" onClick={onConfigure}>
          Configure provider
        </button>
      </div>
      <div className="split-grid">
        <Panel title="Configured bindings" eyebrow="Workspace default">
          {models.length ? (
            <div className="binding-list">
              {models.map((binding) => (
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

function RunsPage({ runs }: { runs: HistoryRun[] }) {
  return (
    <section className="page-stack">
      <div className="toolbar-panel">
        <div>
          <span className="eyebrow">Immutable execution history</span>
          <h2>Runs</h2>
          <p>
            Each summary exposes its Project, immutable Workflow Version, node
            state, typed Artifacts, validation, recovery, model, usage, cost,
            timeout, checkpoint, and review suspension.
          </p>
        </div>
      </div>
      <Panel title="Run history" eyebrow={`${runs.length} recorded`}>
        <div className="runs-table">
          {runs.map((run) => (
            <article key={run.id}>
              <span className="event-rail" />
              <div>
                <strong>{run.project_name}</strong>
                <small>
                  {run.workflow_name}@v{run.workflow_version} ·{" "}
                  {run.skill_versions.join(", ")}
                </small>
                <code>
                  {run.model_identity} · {run.artifact_count} Artifact
                  {run.artifact_count === 1 ? "" : "s"}
                </code>
                <small>
                  Node {run.current_node ?? "none"} · {run.current_node_status ?? "not started"}
                  {` · retries ${run.retry_count}`}
                  {run.fallback_nodes.length
                    ? ` · fallback ${run.fallback_nodes.join(", ")}`
                    : " · no fallback"}
                </small>
                <small>
                  {run.validation_issue_codes.length
                    ? `Issues: ${run.validation_issue_codes.join(", ")}`
                    : "No validation issues"}
                  {run.timed_out ? " · timed out" : " · no timeout"}
                  {run.checkpoint_present ? " · checkpoint saved" : " · no checkpoint"}
                  {run.review_suspended ? " · review suspended" : ""}
                </small>
                {run.terminal_reason && (
                  <small className="run-reason">{run.terminal_reason}</small>
                )}
              </div>
              <div className="run-usage">
                <span>
                  {(run.input_tokens + run.output_tokens).toLocaleString()}{" "}
                  tokens
                </span>
                <span>${run.cost}</span>
              </div>
              <Status status={run.status} />
            </article>
          ))}
          {runs.length === 0 && (
            <Empty
              title="No runs recorded"
              detail="Start a Project run to create auditable history."
            />
          )}
        </div>
      </Panel>
    </section>
  );
}

function ReviewPage({
  project,
  projects,
  events,
  onError,
}: {
  project?: ProjectSummary;
  projects: ProjectSummary[];
  events: RunEvent[];
  onError: (value: string) => void;
}) {
  const [reviews, setReviews] = useState<ReviewItem[]>([]);
  const [selectedId, setSelectedId] = useState("");
  const [draft, setDraft] = useState<Annotation>();
  const [past, setPast] = useState<Annotation[]>([]);
  const [future, setFuture] = useState<Annotation[]>([]);
  const [isNew, setIsNew] = useState(false);
  const [compareMode, setCompareMode] = useState<"after" | "before" | "split">("after");
  const [attributesText, setAttributesText] = useState("{}");
  const [reason, setReason] = useState("");
  const [reasonOptions, setReasonOptions] = useState<string[]>([]);
  const [note, setNote] = useState("");
  const [images, setImages] = useState<ImageItem[]>([]);
  const visibleReviews = project
    ? reviews.filter(
        (review) =>
          review.project_id === project.id ||
          (!review.project_id && review.project_name === project.name),
      )
    : reviews;
  const selected =
    visibleReviews.find((review) => review.id === selectedId) ?? visibleReviews[0];
  const reviewProject = project ?? projects.find(
    (candidate) =>
      candidate.id === selected?.project_id ||
      (!selected?.project_id && candidate.name === selected?.project_name),
  );
  const refresh = () =>
    api
      .reviews()
      .then((value) => {
        setReviews(value.reviews);
        const first = project
          ? value.reviews.find(
              (review) =>
                review.project_id === project.id ||
                (!review.project_id && review.project_name === project.name),
            )
          : value.reviews[0];
        if (!value.reviews.some((review) => review.id === selectedId) && first)
          setSelectedId(first.id);
      })
      .catch((error: Error) => onError(error.message));
  useEffect(() => {
    void refresh();
  }, []);
  useEffect(() => {
    void api
      .skills()
      .then((skills) => {
        const ids = reviewProject?.enabled_skills.map((skill) => skill.id) ?? [];
        const options =
          skills.find((skill) => ids.includes(skill.id))?.correction_taxonomy ??
          [];
        setReasonOptions(options);
        setReason(options[0] || "manual_edit");
      })
      .catch((error: Error) => onError(error.message));
  }, [reviewProject?.id]);
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
  const save = () => {
    if (!draft || !selected) return;
    let attributes: Record<string, unknown>;
    try {
      attributes = JSON.parse(attributesText) as Record<string, unknown>;
    } catch {
      return onError("Attributes must be a valid JSON object.");
    }
    if (!attributes || Array.isArray(attributes) || typeof attributes !== "object")
      return onError("Attributes must be a JSON object.");
    const annotation = { ...draft, attributes };
    const operation = isNew
      ? api.createAnnotation(selected.run_id, annotation)
      : api.revise(annotation, reason);
    return operation
      .then(() => {
        if (isNew) setSelectedId(annotation.id);
        setIsNew(false);
        setPast([]);
        setFuture([]);
        return refresh();
      })
      .catch((error: Error) => onError(error.message));
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
      provenance: {},
      created_at: new Date().toISOString(),
    };
    if (draft) setPast((items) => [...items, structuredClone(draft)]);
    setFuture([]);
    setDraft(annotation);
    setAttributesText("{}");
    setIsNew(true);
  };
  const decide = (decision: "accept" | "reject" | "delete") => {
    if (!selected || !reviewProject)
      return onError(
        "Select the Review item's Project before recording a decision.",
      );
    return api
      .decide(selected.id, reviewProject.id, decision, reason, note)
      .then(refresh)
      .catch((error: Error) => onError(error.message));
  };
  const visualContext = {
    skillProfiles: visualProfilesForSkills(
      reviewProject?.enabled_skills.map((skill) => skill.id) ?? [],
    ),
  };
  return (
    <section className="review-layout">
      <aside className="review-queue panel">
        <span className="eyebrow">Human attention</span>
        <h2>
          Review queue <b>{visibleReviews.length}</b>
        </h2>
        <div className="queue-items" aria-label="Annotations requiring review">
          {visibleReviews.map((review) => (
            <button
              key={review.id}
              aria-pressed={selected?.id === review.id}
              className={selected?.id === review.id ? "active" : ""}
              onClick={() => setSelectedId(review.id)}
            >
              <span aria-hidden="true">
                {review.annotation.label?.slice(0, 2).toUpperCase() ?? "?"}
              </span>
              <span>
                <strong>
                  {review.annotation.label ?? review.annotation.task_id}
                </strong>
                <small>
                  {!project && `${review.project_name} · `}
                  {review.annotation.task_id} ·{" "}
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
        <div className="review-edit-toolbar" aria-label="Annotation editing controls">
          <button onClick={undo} disabled={!past.length} aria-label="Undo annotation edit">Undo</button>
          <button onClick={redo} disabled={!future.length} aria-label="Redo annotation edit">Redo</button>
          <button
            onClick={() => createShape("bounding_box")}
            disabled={!reviewProject?.annotation_schema.some((task) => task.kind === "bounding_box")}
          >New box</button>
          <button
            onClick={() => createShape("keypoints")}
            disabled={!reviewProject?.annotation_schema.some((task) => task.kind === "keypoints")}
          >New keypoint</button>
          <button
            onClick={() => createShape("polyline")}
            disabled={!reviewProject?.annotation_schema.some((task) => task.kind === "polyline")}
          >New polyline</button>
          <button
            onClick={() => createShape("polygon")}
            disabled={!reviewProject?.annotation_schema.some((task) => task.kind === "polygon")}
          >New polygon</button>
          <select
            aria-label="Before and after comparison"
            value={compareMode}
            onChange={(event) => setCompareMode(event.target.value as typeof compareMode)}
          >
            <option value="after">After</option>
            <option value="before">Before</option>
            <option value="split">Before / after</option>
          </select>
        </div>
        <div
          className={`review-canvas-stage${compareMode === "split" ? " review-canvas-compare" : ""}`}
        >
          {(compareMode === "before" || compareMode === "split") && (
            <div><small>Before</small><AnnotationCanvas
              imageUrl={images[0]?.url}
              annotations={selected ? [selected.annotation] : []}
              selectedId={selected?.annotation.id}
              visualContext={visualContext}
              onSelect={() => undefined}
              onChange={() => undefined}
            /></div>
          )}
          {(compareMode === "after" || compareMode === "split") && (
            <div><small>After</small><AnnotationCanvas
              imageUrl={images[0]?.url}
              annotations={draft ? [draft] : []}
              selectedId={draft?.id}
              visualContext={visualContext}
              onSelect={() => undefined}
              onEditStart={beginEdit}
              onChange={setDraft}
            /></div>
          )}
        </div>
        <Trace
          events={
            selected
              ? events.filter((event) => event.run_id === selected.run_id)
              : events.slice(-12)
          }
        />
      </div>
      <aside className="inspector panel">
        <span className="eyebrow">Validator evidence</span>
        <h2>{draft?.label ?? "No selection"}</h2>
        {draft && (
          <>
            <label>
              Label
              <input
                value={draft.label ?? ""}
                onChange={(event) =>
                  edit({ ...draft, label: event.target.value })
                }
              />
            </label>
            <div className="fact-grid">
              <span>
                Confidence
                <strong>{Math.round((draft.confidence ?? 0) * 100)}%</strong>
              </span>
              <span>
                Source<strong>{draft.source}</strong>
              </span>
              <span>
                Task<strong>{draft.task_id}</strong>
              </span>
              <span>
                Status<strong>{draft.review_status}</strong>
              </span>
            </div>
            <label>
              Attributes (JSON)
              <textarea
                aria-label="Annotation attributes JSON"
                value={attributesText}
                onChange={(event) => setAttributesText(event.target.value)}
              />
            </label>
            <label>
              Correction reason
              {reasonOptions.length ? (
                <select
                  value={reason}
                  onChange={(event) => setReason(event.target.value)}
                >
                  {reasonOptions.map((value) => (
                    <option key={value}>{value}</option>
                  ))}
                </select>
              ) : (
                <input
                  aria-label="Correction reason"
                  value={reason}
                  onChange={(event) => setReason(event.target.value)}
                  placeholder="manual_edit"
                />
              )}
            </label>
            <label>
              Reviewer note
              <textarea
                value={note}
                onChange={(event) => setNote(event.target.value)}
                placeholder="What changed, and why?"
              />
            </label>
            <button onClick={save}>{isNew ? "Create annotation" : "Save revision"}</button>
            <div className="decision-row">
              <button className="primary" onClick={() => decide("accept")}>
                Accept
              </button>
              <button onClick={() => decide("reject")}>Reject</button>
              <button className="danger" onClick={() => decide("delete")}>
                Delete
              </button>
            </div>
            <button
              className="text-button"
              onClick={() =>
                api
                  .revisions(draft.id)
                  .then((value) =>
                    alert(JSON.stringify(value.revisions, null, 2)),
                  )
              }
            >
              View revision history →
            </button>
          </>
        )}
      </aside>
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

function SkillsPage({ onError }: { onError: (value: string) => void }) {
  const [skills, setSkills] = useState<SkillDetail[]>([]);
  useEffect(() => {
    void api
      .skills()
      .then(setSkills)
      .catch((error: Error) => onError(error.message));
  }, []);
  return (
    <section className="page-stack">
      <div className="boundary-note">
        <span>AnnotAgent</span>
        <i>DomainSkill registry boundary</i>
        <span>Installed Skills</span>
      </div>
      {skills.map((skill) => (
        <Panel
          key={skill.id}
          title={`${skill.display_name} · v${skill.version}`}
          eyebrow={skill.id}
        >
          <p className="lede">{skill.description}</p>
          <div className="skill-columns">
            <TagGroup
              title="Node templates"
              values={skill.tasks.map((task) => task.id)}
            />
            <TagGroup title="Registered tools" values={skill.tools} />
            <TagGroup title="Validators" values={skill.validators} />
            <TagGroup title="Refiners" values={skill.refiners} />
            <TagGroup
              title="Correction taxonomy"
              values={skill.correction_taxonomy}
            />
            <TagGroup title="Prompt resources" values={skill.resources} />
          </div>
        </Panel>
      ))}
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
  const preset = getProviderPreset(presetId);
  const providerChanged =
    !preset.offline && credentialPresetRef.current !== preset.id;
  const customModel =
    !preset.custom &&
    !preset.offline &&
    !isCatalogModel(preset, provider.model);
  const setProvider = (field: string, value: unknown) =>
    setSettings({ ...settings, provider: { ...provider, [field]: value } });
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
        finish(value, "Saved API key removed from the system keychain.", true),
      )
      .catch((error: Error) => onError(error.message))
      .finally(() => setSaving(false));
  };
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
                    ? "Stored in the system keychain · paste to replace"
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
                  ? "Keychain protected · never returned by the API"
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
              System keychain unavailable:{" "}
              {String(settings.credential_store_error)}
            </span>
          </div>
        )}
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
          {message ||
            (settings.settings_persisted
              ? `Saved at ${settings.settings_path}`
              : "Save once to keep these settings across restarts.")}
        </span>
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
      </div>
    </section>
  );
}

function CreateProject({
  onClose,
  onCreated,
  onError,
}: {
  onClose: () => void;
  onCreated: () => void;
  onError: (value: string) => void;
}) {
  const [id, setId] = useState("new-project");
  const [skills, setSkills] = useState<SkillDetail[]>([]);
  const [skillId, setSkillId] = useState("");
  const [yaml, setYaml] = useState("");
  useEffect(() => {
    void api
      .skills()
      .then((items) => {
        setSkills(items);
        setSkillId(items[0]?.id ?? "");
        setYaml(items[0]?.project_template ?? "");
      })
      .catch((error: Error) => onError(error.message));
  }, []);
  const chooseSkill = (value: string) => {
    setSkillId(value);
    setYaml(skills.find((skill) => skill.id === value)?.project_template ?? "");
  };
  return (
    <div className="modal-backdrop">
      <div className="modal">
        <span className="eyebrow">Validated Project schema</span>
        <h2>Create Project</h2>
        <label>
          Workspace ID
          <input value={id} onChange={(event) => setId(event.target.value)} />
        </label>
        <label>
          Starter Skill
          <select
            value={skillId}
            onChange={(event) => chooseSkill(event.target.value)}
          >
            {skills.map((skill) => (
              <option key={skill.id} value={skill.id}>
                {skill.display_name}
              </option>
            ))}
          </select>
        </label>
        <label>
          project.yaml
          <textarea
            className="yaml-editor"
            value={yaml}
            onChange={(event) => setYaml(event.target.value)}
          />
        </label>
        <div className="button-row">
          <button onClick={onClose}>Cancel</button>
          <button
            className="primary"
            onClick={() =>
              api
                .createProject(id, yaml)
                .then(onCreated)
                .catch((error: Error) => onError(error.message))
            }
          >
            Validate & create
          </button>
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
              : normalized === "valid"
                ? "Valid"
                : "Completed",
        }
      : normalized === "completed_with_review" || normalized === "needs_review"
        ? { tone: "needs-review", label: "Completed with review" }
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
