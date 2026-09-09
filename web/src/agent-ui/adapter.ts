/** Shared presentation contract. Fixture is preview-only; HTTP fails closed. */
export type ImageId = number | string;
export type Action = { available: boolean; reason: string };
export type Approval = { id: string; title: string; scope: string[]; revision: string; budget: string | null };
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
  role: "user" | "assistant" | "system";
  text: string;
  model?: string;
  reference?: Command["selection"];
};
export type Task = {
  id: string;
  project: string;
  conversationId?: string;
  title: string;
  phase: Phase;
  revision: string;
  items: ThreadItem[];
  queue: string[];
  draft: string;
  model: string;
  operationModel?: string;
  boxes: Box[];
  labelNames?: Record<string,string>;
  labelNamesRevision?: number;
  image: ImageId;
  editBoxes?: Record<ImageId, Box[]>;
  boxesByImage?: Record<ImageId, Box[]>;
  actions?: Partial<Record<"send" | "stop" | "resume" | "approve" | "answer", Action>>;
  humanQuestion?: string;
  human?: { id:string; image:ImageId; kind:string; labels:string[]; label:string; candidate:string };
  resultRevision?: string;
  loaded?: boolean;
  sample?: {id:string;draft:string;revision:number};
  imageResults?: Record<ImageId, {labels:string[]; risks:string[]}>;
  approval?: Approval;
  receipts?: {id:string; title:string; status:string; detail?:string; startedAt?:string; finishedAt?:string; durationMs?:number; stage?:string}[];
  queueEntries?: {id:string;text:string;status:string;canCancel:boolean;canPlan:boolean}[];
  processing?: {id:string;batch:string;status:string;url:string}[];
  stopTargets?: {id:string;label:string}[];
  resumeTargets?: {id:string;label:string;reason:string}[];
  exports?: {id:string;status:string;url?:string;detail:string}[];
  error?: string;
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
  loading?: boolean;
  error?: string;
  workspaceId?: string;
  testOnly?: boolean;
  artifacts: {
    id: ImageId;
    project?: string;
    name: string;
    src: string;
    width: number;
    height: number;
  }[];
  usage: { id: string; model: string; tokens: string; cost: string | null }[];
  knownCost: string;
  protectedCache: number;
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
  readonly deliveryIntake?: import("./DeliveryIntake").DeliveryIntakeService;
  readonly delivery?: import("./deliveryService").DeliveryService;
  readonly reviewManagement?: import("./ReviewManagement").ReviewService;
  readonly runDetail?: import("./RunDetail").RunDetailService;
  readonly batchDetail?: import("./BatchDetail").BatchDetailService;
  readonly workflowEditor?: import("./WorkflowEditor").WorkflowEditorService;
  readonly workflowVersion?: import("./WorkflowVersionDetail").WorkflowVersionService;
  readonly exportManagement?: import("./ExportManagement").ExportService;
  readonly trashManagement?: import("./TrashManagement").TrashService;
  readonly projectManagement?: import("./ProjectManagement").ProjectManagementService;
  readonly runtimeSettingsManagement?: import("./RuntimeSettings").RuntimeSettingsService;
  readonly modelProfileManagement?: import("./ModelProfiles").ModelProfileService;
  readonly pluginManagement?: import("./pluginManagement").PluginManagement;
  readonly visionWorkerManagement?: import("./VisionWorkers").VisionWorkerService;
  readonly kind: "fixture" | "http";
  snapshot(): Snapshot;
  subscribe(listener: () => void): () => void;
  createTask(project: string): Promise<string>;
  saveDraft(task: string, text: string): void;
  saveArtifactDraft(task: string, image: ImageId, boxes: Box[]): void;
  loadTask?(project: string, task: string): Promise<void>;
  refresh?(): Promise<void>;
  saveCredential?(provider: string, secret: string): Promise<void>;
  uploadImages?(command: Command, files: File[]): Promise<void>;
  sendMessage(
    command: Command,
    text: string,
    mode: "plan" | "execute",
    model: string,
  ): Promise<void | string>;
  prepareAction?(command: Command, kind: "plan" | "sample" | "process" | "export"): Promise<void>;
  cancelQueue?(command: Command, message: string): Promise<void>;
  prepareQueue?(command: Command, message: string): Promise<void>;
  approveAction(command: Command): Promise<void>;
  interruptOperation(command: Command): Promise<void>;
  resumeOperation(command: Command, target?:string): Promise<void>;
  selectStop?(command:Command,target:string):Promise<void>;
  selectAgentModel(command: Command, model: string): Promise<void>;
  answerHumanRequest(command: Command, boxes: Box[], classification?:string): Promise<void>;
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
