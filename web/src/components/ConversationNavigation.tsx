import { useState } from "react";
import type { ConversationMessage, ConversationTask } from "../types";

/** Current backend has one primary conversation per Project, with independent tasks.
 * Show those real tasks; do not fabricate cross-project conversation inventory. */
export function ConversationNavigation({ projectName, tasks, messages, selectedTask, ready, onSelect, onProjects, onSettings }: {
  projectName: string;
  tasks: ConversationTask[];
  messages: ConversationMessage[];
  selectedTask?: string;
  ready: boolean;
  onSelect: (task: string) => void;
  onProjects: () => void;
  onSettings: () => void;
}) {
  const [search, setSearch] = useState("");
  const [collapsed, setCollapsed] = useState(() => window.matchMedia("(max-width: 1024px)").matches);
  const entries = tasks.map(task => ({
    id: task.input.id,
    title: messages.find(message => message.input.id === task.input.source_message_id)?.input.text ?? `Task ${task.input.id.slice(0, 8)}`,
  }));
  const filtered = entries.filter(entry => entry.title.toLocaleLowerCase().includes(search.toLocaleLowerCase()));
  return <nav className="agent-conversation-navigation" data-collapsed={collapsed} aria-label="Conversations">
    <button type="button" aria-expanded={!collapsed} aria-controls="agent-conversation-list" onClick={() => setCollapsed(value => !value)}>{collapsed ? "Show tasks" : "Hide tasks"}</button>
    <div id="agent-conversation-list" hidden={collapsed}>
      <p className="agent-navigation-project">{projectName}</p>
      <label>Search tasks<input type="search" value={search} onChange={event => setSearch(event.target.value)} /></label>
      {!ready ? <p role="status">Loading saved tasks…</p> : !entries.length ? <p>No saved tasks yet. Start with a message.</p> : <ul>{filtered.map(entry => <li key={entry.id}><button type="button" aria-current={entry.id === selectedTask ? "page" : undefined} onClick={() => onSelect(entry.id)} title={entry.title}>{entry.title}</button></li>)}</ul>}
      {ready && entries.length > 0 && filtered.length === 0 && <p>No matching tasks.</p>}
      <footer><button type="button" onClick={onProjects}>Projects</button><button type="button" onClick={onSettings}>Settings</button></footer>
    </div>
  </nav>;
}
