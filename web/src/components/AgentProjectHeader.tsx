import { useEffect, useRef, useState, type RefObject } from "react";
import type { ConversationMessage, ConversationTask } from "../types";
import { t } from "../i18n";

/** Task selection is a transient surface, never a reserved workspace column. */
export function AgentProjectHeader({ projectName, tasks, messages, selectedTask, ready, artifactsOpen, titleRef, onSelect, onProjects, onSettings, onManagement, onToggleArtifacts }: {
  projectName: string; tasks: ConversationTask[]; messages: ConversationMessage[];
  selectedTask?: string; ready: boolean; artifactsOpen: boolean;
  titleRef?: RefObject<HTMLHeadingElement | null>;
  onSelect: (id: string) => void; onProjects: () => void; onSettings: () => void;
  onManagement: () => void; onToggleArtifacts: () => void;
}) {
  const [panel, setPanel] = useState<"tasks" | "management">();
  const [search, setSearch] = useState("");
  const root = useRef<HTMLElement>(null);
  const taskTrigger = useRef<HTMLButtonElement>(null);
  const menuTrigger = useRef<HTMLButtonElement>(null);
  const searchInput = useRef<HTMLInputElement>(null);
  const entries = tasks.map(task => ({ id: task.input.id, title: messages.find(message => message.input.id === task.input.source_message_id)?.input.text ?? `Task ${task.input.id.slice(0, 8)}` }));
  const close = () => { (panel === "tasks" ? taskTrigger : menuTrigger).current?.focus(); setPanel(undefined); };
  useEffect(() => {
    if (!panel) return;
    if (panel === "tasks") searchInput.current?.focus();
    const outside = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) setPanel(undefined);
    };
    document.addEventListener("pointerdown", outside);
    return () => document.removeEventListener("pointerdown", outside);
  }, [panel]);
  return <header ref={root} className="agent-project-header" aria-label="Project task context" onKeyDown={event => {
    if (event.key === "Escape" && !event.nativeEvent.isComposing && panel) { event.preventDefault(); close(); }
  }}>
    <button type="button" onClick={onProjects}>← {t("Projects")}</button>
    <h1 ref={titleRef} tabIndex={-1} title={projectName}>{projectName}</h1>
    <div className="agent-task-switch">
      {entries.length > 0 ? <button ref={taskTrigger} type="button" aria-label="Choose task" aria-expanded={panel === "tasks"} aria-controls="agent-task-popover" onClick={() => setPanel(panel === "tasks" ? undefined : "tasks")}>
        <span>{entries.find(entry => entry.id === selectedTask)?.title ?? t("Choose task")}</span> ▾
      </button> : <span className="muted">{ready ? t("New task") : t("Loading…")}</span>}
      {panel === "tasks" && <section id="agent-task-popover" className="agent-header-popover" aria-label="Task selection">
        <label>Search tasks<input ref={searchInput} type="search" value={search} onChange={event => setSearch(event.target.value)} /></label>
        <ul>{entries.filter(entry => entry.title.toLocaleLowerCase().includes(search.toLocaleLowerCase())).map(entry => <li key={entry.id}><button type="button" aria-current={entry.id === selectedTask ? "page" : undefined} onClick={() => { onSelect(entry.id); close(); }}>{entry.title}</button></li>)}</ul>
        {!entries.some(entry => entry.title.toLocaleLowerCase().includes(search.toLocaleLowerCase())) && <p>No matching tasks.</p>}
        <button type="button" onClick={close}>Close task selection</button>
      </section>}
    </div>
    <div className="agent-project-actions">
      <button type="button" aria-expanded={artifactsOpen} aria-controls="conversation-artifacts" aria-label={artifactsOpen ? "Close data and results" : "Open data and results"} onClick={onToggleArtifacts}>{t(artifactsOpen ? "Close data" : "Open data")}</button>
      <button type="button" onClick={onSettings}>{t("Settings")}</button>
      <button ref={menuTrigger} type="button" aria-label="Project menu" aria-expanded={panel === "management"} aria-controls="agent-management-popover" onClick={() => setPanel(panel === "management" ? undefined : "management")}>⋯</button>
      {panel === "management" && <section id="agent-management-popover" className="agent-header-popover" aria-label="Project management">
        <button type="button" onClick={() => { close(); onManagement(); }}>{t("Project management")}</button>
        <p>Data, pipelines, processing history, review, export and recycle bin.</p>
      </section>}
    </div>
  </header>;
}
