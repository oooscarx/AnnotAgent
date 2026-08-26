import { useEffect, useState } from "react";
import { api, subscribeEvents } from "./api";
import { AnnotationCanvas } from "./components/AnnotationCanvas";
import type {
  Annotation,
  HistoryRun,
  ImageItem,
  ProjectSummary,
  ReviewItem,
  RunEvent,
  SkillDetail,
} from "./types";

type Page = "dashboard" | "project" | "review" | "skills" | "settings";

export function App() {
  const [page, setPage] = useState<Page>("dashboard");
  const [projects, setProjects] = useState<ProjectSummary[]>([]);
  const [runs, setRuns] = useState<HistoryRun[]>([]);
  const [projectId, setProjectId] = useState("");
  const [events, setEvents] = useState<RunEvent[]>([]);
  const [error, setError] = useState("");

  const refresh = () =>
    api.dashboard().then((data) => {
      setProjects(data.projects);
      setRuns(data.runs);
      setProjectId((current) => current || data.projects[0]?.id || "");
    }).catch((reason: Error) => setError(reason.message));

  useEffect(() => {
    void refresh();
  }, []);
  useEffect(() => subscribeEvents((event) => {
    setEvents((previous) => [...previous.slice(-149), event]);
    if (["run_completed", "review_requested", "run_failed"].includes(event.kind)) refresh();
  }), []);

  const selectProject = (id: string) => {
    setProjectId(id);
    setPage("project");
  };
  const selectedProject = projects.find((project) => project.id === projectId);

  return (
    <div className="app-shell">
      <a className="skip-link" href="#main-content">Skip to workspace</a>
      <aside className="sidebar aa-dark">
        <a className="brand" href="#dashboard" aria-label="RoboCup AnnotAgent dashboard" onClick={() => setPage("dashboard")}>
          <img className="brand-lockup" src="/brand/robocup-annotagent-lockup-dark.svg" alt="RoboCup AnnotAgent" />
          <img className="brand-mark-compact" src="/brand/annotagent-mark-dark-surface.svg" alt="" aria-hidden="true" />
        </a>
        <nav aria-label="Primary navigation">
          <Nav icon="history" active={page === "dashboard"} onClick={() => setPage("dashboard")}>Dashboard</Nav>
          <Nav icon="bbox" active={page === "project"} onClick={() => setPage("project")}>Project</Nav>
          <Nav icon="review" active={page === "review"} onClick={() => setPage("review")}>Review</Nav>
          <Nav icon="tool-call" active={page === "skills"} onClick={() => setPage("skills")}>Skills</Nav>
          <Nav icon="settings" active={page === "settings"} onClick={() => setPage("settings")}>Settings</Nav>
        </nav>
        <div className="sidebar-foot">
          <span className="live-dot" aria-hidden="true" /> SSE connected
          <small>{events.at(-1)?.kind.replaceAll("_", " ") ?? "waiting for events"}</small>
        </div>
      </aside>
      <main id="main-content">
        <header className="topbar">
          <div>
            <span className="eyebrow">AnnotAgent Core · Perception workspace</span>
            <h1>{page === "dashboard" ? "Operations overview" : page[0].toUpperCase() + page.slice(1)}</h1>
          </div>
          <div className="project-switch">
            {selectedProject?.skill_id === "robocup" && <span className="skill-badge"><img src="/brand/robocup-skill-badge.svg" alt="" aria-hidden="true" />RoboCup Skill</span>}
            <span aria-hidden="true">Active project</span>
            <label className="sr-only" htmlFor="active-project">Active project</label>
            <select id="active-project" value={projectId} onChange={(event) => selectProject(event.target.value)}>
              <option value="">No project</option>
              {projects.map((project) => <option key={project.id} value={project.id}>{project.name}</option>)}
            </select>
          </div>
        </header>
        {error && <div className="error-banner" role="alert"><span>{error}</span><button aria-label="Dismiss error" onClick={() => setError("")}>Dismiss</button></div>}
        {page === "dashboard" && <Dashboard projects={projects} runs={runs} onSelect={selectProject} onRefresh={refresh} onError={setError} />}
        {page === "project" && <ProjectPage project={selectedProject} events={events} onError={setError} />}
        {page === "review" && <ReviewPage projectId={projectId} projects={projects} events={events} onError={setError} />}
        {page === "skills" && <SkillsPage onError={setError} />}
        {page === "settings" && <SettingsPage onError={setError} />}
      </main>
    </div>
  );
}

function Nav({ icon, active, onClick, children }: { icon: string; active: boolean; onClick: () => void; children: string }) {
  return <button className={active ? "active" : ""} aria-current={active ? "page" : undefined} onClick={onClick}><img src={`/brand/icons/${icon}.svg`} alt="" aria-hidden="true" />{children}</button>;
}

function Dashboard({
  projects,
  runs,
  onSelect,
  onRefresh,
  onError,
}: {
  projects: ProjectSummary[];
  runs: HistoryRun[];
  onSelect: (id: string) => void;
  onRefresh: () => void;
  onError: (value: string) => void;
}) {
  const [creating, setCreating] = useState(false);
  const completed = runs.filter((run) => run.status === "completed").length;
  const reviews = runs.filter((run) => run.status === "awaiting_review").length;
  return (
    <section className="page-stack">
      <div className="hero-panel aa-dark">
        <div>
          <span className="kicker">ANNOTAGENT CORE + ROBOCUP SKILL</span>
          <h2>Turn model proposals into<br /><em>defensible ground truth.</em></h2>
          <p>One auditable path from image evidence through tools, deterministic checks, pixel refinement, and human correction.</p>
        </div>
        <div className="hero-actions">
          <button className="primary" onClick={() => setCreating(true)}>New project</button>
          <button onClick={onRefresh}>Refresh state</button>
        </div>
      </div>
      <div className="metrics-grid">
        <Metric label="Projects" value={projects.length} detail={`${projects.reduce((sum, project) => sum + project.image_count, 0)} images registered`} />
        <Metric label="Runs completed" value={completed} detail={`${runs.length} total executions`} />
        <Metric label="Awaiting review" value={reviews} detail="Human attention queue" accent={reviews > 0} />
        <Metric label="Runtime link" value="LIVE" detail="SSE · SQLite · Axum" live />
      </div>
      <div className="split-grid">
        <Panel title="Projects" eyebrow="Workspace inventory">
          <div className="table-list">
            {projects.length === 0 && <Empty title="No projects yet" detail="Create one with validated project YAML." />}
            {projects.map((project) => (
              <button className="project-row" key={project.id} onClick={() => onSelect(project.id)}>
                <span className="project-avatar">{project.name.slice(0, 2).toUpperCase()}</span>
                <span><strong>{project.name}</strong><small>{project.skill_id} · {project.image_count} images</small></span>
                <Status status={project.recent_run?.status ?? "pending"} />
                <b>→</b>
              </button>
            ))}
          </div>
        </Panel>
        <Panel title="Recent runs" eyebrow="Auditable history">
          <div className="run-list">
            {runs.slice(0, 6).map((run) => (
              <div key={run.id}>
                <span className="event-rail" />
                <span><strong>{run.project_name}</strong><small>{new Date(run.created_at).toLocaleString()} · {run.model}</small></span>
                <Status status={run.status} />
              </div>
            ))}
            {runs.length === 0 && <Empty title="No runs recorded" detail="A Mock run will appear here immediately." />}
          </div>
        </Panel>
      </div>
      {creating && <CreateProject onClose={() => setCreating(false)} onCreated={() => { setCreating(false); onRefresh(); }} onError={onError} />}
    </section>
  );
}

function ProjectPage({ project, events, onError }: { project?: ProjectSummary; events: RunEvent[]; onError: (value: string) => void }) {
  const [images, setImages] = useState<ImageItem[]>([]);
  const [activeRun, setActiveRun] = useState("");
  const [filter, setFilter] = useState("all");
  useEffect(() => {
    if (project) api.images(project.id).then((value) => setImages(value.images)).catch((error: Error) => onError(error.message));
  }, [project?.id]);
  const runEvents = events.filter((event) => event.run_id === activeRun);
  const usage = [...runEvents].reverse().find((event) => event.kind === "usage_updated");
  const lastTask = [...runEvents].reverse().find((event) => event.task_id)?.task_id;
  const latestState = [...runEvents].reverse().find((event) => event.payload.type === "state");
  const visibleStatus = (latestState?.payload.data.to as string | undefined) ?? (activeRun ? "running" : "pending");
  if (!project) return <Empty title="Select a project" detail="Choose a workspace project from the switcher." />;
  const start = () => api.startRun(project.id).then((run) => setActiveRun(run.run_id)).catch((error: Error) => onError(error.message));
  const control = (action: "pause" | "resume" | "cancel") => activeRun && api.control(activeRun, action).catch((error: Error) => onError(error.message));
  return (
    <section className="page-stack">
      <div className="toolbar-panel">
        <div><span className="eyebrow">{project.skill_id} skill</span><h2>{project.name}</h2><p>{project.image_count} images · deterministic quality gates enabled</p></div>
        <div className="button-row" aria-label="Run controls">
          <button className="primary" onClick={start}>Start image run</button>
          <button disabled={!activeRun} onClick={() => control("pause")}><img src="/brand/icons/pause.svg" alt="" aria-hidden="true" />Pause</button>
          <button disabled={!activeRun} onClick={() => control("resume")}><img src="/brand/icons/resume.svg" alt="" aria-hidden="true" />Resume</button>
          <button className="danger" disabled={!activeRun} onClick={() => control("cancel")}><img src="/brand/icons/cancel.svg" alt="" aria-hidden="true" />Cancel</button>
        </div>
      </div>
      <div className="filters">
        {["all", "unprocessed", "auto accepted", "needs review", "confirmed", "failed"].map((value) => <button key={value} className={filter === value ? "active" : ""} onClick={() => setFilter(value)}>{value}</button>)}
      </div>
      {activeRun && <div className="run-progress aa-dark" aria-live="polite">
        <div><span className="live-dot" aria-hidden="true" /><strong>Run {activeRun.slice(0, 8)}</strong><small>{lastTask ?? "starting"} · {runEvents.at(-1)?.kind.replaceAll("_", " ") ?? "queued"}</small></div>
        <div className="progress-track"><i style={{ width: `${Math.min(100, runEvents.length * 3)}%` }} /></div>
        <pre>{usage ? JSON.stringify(usage.payload.data, null, 2) : "usage pending"}</pre>
      </div>}
      <div className="image-grid">
        {images.map((image) => <article key={image.index}><img src={image.url} alt={image.name} /><div><span><strong>{image.name}</strong><small>Image {image.index + 1}</small></span><Status status={visibleStatus} /></div></article>)}
        {images.length === 0 && <Empty title="No images" detail="Import images with the CLI or controlled workspace import API." />}
      </div>
    </section>
  );
}

function ReviewPage({ projectId, projects, events, onError }: { projectId: string; projects: ProjectSummary[]; events: RunEvent[]; onError: (value: string) => void }) {
  const [reviews, setReviews] = useState<ReviewItem[]>([]);
  const [selectedId, setSelectedId] = useState("");
  const [draft, setDraft] = useState<Annotation>();
  const [reason, setReason] = useState("");
  const [reasonOptions, setReasonOptions] = useState<string[]>([]);
  const [note, setNote] = useState("");
  const [images, setImages] = useState<ImageItem[]>([]);
  const skillId = projects.find((project) => project.id === projectId)?.skill_id;
  const selected = reviews.find((review) => review.id === selectedId) ?? reviews[0];
  const refresh = () => api.reviews().then((value) => { setReviews(value.reviews); if (!selectedId && value.reviews[0]) setSelectedId(value.reviews[0].id); }).catch((error: Error) => onError(error.message));
  useEffect(() => {
    void refresh();
    void api.skills().then((skills) => {
      const options = skills.find((skill) => skill.id === projects.find((project) => project.id === projectId)?.skill_id)?.correction_taxonomy
        ?? skills[0]?.correction_taxonomy
        ?? [];
      setReasonOptions(options);
      setReason((current) => current || options[0] || "");
    }).catch((error: Error) => onError(error.message));
  }, []);
  useEffect(() => { if (projectId) api.images(projectId).then((value) => setImages(value.images)).catch((error: Error) => onError(error.message)); }, [projectId]);
  useEffect(() => setDraft(selected?.annotation), [selected?.id]);
  const save = () => draft && api.revise(draft, reason).then(refresh).catch((error: Error) => onError(error.message));
  const decide = (decision: "accept" | "reject" | "delete") => {
    const effectiveProject = projectId || projects[0]?.id;
    if (!selected || !effectiveProject) return onError("Select a project before recording a review decision.");
    api.decide(selected.id, effectiveProject, decision, reason, note).then(refresh).catch((error: Error) => onError(error.message));
  };
  return (
    <section className="review-layout">
      <aside className="review-queue panel"><span className="eyebrow">Human attention</span><h2>Review queue <b>{reviews.length}</b></h2>
        <div className="queue-items" aria-label="Annotations requiring review">{reviews.map((review) => <button key={review.id} aria-pressed={selected?.id === review.id} className={selected?.id === review.id ? "active" : ""} onClick={() => setSelectedId(review.id)}><span aria-hidden="true">{review.annotation.label?.slice(0, 2).toUpperCase() ?? "?"}</span><span><strong>{review.annotation.label ?? review.annotation.task_id}</strong><small>{review.annotation.task_id} · {Math.round((review.annotation.confidence ?? 0) * 100)}%</small></span></button>)}</div>
        {reviews.length === 0 && <Empty title="Queue is clear" detail="Low confidence or conflicting evidence will route candidates here." />}
      </aside>
      <div className="review-center">
        <AnnotationCanvas imageUrl={images[0]?.url} annotations={draft ? [draft] : []} selectedId={draft?.id} skillId={skillId} onSelect={setSelectedId} onChange={setDraft} />
        <Trace events={selected ? events.filter((event) => event.run_id === selected.run_id) : events.slice(-12)} />
      </div>
      <aside className="inspector panel"><span className="eyebrow">Validator evidence</span><h2>{draft?.label ?? "No selection"}</h2>
        {draft && <>
          <label>Label<input value={draft.label ?? ""} onChange={(event) => setDraft({ ...draft, label: event.target.value })} /></label>
          <div className="fact-grid"><span>Confidence<strong>{Math.round((draft.confidence ?? 0) * 100)}%</strong></span><span>Source<strong>{draft.source}</strong></span><span>Task<strong>{draft.task_id}</strong></span><span>Status<strong>{draft.review_status}</strong></span></div>
          <label>Correction reason<select value={reason} onChange={(event) => setReason(event.target.value)}>{reasonOptions.map((value) => <option key={value}>{value}</option>)}</select></label>
          <label>Reviewer note<textarea value={note} onChange={(event) => setNote(event.target.value)} placeholder="What changed, and why?" /></label>
          <button onClick={save}>Save geometry revision</button>
          <div className="decision-row"><button className="primary" onClick={() => decide("accept")}>Accept</button><button onClick={() => decide("reject")}>Reject</button><button className="danger" onClick={() => decide("delete")}>Delete</button></div>
          <button className="text-button" onClick={() => api.revisions(draft.id).then((value) => alert(JSON.stringify(value.revisions, null, 2)))}>View revision history →</button>
        </>}
      </aside>
    </section>
  );
}

function Trace({ events }: { events: RunEvent[] }) {
  const icon = (kind: string) => kind.includes("model") ? "model-call" : kind.includes("tool") ? "tool-call" : kind.includes("validation") ? "validate" : kind.includes("review") ? "review" : "agent-trace";
  return <div className="trace-panel panel"><div><span className="eyebrow">Visible execution events</span><h3>Agent trace</h3><small>No hidden chain-of-thought</small></div><div className="trace-strip" aria-label="Agent trace events">{events.slice(-10).map((event) => <article key={event.event_id}><span><img src={`/brand/icons/${icon(event.kind)}.svg`} alt="" aria-hidden="true" /></span><div><strong>{event.kind.replaceAll("_", " ")}</strong><small>{event.task_id ?? "run"} · {new Date(event.occurred_at).toLocaleTimeString()}</small></div></article>)}</div></div>;
}

function SkillsPage({ onError }: { onError: (value: string) => void }) {
  const [skills, setSkills] = useState<SkillDetail[]>([]);
  useEffect(() => { api.skills().then(setSkills).catch((error: Error) => onError(error.message)); }, []);
  return <section className="page-stack"><div className="boundary-note"><span>CORE</span><i>DomainSkill trait boundary</i><span>SKILL</span></div>{skills.map((skill) => <Panel key={skill.id} title={`${skill.display_name} · v${skill.version}`} eyebrow={skill.id}><p className="lede">{skill.description}</p><div className="skill-columns"><TagGroup title="Task DAG" values={skill.tasks.map((task) => task.id)} /><TagGroup title="Registered tools" values={skill.tools} /><TagGroup title="Validators" values={skill.validators} /><TagGroup title="Refiners" values={skill.refiners} /><TagGroup title="Correction taxonomy" values={skill.correction_taxonomy} /><TagGroup title="On-demand resources" values={skill.resources} /></div></Panel>)}</section>;
}

function SettingsPage({ onError }: { onError: (value: string) => void }) {
  const [settings, setSettings] = useState<Record<string, any>>();
  const [key, setKey] = useState("");
  const [message, setMessage] = useState("");
  const [saving, setSaving] = useState(false);
  useEffect(() => { api.settings().then(setSettings).catch((error: Error) => onError(error.message)); }, []);
  if (!settings) return <Empty title="Loading settings" detail="Reading the saved workspace configuration." />;
  const provider = settings.provider ?? {};
  const pricing = settings.pricing ?? {};
  const budget = settings.budget ?? {};
  const setProvider = (field: string, value: string | number) => setSettings({ ...settings, provider: { ...provider, [field]: value } });
  const finish = (value: Record<string, unknown>, nextMessage: string) => {
    setSettings(value);
    setKey("");
    setMessage(nextMessage);
  };
  const save = () => {
    setSaving(true);
    api.saveSettings({ ...settings, api_key: key || undefined })
      .then((value) => finish(value, "Saved locally. Future image runs will use these settings."))
      .catch((error: Error) => onError(error.message))
      .finally(() => setSaving(false));
  };
  const clearKey = () => {
    setSaving(true);
    api.saveSettings({ ...settings, clear_saved_api_key: true })
      .then((value) => finish(value, "Saved API key removed from the system keychain."))
      .catch((error: Error) => onError(error.message))
      .finally(() => setSaving(false));
  };
  return <section className="settings-grid"><Panel title="Vision model provider" eyebrow="Saved workspace default"><div className="form-grid"><label>Default run provider<select value={settings.default_provider ?? "mock"} onChange={(event) => setSettings({ ...settings, default_provider: event.target.value })}><option value="mock">Mock (offline test)</option><option value="openai_compatible">OpenAI-compatible</option></select></label><label>Endpoint<input value={provider.endpoint ?? ""} onChange={(event) => setProvider("endpoint", event.target.value)} /></label><label>Model<input value={provider.model ?? ""} onChange={(event) => setProvider("model", event.target.value)} /></label><label>API key environment<input value={provider.api_key_env ?? ""} onChange={(event) => setProvider("api_key_env", event.target.value)} /></label><label>Protocol<select value={provider.protocol ?? "chat_completions"} onChange={(event) => setProvider("protocol", event.target.value)}><option value="chat_completions">Chat Completions</option></select></label><label>Temperature<input type="number" step="0.05" value={provider.temperature ?? 0.1} onChange={(event) => setProvider("temperature", Number(event.target.value))} /></label><label>Timeout seconds<input type="number" value={provider.request_timeout_seconds ?? 120} onChange={(event) => setProvider("request_timeout_seconds", Number(event.target.value))} /></label></div><label>Saved API key<input type="password" autoComplete="new-password" value={key} onChange={(event) => setKey(event.target.value)} placeholder={settings.api_key_persisted ? "Stored in the system keychain" : "Paste once to save in the system keychain"} /></label><div className="button-row"><button onClick={clearKey} disabled={saving || !settings.api_key_persisted}>Clear saved key</button><small>{settings.api_key_persisted ? "Keychain protected · never returned by the API" : "Environment variable fallback remains available"}</small></div>{settings.credential_store_error && <div className="error-banner" role="alert"><span>System keychain unavailable: {String(settings.credential_store_error)}</span></div>}</Panel><Panel title="Pricing & hard budgets" eyebrow="Exact decimal accounting"><div className="json-settings"><div><h3>Pricing</h3>{Object.entries(pricing).map(([name, value]) => <label key={name}>{name}<input value={String(value)} onChange={(event) => setSettings({ ...settings, pricing: { ...pricing, [name]: event.target.value } })} /></label>)}</div><div><h3>Budget</h3>{Object.entries(budget).map(([name, value]) => <label key={name}>{name}<input value={String(value)} onChange={(event) => setSettings({ ...settings, budget: { ...budget, [name]: name === "max_cost" ? event.target.value : Number(event.target.value) } })} /></label>)}</div></div></Panel><div className="settings-save" aria-live="polite"><span>{message || (settings.settings_persisted ? `Saved at ${settings.settings_path}` : "Save once to keep these settings across restarts.")}</span><button className="primary" onClick={save} disabled={saving}>{saving ? "Saving…" : "Save settings"}</button></div></section>;
}

function CreateProject({ onClose, onCreated, onError }: { onClose: () => void; onCreated: () => void; onError: (value: string) => void }) {
  const [id, setId] = useState("robocup-demo");
  const [yaml, setYaml] = useState("");
  useEffect(() => {
    void api.skills()
      .then((skills) => setYaml(skills[0]?.project_template ?? ""))
      .catch((error: Error) => onError(error.message));
  }, []);
  return <div className="modal-backdrop"><div className="modal"><span className="eyebrow">Validated project schema</span><h2>Create project</h2><label>Workspace ID<input value={id} onChange={(event) => setId(event.target.value)} /></label><label>project.yaml<textarea className="yaml-editor" value={yaml} onChange={(event) => setYaml(event.target.value)} /></label><div className="button-row"><button onClick={onClose}>Cancel</button><button className="primary" onClick={() => api.createProject(id, yaml).then(onCreated).catch((error: Error) => onError(error.message))}>Validate & create</button></div></div></div>;
}

function Panel({ title, eyebrow, children }: { title: string; eyebrow: string; children: React.ReactNode }) { return <section className="panel"><span className="eyebrow">{eyebrow}</span><h2>{title}</h2>{children}</section>; }
function Metric({ label, value, detail, accent, live }: { label: string; value: string | number; detail: string; accent?: boolean; live?: boolean }) { return <article className={`metric ${accent ? "accent" : ""}`}><span>{label}{live && <i className="live-dot" />}</span><strong>{value}</strong><small>{detail}</small></article>; }
function Status({ status }: { status: string }) {
  const normalized = status.replaceAll(" ", "_").toLowerCase();
  const presentation = normalized === "completed" || normalized === "confirmed" || normalized === "auto_accepted"
    ? { tone: "auto-accepted", label: "Auto accepted" }
    : normalized === "awaiting_review" || normalized === "needs_review"
      ? { tone: "needs-review", label: "Needs review" }
      : normalized === "cancelled" || normalized === "rejected"
        ? { tone: "rejected", label: "Rejected" }
        : normalized === "failed" || normalized === "budget_exceeded"
          ? { tone: "failed", label: "Failed" }
          : normalized === "running" || normalized === "paused"
            ? { tone: "running", label: normalized === "paused" ? "Paused" : "Running" }
            : { tone: "draft", label: "Draft" };
  return <span className={`status status-${presentation.tone}`}>{presentation.label}</span>;
}
function Empty({ title, detail }: { title: string; detail: string }) { return <div className="empty" role="status"><img src="/brand/annotagent-mark.svg" alt="" aria-hidden="true" /><strong>{title}</strong><small>{detail}</small></div>; }
function TagGroup({ title, values }: { title: string; values: string[] }) { return <div><h3>{title}</h3><div className="tags">{values.map((value) => <span key={value}>{value}</span>)}</div></div>; }
