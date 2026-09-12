import { api, ApiRequestError, request, type JourneyPreview, type JourneyConsent, type ProcessingAuthorization, type ProcessingReceipt } from "../api";
import type { Annotation, ConversationFormalReference, ConversationMessageInput, ImageItem, ProviderProfile, RegistryModelProfile, GlobalModelDefaults, ExpertPluginRegistry, InstalledModelInstance, ConversationSchemaPreview, ConversationCallReceipt, ConversationBuilderItem, WorkflowSampleTestRecord, SampleFeedbackRevision, ExportReadiness, ProjectExportResult } from "../types";
import { terminalSampleAnnotations } from "../sampleAnnotations";
import { historyScopeApi } from "./historyScope";
import { historyManagementApi } from "./HistoryManagement";
import { sampleFeedbackOverlay } from "../sampleFeedbackOverlay";
import { callStage, failureDetail } from "./ExecutionProgress";
import type { HumanRequest } from "../conversation-human-api";
import { canCancelQueuedMessage, isPendingQueuedMessage, type QueuedMessage } from "../conversation-queue-state";
import type { QueueConsent, QueuePreview } from "../conversation-queue-api";
import type { SendCommand, SendReceipt } from "../conversation-send";
import type { StopRequestRecord } from "../conversation-stop-api";
import {ownedStopSelection} from "./stopSelection";
import {taskFeedbackService} from "./TaskFeedback";
import {taskHistoryApi} from "./taskHistory";
import {stopTargetMatches} from "../conversation-control";
import type { WorkspaceAdapter, Snapshot, Task, Command, Settings, ImageId, Box, Phase, Action } from "./adapter";
import {readPendingDelivery,rememberPendingDelivery,clearPendingDelivery} from "./pendingDelivery";
import {projectCallMessages} from "./messageProjection";
import {createModelPreparationService} from "./modelPreparation";
import {resolveFormalReviewPage,type FormalReviewEvidence,type FormalRunEvidence} from "./deliveryReviewState";
import {assertVisualSelection,deliverySampleResultFromCanonical,formalVisualSelectionFromCanonical,selectedMessage,type CanonicalVisualSelectionItem,type CanonicalVisualSelectionPage,type MainlineAdvanceReceipt,type MainlineTaskView,type VisualSelection} from "./mainline";

type Page<T> = { items: T[]; next_cursor: string | number | null };
type Project = { project_id: string; project_owner_id: string; title: string; conversation_id: string | null };
type NavigationTask = { task_id: string; title: string; schema_revision: string; project_owner_id: string; conversation_id: string; state: Phase };
type Preference = { revision: number; model_profile_id: string | null };
type Workspace = {
  project_id: string; project_owner_id: string; conversation_id: string;
  task: { input: { id: string; schema_revision: string } };
  agent_model: Preference;
  actions: Partial<Record<"send" | "stop" | "resume" | "approve", Action>>;
  calls: ConversationCallReceipt[];
  queue: QueuedMessage[];
  human_requests?: HumanRequest[];
  builder_operations?: {items:ConversationBuilderItem[]};
  sample_operations?: {id:string;draft_id:string;status:string;error?:string}[];
  resume_actions?: {id:string;kind:string;available:boolean;reason:string;url:string;method:string}[];
  processing_operations?: ProcessingReceipt[];
  read_model_revision?:string;
  mainline?:MainlineTaskView;
};
type Thread = { id: string; role: "user"; task_id: string; project_owner_id: string; conversation_id: string; message: { input: ConversationMessageInput } };
type SafeSettings = { revision: string; sections: { data_privacy: { workspace_id: string }; usage_budget: { future_run_budget: Record<string, unknown> & { max_cost?: string } } } };
export type Transport = <T>(path: string, init?: RequestInit) => Promise<T>;
const persistedReferenceText=(input:ConversationMessageInput)=>input.reference?.scope==="sample_candidate"
  ? `引用：样例图片 ${input.image?.image_id || "未知"} · 候选 ${input.reference.candidate_id}`
  : input.reference?.scope==="formal_annotation"
    ? `引用：正式图片 ${input.image?.image_id || "未知"} · 标注 ${input.reference.annotation_id}`
    : input.reference?.scope==="stop_request" ? "引用：停止请求" : undefined;
const esc = encodeURIComponent;
const nativeTrashService = {...api, historyScope:historyScopeApi};
const unsupported = (detail: string): never => { throw new Error(`尚未接通：${detail}。没有执行操作，也没有回退到演示结果。`); };
const initialSettings: Settings = { revision: "", theme: "system", language: "zh", font: "标准", density: "舒适", collapsed: false, providers: [], defaultModel: "", plugins: [], allowExternal: false, cache: 0, budget: "", range: "未来 Run 默认预算" };

/** Only this boundary knows HTTP routes. Reads never create conversations, tasks or execution. */
export class HttpAdapter implements WorkspaceAdapter {
  readonly modelPreparation:import("./modelPreparation").ModelPreparationService;
  readonly bundleInstaller:import("./BundleInstaller").BundleInstallerService|undefined;
  readonly mainlineTask:import("./mainline").MainlineTaskService={
    read:async(project,conversation,task,signal)=>{
      const owned=this.task(task);
      if(owned.project!==project||owned.conversationId!==conversation)throw new Error("任务不属于这个 Project/Conversation；没有读取其他任务。");
      const workspace=await this.transport<Workspace>(`${this.taskRoot(owned)}/workspace`,{signal});
      return this.assertMainline(workspace,owned);
    },
    advance:async(project,conversation,task,input)=>{
      const owned=this.task(task);
      if(owned.project!==project||owned.conversationId!==conversation)throw new Error("任务不属于这个 Project/Conversation；没有推进其他任务。");
      const receipt=await this.transport<MainlineAdvanceReceipt>(`${this.taskRoot(owned)}/advance`,{method:"POST",body:JSON.stringify(input)});
      if(receipt.command_id!==input.command_id||receipt.action_id!==input.action_id)throw new Error("任务推进回执身份不匹配；请核实服务端记录。");
      this.assertMainline({mainline:receipt.workspace},owned);
      await this.reloadCurrent(owned);
      return receipt;
    },
  };
  private assertMainline(workspace:Pick<Workspace,"mainline">,task:Task){
    const value=workspace.mainline;
    if(!value||value.contract_version!=="mainline-task-v1")throw new Error("服务器尚未提供 Mainline Task read model；不会猜测下一步。");
    if(value.project_id!==task.project||value.task_id!==task.id||value.conversation_id!==task.conversationId||value.project_owner_id!==this.projects.get(task.project)?.project_owner_id)throw new Error("Mainline Task read model 的所有权不匹配。");
    return value;
  }
  private deliveryRoot(project: string, id: string) {
    const task = this.task(id);
    if (task.project !== project) throw new Error("任务不属于此项目");
    return this.taskRoot(task);
  }
  readonly delivery: import("./deliveryService").DeliveryService = {
    editObject:(project,task,image,input)=>this.transport(`${this.deliveryRoot(project,task)}/delivery-images/${esc(image)}/objects`,{method:"POST",body:JSON.stringify(input)}),
    createObject:(project,task,image,input)=>this.transport(`${this.deliveryRoot(project,task)}/delivery-images/${esc(image)}/missing-objects`,{method:"POST",body:JSON.stringify(input)}),
    pendingPackage: (project,task)=>{this.deliveryRoot(project,task);return this.storage?readPendingDelivery(this.storage,this.deliveryPendingKey(project,task)):undefined;},
    history: async(project,task,before,signal)=>{
      const rows=await this.transport<{id:string;created_at:string;format:string}[]>(`${this.deliveryRoot(project,task)}/exports?limit=100${before?`&before=${esc(before)}`:""}`,{signal});
      return {items:rows.filter(r=>r.format==="ultralytics_yolo_detection").map(r=>({id:r.id,created_at:r.created_at})),next_cursor:rows.length===100?rows.at(-1)!.id:null};
    },
    image: (project, task, image, run, signal) => this.transport(`${this.deliveryRoot(project,task)}/delivery-images/${esc(image)}${run === null ? "" : `?source_run_id=${esc(run)}`}`, { signal }),
    confirmImage: (project, task, input) => this.transport(`${this.deliveryRoot(project,task)}/delivery-images/${esc(input.image_id)}`, { method: "POST", body: JSON.stringify(input) }),
    startPackage: async(project, task, input) => {
      const root=this.deliveryRoot(project,task);
      if(!this.storage)throw new Error("无法持久保存打包命令；没有发送请求。");
      const key=this.deliveryPendingKey(project,task);
      rememberPendingDelivery(this.storage,key,input);
      const result=await this.transport<import("./deliveryService").DeliveryPackageStart>(`${root}/delivery-packages`, { method: "POST", body: JSON.stringify(input) });
      if(result.job.id!==input.command_id)throw new Error("打包回执身份不匹配；原请求已保留，请核实状态。");
      // Cleanup failure must not turn a confirmed server receipt into a failed command.
      try{clearPendingDelivery(this.storage,key,input.command_id);}catch{/* next read can reconcile the exact command */}
      return result;
    },
    packageStatus: async(project, task, id, signal) => {
      const result=await this.transport<import("./deliveryService").DeliveryPackageRead>(`${this.deliveryRoot(project,task)}/delivery-packages/${esc(id)}`, { signal });
      if(result.job.id!==id)throw new Error("打包状态身份不匹配，未恢复其他数据包。");
      if(this.storage)try{clearPendingDelivery(this.storage,this.deliveryPendingKey(project,task),id);}catch{/* the server receipt remains authoritative */}
      return result;
    },
    cancelPackage: (project, task, id) => this.transport(`${this.deliveryRoot(project,task)}/delivery-packages/${esc(id)}/cancel`, { method: "POST", body: JSON.stringify({ confirmed: true }) }),
    downloadUrl: (project, task, id) => `${this.deliveryRoot(project,task)}/delivery-packages/${esc(id)}/download`,
    formalResult: async(project,task,signal) => {
      const value=await this.transport<import("./deliveryVisualSelection").DeliveryFormalResult|null>(`${this.deliveryRoot(project,task)}/formal-result`,{signal});
      if(value&&(value.project_id!==project||value.task_id!==task))throw new Error("正式结果不属于当前 Project/Task。");
      return value;
    },
    reviewSummary: async(project,task,cursor,signal) => {
      const root=this.deliveryRoot(project,task);
      const [page,formalResult]=await Promise.all([this.transport<{
        project_id:string;task_id:string;intent_revision:number;intent_sha256:string;
        summary:{selected:number;positive:number;negative:number;excluded:number;unreviewed:number};
        items:{image_id:string;content_sha256:string;processing_operation_id:string|null;batch_id:string|null;child_run_id:string|null;execution_status:string|null;execution_error:string|null;unresolved_objects:number;review_revision:number;review_decision:string|null;confirmation_current:boolean;snapshot_sha256:string;annotations:{annotation_id:string;label:string|null;value:Annotation["value"];annotation_revision_id:string|null;feedback_available:boolean;conversation_reference:ConversationFormalReference|null}[]}[];
        next_cursor:string|null;
      }>(`${root}/delivery-review-items?cursor=${esc(cursor||"0")}&limit=50`,{signal}),this.delivery.formalResult!(project,task,signal)]);
      if(page.project_id!==project||page.task_id!==task)throw new Error("审核摘要不属于当前 Project/Task。");
      const reviews:FormalReviewEvidence[]=page.items.map(item=>({
        image_id:item.image_id,confirmation_current:item.confirmation_current,
        review_decision:item.review_decision,unresolved_objects:item.unresolved_objects,
        execution_status:item.execution_status,execution_error:item.execution_error,
      }));
      const formalImages:FormalRunEvidence[]=((formalResult?.images||[]) as {image_id:string;child_run_id:string|null;run_status?:string|null;status?:string|null;error?:string|null}[]).map(image=>({
        image_id:image.image_id,child_run_id:image.child_run_id,run_status:image.run_status||null,
        status:image.status||null,error:image.error||null,
      }));
      const resolutions=resolveFormalReviewPage(reviews,formalImages);
      const items=page.items.map(item=>{
        const resolution=resolutions.get(item.image_id);
        if(!resolution)throw new Error("正式审核项缺少状态归类结果。");
        const entries=item.annotations.flatMap(annotation=>{
          if(!annotation.feedback_available||!annotation.conversation_reference)return [];
          const selection=formalVisualSelectionFromCanonical({project_id:project,conversation_id:this.task(task).conversationId!,task_id:task,image_id:item.image_id,image_sha256:item.content_sha256,annotation});
          const reference=selection.reference;
          if(reference.intent_revision!==page.intent_revision||reference.intent_sha256!==page.intent_sha256||reference.processing_operation_id!==item.processing_operation_id||reference.batch_id!==item.batch_id||reference.source_run_id!==item.child_run_id||reference.expected_snapshot_sha256!==item.snapshot_sha256)throw new Error("正式标注引用与当前审核项的冻结范围不一致。");
          return [[annotation.annotation_id,selection]];
        });
        if(new Set(entries.map(([id])=>id)).size!==entries.length)throw new Error("正式审核项包含重复的 Annotation ID，未创建对象引用。");
        const formal_selections=Object.fromEntries(entries);
        return {image_id:item.image_id,image_sha256:item.content_sha256,state:resolution.state,review_revision:item.review_revision||null,child_run_id:item.child_run_id,error:resolution.diagnostic,formal_selections};
      });
      const failed=items.filter(item=>item.state==="failed").length;
      const complete=page.summary.positive+page.summary.negative+page.summary.excluded;
      return {intent_revision:page.intent_revision,intent_sha256:page.intent_sha256,formal_result:formalResult,counts:{total:page.summary.selected,complete,positive:page.summary.positive,negative:page.summary.negative,excluded:page.summary.excluded,unresolved:page.summary.unreviewed,failed},items,next_cursor:page.next_cursor};
    },
    packageReadiness: async(project,task,signal) => {
      const root=this.deliveryRoot(project,task);
      const owned=this.task(task);
      if(!owned.conversationId)throw new Error("任务没有所属会话。");
      const [view,consents,reviewSummary]=await Promise.all([
        this.mainlineTask.read(project,owned.conversationId,task,signal),
        this.transport<{items:{input:{id:string;intent_revision:number;intent_sha256:string;confirmed:true};state:string;effective_state:string;readiness:{ready:boolean;selected_images:number;confirmed_images:number;blocked_images:number;reasons:string[]};job:import("./deliveryService").DeliveryPackageStatus|null}[];next_cursor:null}>(`${root}/delivery-package-consents`,{signal}),
        this.delivery.reviewSummary!(project,task,undefined,signal),
      ]);
      const delivery=(view.delivery as import("./DeliveryIntake").IntakeView)?.saved;
      if(!delivery)throw new Error("当前任务没有已保存的交付版本。");
      const current=consents.items.find(item=>item.input.intent_revision===delivery.revision&&item.input.intent_sha256===delivery.content_sha256&&["armed","consumed"].includes(item.state))
        || consents.items.find(item=>item.input.intent_revision===delivery.revision&&item.input.intent_sha256===delivery.content_sha256);
      const packageRead=current?.job ? await this.delivery.packageStatus(project,task,current.job.id,signal) : null;
      const counts=reviewSummary.counts;
      const reasons=current?.readiness.reasons||[];
      const blockers=reasons.map(code=>({code,message:code==="whole_image_review_missing_or_stale"?"仍有图片未完成当前快照的整图审核。":code==="delivery_intent_changed"?"交付目标已变化，原打包授权已失效。":code,image_ids:[]}));
      return {intent_revision:delivery.revision,intent_sha256:delivery.content_sha256,ready:current?.readiness.ready===true,counts,review_revisions:{},blockers,consent:current?{input:current.input,state:current.state as import("./deliveryService").PackageConsent["state"]}:null,package:packageRead};
    },
    authorizePackage: async(project,task,input) => {
      const value=await this.transport<{input:typeof input;state:string}>(`${this.deliveryRoot(project,task)}/delivery-package-consents`,{method:"POST",body:JSON.stringify(input)});
      if(value.input.id!==input.id||value.input.intent_revision!==input.intent_revision||value.input.intent_sha256!==input.intent_sha256)throw new Error("打包授权回执范围不匹配。");
      return {input:value.input,state:value.state as import("./deliveryService").PackageConsent["state"]};
    },
    cancelPackageAuthorization: async(project,task,id) => {
      const value=await this.transport<{input:{id:string;intent_revision:number;intent_sha256:string;confirmed:true};state:string}>(`${this.deliveryRoot(project,task)}/delivery-package-consents/${esc(id)}/cancel`,{method:"POST",body:JSON.stringify({confirmed:true})});
      if(value.input.id!==id)throw new Error("取消打包授权的回执身份不匹配。");
      return {input:value.input,state:value.state as import("./deliveryService").PackageConsent["state"]};
    },
  };
  private deliveryPendingKey(project:string,task:string) {
    if(!this.state.workspaceId)throw new Error("工作区身份尚未读取，不能恢复或发送打包命令。");
    return this.key(`delivery-package.pending.${esc(project)}.${esc(task)}`);
  }
  readonly deliveryIntake: import("./DeliveryIntake").DeliveryIntakeService = {
    read: async (project, id, signal) => { const task = this.task(id); if (task.project !== project) throw new Error("任务不属于此项目"); const view=await this.transport<import("./DeliveryIntake").IntakeView>(`${this.taskRoot(task)}/delivery-intent`, { signal });if(!signal?.aborted)this.rememberLabelNames(project,id,view);return view; },
    save: async (project, id, input) => { const task = this.task(id); if (task.project !== project) throw new Error("任务不属于此项目"); const view=await this.transport<import("./DeliveryIntake").IntakeView>(`${this.taskRoot(task)}/delivery-intent`, { method: "POST", body: JSON.stringify(input) });this.rememberLabelNames(project,id,view);return view; },
    prepare: async (project,id,input)=>{
      const task=this.task(id);if(task.project!==project||!task.conversationId)throw new Error("任务不属于此项目");
      const view=await this.mainlineTask.read(project,task.conversationId,id);
      const saved=(view.delivery as import("./DeliveryIntake").IntakeView)?.saved;
      if(!saved||saved.revision!==input.expected_revision||saved.content_sha256!==input.expected_sha256)throw new Error("交付目标版本已变化；请重新查看后再准备方案。");
      const receipt=await this.advanceAuthorized(task,"prepare_delivery_schema",input.command_id,view);
      const result=receipt.result as {id?:unknown;revision?:unknown};
      if(typeof result?.id!=="string"||typeof result.revision!=="number")throw new Error("服务器未返回有效的目标规范回执。");
      return {id:result.id,revision:result.revision};
    },
  };
  readonly taskUsage: import("./TaskUsage").TaskUsageService = {
    getTaskUsage: async(project,id,cursor,signal) => this.transport<import("./TaskUsage").TaskUsagePage>(
      `${this.deliveryRoot(project,id)}/model-usage?limit=50${cursor == null ? "" : `&cursor=${esc(cursor)}`}`,
      {signal},
    ),
    subscribeTaskUsage: (_project,_id,onChange) => {
      const timer=setInterval(onChange,2000);
      return()=>clearInterval(timer);
    },
  };
  readonly kind = "http" as const;
  private rememberLabelNames(project:string,id:string,view:import("./DeliveryIntake").IntakeView) {
    const revision=view.saved?.revision||0;
    this.emit({tasks:this.state.tasks.map(t=>t.id===id&&t.project===project&&revision>=(t.labelNamesRevision||0)?{...t,labelNamesRevision:revision,labelNames:Object.fromEntries((view.saved?.intent.label_spec||[]).map(l=>[l.stable_id,l.display_name]))}:t)});
  }
  private state: Snapshot = { loading: true, projects: [], tasks: [], models: [], artifacts: [], usage: [], knownCost: "", protectedCache: 0, settings: initialSettings };
  private listeners = new Set<() => void>();
  private projects = new Map<string, Project>();
  private workspaces = new Map<string, Workspace>();
  private sequence = 0;
  private navigationSequence = 0;
  private controller?: AbortController;
  private viewedTask?: string;
  private async reloadCurrent(task:Task) { if(this.viewedTask===task.id)await this.loadTask(task.project,task.id); }
  private safeSettings?: SafeSettings;
  private defaults: GlobalModelDefaults = {};
  private modelCommands = new Map<string, {request_id: string; expected_revision: number; model_profile_id: string}>();
  private approvals = new Map<string, {id:string; url:string; body: unknown; execution?:string; view?:Task["approval"]}>();
  private dimensions = new Map<string, Promise<{width:number;height:number}>>();
  private measure(src:string) {
    if(!this.dimensions.has(src)) this.dimensions.set(src,new Promise((resolve,reject)=>{
      const image = new Image();image.onload=()=>resolve({width:image.naturalWidth,height:image.naturalHeight});image.onerror=()=>{this.dimensions.delete(src);reject(new Error("无法读取原始图片尺寸，不渲染猜测的标注框"));};image.src=src;
    }));
    return this.dimensions.get(src)!;
  }
  constructor(private transport: Transport = request, private storage?: Storage) {
    this.modelPreparation=createModelPreparationService(transport as import("./modelPreparation").ModelPreparationTransport);
    this.bundleInstaller=transport===request?api:undefined;
  }
  get pluginManagement() { return this.transport === request ? api : undefined; }
  get visionWorkerManagement() { return this.transport === request ? api : undefined; }
  private modelProfileService?: import("./ModelProfiles").ModelProfileService;
  get modelProfileManagement() {
    if (this.transport !== request) return undefined;
    return this.modelProfileService ??= {
      ...api,
      getEffectiveModelRequest: (modelProfileId, signal) => this.transport<import("./ModelRequestEvidence").EffectiveModelRequest>(
        `/api/model-profiles/${esc(modelProfileId)}/effective-request`,
        { signal },
      ),
    };
  }
  get runtimeSettingsManagement() { return this.transport === request ? api : undefined; }
  get projectManagement() { return this.transport === request ? api : undefined; }
  get providerControls() { return this.transport === request ? api : undefined; }
  get trashManagement() { return this.transport === request ? nativeTrashService : undefined; }
  get historyManagement() { return this.transport === request ? historyManagementApi : undefined; }
  get reviewManagement() { return this.transport === request ? api : undefined; }
  get runDetail() { return this.transport === request ? api : undefined; }
  get batchDetail() { return this.transport === request ? api : undefined; }
  get workflowEditor() { return this.transport === request ? api : undefined; }
  get workflowVersion() { return this.transport === request ? api : undefined; }
  get exportManagement() { return this.transport === request ? api : undefined; }
  get taskExportHistory() { return this.transport === request ? api : undefined; }
  get taskHistory() { return this.transport === request ? taskHistoryApi : undefined; }
  get taskSchemaDrafts() { return this.transport === request ? api : undefined; }
  get taskFeedback() { return this.transport === request ? taskFeedbackService : undefined; }
  private async testEnvironment() {
    if(this.transport!==request)return false;
    const response=await fetch("/api/health",{credentials:"same-origin"});
    if(!response.ok)throw new Error(`服务器健康检查失败 (${response.status})`);
    return response.headers.get("x-annotagent-fixture")==="external-model-only";
  }
  snapshot = () => this.state;
  subscribe = (listener: () => void) => { this.listeners.add(listener); return () => { this.listeners.delete(listener); }; };
  private emit(patch: Partial<Snapshot>) { this.state = { ...this.state, ...patch }; this.listeners.forEach(fn => fn()); }
  private key(suffix: string) { return `annotagent.http-ui.${this.state.workspaceId}.${suffix}`; }
  private stored<T>(suffix: string, fallback: T): T { try { return JSON.parse(this.storage?.getItem(this.key(suffix)) || "null") ?? fallback; } catch { return fallback; } }
  private save(suffix: string, value: unknown) { this.storage?.setItem(this.key(suffix), JSON.stringify(value)); }
  private async advanceAuthorized(task:Task,actionId:string,commandId:string,view?:MainlineTaskView){
    if(!task.conversationId)throw new Error("任务没有所属会话；没有推进。");
    const suffix=`mainline-advance.${task.id}.${actionId}`;
    let input=this.stored<import("./mainline").MainlineAdvanceInput|null>(suffix,null);
    if(!input){
      const current=view||await this.mainlineTask.read(task.project,task.conversationId,task.id);
      const action=current.available_actions.find(item=>item.id===actionId);
      if(!action||action.state!=="authorized"||action.method!=="POST"||action.requires_confirmation)throw new Error("服务端没有授权自动执行这个任务步骤；请使用当前批准入口。");
      input={command_id:commandId,expected_read_model_revision:current.read_model_revision,action_id:actionId};
      this.save(suffix,input);
    }
    try{
      const receipt=await this.mainlineTask.advance(task.project,task.conversationId,task.id,input);
      this.save(suffix,null);return receipt;
    }catch(error){
      if(error instanceof ApiRequestError&&error.status===409&&error.code==="task_revision_conflict")this.save(suffix,null);
      throw error;
    }
  }
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
  private async canonicalSampleSelection(task:Task,sampleTestId:string,signal?:AbortSignal):Promise<CanonicalVisualSelectionItem>{
    let cursor:string|null=null;
    const seen=new Set<string>();
    do{
      const page:CanonicalVisualSelectionPage=await this.transport<CanonicalVisualSelectionPage>(`${this.taskRoot(task)}/visual-selections?cursor=${esc(cursor||"0")}&limit=20`,{signal});
      if(page.project_id!==task.project||page.conversation_id!==task.conversationId||page.task_id!==task.id)throw new Error("Canonical candidate list does not belong to the current task");
      const found=page.items.find(item=>item.sample_test_id===sampleTestId);
      if(found){
        if(found.project_id!==task.project||found.conversation_id!==task.conversationId||found.task_id!==task.id||!found.result_available)throw new Error("Canonical Sample result identity is incomplete");
        return found;
      }
      cursor=page.next_cursor;
      if(cursor&&seen.has(cursor))throw new Error("Canonical candidate pagination did not advance");
      if(cursor)seen.add(cursor);
    }while(cursor);
    throw new Error("Current Sample Test is missing from the canonical candidate list");
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
      const [nav, safe, providers, profiles, defaults, plugins, instances, testOnly] = await Promise.all([
        this.pages<Project>("/api/navigation"), this.transport<SafeSettings>("/api/settings?view=agent-ui"),
        this.transport<{ providers: ProviderProfile[] }>("/api/providers"), this.transport<{ models: RegistryModelProfile[] }>("/api/model-profiles"),
        this.transport<GlobalModelDefaults>("/api/agent-model-bindings"), this.transport<ExpertPluginRegistry>("/api/plugins"), this.transport<{instances: InstalledModelInstance[]}>("/api/model-instances"),
        this.testEnvironment(),
      ]);
      const rows = await Promise.all(nav.map(async p => ({ p, tasks: p.conversation_id ? await this.pages<NavigationTask>(`/api/projects/${esc(p.project_id)}/conversations/${esc(p.conversation_id)}/task-navigation`) : [] })));
      if (seq !== this.navigationSequence) return;
      this.projects = new Map(nav.map(p => [p.project_id, p])); this.safeSettings = safe; this.defaults = defaults;
      this.state = { ...this.state, workspaceId: safe.sections.data_privacy.workspace_id, testOnly };
      const prefs = this.stored<Partial<Settings>>("preferences", {});
      const models = profiles.models.map(m => {
        const p = providers.providers.find(p => p.id === m.provider_id);
        const reason = !m.enabled || m.status === "disabled" ? "模型已禁用" : m.status !== "available" ? "模型尚未验证可用" : !p?.enabled ? "Provider 不可用" : !m.input_modalities.includes("text") || !m.task_capabilities.includes("text_generation") || !m.protocol_features.tool_calls ? "不支持规划所需的文本和工具调用" : undefined;
        return { id: m.id, name: m.display_name, providerId: m.provider_id, provider: p?.display_name || "Provider 不存在", reason };
      });
      const tasks: Task[] = rows.flatMap(({ p, tasks }) => tasks.map(t => {
        if (t.project_owner_id !== p.project_owner_id || t.conversation_id !== p.conversation_id) throw new Error("服务器任务归属不匹配");
        const old = this.state.tasks.find(x => x.id === t.task_id && x.project === p.project_id);
        return { ...old, id: t.task_id, project: p.project_id, conversationId:p.conversation_id || undefined, title: t.title, revision: t.schema_revision, phase: t.state, items: old?.items || [], queue: old?.queue || [], draft: this.stored(`draft.${t.task_id}`, ""), model: old?.model || "", boxes: old?.boxes || [], image: old?.image || "", actions: old?.actions || {} };
      }));
      // Unsaved composers are local input only, never fabricated persisted tasks/messages.
      for (const p of nav) tasks.push({ id: `new:${p.project_id}`, project: p.project_id, title: "新任务", revision: "", phase: "idle", items: [], queue: [], draft: this.stored(`draft.new:${p.project_id}`, ""), model: "", boxes: [], image: "", actions: { send: { available: true, reason: "只保存目标，执行需另行批准" } } });
      this.emit({ loading: false, projects: nav.map(p => ({ id: p.project_id, title: p.title })), tasks, models,
        settings: { ...initialSettings, ...prefs, revision: safe.revision, defaultModel: defaults.pipeline_builder || "", budget: safe.sections.usage_budget.future_run_budget.max_cost || "", providers: providers.providers.map(p => ({ id: p.id, name: p.display_name, endpoint: p.base_url, credential: p.credential_configured, status: p.health.status })), plugins: [...plugins.installations.map(p => ({id: p.manifest.id, name: p.manifest.display_name, version: p.manifest.version, status: p.enabled ? "已启用插件（不代表模型 Ready）" : "已禁用"})), ...instances.instances.map(i => ({id: i.id, name: i.model_id, version: i.model_bundle_version, status: i.status}))] },
      });
    } catch (e) { if (seq === this.navigationSequence) this.emit({ loading: false, error: (e as Error).message }); throw e; }
  };

  loadTask = async (project: string, id: string) => {
    const task = this.task(id); if (task.project !== project) throw new Error("任务不属于此项目");
    this.viewedTask=id;
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
      if(ws?.queue.length===100) {
        let page=ws.queue;
        do {
          const after=page.at(-1)!.receipt.message.sequence;
          page=await this.transport<QueuedMessage[]>(`${this.taskRoot(task)}/message-queue?after=${after}`,{signal:ctrl.signal});
          if(page.some(q=>q.receipt.message.sequence<=after))throw new Error("队列分页未前进，已停止读取");
          ws.queue.push(...page);
        } while(page.length===100);
      }
      if (thread.some(t => t.task_id !== id || t.project_owner_id !== p.project_owner_id || t.conversation_id !== p.conversation_id)) throw new Error("消息归属不匹配");
      if (ws) this.workspaces.set(id, ws);
      const artifacts = images.images.map(image => {
        if (!image.url.startsWith("/api/") || image.url.startsWith("//")) throw new Error("图片地址不是受控站内资源");
        if(image.thumbnail_url&&!image.thumbnail_url.startsWith("/api/"))throw new Error("缩略图地址不是受控站内资源");
        return { id: image.image_id, project, name: image.name, src: image.url, thumbnail:image.thumbnail_url, width: 0, height: 0 };
      });
      const current = this.task(id);
      const pendingApproval=this.stored<{id:string;url:string;body:unknown;execution?:string;view?:Task["approval"]}|null>(`approval.${id}`,null);
      if(pendingApproval) {
        const safeUrl = (value:string) => value.startsWith(`${this.taskRoot(task)}/`) || value===`${this.root(project)}/export` || value===`${this.root(project)}/processing-operations`;
        if(!safeUrl(pendingApproval.url) || (pendingApproval.execution&&!safeUrl(pendingApproval.execution))) throw new Error("已保存操作的地址不属于当前任务");
        this.approvals.set(id,pendingApproval);
      }
      const human = ws?.human_requests?.find(h=>h.status==="pending"&&!h.deferred);
      const sampleOp = human ? ws?.sample_operations?.find(s=>s.id===human.input.sample_test_id) : ws?.sample_operations?.find(s=>s.status==="succeeded");
      const sampleId = human?.input.sample_test_id || sampleOp?.id;
      const draftId = sampleOp?.draft_id;
      if(human&&!draftId)throw new Error("人工问题的 Sample 未提供所属 Draft 映射；不会把 checkpoint 当作 Draft ID");
      const result: Partial<Task> = {human:undefined,geometryEvidence:{},excludedCandidates:{}};
      const proposal=ws?.builder_operations?.items.find(item=>item.session?.builder_proposal)?.session?.builder_proposal;
      if(proposal) {
        const steps=proposal.draft.label_pipeline ? [...proposal.draft.label_pipeline.shared_stages.flatMap(s=>s.steps),...proposal.draft.label_pipeline.label_pipelines.flatMap(p=>p.steps)] : [];
        const modelName=(selection:string)=>this.state.models.find(m=>m.id===selection.replace(/^model-profile:/,""))?.name||this.state.settings.plugins.find(p=>p.id===selection.replace(/^model-instance:/,""))?.name||selection;
        const nodeModel=(n:typeof proposal.draft.nodes[number])=>n.model_profile_binding?modelName(n.model_profile_binding.model_profile_id):n.model_binding?modelName(n.model_binding):"";
        result.plan={revision:String(proposal.draft.revision),steps:steps.length ? steps.map(s=>`${s.node_type}${s.model_binding?` · ${modelName(s.model_binding.model_id)}`:""}`) : proposal.draft.nodes.map(n=>`${n.node_type}${nodeModel(n)?` · ${nodeModel(n)}`:""}`),images:0,models:[...new Set(proposal.draft.nodes.map(nodeModel).filter(Boolean))],destination:"已保存的 Builder proposal（不是新推理）",budget:null};
      }
      if(ws) {
        const jobs=await this.transport<{id:string;format?:string;result?:ProjectExportResult;error?:string}[]>(`${this.taskRoot(task)}/exports`,{signal:ctrl.signal});
        // Training packages have a different frozen receipt and their own same-chat card.
        result.exports=jobs.filter(j=>j.format!=="ultralytics_yolo_detection").map(j=>({id:j.id,status:j.error?"failed":j.result?.delivery?"ready":"unknown",url:j.result?.delivery?`${this.root(project)}/exports/${esc(j.result.delivery.id)}/download`:undefined,detail:j.error || (j.result?.report?`${j.result.report.exported_count} 条已导出；${j.result.report.skipped_count} 条跳过`:"未取得完成回执，不显示成功下载")}));
        result.processing=await Promise.all((ws.processing_operations||[]).filter(p=>p.batch_id).map(async p=>{
          const batch=await this.transport<{batch:{project_id:string;status:string}}>(`/api/batches/${esc(p.batch_id!)}`,{signal:ctrl.signal});
          if(batch.batch.project_id!==project)throw new Error("处理批次不属于当前项目");
          return {id:p.id,batch:p.batch_id!,status:batch.batch.status,url:`/projects/${esc(project)}/manage/batches/${esc(p.batch_id!)}`};
        }));
      }
      if(sampleId && draftId) {
        const value=await this.transport<{sample_test:WorkflowSampleTestRecord;annotation_schema?:{task:{kind:string;labels:string[]}}}>(`/api/workflow-drafts/${esc(draftId)}/sample-test?test_id=${esc(sampleId)}`,{signal:ctrl.signal});
        const record=value.sample_test;
        if(!record || record.id!==sampleId || record.draft_id!==draftId || record.project_id!==project) throw new Error("样例不属于当前项目和任务");
        const canonical=await this.canonicalSampleSelection(task,sampleId,ctrl.signal);
        if(canonical.draft_id!==draftId||canonical.draft_revision!==record.draft_revision)throw new Error("Canonical candidate Draft does not match the saved Sample Test");
        const boxesByImage:Record<ImageId,Box[]>={}, imageResults:NonNullable<Task["imageResults"]>={}, annotationsByImage:Record<string,import("../types").Annotation[]>={};
        let requestedLabel="", requestedKind="", feedbackVersion="";
        for(const [index,input] of record.inputs.entries()) {
          const image=images.images.find(i=>i.image_id===input.image_id), asset=artifacts.find(a=>a.id===input.image_id);
          if(!image || !asset || image.content_hash!==input.content_hash) throw new Error("样例图片内容哈希已变化；不会替换原始证据");
          const sample=record.report.samples[index]; if(!sample) continue;
          result.geometryEvidence![input.image_id]=sample;
          const feedback=await this.transport<{revisions:SampleFeedbackRevision[]}>(`/api/workflow-sample-tests/${esc(sampleId)}/images/${esc(input.image_id)}/feedback`,{signal:ctrl.signal});
          feedbackVersion+=`${input.image_id}:${feedback.revisions.at(-1)?.sequence || 0};`;
          const original=terminalSampleAnnotations(sample,input.image_id,sampleId);
          const overlay=sampleFeedbackOverlay(original,feedback.revisions);
          const annotations=overlay.annotations;result.excludedCandidates![input.image_id]=overlay.excluded;
          annotationsByImage[input.image_id]=annotations;
          const dims=await this.measure(asset.src);asset.width=dims.width;asset.height=dims.height;
          boxesByImage[input.image_id]=annotations.flatMap(a=>a.value.kind==="bounding_box"?[{id:a.id,label:a.label || "",x:a.value.rect[0]*dims.width,y:a.value.rect[1]*dims.height,w:a.value.rect[2]*dims.width,h:a.value.rect[3]*dims.height}]:[]);
          imageResults[input.image_id]={labels:annotations.flatMap(a=>a.value.kind==="classification"?a.value.labels:[]),risks:sample.projection?.review_candidates.map(r=>r.explanation.summary) || (sample.projection?[]:["旧样例没有终端投影，未显示中间框"])};
          if(human?.input.image_id===input.image_id) {
            const annotation=annotations.find(a=>a.id===human.input.outcome_id);
            if(!annotation) throw new Error("人工请求的候选不是当前样例终端结果；没有替换成其他候选");
            requestedLabel=annotation.label || "";
            requestedKind=annotation.value.kind;
          }
        }
        result.boxesByImage=boxesByImage;result.imageResults=imageResults;result.resultRevision=`${sampleId}:${feedbackVersion}`;
        result.sample={id:sampleId,draft:draftId,revision:record.draft_revision};
        result.sampleResult=deliverySampleResultFromCanonical(canonical,annotationsByImage);
        if(human) result.human={id:human.input.id,image:human.input.image_id,kind:requestedKind || "unsupported",labels:value.annotation_schema?.task.labels || (requestedLabel?[requestedLabel]:[]),label:requestedLabel,candidate:human.input.outcome_id || ""};
      }
      const persistedStop = this.stored<{id:string}|null>(`stop.${id}`, null) || [...thread].reverse().find(t=>t.message.input.reference?.scope==="stop_request");
      const stop = persistedStop && ws ? await this.transport<StopRequestRecord & {normalized_state: Phase|null}>(`${this.conversation(project)}/stop-requests/${esc(persistedStop.id)}`, {signal:ctrl.signal}) : null;
      const selectionRaw=persistedStop?this.storage?.getItem(this.key(`stop-selection.${id}.${persistedStop.id}`))||null:null;
      const stopSelection=stop?ownedStopSelection(stop,p.conversation_id!,persistedStop!.id,selectionRaw):undefined;
      if(stopSelection&&stop?.selected_target)this.save(`stop-selection.${id}.${persistedStop!.id}`,null);
      if (seq !== this.sequence) return;
      const receipts = [
        ...(ws?.calls || []).map(c=>({id:c.id,title:"模型结构化决策",status:c.status==="completed" && (c.failure || c.evidence?.decision?.Err) ? "invalid_result" : c.status,detail:failureDetail(c.failure) || c.evidence?.decision?.Ok?.rationale || c.evidence?.decision?.Err || c.evidence?.error,startedAt:c.started_at || undefined,finishedAt:c.completed_at || undefined,durationMs:c.duration_ms ?? undefined,stage:callStage(c.stage)})),
        ...(ws?.builder_operations?.items || []).map(b=>({id:b.operation.id,title:"方案构建回执",status:b.operation.status,detail:b.operation.evidence?.error || b.operation.evidence?.outcome})),
        ...(ws?.sample_operations || []).map(s=>({id:s.id,title:"样例测试回执",status:s.status,detail:s.error})),
      ];
      const mainline=ws?.mainline?this.assertMainline(ws,task):undefined;
      const automaticJourney = mainline?.available_actions.some(candidate=>candidate.id==="inspect_automatic_sample_progress"&&candidate.state==="available"&&candidate.method==="GET"&&!candidate.requires_confirmation);
      const active = ws?.calls.some(c=>c.status==="reserved") || ws?.sample_operations?.some(s=>["running","queued","cancelling"].includes(s.status)) || result.processing?.some(p=>["pending","running","pausing"].includes(p.status)) || automaticJourney;
      const phase: Phase = stop?.normalized_state || (active ? "running" : ws?.calls.some(c=>c.status==="in_doubt") ? "outcome_unknown" : human ? "waiting_for_human" : "idle");
      const edits=this.stored<{revision?:string;boxes?:Record<ImageId,Box[]>}>(`edits.${id}`,{});
      this.emit({ error: undefined, artifacts, tasks: this.state.tasks.map(t => t.id !== id ? t : { ...t,
        items: [...thread.map(t => {const referenceText=persistedReferenceText(t.message.input);return { id: t.id, role: "user" as const, kind:"input" as const, text: t.message.input.text,source:{kind:"message" as const,id:t.id},...(referenceText?{referenceText}:{}) };}),...projectCallMessages(ws?.calls||[])],
        ...result, approval:pendingApproval?.view || t.approval, actions: {...ws?.actions || t.actions,answer:{available:!!result.human && ["classification","bounding_box"].includes(result.human.kind),reason:"仅保存当前人工作答的样例修正"}}, model: ws?.agent_model.model_profile_id || this.defaults.pipeline_builder || t.model,
        loaded:true, image: human?.input.image_id || artifacts[0]?.id || "", editBoxes: edits.revision===result.resultRevision ? edits.boxes || {} : {},
        phase, receipts, humanQuestion:human?.input.question,mainline,
        stopTargets:stop?.status==="needs_selection"?stop.targets.filter(t=>!stopSelection||stopTargetMatches(t,stopSelection.target)).map(target=>({id:`${target.kind}:${target.id}`,label:`${stopSelection?"核实原选择 · ":""}${this.state.tasks.find(t=>t.id===target.task_id)?.title||target.task_id} · ${target.kind} · ${target.id.slice(0,8)} · ${target.state}`})):[],
        resumeTargets:ws?.resume_actions?.filter(a=>a.available).map(a=>({id:`${a.kind}:${a.id}`,label:a.kind,reason:a.reason})),
        queue: ws?.queue.filter(q => isPendingQueuedMessage(q.status)).map(q => q.input.message.text) || [],
        queueEntries: ws?.queue.map(q=>({id:q.input.message.id,text:q.input.message.text,status:q.status,canCancel:canCancelQueuedMessage(q.status),canPlan:!human&&q.status==="waiting_for_dispatch"&&!q.planning_call_id})),
      }) });
    } catch (e) { if (seq !== this.sequence || ctrl.signal.aborted) return; this.emit({ error: (e as Error).message, artifacts: [] }); throw e; }
  };
  async createTask(project: string) { this.root(project); return `new:${project}`; }
  async uploadImages(c:Command, files:File[]) {
    const task=this.checked(c);
    for (const file of files) {
      const receipt=await api.uploadImage(task.project,file);
      if(receipt.corrupt.length) throw new Error(receipt.corrupt.map(e=>`${e.name}: ${e.message}`).join("；"));
      if(!receipt.imported&&!receipt.duplicates) throw new Error(`${file.name} 未被服务器导入`);
    }
    await this.reloadCurrent(task);
  }
  saveDraft(id: string, text: string) { this.task(id); this.save(`draft.${id}`, text); this.emit({ tasks: this.state.tasks.map(t => t.id === id ? { ...t, draft: text } : t) }); }
  saveArtifactDraft(id: string, image: ImageId, boxes: Box[]) {
    const task=this.task(id);if(!task.loaded)return;
    const editBoxes={...task.editBoxes};
    if(JSON.stringify(boxes)===JSON.stringify(task.boxesByImage?.[image] || []))delete editBoxes[image];
    else editBoxes[image]=boxes;
    this.save(`edits.${id}`,{revision:task.resultRevision,boxes:editBoxes});
    this.emit({tasks:this.state.tasks.map(t=>t.id===id?{...t,editBoxes}:t)});
  }
  async sendMessage(c: Command, text: string, mode: "plan" | "execute", _model: string) {
    const task = this.checked(c);
    const selection=c.selection;
    if(selection&&"preview" in selection)unsupported("演示候选没有服务器 lineage，不能发送到真实任务");
    const frozen=selection?assertVisualSelection(selection as VisualSelection,task.project,task.id):undefined;
    let p = this.projects.get(task.project)!;
    if (!p.conversation_id) { const value = await this.transport<{conversation_id:string}>(`${this.root(task.project)}/conversations`, {method:"POST"}); p = {...p,conversation_id:value.conversation_id};this.projects.set(task.project,p); }
    const root = this.conversation(task.project);
    const pending = this.stored<SendCommand|null>(`send.${task.id}`,null);
    if (pending && (pending.message.text !== text || pending.mode !== mode)) throw new Error("上一条发送结果尚未确认。请保留原内容重试，不能换新命令掩盖未知结果。");
    const currentSchema=(await this.transport<{revision:string}>(`${this.root(task.project)}/goal`)).revision;
    if(frozen&&frozen.project_schema_revision!==currentSchema)throw new Error("候选引用所用的 Project Schema 已变化；请重新打开当前样例后再发送，未调用模型。");
    const input = pending || {message:frozen?selectedMessage(c.id,text,frozen):{id:c.id,text,image:null},task_id:task.id.startsWith("new:")?null:task.id,schema_revision:currentSchema,agent_model:await this.transport<Preference>(`${root}/agent-model`),mode};
    this.save(`send.${task.id}`,input);
    let receipt:SendReceipt;
    try { receipt=await this.transport<SendReceipt>(`${root}/send`, {method:"POST",body:JSON.stringify(input)}); }
    catch(error) {
      if(error instanceof ApiRequestError && error.status===409 && error.code==="send_model_selection_changed") {
        this.save(`send.${task.id}`,null);await this.reloadCurrent(task);
        throw new Error("服务器未接受发送：模型偏好已变化。文字已保留，请确认当前模型后重新发送。");
      }
      throw error;
    }
    if (receipt.message.input.id !== input.message.id) throw new Error("发送回执 ID 不匹配");
    this.save(`send.${task.id}`,null); this.saveDraft(task.id, "");
    await this.refresh(); if(this.viewedTask===task.id)await this.loadTask(task.project,receipt.task_id); return receipt.task_id;
  }
  async prepareAction(c: Command, kind: "plan" | "sample" | "process" | "export") {
    const task = this.checked(c); if(task.id.startsWith("new:")) throw new Error("请先保存目标");
    if(this.stored(`approval.${task.id}`,null)) throw new Error("上次批准的结果待核对；请读取原回执，不能自动发起新的付费操作");
    const root = this.taskRoot(task);
    if(kind === "sample") {
      const combined=task.mainline?.available_actions.find(item=>item.id==="build_and_test_pipeline");
      if(combined) {
        if(combined.state!=="requires_confirmation"||combined.method!=="GET"||!combined.requires_confirmation)throw new Error("服务器没有提供可确认的方案与样例范围");
        const read=new URL(combined.url,"http://annotagent.local");
        if(read.origin!=="http://annotagent.local"||read.pathname!==`${root}/journey-preview`||read.search||read.hash)throw new Error("服务器返回的方案与样例预览地址不属于当前任务");
        const p=await this.transport<JourneyPreview>(combined.url);
        const raw=p.consent;
        const complete=[raw.id,raw.task_id,raw.builder_operation_id,raw.sample_operation_id,raw.builder_scope_hash,raw.schema_digest,raw.expires_at].every(value=>typeof value==="string"&&value.length>0);
        const models=p.data?.models||[];
        const modelScopes=new Map(models.map(model=>[`${model.scope.model_id}:${model.scope.binding_digest}`,model]));
        if(!complete||raw.task_id!==task.id||!Array.isArray(raw.images)||raw.images.length<1||raw.images.length>3||raw.images.some(image=>!image.image_id||!image.content_hash)||!Array.isArray(raw.allowed_models)||raw.allowed_models.length<1||raw.allowed_models.some(model=>!model.model_id||!model.binding_digest||!modelScopes.has(`${model.model_id}:${model.binding_digest}`))||!Number.isSafeInteger(raw.maximum_builder_calls)||raw.maximum_builder_calls<1||!Number.isSafeInteger(raw.maximum_sample_calls)||raw.maximum_sample_calls<1)throw new Error("服务器返回的方案与样例范围不完整；未执行模型调用");
        const consent:JourneyConsent={...raw,allow_unknown_cost:true,...(raw.schema_proposal?{schema_proposal:{...raw.schema_proposal,allow_unknown_cost:true}}:{})};
        this.approvals.set(task.id,{id:c.id,url:`${root}/journey-consents`,body:consent});
        this.emit({tasks:this.state.tasks.map(t=>t.id===task.id?{...t,approval:{id:c.id,title:"确认这次样例范围",revision:consent.builder_scope_hash,budget:null,scope:[`${consent.images.length} 张图片已冻结；最多 ${consent.maximum_builder_calls} 次规划调用、${consent.maximum_sample_calls} 次样例调用`,p.builder.model_name,p.builder.destination,...models.map(model=>`${model.display_name} → ${model.destination}`),`有效期：${consent.expires_at}`,"保存这一次授权后由服务器连续准备标注规范、生成方案并运行样例；不发布、不批量处理、不写正式标注"]}}:t)});
        return;
      }
      const action=task.mainline?.available_actions.find(item=>item.id==="test_pipeline_samples");
      if(action) {
        if(action.state!=="requires_confirmation"||action.method!=="GET"||action.execution_method!=="POST"||!action.requires_confirmation||!action.execution_url)throw new Error("已保存方案的样例范围不可执行；请重新读取任务状态");
        const allowedRoot=`${root}/journey-consents/`;
        const read=new URL(action.url,"http://annotagent.local"),execute=new URL(action.execution_url,"http://annotagent.local");
        if(read.origin!=="http://annotagent.local"||execute.origin!=="http://annotagent.local"||!read.pathname.startsWith(allowedRoot)||read.pathname.length<=allowedRoot.length||read.search||read.hash||execute.pathname!==`${read.pathname}/execution`||execute.search||execute.hash)throw new Error("服务器返回的样例继续地址不属于当前任务");
        const scope=action.scope as {journey_consent_id?:string;sample_operation_id?:string;draft_id?:string;draft_revision?:number;draft_content_hash?:string;images?:{image_id:string;content_hash:string}[];allowed_models?:{model_id:string;binding_digest:string}[];maximum_sample_calls?:number;expires_at?:string}|undefined;
        if(!scope||![scope.journey_consent_id,scope.sample_operation_id,scope.draft_id,scope.draft_content_hash,scope.expires_at].every(value=>typeof value==="string"&&value.length>0)||!Number.isSafeInteger(scope.draft_revision)||(scope.draft_revision ?? 0)<1||!Number.isSafeInteger(scope.maximum_sample_calls)||(scope.maximum_sample_calls ?? 0)<1||!Array.isArray(scope.images)||scope.images.length<1||!Array.isArray(scope.allowed_models)||scope.allowed_models.length<1||read.pathname!==`${allowedRoot}${esc(scope.journey_consent_id!)}`)throw new Error("服务器返回的样例范围不完整；未执行模型调用");
        const record=await this.transport<{consent:JourneyConsent;resolved_consent?:JourneyConsent|null;revoked:boolean;sample:{operation_id?:string;draft_id:string;draft_revision:number;images?:{image_id:string;content_hash:string}[];models?:{model_id:string;binding_digest:string}[];maximum_calls?:number}|null}>(action.url);
        const consent=record.resolved_consent||record.consent;
        const sameImages=(left:{image_id:string;content_hash:string}[],right:{image_id:string;content_hash:string}[])=>left.length===right.length&&left.every((item,index)=>item.image_id===right[index]?.image_id&&item.content_hash===right[index]?.content_hash);
        const sameModels=(left:{model_id:string;binding_digest:string}[],right:{model_id:string;binding_digest:string}[])=>left.length===right.length&&left.every((item,index)=>item.model_id===right[index]?.model_id&&item.binding_digest===right[index]?.binding_digest);
        const sealedSampleMismatch=record.sample&&(record.sample.operation_id!==scope.sample_operation_id||record.sample.draft_id!==scope.draft_id||record.sample.draft_revision!==scope.draft_revision);
        if(record.revoked||record.consent.id!==scope.journey_consent_id||consent.sample_operation_id!==scope.sample_operation_id||consent.maximum_sample_calls!==scope.maximum_sample_calls||consent.expires_at!==scope.expires_at||!sameImages(consent.images,scope.images)||!sameModels(consent.allowed_models,scope.allowed_models)||sealedSampleMismatch)throw new Error("已保存方案与服务器冻结的样例范围不一致；未执行模型调用");
        const frozenImages=scope.images,frozenModels=scope.allowed_models;
        this.approvals.set(task.id,{id:c.id,url:action.execution_url,body:{}});
        this.emit({tasks:this.state.tasks.map(t=>t.id===task.id?{...t,approval:{id:c.id,title:"测试当前方案样例",revision:`Draft ${scope.draft_revision} · ${scope.draft_content_hash!.slice(0,8)}`,budget:null,scope:[`${frozenImages.length} 张已冻结图片；最多 ${scope.maximum_sample_calls} 次样例调用`,...frozenModels.map(model=>`${model.model_id} · ${model.binding_digest.slice(0,8)}`),`有效期：${scope.expires_at}`,"沿用已保存方案；不重建、不发布、不批量处理、不写正式标注"]}}:t)});
        return;
      }
    }
    if(kind === "process") {
      const action=task.mainline?.available_actions.find(item=>item.id==="start_delivery_processing"&&item.state==="requires_confirmation");
      if(!action||action.method!=="GET"||!action.requires_confirmation)throw new Error("服务器尚未提供当前交付版本的正式处理确认范围");
      const parsed=new URL(action.url,"http://annotagent.local");
      const allowedPath=`${this.root(task.project)}/processing-preview`;
      const keys=[...parsed.searchParams.keys()];
      if(parsed.origin!=="http://annotagent.local"||parsed.pathname!==allowedPath||keys.length!==2||!keys.includes("draft_id")||!keys.includes("sample_test_id"))throw new Error("服务器返回的正式处理预览地址不属于当前项目或范围不完整");
      const selection={draft_id:parsed.searchParams.get("draft_id")!,sample_test_id:parsed.searchParams.get("sample_test_id")!};
      const p=await this.transport<ProcessingAuthorization>(action.url);
      const scope=action.scope as {delivery_revision?:number;delivery_sha256?:string;draft?:{draft_id?:string;draft_revision?:number;sample_test_id?:string};images?:{image_id:string;content_sha256:string}[]} | undefined;
      if(!scope||scope.draft?.draft_id!==selection.draft_id||scope.draft.sample_test_id!==selection.sample_test_id||scope.draft.draft_revision!==p.revision||!Number.isSafeInteger(scope.delivery_revision)||!scope.delivery_sha256||!Array.isArray(scope.images))throw new Error("正式处理预览与任务冻结范围不一致；未创建处理操作");
      const frozenImageCount=scope.images.length;
      const body={request_id:c.id,selection,expected_revision:p.revision,authorization_fingerprint:p.authorization_fingerprint};
      this.approvals.set(task.id,{id:c.id,url:`${this.root(task.project)}/processing-operations`,body});
      this.emit({tasks:this.state.tasks.map(t=>t.id===task.id?{...t,approval:{id:c.id,title:"确认方案并开始处理",revision:`交付 ${scope.delivery_revision} · Draft ${p.revision}`,budget:null,scope:[p.plan_name,`${p.image_count} 张图片（服务器冻结 ${frozenImageCount} 个内容哈希）；最多 ${p.maximum_model_calls} 次模型调用`,...p.models.map(m=>`${m.remote_model_id} → ${m.provider_base_url}`),...(p.native_models||[]).map(m=>`${m.name} → ${m.destination}`),"发布不可变版本并启动一次 Dataset Run；需审核结果不自动接受"]}}:t)});
    } else if(kind === "export") {
      const p=await this.transport<ExportReadiness>(`${this.root(task.project)}/export-readiness`);
      if(p.project_id!==task.project || !p.ready || !p.formats.some(f=>f.format==="native"&&f.supported)) throw new Error("当前正式标注尚未满足 Native 导出要求；请先处理或审核。样例通过不等于正式标注。 "+JSON.stringify(p.blocking_issues));
      const body={format:"native",background:true,conversation:{id:c.id,conversation_id:this.projects.get(task.project)!.conversation_id,task_id:task.id}};
      this.approvals.set(task.id,{id:c.id,url:`${this.root(task.project)}/export`,body});
      this.emit({tasks:this.state.tasks.map(t=>t.id===task.id?{...t,approval:{id:c.id,title:"导出为 Native",revision:task.revision,budget:"不调用模型",scope:[`${p.image_count} 张图片；${p.accepted_annotations} 条已确认标注；${p.unresolved_reviews} 条待审核`,"实际范围以服务器导出时的可用数据及报告为准",...p.formats.filter(f=>f.format==="native").flatMap(f=>f.warnings)]}}:t)});
    } else if (kind === "plan") {
      // Composer preference applies to the next Send, not this admitted task.
      // Let the server resolve its persisted Send model before freezing consent.
      const p = await this.transport<ConversationSchemaPreview>(`${root}/schema-preview`);
      const body = {call_id:c.id,model_id:p.model_id,scope_hash:p.scope_hash,expires_at:p.expires_at,allow_unknown_cost:true};
      this.approvals.set(task.id,{id:c.id,url:`${root}/schema-proposals`,body});
      this.emit({tasks:this.state.tasks.map(t=>t.id===task.id?{...t,approval:{id:c.id,title:"批准目标规划（仅文本规划）",revision:p.scope_hash,budget:null,scope:[p.model_name,p.destination,p.data_scope,`${p.image_count} 张图片；最多 ${p.maximum_calls} 次调用`,`有效期：${p.expires_at}`,"不会发布、批处理或自动接受标注"]}}:t)});
    } else {
      const bindings = await this.transport<{bindings:{model_profile_id:string}[]}>(`${this.root(task.project)}/model-bindings`);
      const models = [...new Set(bindings.bindings.map(b=>`model-profile:${b.model_profile_id}`))];
      if(!models.length) throw new Error("项目尚未绑定视觉模型，请先设置；不自动扩大到全部 Registry 模型");
      // Ready local refinement is part of the explicitly displayed approval set,
      // never a hidden post-approval expansion or an installation request.
      const local=await this.transport<{model_profiles?:import("../types").ModelInstanceProfile[]}>("/api/model-instances");
      models.push(...(local.model_profiles||[]).filter(m=>m.selectable&&m.capabilities.includes("prompted_segmentation")&&m.selection_id.startsWith("model-instance:")).map(m=>m.selection_id));
      const delivery=await this.transport<{required:boolean;schema:null|{id:string;revision:number}}>(`${root}/delivery-schema`);
      if(delivery.required && !delivery.schema)throw new Error("请先确认已保存的交付目标；旧目标规范不适用于当前版本，不会额外调用模型猜测类别。");
      const query = new URLSearchParams({consent_id:c.id,builder_operation_id:crypto.randomUUID(),sample_operation_id:crypto.randomUUID(),allowed_models:JSON.stringify(models)});
      if(delivery.required && delivery.schema){query.set("schema_id",delivery.schema.id);query.set("schema_revision",String(delivery.schema.revision));}else{query.set("schema_call_id",crypto.randomUUID());}
      const p = await this.transport<JourneyPreview>(`${root}/journey-preview?${query}`);
      const consent: JourneyConsent = {...p.consent,allow_unknown_cost:true,...(p.consent.schema_proposal?{schema_proposal:{...p.consent.schema_proposal,allow_unknown_cost:true}}:{})};
      this.approvals.set(task.id,{id:c.id,url:`${root}/journey-consents`,body:consent,execution:`${root}/journey-consents/${esc(consent.id)}/execution`});
      this.emit({tasks:this.state.tasks.map(t=>t.id===task.id?{...t,approval:{id:c.id,title:"批准生成方案并测试样例",revision:consent.builder_scope_hash,budget:null,scope:[`${consent.images.length} 张图片已冻结；最多 ${consent.maximum_builder_calls} 次规划调用、${consent.maximum_sample_calls} 次样例调用`,p.builder.model_name,p.builder.destination,...p.data.models.map(m=>`${m.display_name} → ${m.destination}`),`有效期：${consent.expires_at}`,"生成并保存方案草稿后，在同一冻结授权内测试样例；不发布、不批量处理、不写正式标注"]}}:t)});
    }
  }
  async prepareQueue(c:Command,message:string) {
    const task=this.checked(c);
    if(!task.queueEntries?.some(q=>q.id===message&&q.canPlan))throw new Error("此输入当前不能开始新的规划");
    if(this.stored(`approval.${task.id}`,null))throw new Error("上次授权结果待核对，请先恢复原回执");
    const root=`${this.taskRoot(task)}/message-queue/${esc(message)}`;
    const p=await this.transport<QueuePreview>(`${root}/schema-preview`);
    const body:QueueConsent={call_id:c.id,model_id:p.model_id,scope_hash:p.scope_hash,request_hash:p.request_hash,previous_grant_id:p.previous_grant_id,maximum_calls:p.maximum_calls,expires_at:p.expires_at,allow_unknown_cost:true};
    this.approvals.set(task.id,{id:c.id,url:`${root}/schema-proposals`,body});
    this.emit({tasks:this.state.tasks.map(t=>t.id===task.id?{...t,approval:{id:c.id,title:"批准此补充输入的文本规划",revision:p.scope_hash,budget:null,scope:[p.model_name,p.destination,p.data_scope,`${p.new_request_limit} 次新调用；累计上限 ${p.maximum_calls} 次`,p.operation,"不自动修改 Workflow、不执行图片、不派发剩余队列"]}}:t)});
  }
  async approveAction(c: Command) {
    const task = this.checked(c), p = this.approvals.get(task.id);
    if(!p || p.id!==task.approval?.id) throw new Error("精确授权已失效，请重新读取范围");
    // Persist the exact intent before POST. Neither mount nor refresh executes it.
    this.save(`approval.${task.id}`,{...p,view:task.approval});
    await this.transport(p.url,{method:"POST",body:JSON.stringify(p.body)});
    if(p.execution) await this.transport(p.execution,{method:"POST",body:"{}"});
    this.approvals.delete(task.id); this.save(`approval.${task.id}`,null);
    this.emit({tasks:this.state.tasks.map(t=>t.id===task.id?{...t,approval:undefined}:t)});
    await this.reloadCurrent(task);
  }
  async interruptOperation(c: Command) {
    const task = this.checked(c); if(!this.workspaces.get(task.id)?.actions.stop?.available) throw new Error("服务端未提供可停止操作");
    let prior=this.stored<{id:string;text:string;image:null;reference:{scope:"stop_request";task_id:string}}|null>(`stop.${task.id}`,null);
    if(prior) {
      const old=await this.transport<(StopRequestRecord&{normalized_state?:Phase})|null>(`${this.conversation(task.project)}/stop-requests/${esc(prior.id)}`);
      if(old && (old.status==="finished" || old.status==="no_active_work" || ["interrupted","outcome_unknown"].includes(old.normalized_state || "")))prior=null;
    }
    const input = prior || {id:c.id,text:"停止",image:null,reference:{scope:"stop_request" as const,task_id:task.id}};
    this.save(`stop.${task.id}`,input);
    const result = await this.transport<StopRequestRecord>(`${this.conversation(task.project)}/stop-requests`,{method:"POST",body:JSON.stringify(input)});
    await this.reloadCurrent(task);
    if(result.dispatch_error) throw new Error(result.dispatch_error);
  }
  async selectStop(c:Command,target:string) {
    const task=this.checked(c),input=this.stored<{id:string}|null>(`stop.${task.id}`,null);
    if(!input || !task.stopTargets?.some(t=>t.id===target))throw new Error("停止目标已变化，请重新读取");
    const root=`${this.conversation(task.project)}/stop-requests/${esc(input.id)}`;
    const record=await this.transport<StopRequestRecord>(root);
    const suffix=`stop-selection.${task.id}.${input.id}`;
    const pending=ownedStopSelection(record,task.conversationId!,input.id,this.storage?.getItem(this.key(suffix))||null);
    const chosen=record.targets.find(t=>`${t.kind}:${t.id}`===target);
    if(!chosen)throw new Error("此目标不在已冻结的停止请求中");
    if(pending&&!stopTargetMatches(pending.target,chosen))throw new Error("原停止选择尚未核实，不能切换目标");
    const selection=pending||{message_id:input.id,target:{kind:chosen.kind,id:chosen.id,task_id:chosen.task_id},pending:true};
    if(!this.storage)throw new Error("无法保存停止目标恢复记录，未发送请求");
    this.save(suffix,selection);
    const result=await this.transport<StopRequestRecord>(`${root}/select`,{method:"POST",body:JSON.stringify({target:selection.target})});
    ownedStopSelection(result,task.conversationId!,input.id,JSON.stringify(selection));
    if(!result.selected_target||!stopTargetMatches(result.selected_target,selection.target))throw new Error("停止选择尚未被服务器确认");
    this.save(suffix,null);
    await this.reloadCurrent(task);
  }
  async resumeOperation(c: Command, target?:string) {
    const task = this.checked(c), actions = this.workspaces.get(task.id)?.resume_actions?.filter(a=>a.available)||[];
    const a=target?actions.find(a=>`${a.kind}:${a.id}`===target):actions.length===1?actions[0]:undefined;
    if(!a) throw new Error("需要唯一、明确可用的恢复点；不会恢复未知或已取消操作");
    if(a.method!=="POST" || !(a.url.startsWith(`${this.taskRoot(task)}/human-requests/`) || /^\/api\/batches\/[a-f0-9-]+\/resume$/.test(a.url))) throw new Error("继续地址不是受控站内任务动作");
    await this.transport(a.url,{method:"POST",body:"{}"}); await this.reloadCurrent(task);
  }
  async cancelQueue(c:Command,id:string) { const task=this.checked(c); if(!task.queueEntries?.some(q=>q.id===id&&q.canCancel)) throw new Error("队列项不可取消");await this.transport(`${this.taskRoot(task)}/message-queue/${esc(id)}/cancel`,{method:"POST",body:"{}"});await this.reloadCurrent(task); }
  async selectAgentModel(c: Command, model: string) {
    const task = this.checked(c), selected = this.state.models.find(m => m.id === model);
    if (!selected || selected.reason) throw new Error(selected?.reason || "模型不存在");
    let p = this.projects.get(task.project)!;
    if (!p.conversation_id) { const result = await this.transport<{conversation_id:string}>(`${this.root(task.project)}/conversations`, {method:"POST"}); p = {...p, conversation_id:result.conversation_id}; this.projects.set(task.project,p); }
    const root = `${this.conversation(task.project)}/agent-model`;
    let input = this.stored<{request_id:string;expected_revision:number;model_profile_id:string}|null>(`model.${task.project}`,null) || this.modelCommands.get(c.id);
    if (input && input.model_profile_id !== model) throw new Error("模型选择重试的内容发生变化");
    if (!input) { const current = await this.transport<Preference>(root); input = {request_id:c.id,expected_revision:current.revision,model_profile_id:model}; this.modelCommands.set(c.id,input); }
    this.save(`model.${task.project}`,input);
    await this.transport(root, {method:"POST", body:JSON.stringify(input)});
    this.save(`model.${task.project}`,null);
    await this.reloadCurrent(task);
    // New composers have no workspace yet; display only the confirmed preference.
    if (task.id.startsWith("new:")) this.emit({tasks:this.state.tasks.map(t=>t.project===task.project?{...t,model}:t)});
  }
  async answerHumanRequest(c: Command, boxes: Box[], classification?:string) {
    const task=this.checked(c), request=this.workspaces.get(task.id)?.human_requests?.find(h=>h.input.id===task.human?.id&&h.status==="pending"&&!h.deferred);
    if(!request || !task.human) throw new Error("没有当前可提交的人工问题");
    const candidate=boxes.find(b=>b.id===request.input.outcome_id), asset=this.state.artifacts.find(a=>a.id===request.input.image_id);
    let corrected:SampleFeedbackRevision["corrected_value"];
    let label=classification;
    if(task.human.kind==="classification") { if(!classification || !task.human.labels.includes(classification)) throw new Error("请选择 Schema 中的类别"); corrected={kind:"classification",labels:[classification]}; }
    else if(candidate && asset?.width && asset.height) {
      if(![candidate.x,candidate.y,candidate.w,candidate.h].every(Number.isFinite) || candidate.w<=0 || candidate.h<=0 || candidate.x<0 || candidate.y<0 || candidate.x+candidate.w>asset.width || candidate.y+candidate.h>asset.height) throw new Error("边界框超出原图或尺寸无效");
      corrected={kind:"bounding_box",rect:[candidate.x/asset.width,candidate.y/asset.height,candidate.w/asset.width,candidate.h/asset.height]};label=candidate.label;
    } else throw new Error("当前候选/原始尺寸不可用");
    const previous=this.stored<SampleFeedbackRevision|null>(`answer.${request.input.id}`,null);
    if(previous && JSON.stringify(previous.corrected_value)!==JSON.stringify(corrected)) throw new Error("上次答案回执未知，请使用原答案重试或读取保存结果");
    const answer=previous || {revision_id:c.id,sample_test_id:request.input.sample_test_id,image_id:request.input.image_id,sequence:request.input.expected_feedback_sequence+1,reason:task.human.kind==="classification"?"wrong_target":"poor_boundary",outcome_id:request.input.outcome_id,corrected_value:corrected,corrected_label:label,note:"用户在 Agent 工作区提交样例修正",created_at:new Date().toISOString()} satisfies SampleFeedbackRevision;
    this.save(`answer.${request.input.id}`,answer);
    const saved=await this.transport<HumanRequest>(`${this.taskRoot(task)}/human-requests/${esc(request.input.id)}/answer`,{method:"POST",body:JSON.stringify({answer})});
    if(saved.answer?.revision_id!==answer.revision_id) throw new Error("服务器没有确认相同答案版本");
    this.save(`answer.${request.input.id}`,null); this.save(`edits.${task.id}`,{});
    await this.reloadCurrent(task);
  }
  async updateSettings(revision: string, settings: Settings) {
    const old = this.state.settings;
    const budgetChanged = settings.budget !== old.budget;
    const providersChanged = JSON.stringify(settings.providers) !== JSON.stringify(old.providers);
    const modelChanged = settings.defaultModel !== old.defaultModel;
    if ([budgetChanged, providersChanged, modelChanged].filter(Boolean).length > 1) throw new Error("请分别保存不同设置分类；服务端不支持跨分类原子提交");
    if (settings.allowExternal !== old.allowExternal || settings.cache !== old.cache) throw new Error("外传需要逐次授权；清理需要真实范围预览，不能用设置偏好代替");
    if (budgetChanged) {
      if (!/^(0|[1-9]\d*)(\.\d{1,8})?$/.test(settings.budget)) throw new Error("预算必须是有效的非负美元金额");
      if (!this.safeSettings) throw new Error("请先读取预算");
      await this.transport("/api/settings", {method:"PATCH",body:JSON.stringify({expected_revision:revision,budget:{...this.safeSettings.sections.usage_budget.future_run_budget,max_cost:settings.budget}})});
    }
    if (modelChanged) await this.transport("/api/agent-model-bindings", {method:"PUT", body:JSON.stringify({...this.defaults,pipeline_builder:settings.defaultModel || null})});
    if (providersChanged) {
      const changed = settings.providers.filter(p => JSON.stringify(p) !== JSON.stringify(old.providers.find(o=>o.id===p.id)));
      const removed = old.providers.filter(p=>!settings.providers.some(n=>n.id===p.id));
      if (changed.length + removed.length !== 1) throw new Error("一次只能修改一个 Provider");
      for (const p of changed) {
        const existing = old.providers.find(o=>o.id===p.id);
        if (p.credential !== (existing?.credential || false)) throw new Error("凭证必须使用独立的只写接口");
        const value = {display_name:p.name,base_url:p.endpoint,...(!existing?{adapter:"open_ai_compatible"}:{})};
        await this.transport(existing ? `/api/providers/${esc(p.id)}` : "/api/providers", {method:existing?"PATCH":"POST",body:JSON.stringify(value)});
      }
      for (const p of removed) await this.transport(`/api/providers/${esc(p.id)}`, {method:"DELETE"});
    }
    const {theme,language,font,density,collapsed} = settings;
    this.save("preferences", {theme,language,font,density,collapsed});
    await this.refresh();
  }
  async testProvider(id: string, _result: "success" | "failed" | "unknown") {
    if (!this.state.settings.providers.some(p=>p.id===id)) throw new Error("Provider 不存在");
    await this.transport(`/api/providers/${esc(id)}/check`, {method:"POST"}); await this.refresh();
  }
  async saveCredential(id:string,secret:string) {
    if(!this.state.settings.providers.some(p=>p.id===id) || !secret.trim())throw new Error("请先保存 Provider，再输入新凭证");
    await this.transport(`/api/providers/${esc(id)}/credential`,{method:"POST",body:JSON.stringify({source:"workspace_file",secret})});
    await this.refresh();
  }
  async installPlugin(_id: string, _fail: boolean) { unsupported("模型安装必须使用真实权限和许可证流程"); }
}
