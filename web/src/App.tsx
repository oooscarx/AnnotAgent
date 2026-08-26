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
  ProjectSummary,
  ReviewItem,
  RunEvent,
  SkillDetail,
  WorkflowVersion,
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

  const refresh = () => api.dashboard().then((data) => {
    setProjects(data.projects);
    setRuns(data.runs);
    setModels(data.models);
    setInstalledSkills(data.installed_skills);
    setReviewQueue(data.review_queue);
  }).catch((reason: Error) => setError(reason.message));

  useEffect(() => { void refresh(); }, []);
  useEffect(() => subscribeEvents((event) => {
    setEvents((previous) => [...previous.slice(-149), event]);
    if (["run_completed", "review_requested", "run_failed"].includes(event.kind)) void refresh();
  }), []);

  const openProject = (id: string) => {
    setProjectId(id);
    setPage(id ? "project" : "projects");
  };
  const selectedProject = projects.find((project) => project.id === projectId);

  return <div className="app-shell">
    <a className="skip-link" href="#main-content">Skip to workspace</a>
    <aside className="sidebar aa-dark">
      <a className="brand" href="#dashboard" aria-label={`${PRODUCT_NAME} dashboard`} onClick={() => setPage("dashboard")}>
        <img className="brand-lockup" src="/brand/core/annotagent-lockup-dark.svg" alt={PRODUCT_NAME} />
        <img className="brand-mark-compact" src="/brand/core/annotagent-mark-dark-surface.svg" alt="" aria-hidden="true" />
      </a>
      <nav aria-label="Primary navigation">
        {PRIMARY_NAVIGATION.map((item) => <Nav key={item.page} icon={item.icon} active={item.page === "projects" ? page === "projects" || page === "project" : page === item.page} onClick={() => setPage(item.page)}>{item.label}</Nav>)}
      </nav>
      <div className="sidebar-foot"><span className="live-dot" aria-hidden="true" /> SSE connected<small>{events.at(-1)?.kind.replaceAll("_", " ") ?? "waiting for events"}</small></div>
    </aside>
    <main id="main-content">
      <header className="topbar">
        <div><span className="product-tagline">{PRODUCT_TAGLINE}</span><h1>{PAGE_TITLES[page]}</h1></div>
        <div className="project-switch">
          {activeSkills(selectedProject).map((skill) => {
            const profile = visualProfilesForSkills([skill.id])[0];
            return <span className="skill-badge" key={skill.id}>{profile?.icon && <img src={profile.icon} alt="" aria-hidden="true" />}{skill.display_name}</span>;
          })}
          <span aria-hidden="true">Active project</span>
          <label className="sr-only" htmlFor="active-project">Active project</label>
          <select id="active-project" value={projectId} onChange={(event) => openProject(event.target.value)}>
            <option value="">{NO_PROJECT_MESSAGE}</option>
            {projects.map((project) => <option key={project.id} value={project.id}>{project.name}</option>)}
          </select>
        </div>
      </header>
      {error && <div className="error-banner" role="alert"><span>{error}</span><button aria-label="Dismiss error" onClick={() => setError("")}>Dismiss</button></div>}
      {page === "dashboard" && <Dashboard projects={projects} runs={runs} models={models} skills={installedSkills} reviewQueue={reviewQueue} onSelect={openProject} onRefresh={refresh} />}
      {page === "projects" && <ProjectsPage projects={projects} onSelect={openProject} onRefresh={refresh} onError={setError} />}
      {page === "project" && <ProjectPage project={selectedProject} runs={runs} events={events} onOpenWorkflows={() => setPage("workflows")} onError={setError} />}
      {page === "workflows" && <WorkflowsPage projects={projects} activeProjectId={projectId} onActivate={setProjectId} />}
      {page === "models" && <ModelsPage models={models} onConfigure={() => setPage("settings")} />}
      {page === "skills" && <SkillsPage onError={setError} />}
      {page === "runs" && <RunsPage runs={runs} />}
      {page === "review" && <ReviewPage project={selectedProject} events={events} onError={setError} />}
      {page === "settings" && <SettingsPage onError={setError} />}
    </main>
  </div>;
}

function Nav({ icon, active, onClick, children }: { icon: string; active: boolean; onClick: () => void; children: string }) {
  return <button className={active ? "active" : ""} aria-current={active ? "page" : undefined} onClick={onClick}><img src={`/brand/core/icons/${icon}.svg`} alt="" aria-hidden="true" />{children}</button>;
}

function Dashboard({ projects, runs, models, skills, reviewQueue, onSelect, onRefresh }: { projects: ProjectSummary[]; runs: HistoryRun[]; models: ModelBinding[]; skills: EnabledSkill[]; reviewQueue: number; onSelect: (id: string) => void; onRefresh: () => void }) {
  const activeRuns = runs.filter((run) => run.status === "running" || run.status === "paused").length;
  const publishedWorkflows = projects.flatMap((project) => project.workflows).filter((workflow) => workflow.status === "published").length;
  const tokens = runs.reduce((sum, run) => sum + run.input_tokens + run.output_tokens, 0);
  const cost = runs.reduce((sum, run) => sum + Number(run.cost || 0), 0);
  return <section className="page-stack">
    <div className="hero-panel aa-dark"><div><span className="kicker">AnnotAgent workflow platform</span><h2>Compose annotation work<br /><em>that stays auditable.</em></h2><p>Projects bind datasets, typed workflows, vision models, reusable Skills, deterministic validation, and human review.</p></div><div className="hero-actions"><button className="primary" onClick={() => onSelect(projects[0]?.id ?? "")}>Open a project</button><button onClick={onRefresh}>Refresh state</button></div></div>
    <div className="metrics-grid platform-metrics">
      <Metric label="Projects" value={projects.length} detail={`${projects.reduce((sum, project) => sum + project.image_count, 0)} images registered`} />
      <Metric label="Published workflows" value={publishedWorkflows} detail="Validated compatibility versions" />
      <Metric label="Active runs" value={activeRuns} detail={`${runs.length} total executions`} live={activeRuns > 0} />
      <Metric label="Review queue" value={reviewQueue} detail="Annotations requiring attention" accent={reviewQueue > 0} />
      <Metric label="Tokens" value={tokens.toLocaleString()} detail="Recorded input + output" />
      <Metric label="Cost" value={`$${cost.toFixed(4)}`} detail="Exact persisted run totals" />
      <Metric label="Installed skills" value={skills.length} detail="Registered domain extensions" />
      <Metric label="Configured models" value={models.length} detail="Workspace model bindings" />
    </div>
    <div className="platform-grid">
      <Panel title="Recent projects" eyebrow="Concrete annotation work"><ProjectList projects={projects.slice(0, 5)} onSelect={onSelect} /></Panel>
      <Panel title="Installed Skills" eyebrow="Reusable domain capability">{skills.length ? <div className="catalog-summary"><strong>{skills.length} registered extension{skills.length === 1 ? "" : "s"}</strong><small>Open Skills to inspect domain capabilities and templates.</small></div> : <Empty title="No Skills installed" detail="Install a registered extension before creating a runnable Project." />}</Panel>
      <Panel title="Configured models" eyebrow="Workspace bindings">{models.length ? <div className="catalog-list">{models.map((binding) => <article key={binding.id}><span className="catalog-monogram">AI</span><span><strong>{binding.model}</strong><small>{binding.provider} · {binding.scope.replaceAll("_", " ")}</small></span></article>)}</div> : <Empty title="No model configured" detail="Configure a provider in Settings." />}</Panel>
    </div>
  </section>;
}

function ProjectsPage({ projects, onSelect, onRefresh, onError }: { projects: ProjectSummary[]; onSelect: (id: string) => void; onRefresh: () => void; onError: (value: string) => void }) {
  const [creating, setCreating] = useState(false);
  return <section className="page-stack"><div className="toolbar-panel"><div><span className="eyebrow">Project inventory</span><h2>Datasets, schemas, Workflows, and bindings</h2><p>A Project is concrete annotation work; Skills remain reusable extensions.</p></div><button className="primary" onClick={() => setCreating(true)}>New project</button></div><Panel title="All projects" eyebrow={`${projects.length} configured`}><ProjectList projects={projects} onSelect={onSelect} /></Panel>{creating && <CreateProject onClose={() => setCreating(false)} onCreated={() => { setCreating(false); onRefresh(); }} onError={onError} />}</section>;
}

function ProjectList({ projects, onSelect }: { projects: ProjectSummary[]; onSelect: (id: string) => void }) {
  return <div className="table-list">{projects.length === 0 && <Empty title="No projects yet" detail="Create a Project from a validated schema and a registered Skill template." />}{projects.map((project) => <button className="project-row" key={project.id} onClick={() => onSelect(project.id)}><span className="project-avatar">{project.name.slice(0, 2).toUpperCase()}</span><span><strong>{project.name}</strong><small>{project.image_count} images · {project.active_workflow.name}@v{project.active_workflow.version}</small></span><Status status={project.recent_run?.status ?? project.active_workflow.status} /><b>→</b></button>)}</div>;
}

function ProjectPage({ project, runs, events, onOpenWorkflows, onError }: { project?: ProjectSummary; runs: HistoryRun[]; events: RunEvent[]; onOpenWorkflows: () => void; onError: (value: string) => void }) {
  const [images, setImages] = useState<ImageItem[]>([]);
  const [activeRun, setActiveRun] = useState("");
  useEffect(() => { if (project) void api.images(project.id).then((value) => setImages(value.images)).catch((error: Error) => onError(error.message)); else setImages([]); }, [project?.id]);
  if (!project) return <section className="page-stack"><Empty title="No project opened" detail="Choose a Project from Projects or the active Project switcher." /></section>;
  const runEvents = events.filter((event) => event.run_id === activeRun);
  const usage = [...runEvents].reverse().find((event) => event.kind === "usage_updated");
  const lastTask = [...runEvents].reverse().find((event) => event.task_id)?.task_id;
  const latestState = [...runEvents].reverse().find((event) => event.payload.type === "state");
  const visibleStatus = (latestState?.payload.data.to as string | undefined) ?? (activeRun ? "running" : "pending");
  const projectRuns = runs.filter((run) => run.project_name === project.name);
  const start = () => api.startRun(project.id).then((run) => setActiveRun(run.run_id)).catch((error: Error) => onError(error.message));
  const control = (action: "pause" | "resume" | "cancel") => activeRun && api.control(activeRun, action).catch((error: Error) => onError(error.message));
  return <section className="page-stack">
    <div className="toolbar-panel project-heading"><div><span className="eyebrow">Project · {project.id}</span><h2>{project.name}</h2><p>{project.description || "No Project description provided."}</p><div className="context-line"><span>Workflow: {project.active_workflow.name}@v{project.active_workflow.version}</span><span>Skills: {project.enabled_skills.map((skill) => skill.display_name).join(", ") || "None"}</span></div></div><div className="button-row" aria-label="Run controls"><button className="primary" onClick={start}>Start image run</button><button disabled={!activeRun} onClick={() => control("pause")}><img src="/brand/core/icons/pause.svg" alt="" aria-hidden="true" />Pause</button><button disabled={!activeRun} onClick={() => control("resume")}><img src="/brand/core/icons/resume.svg" alt="" aria-hidden="true" />Resume</button><button className="danger" disabled={!activeRun} onClick={() => control("cancel")}><img src="/brand/core/icons/cancel.svg" alt="" aria-hidden="true" />Cancel</button></div></div>
    {activeRun && <div className="run-progress aa-dark" aria-live="polite"><div><span className="live-dot" aria-hidden="true" /><strong>Run {activeRun.slice(0, 8)}</strong><small>{lastTask ?? "starting"} · {runEvents.at(-1)?.kind.replaceAll("_", " ") ?? "queued"}</small></div><div className="progress-track"><i style={{ width: `${Math.min(100, runEvents.length * 3)}%` }} /></div><pre>{usage ? JSON.stringify(usage.payload.data, null, 2) : "usage pending"}</pre></div>}
    <div className="project-overview-grid">
      <Panel title="Dataset" eyebrow="Project-owned"><Fact label="Root" value={project.dataset.root} /><Fact label="Images" value={project.dataset.image_count} /><Fact label="Discovery" value={project.dataset.recursive ? "Recursive" : "Top level"} /><TagGroup title="Include patterns" values={project.dataset.include} /></Panel>
      <Panel title="Active Workflow" eyebrow={`${project.active_workflow.validation_status} · ${project.active_workflow.status}`}><Fact label="Version" value={`v${project.active_workflow.version}`} /><Fact label="Nodes" value={project.active_workflow.nodes.length} /><Fact label="Source" value={project.active_workflow.source} /><button onClick={onOpenWorkflows}>View Workflow definition</button></Panel>
      <Panel title="Enabled Skills" eyebrow="Domain extensions">{project.enabled_skills.length ? <div className="catalog-list">{project.enabled_skills.map((skill) => <article key={skill.id}><span className="catalog-monogram">{skill.display_name.slice(0, 2).toUpperCase()}</span><span><strong>{skill.display_name}</strong><small>{skill.id}@{skill.version}</small></span></article>)}</div> : <Empty title="No Skills enabled" detail="Stable schema and hash visuals remain available." />}</Panel>
      <Panel title="Model Bindings" eyebrow="Node execution">{project.model_bindings.map((binding) => <Fact key={binding.id} label={`${binding.id} · ${binding.role}`} value={`${binding.provider} / ${binding.model}`} />)}</Panel>
      <Panel title="Annotation Schema" eyebrow={`${project.annotation_schema.length} typed tasks`}><div className="schema-list">{project.annotation_schema.map((task) => <article key={task.id}><span><strong>{task.id}</strong><small>{task.kind}{task.required ? " · required" : ""}</small></span><span>{task.labels.join(", ") || "No labels"}</span></article>)}</div></Panel>
      <Panel title="Versions, Runs, Reviews & Exports" eyebrow="Project outputs"><Fact label="Workflow versions" value={project.available_workflow_versions.length} /><Fact label="Runs" value={projectRuns.length} /><Fact label="Runs awaiting review" value={projectRuns.filter((run) => run.status === "awaiting_review").length} /><TagGroup title="Export formats" values={project.export_formats} /></Panel>
    </div>
    <Panel title="Dataset images" eyebrow={`${images.length} visible`}><div className="image-grid">{images.map((image) => <article key={image.index}><img src={image.url} alt={image.name} /><div><span><strong>{image.name}</strong><small>Image {image.index + 1}</small></span><Status status={visibleStatus} /></div></article>)}{images.length === 0 && <Empty title="No images" detail="Import images with the CLI or controlled workspace import API." />}</div></Panel>
  </section>;
}

function WorkflowsPage({ projects, activeProjectId, onActivate }: { projects: ProjectSummary[]; activeProjectId: string; onActivate: (id: string) => void }) {
  const entries = projects.flatMap((project) => project.available_workflow_versions.map((workflow) => ({ project, workflow })));
  const selected = entries.find((entry) => entry.project.id === activeProjectId);
  return <section className="page-stack"><div className="toolbar-panel"><div><span className="eyebrow">Versioned execution definitions</span><h2>Workflow Designer</h2><p>The current release provides a real, validated view of the configured Project task graph.</p></div><div className="button-row"><button disabled title="LLM Workflow Suggestion is roadmap work">Suggest with LLM</button><button disabled title="Workflow editing is not implemented">Edit Workflow</button><button disabled title="Dry Run requires persisted Workflow drafts">Dry Run</button><button disabled title="Publishing immutable versions is not implemented">Publish</button></div></div><div className="roadmap-note">Draft editing, static validation of new typed edges, dry run, and publishing are not implemented. Current published entries are compatibility views over validated Project tasks and the registered Skill graph.</div><div className="workflow-layout"><aside className="panel workflow-list"><span className="eyebrow">Workflow list</span><h2>{entries.length} versions</h2>{entries.map(({ project, workflow }) => <button key={`${project.id}-${workflow.workflow_id}-${workflow.version}`} className={selected?.project.id === project.id ? "active" : ""} onClick={() => onActivate(project.id)}><span><strong>{workflow.name}</strong><small>{project.name} · v{workflow.version}</small></span><Status status={workflow.status} /></button>)}{entries.length === 0 && <Empty title="No Workflows" detail="Create a Project to expose its configured task graph." />}</aside><div>{selected ? <WorkflowDetail project={selected.project} workflow={selected.workflow} /> : <Empty title="Select a Workflow" detail="Workflow definitions are Project-scoped." />}</div></div></section>;
}

function WorkflowDetail({ project, workflow }: { project: ProjectSummary; workflow: WorkflowVersion }) {
  return <Panel title={`${workflow.name} · v${workflow.version}`} eyebrow={`${project.name} · ${workflow.status}`}><div className="workflow-facts"><Fact label="Validation" value={workflow.validation_status} /><Fact label="Default" value={workflow.is_default ? "Yes" : "No"} /><Fact label="Source" value={workflow.source} /><Fact label="Enabled Skills" value={project.enabled_skills.map((skill) => `${skill.id}@${skill.version}`).join(", ") || "None"} /></div><div className="workflow-nodes">{workflow.nodes.map((node, index) => <article key={node.id}><span className="node-index">{String(index + 1).padStart(2, "0")}</span><div><span className="eyebrow">{node.node_type}</span><h3>{node.id}</h3><small>Depends on: {node.depends_on.join(", ") || "start"}</small><div className="node-meta"><span>Model · {node.model_binding || "unbound"}</span><span>Fallback · {node.fallback || "none"}</span><span>Human review · {node.human_review_gate ? "gate enabled" : "not configured"}</span></div><TagGroup title="Validators" values={node.validators} /><TagGroup title="Refiners" values={node.refiners} /></div></article>)}</div></Panel>;
}

function ModelsPage({ models, onConfigure }: { models: ModelBinding[]; onConfigure: () => void }) {
  return <section className="page-stack"><div className="toolbar-panel"><div><span className="eyebrow">Provider catalog and bindings</span><h2>Models</h2><p>Credentials stay in the system keychain; Workflows refer to stable binding IDs.</p></div><button className="primary" onClick={onConfigure}>Configure provider</button></div><div className="split-grid"><Panel title="Configured bindings" eyebrow="Workspace default">{models.length ? <div className="binding-list">{models.map((binding) => <article key={binding.id}><span className="catalog-monogram">AI</span><div><strong>{binding.id}</strong><small>{binding.role} · {binding.scope.replaceAll("_", " ")}</small><code>{binding.provider} / {binding.model}</code></div></article>)}</div> : <Empty title="No model bindings" detail="Choose a provider in Settings." />}</Panel><Panel title="Provider catalog" eyebrow="Curated compatible options"><div className="catalog-list">{PROVIDER_PRESETS.filter((preset) => !preset.offline).map((preset) => <article key={preset.id}><span className="catalog-monogram">{preset.shortLabel.slice(0, 2).toUpperCase()}</span><span><strong>{preset.label}</strong><small>{preset.models.length ? `${preset.models.length} curated models` : "Custom model IDs"}</small></span></article>)}</div></Panel></div></section>;
}

function RunsPage({ runs }: { runs: HistoryRun[] }) {
  return <section className="page-stack"><div className="toolbar-panel"><div><span className="eyebrow">Immutable execution history</span><h2>Runs</h2><p>Each summary exposes its Project, compatibility Workflow version, Skill versions, model binding, usage, cost, and status.</p></div></div><Panel title="Run history" eyebrow={`${runs.length} recorded`}><div className="runs-table">{runs.map((run) => <article key={run.id}><span className="event-rail" /><div><strong>{run.project_name}</strong><small>{run.workflow_name}@v{run.workflow_version} · {run.skill_versions.join(", ")}</small><code>{run.model_bindings.map((binding) => `${binding.id}: ${binding.model}`).join(", ")}</code></div><div className="run-usage"><span>{(run.input_tokens + run.output_tokens).toLocaleString()} tokens</span><span>${run.cost}</span></div><Status status={run.status} /></article>)}{runs.length === 0 && <Empty title="No runs recorded" detail="Start a Project run to create auditable history." />}</div></Panel></section>;
}

function ReviewPage({ project, events, onError }: { project?: ProjectSummary; events: RunEvent[]; onError: (value: string) => void }) {
  const [reviews, setReviews] = useState<ReviewItem[]>([]);
  const [selectedId, setSelectedId] = useState("");
  const [draft, setDraft] = useState<Annotation>();
  const [reason, setReason] = useState("");
  const [reasonOptions, setReasonOptions] = useState<string[]>([]);
  const [note, setNote] = useState("");
  const [images, setImages] = useState<ImageItem[]>([]);
  const selected = reviews.find((review) => review.id === selectedId) ?? reviews[0];
  const refresh = () => api.reviews().then((value) => { setReviews(value.reviews); if (!selectedId && value.reviews[0]) setSelectedId(value.reviews[0].id); }).catch((error: Error) => onError(error.message));
  useEffect(() => { void refresh(); }, []);
  useEffect(() => { void api.skills().then((skills) => { const ids = project?.enabled_skills.map((skill) => skill.id) ?? []; const options = skills.find((skill) => ids.includes(skill.id))?.correction_taxonomy ?? []; setReasonOptions(options); setReason(options[0] || ""); }).catch((error: Error) => onError(error.message)); }, [project?.id]);
  useEffect(() => { if (project) void api.images(project.id).then((value) => setImages(value.images)).catch((error: Error) => onError(error.message)); else setImages([]); }, [project?.id]);
  useEffect(() => setDraft(selected?.annotation), [selected?.id]);
  const save = () => draft && api.revise(draft, reason).then(refresh).catch((error: Error) => onError(error.message));
  const decide = (decision: "accept" | "reject" | "delete") => { if (!selected || !project) return onError("Select the Review item's Project before recording a decision."); return api.decide(selected.id, project.id, decision, reason, note).then(refresh).catch((error: Error) => onError(error.message)); };
  const visualContext = { skillProfiles: visualProfilesForSkills(project?.enabled_skills.map((skill) => skill.id) ?? []) };
  return <section className="review-layout"><aside className="review-queue panel"><span className="eyebrow">Human attention</span><h2>Review queue <b>{reviews.length}</b></h2><div className="queue-items" aria-label="Annotations requiring review">{reviews.map((review) => <button key={review.id} aria-pressed={selected?.id === review.id} className={selected?.id === review.id ? "active" : ""} onClick={() => setSelectedId(review.id)}><span aria-hidden="true">{review.annotation.label?.slice(0, 2).toUpperCase() ?? "?"}</span><span><strong>{review.annotation.label ?? review.annotation.task_id}</strong><small>{review.annotation.task_id} · {Math.round((review.annotation.confidence ?? 0) * 100)}%</small></span></button>)}</div>{reviews.length === 0 && <Empty title="Queue is clear" detail="Low confidence or conflicting evidence will route candidates here." />}</aside><div className="review-center"><AnnotationCanvas imageUrl={images[0]?.url} annotations={draft ? [draft] : []} selectedId={draft?.id} visualContext={visualContext} onSelect={setSelectedId} onChange={setDraft} /><Trace events={selected ? events.filter((event) => event.run_id === selected.run_id) : events.slice(-12)} /></div><aside className="inspector panel"><span className="eyebrow">Validator evidence</span><h2>{draft?.label ?? "No selection"}</h2>{draft && <><label>Label<input value={draft.label ?? ""} onChange={(event) => setDraft({ ...draft, label: event.target.value })} /></label><div className="fact-grid"><span>Confidence<strong>{Math.round((draft.confidence ?? 0) * 100)}%</strong></span><span>Source<strong>{draft.source}</strong></span><span>Task<strong>{draft.task_id}</strong></span><span>Status<strong>{draft.review_status}</strong></span></div><label>Correction reason<select value={reason} onChange={(event) => setReason(event.target.value)}>{reasonOptions.map((value) => <option key={value}>{value}</option>)}</select></label><label>Reviewer note<textarea value={note} onChange={(event) => setNote(event.target.value)} placeholder="What changed, and why?" /></label><button onClick={save}>Save geometry revision</button><div className="decision-row"><button className="primary" onClick={() => decide("accept")}>Accept</button><button onClick={() => decide("reject")}>Reject</button><button className="danger" onClick={() => decide("delete")}>Delete</button></div><button className="text-button" onClick={() => api.revisions(draft.id).then((value) => alert(JSON.stringify(value.revisions, null, 2)))}>View revision history →</button></>}</aside></section>;
}

function Trace({ events }: { events: RunEvent[] }) {
  const icon = (kind: string) => kind.includes("model") ? "model-call" : kind.includes("tool") ? "tool-call" : kind.includes("validation") ? "validate" : kind.includes("review") ? "review" : "agent-trace";
  return <div className="trace-panel panel"><div><span className="eyebrow">Visible execution events</span><h3>Agent trace</h3><small>No hidden chain-of-thought</small></div><div className="trace-strip" aria-label="Agent trace events">{events.slice(-10).map((event) => <article key={event.event_id}><span><img src={`/brand/core/icons/${icon(event.kind)}.svg`} alt="" aria-hidden="true" /></span><div><strong>{event.kind.replaceAll("_", " ")}</strong><small>{event.task_id ?? "run"} · {new Date(event.occurred_at).toLocaleTimeString()}</small></div></article>)}</div></div>;
}

function SkillsPage({ onError }: { onError: (value: string) => void }) {
  const [skills, setSkills] = useState<SkillDetail[]>([]);
  useEffect(() => { void api.skills().then(setSkills).catch((error: Error) => onError(error.message)); }, []);
  return <section className="page-stack"><div className="boundary-note"><span>AnnotAgent</span><i>DomainSkill registry boundary</i><span>Installed Skills</span></div>{skills.map((skill) => <Panel key={skill.id} title={`${skill.display_name} · v${skill.version}`} eyebrow={skill.id}><p className="lede">{skill.description}</p><div className="skill-columns"><TagGroup title="Node templates" values={skill.tasks.map((task) => task.id)} /><TagGroup title="Registered tools" values={skill.tools} /><TagGroup title="Validators" values={skill.validators} /><TagGroup title="Refiners" values={skill.refiners} /><TagGroup title="Correction taxonomy" values={skill.correction_taxonomy} /><TagGroup title="Prompt resources" values={skill.resources} /></div></Panel>)}{skills.length === 0 && <Empty title="No Skills installed" detail="Install a registered extension before creating a runnable Project." />}</section>;
}

function SettingsPage({ onError }: { onError: (value: string) => void }) {
  const [settings, setSettings] = useState<Record<string, any>>();
  const [presetId, setPresetId] = useState("mock");
  const [key, setKey] = useState("");
  const [message, setMessage] = useState("");
  const [saving, setSaving] = useState(false);
  const credentialPresetRef = useRef("custom");
  useEffect(() => { void api.settings().then((value) => { setSettings(value); setPresetId(inferProviderPreset(value).id); credentialPresetRef.current = inferConfiguredProviderPreset(value).id; }).catch((error: Error) => onError(error.message)); }, []);
  if (!settings) return <section className="page-stack"><Empty title="Loading settings" detail="Reading the saved workspace configuration." /></section>;
  const provider = settings.provider ?? {};
  const pricing = settings.pricing ?? {};
  const budget = settings.budget ?? {};
  const preset = getProviderPreset(presetId);
  const providerChanged = !preset.offline && credentialPresetRef.current !== preset.id;
  const customModel = !preset.custom && !preset.offline && !isCatalogModel(preset, provider.model);
  const setProvider = (field: string, value: unknown) => setSettings({ ...settings, provider: { ...provider, [field]: value } });
  const chooseProvider = (id: string) => { setPresetId(id); setSettings(applyProviderPreset(settings, id)); setKey(""); setMessage(""); };
  const finish = (value: Record<string, unknown>, nextMessage: string, updateCredential = false) => { setSettings(value); setKey(""); setMessage(nextMessage); if (updateCredential && !preset.offline) credentialPresetRef.current = preset.id; };
  const save = () => { setSaving(true); const clearMismatchedKey = providerChanged && settings.api_key_persisted && !key; void api.saveSettings({ ...settings, api_key: key || undefined, clear_saved_api_key: clearMismatchedKey || undefined }).then((value) => finish(value, clearMismatchedKey ? `Saved ${preset.shortLabel}. The previous provider key was removed; add a ${preset.shortLabel} key before running.` : `Saved ${preset.shortLabel} locally. Future runs will use this workspace model binding.`, true)).catch((error: Error) => onError(error.message)).finally(() => setSaving(false)); };
  const clearKey = () => { setSaving(true); void api.saveSettings({ ...settings, clear_saved_api_key: true }).then((value) => finish(value, "Saved API key removed from the system keychain.", true)).catch((error: Error) => onError(error.message)).finally(() => setSaving(false)); };
  return <section className="settings-grid"><Panel title="Vision model provider" eyebrow="Workspace default binding"><label>Provider<select value={presetId} onChange={(event) => chooseProvider(event.target.value)}>{PROVIDER_PRESETS.map((candidate) => <option key={candidate.id} value={candidate.id}>{candidate.label}</option>)}</select></label><div className={`provider-summary ${preset.offline ? "offline" : ""}`}><span className="provider-monogram" aria-hidden="true">{preset.shortLabel.slice(0, 2).toUpperCase()}</span><span><strong>{preset.shortLabel}</strong><small>{preset.description}</small></span>{preset.docsUrl && <a href={preset.docsUrl} target="_blank" rel="noreferrer">Provider docs ↗</a>}</div>{!preset.offline && <>{preset.custom ? <div className="form-grid"><label>Endpoint<input type="url" value={provider.endpoint ?? ""} onChange={(event) => setProvider("endpoint", event.target.value)} placeholder="https://provider.example/v1" /></label><label>Model<input value={provider.model ?? ""} onChange={(event) => setProvider("model", event.target.value)} placeholder="vision-model-id" /></label></div> : <><label>Vision model<select value={customModel ? CUSTOM_MODEL : provider.model ?? ""} onChange={(event) => setProvider("model", event.target.value === CUSTOM_MODEL ? "" : event.target.value)}>{preset.models.map((model) => <option key={model.id} value={model.id}>{model.label} — {model.hint}</option>)}<option value={CUSTOM_MODEL}>Another model ID…</option></select></label>{customModel && <label>Custom model ID<input autoFocus value={provider.model ?? ""} onChange={(event) => setProvider("model", event.target.value)} placeholder="Enter the exact model ID" /></label>}</>}{providerChanged && settings.api_key_persisted && !key && <div className="credential-notice" role="status">The saved key belongs to the previous provider. Paste your {preset.shortLabel} key now, or saving will safely remove the old key.</div>}<label>{preset.shortLabel} API key<input type="password" autoComplete="new-password" value={key} onChange={(event) => setKey(event.target.value)} placeholder={settings.api_key_persisted && !providerChanged ? "Stored in the system keychain · paste to replace" : `Paste your ${preset.shortLabel} key once`} /></label><div className="button-row"><button onClick={clearKey} disabled={saving || !settings.api_key_persisted}>Clear saved key</button><small>{settings.api_key_persisted && !providerChanged ? "Keychain protected · never returned by the API" : `Environment fallback: ${provider.api_key_env ?? "ANNOTAGENT_API_KEY"}`}</small></div><details className="advanced-settings"><summary>Advanced settings</summary><div className="form-grid">{!preset.custom && <label>Endpoint<input readOnly value={provider.endpoint ?? ""} /></label>}<label>API key environment<input value={provider.api_key_env ?? ""} onChange={(event) => setProvider("api_key_env", event.target.value)} /></label><label>Temperature<input type="number" min="0" max="2" step="0.05" value={provider.temperature ?? 0.1} onChange={(event) => setProvider("temperature", Number(event.target.value))} /></label><label>Timeout seconds<input type="number" min="1" value={provider.request_timeout_seconds ?? 120} onChange={(event) => setProvider("request_timeout_seconds", Number(event.target.value))} /></label><label>Max output tokens<input type="number" min="1" value={provider.max_output_tokens ?? 4096} onChange={(event) => setProvider("max_output_tokens", Number(event.target.value))} /></label><label>Retries<input type="number" min="0" value={provider.max_retries ?? 2} onChange={(event) => setProvider("max_retries", Number(event.target.value))} /></label></div><small>Protocol: OpenAI Chat Completions · image input + function tools</small></details></>}{preset.offline && <div className="offline-note">Ready to run immediately. Mock keeps your real provider configuration and saved key untouched.</div>}{settings.credential_store_error && <div className="error-banner" role="alert"><span>System keychain unavailable: {String(settings.credential_store_error)}</span></div>}</Panel><Panel title="Pricing & hard budgets" eyebrow="Exact decimal accounting"><div className="json-settings"><div><h3>Pricing</h3>{Object.entries(pricing).map(([name, value]) => <label key={name}>{name}<input value={String(value)} onChange={(event) => setSettings({ ...settings, pricing: { ...pricing, [name]: event.target.value } })} /></label>)}</div><div><h3>Budget</h3>{Object.entries(budget).map(([name, value]) => <label key={name}>{name}<input value={String(value)} onChange={(event) => setSettings({ ...settings, budget: { ...budget, [name]: name === "max_cost" ? event.target.value : Number(event.target.value) } })} /></label>)}</div></div></Panel><div className="settings-save" aria-live="polite"><span>{message || (settings.settings_persisted ? `Saved at ${settings.settings_path}` : "Save once to keep these settings across restarts.")}</span><button className="primary" onClick={save} disabled={saving || (!preset.offline && (!provider.endpoint || !provider.model))}>{saving ? "Saving…" : "Save settings"}</button></div></section>;
}

function CreateProject({ onClose, onCreated, onError }: { onClose: () => void; onCreated: () => void; onError: (value: string) => void }) {
  const [id, setId] = useState("new-project");
  const [skills, setSkills] = useState<SkillDetail[]>([]);
  const [skillId, setSkillId] = useState("");
  const [yaml, setYaml] = useState("");
  useEffect(() => { void api.skills().then((items) => { setSkills(items); setSkillId(items[0]?.id ?? ""); setYaml(items[0]?.project_template ?? ""); }).catch((error: Error) => onError(error.message)); }, []);
  const chooseSkill = (value: string) => { setSkillId(value); setYaml(skills.find((skill) => skill.id === value)?.project_template ?? ""); };
  return <div className="modal-backdrop"><div className="modal"><span className="eyebrow">Validated Project schema</span><h2>Create Project</h2><label>Workspace ID<input value={id} onChange={(event) => setId(event.target.value)} /></label><label>Starter Skill<select value={skillId} onChange={(event) => chooseSkill(event.target.value)}>{skills.map((skill) => <option key={skill.id} value={skill.id}>{skill.display_name}</option>)}</select></label><label>project.yaml<textarea className="yaml-editor" value={yaml} onChange={(event) => setYaml(event.target.value)} /></label><div className="button-row"><button onClick={onClose}>Cancel</button><button className="primary" onClick={() => api.createProject(id, yaml).then(onCreated).catch((error: Error) => onError(error.message))}>Validate & create</button></div></div></div>;
}

function Panel({ title, eyebrow, children }: { title: string; eyebrow: string; children: React.ReactNode }) { return <section className="panel"><span className="eyebrow">{eyebrow}</span><h2>{title}</h2>{children}</section>; }
function Metric({ label, value, detail, accent, live }: { label: string; value: string | number; detail: string; accent?: boolean; live?: boolean }) { return <article className={`metric ${accent ? "accent" : ""}`}><span>{label}{live && <i className="live-dot" />}</span><strong>{value}</strong><small>{detail}</small></article>; }
function Fact({ label, value }: { label: string; value: string | number }) { return <div className="detail-fact"><span>{label}</span><strong>{value}</strong></div>; }
function Status({ status }: { status: string }) { const normalized = status.replaceAll(" ", "_").toLowerCase(); const presentation = normalized === "completed" || normalized === "confirmed" || normalized === "auto_accepted" || normalized === "published" || normalized === "valid" ? { tone: "auto-accepted", label: normalized === "published" ? "Published" : normalized === "valid" ? "Valid" : "Auto accepted" } : normalized === "awaiting_review" || normalized === "needs_review" ? { tone: "needs-review", label: "Needs review" } : normalized === "cancelled" || normalized === "rejected" || normalized === "archived" ? { tone: "rejected", label: normalized === "archived" ? "Archived" : "Rejected" } : normalized === "failed" || normalized === "budget_exceeded" ? { tone: "failed", label: "Failed" } : normalized === "running" || normalized === "paused" ? { tone: "running", label: normalized === "paused" ? "Paused" : "Running" } : { tone: "draft", label: "Draft" }; return <span className={`status status-${presentation.tone}`}>{presentation.label}</span>; }
function Empty({ title, detail }: { title: string; detail: string }) { return <div className="empty" role="status"><img src="/brand/core/annotagent-mark.svg" alt="" aria-hidden="true" /><strong>{title}</strong><small>{detail}</small></div>; }
function TagGroup({ title, values }: { title: string; values: string[] }) { return <div><h3>{title}</h3><div className="tags">{values.length ? values.map((value) => <span key={value}>{value}</span>) : <small>None</small>}</div></div>; }
