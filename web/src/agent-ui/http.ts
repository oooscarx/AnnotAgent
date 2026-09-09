import { request } from "../api";
import type { ImageItem, ProviderProfile, RegistryModelProfile } from "../types";
import type { WorkspaceAdapter, Snapshot, Task, Command, Settings, ImageId, Box, Phase, Action } from "./adapter";

type Page<T> = { items: T[]; next_cursor: string | number | null };
type Project = { project_id: string; project_owner_id: string; title: string; conversation_id: string | null };
type NavigationTask = { task_id: string; title: string; schema_revision: string; project_owner_id: string; conversation_id: string; state: Phase };
type Preference = { revision: number; model_profile_id: string | null };
type Workspace = {
  project_id: string; project_owner_id: string; conversation_id: string;
  task: { input: { id: string; schema_revision: string } };
  agent_model: Preference;
  actions: Partial<Record<"send" | "stop" | "resume" | "approve", Action>>;
  calls: { id: string; status: string; result: unknown }[];
  queue: { input?: { text?: string }; message?: { input?: { text?: string } }; cancelled_at?: string | null }[];
};
type Thread = { id: string; role: "user"; task_id: string; project_owner_id: string; conversation_id: string; message: { input: { text: string } } };
type SafeSettings = { revision: string; sections: { data_privacy: { workspace_id: string }; usage_budget: { future_run_budget: Record<string, unknown> & { max_cost?: string } } } };
export type Transport = <T>(path: string, init?: RequestInit) => Promise<T>;
const esc = encodeURIComponent;
const unsupported = (detail: string): never => { throw new Error(`尚未接通：${detail}。没有执行操作，也没有回退到演示结果。`); };
const initialSettings: Settings = { revision: "", theme: "system", language: "zh", font: "标准", density: "舒适", collapsed: false, providers: [], defaultModel: "", plugins: [], allowExternal: false, cache: 0, budget: "", range: "未来 Run 默认预算" };

/** Only this boundary knows HTTP routes. Reads never create conversations, tasks or execution. */
export class HttpAdapter implements WorkspaceAdapter {
  readonly kind = "http" as const;
  private state: Snapshot = { loading: true, projects: [], tasks: [], models: [], artifacts: [], usage: [], knownCost: "", protectedCache: 0, settings: initialSettings };
  private listeners = new Set<() => void>();
  private projects = new Map<string, Project>();
  private workspaces = new Map<string, Workspace>();
  private sequence = 0;
  private navigationSequence = 0;
  private controller?: AbortController;
  private safeSettings?: SafeSettings;
  constructor(private transport: Transport = request, private storage?: Storage) {}
  snapshot = () => this.state;
  subscribe = (listener: () => void) => { this.listeners.add(listener); return () => { this.listeners.delete(listener); }; };
  private emit(patch: Partial<Snapshot>) { this.state = { ...this.state, ...patch }; this.listeners.forEach(fn => fn()); }
  private key(suffix: string) { return `annotagent.http-ui.${this.state.workspaceId}.${suffix}`; }
  private stored<T>(suffix: string, fallback: T): T { try { return JSON.parse(this.storage?.getItem(this.key(suffix)) || "null") ?? fallback; } catch { return fallback; } }
  private save(suffix: string, value: unknown) { this.storage?.setItem(this.key(suffix), JSON.stringify(value)); }
  private async pages<T>(path: string, signal?: AbortSignal): Promise<T[]> {
    const items: T[] = [], seen = new Set<string>();
    let cursor: string | number | null = null;
    do {
      const url = `${path}${path.includes("?") ? "&" : "?"}limit=100${cursor === null ? "" : `&cursor=${esc(String(cursor))}`}`;
      const page: Page<T> = await this.transport(url, { signal });
      if (!Array.isArray(page.items)) throw new Error("服务器分页响应格式不正确");
      items.push(...page.items); cursor = page.next_cursor;
      if (cursor !== null && cursor !== undefined) {
        if (seen.has(String(cursor))) throw new Error("服务器分页游标重复，已停止读取");
        seen.add(String(cursor));
      }
    } while (cursor !== null && cursor !== undefined);
    return items;
  }
  private root(project: string) { const p = this.projects.get(project); if (!p) throw new Error("项目不存在"); return `/api/projects/${esc(project)}`; }
  private conversation(project: string) { const p = this.projects.get(project); if (!p?.conversation_id) throw new Error("项目尚无会话"); return `${this.root(project)}/conversations/${esc(p.conversation_id)}`; }
  private taskRoot(task: Task) { return `${this.conversation(task.project)}/tasks/${esc(task.id)}`; }
  private task(id: string) { const t = this.state.tasks.find(t => t.id === id); if (!t) throw new Error("任务不存在，不会自动切换到其他任务"); return t; }
  private checked(c: Command) { const task = this.task(c.task); if (task.project !== c.project || task.revision !== c.revision) throw new Error("任务归属或版本已变化，请重新读取"); return task; }

  refresh = async () => {
    const seq = ++this.navigationSequence;
    this.emit({ loading: true, error: undefined });
    try {
      const [nav, safe, providers, profiles] = await Promise.all([
        this.pages<Project>("/api/navigation"), this.transport<SafeSettings>("/api/settings?view=agent-ui"),
        this.transport<{ providers: ProviderProfile[] }>("/api/providers"), this.transport<{ models: RegistryModelProfile[] }>("/api/model-profiles"),
      ]);
      const rows = await Promise.all(nav.map(async p => ({ p, tasks: p.conversation_id ? await this.pages<NavigationTask>(`/api/projects/${esc(p.project_id)}/conversations/${esc(p.conversation_id)}/task-navigation`) : [] })));
      if (seq !== this.navigationSequence) return;
      this.projects = new Map(nav.map(p => [p.project_id, p])); this.safeSettings = safe;
      this.state = { ...this.state, workspaceId: safe.sections.data_privacy.workspace_id };
      const prefs = this.stored<Partial<Settings>>("preferences", {});
      const models = profiles.models.map(m => {
        const p = providers.providers.find(p => p.id === m.provider_id);
        const reason = !m.enabled || m.status === "disabled" ? "模型已禁用" : m.status !== "available" ? "模型尚未验证可用" : !p?.enabled ? "Provider 不可用" : !m.task_capabilities.includes("text_generation") || !m.protocol_features.tool_calls ? "不支持规划所需的文本和工具调用" : undefined;
        return { id: m.id, name: m.display_name, providerId: m.provider_id, provider: p?.display_name || "Provider 不存在", reason };
      });
      const tasks: Task[] = rows.flatMap(({ p, tasks }) => tasks.map(t => {
        if (t.project_owner_id !== p.project_owner_id || t.conversation_id !== p.conversation_id) throw new Error("服务器任务归属不匹配");
        const old = this.state.tasks.find(x => x.id === t.task_id && x.project === p.project_id);
        return { ...old, id: t.task_id, project: p.project_id, title: t.title, revision: t.schema_revision, phase: t.state, items: old?.items || [], queue: old?.queue || [], draft: this.stored(`draft.${t.task_id}`, ""), model: old?.model || "", boxes: old?.boxes || [], image: old?.image || "", actions: old?.actions || {} };
      }));
      // Unsaved composers are local input only, never fabricated persisted tasks/messages.
      for (const p of nav) tasks.push({ id: `new:${p.project_id}`, project: p.project_id, title: "新任务", revision: "", phase: "idle", items: [], queue: [], draft: this.stored(`draft.new:${p.project_id}`, ""), model: "", boxes: [], image: "", actions: { send: { available: true, reason: "只保存目标，执行需另行批准" } } });
      this.emit({ loading: false, projects: nav.map(p => ({ id: p.project_id, title: p.title })), tasks, models,
        settings: { ...initialSettings, ...prefs, revision: safe.revision, budget: safe.sections.usage_budget.future_run_budget.max_cost || "", providers: providers.providers.map(p => ({ id: p.id, name: p.display_name, endpoint: p.base_url, credential: p.credential_configured, status: p.health.status })) },
      });
    } catch (e) { if (seq === this.navigationSequence) this.emit({ loading: false, error: (e as Error).message }); throw e; }
  };

  loadTask = async (project: string, id: string) => {
    const task = this.task(id); if (task.project !== project) throw new Error("任务不属于此项目");
    const seq = ++this.sequence; this.controller?.abort(); const ctrl = this.controller = new AbortController();
    try {
      const [images, ws, thread] = await Promise.all([
        this.transport<{ images: ImageItem[] }>(`${this.root(project)}/images`, { signal: ctrl.signal }),
        id.startsWith("new:") ? null : this.transport<Workspace>(`${this.taskRoot(task)}/workspace`, { signal: ctrl.signal }),
        id.startsWith("new:") ? [] : this.pages<Thread>(`${this.taskRoot(task)}/thread`, ctrl.signal),
      ]);
      if (seq !== this.sequence) return;
      const p = this.projects.get(project)!;
      if (ws && (ws.project_id !== project || ws.project_owner_id !== p.project_owner_id || ws.task.input.id !== id || ws.conversation_id !== p.conversation_id)) throw new Error("任务快照归属不匹配");
      if (thread.some(t => t.task_id !== id || t.project_owner_id !== p.project_owner_id || t.conversation_id !== p.conversation_id)) throw new Error("消息归属不匹配");
      if (ws) this.workspaces.set(id, ws);
      const artifacts = images.images.map(image => {
        if (!image.url.startsWith("/api/") || image.url.startsWith("//")) throw new Error("图片地址不是受控站内资源");
        return { id: image.image_id, project, name: image.name, src: image.url, width: 0, height: 0 };
      });
      const current = this.task(id);
      this.emit({ error: undefined, artifacts, tasks: this.state.tasks.map(t => t.id !== id ? t : { ...current,
        items: thread.map(t => ({ id: t.id, role: "user", text: t.message.input.text })),
        actions: ws?.actions || current.actions, model: ws?.agent_model.model_profile_id || current.model,
        image: artifacts[0]?.id || "", editBoxes: this.stored(`edits.${id}`, {}),
        queue: ws?.queue.filter(q => !q.cancelled_at).map(q => q.message?.input?.text || q.input?.text || "待处理输入") || [],
      }) });
    } catch (e) { if (seq !== this.sequence || ctrl.signal.aborted) return; this.emit({ error: (e as Error).message, artifacts: [] }); throw e; }
  };
  async createTask(project: string) { this.root(project); return `new:${project}`; }
  saveDraft(id: string, text: string) { this.task(id); this.save(`draft.${id}`, text); this.emit({ tasks: this.state.tasks.map(t => t.id === id ? { ...t, draft: text } : t) }); }
  saveArtifactDraft(id: string, image: ImageId, boxes: Box[]) { const t = this.task(id); const editBoxes = { ...t.editBoxes, [image]: boxes }; this.save(`edits.${id}`, editBoxes); this.emit({ tasks: this.state.tasks.map(t => t.id === id ? { ...t, editBoxes } : t) }); }
  async sendMessage(c: Command, _text: string, _mode: "plan" | "execute", _model: string) { this.checked(c); unsupported("发送与精确批准尚在联调"); }
  async approveAction(c: Command) { this.checked(c); unsupported("精确授权尚在联调"); }
  async interruptOperation(c: Command) { this.checked(c); unsupported("停止回执尚在联调"); }
  async resumeOperation(c: Command) { this.checked(c); unsupported("继续能力尚在联调"); }
  async selectAgentModel(c: Command, _model: string) { this.checked(c); unsupported("模型 CAS 尚在联调"); }
  async answerHumanRequest(c: Command, _boxes: Box[]) { this.checked(c); unsupported("人工答案尚在联调"); }
  async updateSettings(_revision: string, _settings: Settings) { unsupported("设置写入尚在联调"); }
  async testProvider(_id: string, _result: "success" | "failed" | "unknown") { unsupported("连接检查尚在联调"); }
  async installPlugin(_id: string, _fail: boolean) { unsupported("模型安装必须使用真实权限和许可证流程"); }
}
