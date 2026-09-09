/** Transport-neutral UI contract. HTTP binding is deliberately not connected before visual approval. */
export type Phase =
  | "idle"
  | "planning"
  | "awaiting_approval"
  | "running"
  | "stopping"
  | "interrupted"
  | "waiting_for_human"
  | "completed"
  | "failed"
  | "cancelled"
  | "outcome_unknown";
export type Section =
  | "general"
  | "providers"
  | "agent"
  | "vision"
  | "privacy"
  | "usage";
export type Box = {
  id: string;
  label: string;
  x: number;
  y: number;
  w: number;
  h: number;
};
export type Model = {
  id: string;
  name: string;
  provider: string;
  providerId?: string;
  reason?: string;
};
export type Provider = {
  id: string;
  name: string;
  endpoint: string;
  credential: boolean;
  status: string;
};
export type ThreadItem = {
  id: string;
  role: "user" | "assistant";
  text: string;
  model?: string;
};
export type Task = {
  id: string;
  project: string;
  title: string;
  phase: Phase;
  revision: string;
  items: ThreadItem[];
  queue: string[];
  draft: string;
  model: string;
  operationModel?: string;
  boxes: Box[];
  image: number;
  editBoxes?: Record<number, Box[]>;
  plan?: {
    revision: string;
    steps: string[];
    images: number;
    models: string[];
    destination: string;
    budget: string | null;
  };
};
export type Settings = {
  revision: string;
  theme: "light" | "dark" | "system";
  language: "zh" | "en";
  font: string;
  density: string;
  collapsed: boolean;
  providers: Provider[];
  defaultModel: string;
  plugins: { id: string; name: string; version: string; status: string }[];
  allowExternal: boolean;
  cache: number;
  budget: string;
  range: string;
};
export type Snapshot = {
  projects: { id: string; title: string }[];
  tasks: Task[];
  models: Model[];
  settings: Settings;
};
export type Command = {
  id: string;
  project: string;
  task: string;
  revision: string;
  selection?: { image: string; candidate: string; revision: string };
};
export interface WorkspaceAdapter {
  readonly kind: "fixture" | "http";
  snapshot(): Snapshot;
  subscribe(listener: () => void): () => void;
  createTask(project: string): Promise<string>;
  saveDraft(task: string, text: string): void;
  saveArtifactDraft(task: string, image: number, boxes: Box[]): void;
  sendMessage(
    command: Command,
    text: string,
    mode: "plan" | "execute",
    model: string,
  ): Promise<void>;
  approveAction(command: Command): Promise<void>;
  interruptOperation(command: Command): Promise<void>;
  resumeOperation(command: Command): Promise<void>;
  selectAgentModel(command: Command, model: string): Promise<void>;
  answerHumanRequest(command: Command, boxes: Box[]): Promise<void>;
  updateSettings(revision: string, settings: Settings): Promise<void>;
  testProvider(
    id: string,
    result: "success" | "failed" | "unknown",
  ): Promise<void>;
  installPlugin(id: string, fail: boolean): Promise<void>;
}
export const sections: {
  id: Section;
  zh: string;
  en: string;
  description: string;
}[] = [
  {
    id: "general",
    zh: "通用",
    en: "General",
    description: "让工作区适合你的阅读和操作习惯。",
  },
  {
    id: "providers",
    zh: "Providers 与账户",
    en: "Providers",
    description: "连接状态、账户与凭证槽位。预览不接收真实密钥。",
  },
  {
    id: "agent",
    zh: "Agent 模型",
    en: "Agent models",
    description: "选择规划与对话模型，不改变工作流中的视觉模型。",
  },
  {
    id: "vision",
    zh: "视觉模型与插件",
    en: "Vision & plugins",
    description: "查看图像模型的安装状态和兼容信息。",
  },
  {
    id: "privacy",
    zh: "数据与隐私",
    en: "Data & privacy",
    description: "了解数据存放位置、外传范围和受保护的内容。",
  },
  {
    id: "usage",
    zh: "用量与预算",
    en: "Usage & budget",
    description: "明确统计范围和费用未知项，再决定预算。",
  },
];
