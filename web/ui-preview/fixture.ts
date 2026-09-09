import type {
  WorkspaceAdapter,
  Snapshot,
  Task,
  Command,
  Settings,
  Box,
  Phase,
} from "../src/agent-ui/adapter";
const key = "annotagent.ui-preview.v1";
export const initialBoxes: Box[] = [
  { id: "cup-1", label: "cup", x: 203, y: 373, w: 273, h: 221 },
  { id: "bottle-1", label: "bottle", x: 587, y: 206, w: 156, h: 381 },
  { id: "cup-2", label: "cup", x: 395, y: 482, w: 169, h: 142 },
];
const newTask = (id: string, project: string, title = "新任务"): Task => ({
  id,
  project,
  title,
  phase: "idle",
  revision: crypto.randomUUID(),
  items: [],
  queue: [],
  draft: "",
  model: "planner",
  boxes: structuredClone(initialBoxes),
  image: 1,
});
export function seed(): Snapshot {
  return {
    projects: [
      { id: "products", title: "商品图片标注" },
      { id: "scenes", title: "场景分类" },
      { id: "defects", title: "缺陷图片" },
    ],
    tasks: [
      newTask("new", "products"),
      newTask("previous", "products", "检查上一批样例"),
      newTask("scene", "scenes", "室内与室外分类"),
    ],
    models: [
      {
        id: "planner",
        name: "GLM-5.2 · 演示",
        provider: "智谱 / 示例账户",
        providerId: "lab",
      },
      {
        id: "qwen",
        name: "Qwen · 演示",
        provider: "阿里云 / 示例账户",
        providerId: "cloud",
      },
      {
        id: "local",
        name: "本地规划模型",
        provider: "本地",
        reason: "未配置工具调用能力",
      },
      {
        id: "disabled",
        name: "旧规划模型",
        provider: "本地",
        reason: "已禁用",
      },
    ],
    settings: {
      revision: "settings-initial",
      theme: "light",
      language: "zh",
      font: "标准",
      density: "舒适",
      collapsed: false,
      providers: [
        {
          id: "cloud",
          name: "云端示例账户",
          endpoint: "https://example.invalid/cloud",
          credential: true,
          status: "演示 · 未测试",
        },
        {
          id: "lab",
          name: "示例账户",
          endpoint: "https://example.invalid/v1",
          credential: true,
          status: "演示 · 未测试",
        },
      ],
      defaultModel: "planner",
      plugins: [
        {
          id: "sam",
          name: "SAM · 演示分割插件",
          version: "1.0 · ONNX",
          status: "待安装模型",
        },
        {
          id: "vision",
          name: "视觉语言模型 · 演示",
          version: "HTTP Vision v1",
          status: "Ready",
        },
        {
          id: "missing",
          name: "本地检测器",
          version: "1.0",
          status: "缺少权重",
        },
        { id: "disabled", name: "历史分类器", version: "0.9", status: "禁用" },
        { id: "error", name: "实验分割器", version: "0.1", status: "异常" },
      ],
      allowExternal: false,
      cache: 128,
      budget: "5.00",
      range: "当前任务",
    },
  };
}
const delay = (ms = 350) => new Promise<void>((r) => setTimeout(r, ms));
/** Entire simulator is reachable only from the separate preview entry, never production App. */
export class FixtureAdapter implements WorkspaceAdapter {
  readonly kind = "fixture" as const;
  private state: Snapshot;
  private listeners = new Set<() => void>();
  private commands = new Map<string, string>();
  failNext = false;
  constructor(private storage?: Pick<Storage, "getItem" | "setItem">) {
    const saved = storage?.getItem(key);
    try {
      this.state = saved ? JSON.parse(saved) : seed();
      if (!this.state.settings?.revision) throw Error();
    } catch {
      this.state = seed();
    }
    for (const task of this.state.tasks) {
      if (task.phase === "stopping")
        setTimeout(() => {
          const current = this.task(task.id);
          if (current.phase === "stopping") this.phase(current, "interrupted");
        }, 900);
      // A simulated planner has no resumable remote operation after browser reload.
      if (task.phase === "planning") {
        task.phase = "outcome_unknown";
        task.revision = crypto.randomUUID();
      }
    }
  }
  snapshot = () => this.state;
  subscribe = (fn: () => void) => {
    this.listeners.add(fn);
    return () => {
      this.listeners.delete(fn);
    };
  };
  private publish() {
    this.state = structuredClone(this.state);
    this.storage?.setItem(key, JSON.stringify(this.state));
    this.listeners.forEach((fn) => fn());
  }
  private task(id: string) {
    // Never mutate a snapshot already observed by React or a settings editor baseline.
    this.state = structuredClone(this.state);
    const t = this.state.tasks.find((t) => t.id === id);
    if (!t) throw Error("任务已删除或不存在");
    return t;
  }
  private check(c: Command, payload: unknown): Task | null {
    const sig = JSON.stringify({
      project: c.project,
      task: c.task,
      selection: c.selection,
      payload,
    });
    if (this.commands.has(c.id)) {
      if (this.commands.get(c.id) !== sig) throw Error("同一命令不能改变内容");
      return null;
    }
    const t = this.task(c.task);
    if (t.project !== c.project) throw Error("项目归属不匹配");
    if (t.revision !== c.revision) throw Error("版本已变化，请重新载入后确认");
    if (this.failNext) {
      this.failNext = false;
      throw Error("模拟保存失败：输入仍然保留，尚未执行。");
    }
    this.commands.set(c.id, sig);
    return t;
  }
  private phase(t: Task, phase: Phase) {
    t.phase = phase;
    t.revision = crypto.randomUUID();
    this.publish();
  }
  async createTask(project: string) {
    this.state = structuredClone(this.state);
    if (!this.state.projects.some((p) => p.id === project))
      throw Error("项目不存在");
    const t = newTask(crypto.randomUUID(), project);
    t.model = this.state.settings.defaultModel;
    this.state.tasks.push(t);
    this.publish();
    return t.id;
  }
  saveDraft(id: string, text: string) {
    this.task(id).draft = text;
    this.publish();
  }
  saveArtifactDraft(id: string, image: number, boxes: Box[]) {
    const t = this.task(id);
    t.editBoxes = { ...t.editBoxes, [image]: structuredClone(boxes) };
    this.publish();
  }
  async sendMessage(
    c: Command,
    text: string,
    mode: "plan" | "execute",
    model: string,
  ) {
    if (!text.trim()) throw Error("请输入目标");
    const t = this.check(c, { text, mode, model });
    if (!t) return;
    if (!this.state.models.some((m) => m.id === model && !m.reason))
      throw Error("模型不兼容");
    if (["running", "planning", "stopping"].includes(t.phase)) {
      t.queue.push(text);
      t.draft = "";
      this.publish();
      return;
    }
    t.items.push({
      id: c.id,
      role: "user",
      text,
      model,
      reference: c.selection,
    });
    t.title = text.slice(0, 35);
    t.draft = "";
    t.operationModel = model;
    this.phase(t, "planning");
    await delay(650);
    const current = this.task(c.task);
    if (current.phase !== "planning") return;
    current.plan = {
      revision: crypto.randomUUID(),
      steps: [
        "建立标注规范：cup / bottle 边界框",
        "测试 3 张示意图：寻找候选并检查局部",
        "有有效提示时精修边界；证据不足交给你确认",
        "检查样例，再决定是否处理其余图片",
      ],
      images: 3,
      models: ["演示 VLM", "演示 SAM"],
      destination: "浏览器内 Fixture，无网络发送",
      budget: null,
    };
    current.items.push({
      id: crypto.randomUUID(),
      role: "assistant",
      text: "以下是固定演示计划，用于检查界面交互，并非模型对你的请求生成的方案。批准仅模拟样例处理，不写入正式标注。",
    });
    this.phase(current, "awaiting_approval");
  }
  async approveAction(c: Command) {
    const t = this.check(c, { action: "approve" });
    if (!t) return;
    if (t.phase !== "awaiting_approval" || !t.plan)
      throw Error("当前没有可批准计划");
    this.phase(t, "running");
  }
  async interruptOperation(c: Command) {
    const t = this.check(c, { action: "stop" });
    if (!t) return;
    if (!["running", "planning"].includes(t.phase))
      throw Error("没有可停止操作");
    this.phase(t, "stopping");
    await delay(900);
    const current = this.task(c.task);
    if (current.phase === "stopping") this.phase(current, "interrupted");
  }
  async resumeOperation(c: Command) {
    const t = this.check(c, { action: "resume" });
    if (!t) return;
    if (t.phase !== "interrupted")
      throw Error("当前状态不能继续；未知结果必须先核实");
    this.phase(t, "running");
  }
  async selectAgentModel(c: Command, model: string) {
    const t = this.check(c, { action: "model", model });
    if (!t) return;
    if (!this.state.models.some((m) => m.id === model && !m.reason))
      throw Error("模型不兼容");
    t.model = model;
    t.revision = crypto.randomUUID();
    this.publish();
  }
  async answerHumanRequest(c: Command, boxes: Box[]) {
    if (
      boxes.some(
        (b) =>
          !b.label.trim() ||
          ![b.x, b.y, b.w, b.h].every(Number.isFinite) ||
          b.x < 0 ||
          b.y < 0 ||
          b.w <= 0 ||
          b.h <= 0 ||
          b.x + b.w > 960 ||
          b.y + b.h > 760,
      )
    )
      throw Error("框必须有标签且位于原图范围内");
    const t = this.check(c, { action: "answer", boxes });
    if (!t) return;
    if (t.phase !== "waiting_for_human") throw Error("人工请求已变化");
    t.boxes = boxes;
    t.items.push({
      id: c.id,
      role: "assistant",
      text: "演示修正已保存到浏览器预览。未写入正式 Annotation。",
    });
    this.phase(t, "completed");
  }
  async updateSettings(revision: string, settings: Settings) {
    await delay();
    if (revision !== this.state.settings.revision)
      throw Error("设置已在另一处修改，请重新载入");
    if (this.failNext) {
      this.failNext = false;
      throw Error("模拟保存失败。你的编辑仍保留。");
    }
    if (!/^\d+(\.\d{1,2})?$/.test(settings.budget))
      throw Error("预算需要非负金额，最多两位小数");
    this.state = structuredClone(this.state);
    this.state.settings = {
      ...structuredClone(settings),
      revision: crypto.randomUUID(),
    };
    this.state.models = this.state.models.map((m) => {
      if (!m.providerId) return m;
      const provider = settings.providers.find((p) => p.id === m.providerId);
      return {
        ...m,
        reason: !provider
          ? "Provider 已移除"
          : !provider.credential
            ? "缺少凭证槽位"
            : undefined,
      };
    });
    this.publish();
  }
  async testProvider(id: string, result: "success" | "failed" | "unknown") {
    this.state = structuredClone(this.state);
    const p = this.state.settings.providers.find((p) => p.id === id);
    if (!p) throw Error("账户不存在");
    p.status = "正在模拟测试";
    this.publish();
    await delay(800);
    this.state = structuredClone(this.state);
    this.state.settings.providers.find((p) => p.id === id)!.status = {
      success: "模拟连接成功",
      failed: "模拟连接失败",
      unknown: "模拟结果未知",
    }[result];
    this.state.settings.revision = crypto.randomUUID();
    this.publish();
  }
  async installPlugin(id: string, fail: boolean) {
    this.state = structuredClone(this.state);
    const p = this.state.settings.plugins.find((p) => p.id === id);
    if (!p) throw Error("插件不存在");
    p.status = "模拟安装中";
    this.publish();
    await delay(800);
    this.state = structuredClone(this.state);
    this.state.settings.plugins.find((p) => p.id === id)!.status = fail
      ? "模拟校验失败"
      : "Ready";
    this.state.settings.revision = crypto.randomUUID();
    this.publish();
  }
  /** Preview scenario controls, deliberately not part of the future HTTP contract. */
  scenario(id: string, phase: Phase) {
    const t = this.task(id);
    if (!t.items.length)
      t.items.push({
        id: crypto.randomUUID(),
        role: "user",
        text: "演示：框出杯子和瓶子，需要时请我确认。",
      });
    this.phase(t, phase);
  }
}
