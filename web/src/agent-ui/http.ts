import { api, ApiRequestError, request, type JourneyPreview, type JourneyConsent, type ProcessingAuthorization, type ProcessingReceipt } from "../api";
import type { ImageItem, ProviderProfile, RegistryModelProfile, GlobalModelDefaults, ExpertPluginRegistry, InstalledModelInstance, ConversationSchemaPreview, ConversationCallReceipt, ConversationBuilderItem, WorkflowSampleTestRecord, SampleFeedbackRevision, ExportReadiness, ProjectExportResult } from "../types";
import { terminalSampleAnnotations } from "../sampleAnnotations";
import { sampleFeedbackOverlay } from "../sampleFeedbackOverlay";
import { callStage, failureDetail } from "./ExecutionProgress";
import type { HumanRequest } from "../conversation-human-api";
import type { QueuedMessage } from "../components/ConversationQueue";
import type { QueueConsent, QueuePreview } from "../conversation-queue-api";
import type { SendCommand, SendReceipt } from "../conversation-send";
import type { StopRequestRecord } from "../conversation-stop-api";
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
  calls: ConversationCallReceipt[];
  queue: QueuedMessage[];
  human_requests?: HumanRequest[];
  builder_operations?: {items:ConversationBuilderItem[]};
  sample_operations?: {id:string;draft_id:string;status:string;error?:string;created_at?:string}[];
  resume_actions?: {id:string;kind:string;available:boolean;reason:string;url:string;method:string}[];
  processing_operations?: ProcessingReceipt[];
  journey_consents?: {record:{consent:{id:string;sample_operation_id?:string;repair?:{request_id:string}|null}},dispatch?:{status:string;error?:string|null;updated_at?:string}|null,sample?:{status:string}|null,builder?:{evidence?:{outcome?:string}}|null}[];
};
type Thread = { id: string; role: "user"; task_id: string; project_owner_id: string; conversation_id: string; message: { input: { text: string; reference?:{scope:string} } } };
type SafeSettings = { revision: string; sections: { data_privacy: { workspace_id: string }; usage_budget: { future_run_budget: Record<string, unknown> & { max_cost?: string } } } };
export type Transport = <T>(path: string, init?: RequestInit) => Promise<T>;
const esc = encodeURIComponent;
const unsupported = (detail: string): never => { throw new Error(`尚未接通：${detail}。没有执行操作，也没有回退到演示结果。`); };
function journeyReceipt(j: NonNullable<Workspace["journey_consents"]>[number]) {
  const outcome=j.builder?.evidence?.outcome;
  const detail=j.dispatch?.error || (outcome==="budget_exceeded" ? "规划调用额度已用尽，草稿已保留；样例未完成。" : outcome==="failed" ? "方案构建未完成，样例未执行；请查看模型调用回执。" : undefined);
  const active=j.dispatch?.status==="running";
  return {id:j.record.consent.id,title:"规划与样例执行",status:j.dispatch?.error?"failed":active?"running":j.sample?.status || (detail?"failed":"等待后续操作"),detail,finishedAt:active?undefined:j.dispatch?.updated_at};
}
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
  constructor(private transport: Transport = request, private storage?: Storage) {}
  get pluginManagement() { return this.transport === request ? api : undefined; }
  get modelProfileManagement() { return this.transport === request ? api : undefined; }
  get runtimeSettingsManagement() { return this.transport === request ? api : undefined; }
  get projectManagement() { return this.transport === request ? api : undefined; }
  get trashManagement() { return this.transport === request ? api : undefined; }
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
        return { id: image.image_id, project, name: image.name, src: image.url, width: 0, height: 0 };
      });
      const current = this.task(id);
      const pendingApproval=this.stored<{id:string;url:string;body:unknown;execution?:string;view?:Task["approval"]}|null>(`approval.${id}`,null);
      if(pendingApproval) {
        const safeUrl = (value:string) => value.startsWith(`${this.taskRoot(task)}/`) || value===`${this.root(project)}/export` || value===`${this.root(project)}/processing-operations`;
        if(!safeUrl(pendingApproval.url) || (pendingApproval.execution&&!safeUrl(pendingApproval.execution))) throw new Error("已保存操作的地址不属于当前任务");
        this.approvals.set(id,pendingApproval);
      }
      const latestSample = [...(ws?.sample_operations || [])].filter(s=>s.status==="succeeded").sort((a,b)=>(b.created_at || "").localeCompare(a.created_at || ""))[0];
      const human = ws?.human_requests?.find(h=>h.status==="pending"&&!h.deferred&&(!latestSample||h.input.sample_test_id===latestSample.id));
      const sampleOp = latestSample || (human ? ws?.sample_operations?.find(s=>s.id===human.input.sample_test_id) : undefined);
      const sampleId = human?.input.sample_test_id || sampleOp?.id;
      const draftId = sampleOp?.draft_id;
      if(human&&!draftId)throw new Error("人工问题的 Sample 未提供所属 Draft 映射；不会把 checkpoint 当作 Draft ID");
      const result: Partial<Task> = {human:undefined,repairRequests:(ws?.human_requests || []).filter(h=>!h.deferred&&["pending","applied"].includes(h.status)).map(h=>({id:h.input.id,image:h.input.image_id,sample:h.input.sample_test_id,status:h.status as "pending"|"applied"}))};
      const proposal=ws?.builder_operations?.items.find(item=>item.session?.builder_proposal)?.session?.builder_proposal;
      if(proposal) {
        const steps=proposal.draft.label_pipeline ? [...proposal.draft.label_pipeline.shared_stages.flatMap(s=>s.steps),...proposal.draft.label_pipeline.label_pipelines.flatMap(p=>p.steps)] : [];
        result.plan={revision:String(proposal.draft.revision),steps:steps.length ? steps.map(s=>`${s.node_type}${s.model_binding?` · ${s.model_binding.model_id}`:""}`) : proposal.draft.nodes.map(n=>n.id),images:0,models:steps.flatMap(s=>s.model_binding?[s.model_binding.model_id]:[]),destination:"已保存的 Builder proposal（不是新推理）",budget:null};
      }
      if(ws) {
        const jobs=await this.transport<{id:string;result?:ProjectExportResult;error?:string}[]>(`${this.taskRoot(task)}/exports`,{signal:ctrl.signal});
        result.exports=jobs.map(j=>({id:j.id,status:j.error?"failed":j.result?.delivery?"ready":"unknown",url:j.result?.delivery?`${this.root(project)}/exports/${esc(j.result.delivery.id)}/download`:undefined,detail:j.error || (j.result?`${j.result.report.exported_count} 条已导出；${j.result.report.skipped_count} 条跳过`:"未取得完成回执，不显示成功下载")}));
        result.processing=await Promise.all((ws.processing_operations||[]).filter(p=>p.batch_id).map(async p=>{
          const batch=await this.transport<{batch:{project_id:string;status:string}}>(`/api/batches/${esc(p.batch_id!)}`,{signal:ctrl.signal});
          if(batch.batch.project_id!==project)throw new Error("处理批次不属于当前项目");
          return {id:p.id,batch:p.batch_id!,status:batch.batch.status,url:`/projects/${esc(project)}/batches/${esc(p.batch_id!)}`};
        }));
      }
      if(sampleId && draftId) {
        const value=await this.transport<{sample_test:WorkflowSampleTestRecord;annotation_schema?:{task:{kind:string;labels:string[]}}}>(`/api/workflow-drafts/${esc(draftId)}/sample-test?test_id=${esc(sampleId)}`,{signal:ctrl.signal});
        const record=value.sample_test;
        if(!record || record.id!==sampleId || record.draft_id!==draftId || record.project_id!==project) throw new Error("样例不属于当前项目和任务");
        result.beforeRepair=undefined;
        const repairId=ws?.journey_consents?.find(j=>j.record.consent.sample_operation_id===sampleId)?.record.consent.repair?.request_id;
        const priorId=ws?.human_requests?.find(h=>h.input.id===repairId)?.input.sample_test_id;
        const priorOp=ws?.sample_operations?.find(s=>s.id===priorId);
        let prior:WorkflowSampleTestRecord|undefined;
        if(priorOp&&priorId!==sampleId) {
          prior=(await this.transport<{sample_test:WorkflowSampleTestRecord}>(`/api/workflow-drafts/${esc(priorOp.draft_id)}/sample-test?test_id=${esc(priorOp.id)}`,{signal:ctrl.signal})).sample_test;
          if(prior.id!==priorId||prior.project_id!==project||prior.draft_id!==priorOp.draft_id)throw new Error("修复前证据归属不匹配");
          result.beforeRepair={sample:prior.id,boxes:{}};
        }
        const boxesByImage:Record<ImageId,Box[]>={}, imageResults:NonNullable<Task["imageResults"]>={};
        let requestedLabel="", requestedKind="", feedbackVersion="";
        for(const [index,input] of record.inputs.entries()) {
          const image=images.images.find(i=>i.image_id===input.image_id), asset=artifacts.find(a=>a.id===input.image_id);
          if(!image || !asset || image.content_hash!==input.content_hash) throw new Error("样例图片内容哈希已变化；不会替换原始证据");
          const sample=record.report.samples[index]; if(!sample) continue;
          const feedback=await this.transport<{revisions:SampleFeedbackRevision[]}>(`/api/workflow-sample-tests/${esc(sampleId)}/images/${esc(input.image_id)}/feedback`,{signal:ctrl.signal});
          feedbackVersion+=`${input.image_id}:${feedback.revisions.at(-1)?.sequence || 0};`;
          const original=terminalSampleAnnotations(sample,input.image_id,sampleId);
          const annotations=sampleFeedbackOverlay(original,feedback.revisions).annotations;
          const dims=await this.measure(asset.src);asset.width=dims.width;asset.height=dims.height;
          if(prior&&result.beforeRepair) {
            const pi=prior.inputs.findIndex(i=>i.image_id===input.image_id&&i.content_hash===input.content_hash);
            if(pi>=0&&prior.report.samples[pi])result.beforeRepair.boxes[input.image_id]=terminalSampleAnnotations(prior.report.samples[pi],input.image_id,prior.id).flatMap(a=>a.value.kind==="bounding_box"?[{id:a.id,label:a.label||"",x:a.value.rect[0]*dims.width,y:a.value.rect[1]*dims.height,w:a.value.rect[2]*dims.width,h:a.value.rect[3]*dims.height}]:[]);
          }
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
        if(human) result.human={id:human.input.id,image:human.input.image_id,kind:requestedKind || "unsupported",labels:value.annotation_schema?.task.labels || (requestedLabel?[requestedLabel]:[]),label:requestedLabel,candidate:human.input.outcome_id || ""};
      }
      const persistedStop = this.stored<{id:string}|null>(`stop.${id}`, null) || [...thread].reverse().find(t=>t.message.input.reference?.scope==="stop_request");
      const stop = persistedStop && ws ? await this.transport<StopRequestRecord & {normalized_state: Phase|null}>(`${this.conversation(project)}/stop-requests/${esc(persistedStop.id)}`, {signal:ctrl.signal}) : null;
      if (seq !== this.sequence) return;
      const receipts = [
        ...(ws?.calls || []).map(c=>({id:c.id,title:"模型结构化决策",status:c.status==="completed" && (c.failure || c.evidence?.decision?.Err) ? "invalid_result" : c.status,detail:failureDetail(c.failure) || c.evidence?.decision?.Ok?.rationale || c.evidence?.decision?.Err || c.evidence?.error,startedAt:c.started_at || undefined,finishedAt:c.completed_at || undefined,durationMs:c.duration_ms ?? undefined,stage:callStage(c.stage)})),
        ...(ws?.builder_operations?.items || []).map(b=>({id:b.operation.id,title:"方案构建回执",status:b.operation.evidence?.outcome==="failed"?"failed":b.operation.status,detail:b.operation.evidence?.error || (b.operation.evidence?.outcome==="failed"?b.session?.next_action:undefined) || b.operation.evidence?.outcome})),
        ...(ws?.sample_operations || []).map(s=>({id:s.id,title:"样例测试回执",status:s.status,detail:s.error})),
        ...(ws?.journey_consents || []).filter(j=>j.dispatch).sort((a,b)=>(a.dispatch?.updated_at || "").localeCompare(b.dispatch?.updated_at || "")).map(journeyReceipt),
      ];
      const active = ws?.calls.some(c=>c.status==="reserved") || ws?.journey_consents?.some(j=>j.dispatch?.status==="running") || ws?.sample_operations?.some(s=>["running","queued","cancelling"].includes(s.status)) || result.processing?.some(p=>["pending","running","pausing"].includes(p.status));
      const phase: Phase = stop?.normalized_state || (active ? "running" : ws?.calls.some(c=>c.status==="in_doubt") ? "outcome_unknown" : human ? "waiting_for_human" : "idle");
      const edits=this.stored<{revision?:string;boxes?:Record<ImageId,Box[]>}>(`edits.${id}`,{});
      this.emit({ error: undefined, artifacts, tasks: this.state.tasks.map(t => t.id !== id ? t : { ...t,
        items: thread.map(t => ({ id: t.id, role: "user", text: t.message.input.text })),
        ...result, approval:pendingApproval?.view || t.approval, actions: {...ws?.actions || t.actions,answer:{available:!!result.human && ["classification","bounding_box"].includes(result.human.kind),reason:"仅保存当前人工作答的样例修正"}}, model: ws?.agent_model.model_profile_id || this.defaults.pipeline_builder || t.model,
        loaded:true, image: human?.input.image_id || artifacts[0]?.id || "", editBoxes: edits.revision===result.resultRevision ? edits.boxes || {} : {},
        phase, receipts, humanQuestion:human?.input.question,
        schemaProposed: ws?.calls.some(c=>c.status==="completed" && c.evidence?.decision?.Ok?.decision==="draft") || false,
        remoteFailure: (()=>{const failure=[...(ws?.calls||[])].reverse().find(c=>c.status==="in_doubt")?.failure;return failure?{httpStatus:failure.http_status ?? undefined,stage:failure.stage,category:failure.category}:undefined;})(),
        stopTargets:stop?.status==="needs_selection"?stop.targets.map(t=>({id:`${t.kind}:${t.id}`,label:`${t.kind} · ${t.state}`})):[],
        resumeTargets:ws?.resume_actions?.filter(a=>a.available).map(a=>({id:`${a.kind}:${a.id}`,label:a.kind,reason:a.reason})),
        queue: ws?.queue.filter(q => ["waiting_for_dispatch","authorized","running","in_doubt"].includes(q.status)).map(q => q.input.message.text) || [],
        queueEntries: ws?.queue.map(q=>({id:q.input.message.id,text:q.input.message.text,status:q.status,canCancel:["waiting_for_dispatch","authorized","in_doubt"].includes(q.status),canPlan:!human&&q.status==="waiting_for_dispatch"&&!q.planning_call_id})),
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
    if (c.selection) unsupported("候选引用需要完整样例与 Geometry lineage；不能仅凭 bbox ID 发送");
    let p = this.projects.get(task.project)!;
    if (!p.conversation_id) { const value = await this.transport<{conversation_id:string}>(`${this.root(task.project)}/conversations`, {method:"POST"}); p = {...p,conversation_id:value.conversation_id};this.projects.set(task.project,p); }
    const root = this.conversation(task.project);
    const pending = this.stored<SendCommand|null>(`send.${task.id}`,null);
    if (pending && (pending.message.text !== text || pending.mode !== mode)) throw new Error("上一条发送结果尚未确认。请保留原内容重试，不能换新命令掩盖未知结果。");
    const input = pending || {message:{id:c.id,text,image:null},task_id:task.id.startsWith("new:")?null:task.id,schema_revision:(await this.transport<{revision:string}>(`${this.root(task.project)}/goal`)).revision,agent_model:await this.transport<Preference>(`${root}/agent-model`),mode};
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
  async prepareAction(c: Command, kind: "plan" | "sample" | "repair" | "process" | "export") {
    const task = this.checked(c); if(task.id.startsWith("new:")) throw new Error("请先保存目标");
    if (kind === "plan" && task.schemaProposed) throw new Error("此任务已有已保存的目标草稿，请继续构建方案并测试样例，不要重新申请初始规划授权。");
    if (["plan","sample","repair"].includes(kind) && task.phase === "outcome_unknown") throw new Error("此任务已有结果未知的请求，不能重建初始授权。请保留原回执；如需重新尝试，明确创建独立请求并重新批准费用范围。");
    if(this.stored(`approval.${task.id}`,null)) throw new Error("上次批准的结果待核对；请读取原回执，不能自动发起新的付费操作");
    const root = this.taskRoot(task);
    if(kind === "process") {
      if(!task.sample) throw new Error("需要当前任务已保存的样例测试");
      const selection={draft_id:task.sample.draft,sample_test_id:task.sample.id};
      const p=await this.transport<ProcessingAuthorization>(`${this.root(task.project)}/processing-preview?${new URLSearchParams(selection)}`);
      if(p.revision!==task.sample.revision)throw new Error("样例对应的草稿版本已变化，需要重新测试后确认");
      const body={request_id:c.id,selection,expected_revision:p.revision,authorization_fingerprint:p.authorization_fingerprint};
      this.approvals.set(task.id,{id:c.id,url:`${this.root(task.project)}/processing-operations`,body});
      this.emit({tasks:this.state.tasks.map(t=>t.id===task.id?{...t,approval:{id:c.id,title:"确认方案并开始处理",revision:String(p.revision),budget:null,scope:[p.plan_name,`${p.image_count} 张图片；最多 ${p.maximum_model_calls} 次模型调用`,...p.models.map(m=>`${m.remote_model_id} → ${m.provider_base_url}`),...(p.native_models||[]).map(m=>`${m.name} → ${m.destination}`),"发布不可变版本并启动一次 Dataset Run；需审核结果不自动接受"]}}:t)});
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
      const [bindings,registry,local] = await Promise.all([
        this.transport<{bindings:{model_profile_id:string}[]}>(`${this.root(task.project)}/model-bindings`),
        this.transport<{models:RegistryModelProfile[]}>("/api/model-profiles"),
        this.transport<{model_profiles:{selection_id:string;selectable:boolean;availability:string;capabilities:string[]}[]}>("/api/model-instances"),
      ]);
      const models = [...new Set([
        ...bindings.bindings.filter(b=>registry.models.some(m=>m.id===b.model_profile_id && m.enabled && m.status==="available" && m.input_modalities.includes("image"))).map(b=>`model-profile:${b.model_profile_id}`),
        ...(local.model_profiles || []).filter(m=>m.selectable && m.availability==="available" && m.capabilities.includes("prompted_segmentation")).map(m=>m.selection_id),
      ])];
      if(!models.length) throw new Error("项目尚未绑定视觉模型，请先设置；不自动扩大到全部 Registry 模型");
      const query = new URLSearchParams({consent_id:c.id,builder_operation_id:crypto.randomUUID(),sample_operation_id:crypto.randomUUID(),allowed_models:JSON.stringify(models)});
      if(kind==="repair") {
        if(c.selection?.revision!==(task.resultRevision || task.revision))throw new Error("样例版本已变化，请重新查看结果后再修复");
        const repair=task.repairRequests?.find(r=>r.image===c.selection?.image&&r.sample===task.sample?.id&&r.status==="applied");
        if(!repair)throw new Error("先保存当前样例的问题反馈；不能猜测修复来源");
        query.set("repair_request_id",repair.id);
      }
      const source=[...(this.workspaces.get(task.id)?.calls || [])].reverse().find(call=>call.status==="completed" && call.evidence?.decision?.Ok?.decision==="draft");
      if(source) {
        const schema=await this.transport<{id:string;task_id:string;revision:number}|null>(`${root}/calls/${esc(source.id)}/schema-draft`);
        if(!schema)throw new Error("模型已有规划回执，但目标草稿尚未保存；没有重新调用模型，请核对草稿保存结果。");
        if(schema.task_id!==task.id)throw new Error("目标草稿不属于当前任务");
        query.set("schema_id",schema.id);query.set("schema_revision",String(schema.revision));
        if(task.model)query.set("planner_model_id",task.model);
      } else if(task.plan) {
        const schemas=await this.transport<{id:string;task_id:string;revision:number}[]>(`${root}/human-schema-drafts`);
        if(schemas.length!==1 || schemas[0].task_id!==task.id)throw new Error("需要明确选择本任务的目标草稿，不能自动重建初始授权。");
        query.set("schema_id",schemas[0].id);query.set("schema_revision",String(schemas[0].revision));
        if(task.model)query.set("planner_model_id",task.model);
      } else query.set("schema_call_id",crypto.randomUUID());
      const p = await this.transport<JourneyPreview>(`${root}/journey-preview?${query}`);
      const consent: JourneyConsent = {...p.consent,allow_unknown_cost:true,...(p.consent.schema_proposal?{schema_proposal:{...p.consent.schema_proposal,allow_unknown_cost:true}}:{})};
      this.approvals.set(task.id,{id:c.id,url:`${root}/journey-consents`,body:consent,execution:`${root}/journey-consents/${esc(consent.id)}/execution`});
      this.emit({tasks:this.state.tasks.map(t=>t.id===task.id?{...t,approval:{id:c.id,title:kind==="repair"?"批准根据反馈修复并重测":"批准构建方案并测试样例",revision:consent.builder_scope_hash,budget:null,scope:[`${consent.images.length} 张图片；最多 ${consent.maximum_builder_calls} 次规划调用 + ${consent.maximum_sample_calls} 次样例调用`,p.builder.model_name,p.builder.destination,...p.data.models.map(m=>`${m.display_name} → ${m.destination}`),`有效期：${consent.expires_at}`,"仅保存草稿与样例测试，不发布、不批量处理、不写正式标注"]}}:t)});
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
    const chosen=record.targets.find(t=>`${t.kind}:${t.id}`===target);
    if(!chosen)throw new Error("此目标不在已冻结的停止请求中");
    await this.transport(`${root}/select`,{method:"POST",body:JSON.stringify({target:{kind:chosen.kind,id:chosen.id,task_id:chosen.task_id}})});
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
  async reportSampleIssue(c:Command,image:string,reason:"poor_boundary"|"wrong_target") {
    const task=this.checked(c);
    if(c.selection?.image!==image||c.selection.revision!==(task.resultRevision || task.revision))throw new Error("样例选择或版本已变化，请重新查看结果后再反馈");
    const request=this.workspaces.get(task.id)?.human_requests?.find(h=>h.status==="pending"&&!h.deferred&&h.input.image_id===image&&h.input.sample_test_id===task.sample?.id);
    if(!request)throw new Error("这张图没有当前样例的待回答问题；没有创建或修改其他结果");
    const key=`answer.${request.input.id}`;
    const previous=this.stored<SampleFeedbackRevision|null>(key,null);
    if(previous&&(previous.reason!==reason||previous.corrected_value))throw new Error("上次答案待核实，不能改变重试内容");
    const answer:SampleFeedbackRevision=previous || {revision_id:c.id,sample_test_id:request.input.sample_test_id,image_id:image,sequence:request.input.expected_feedback_sequence+1,reason,outcome_id:request.input.outcome_id,corrected_value:null,corrected_label:null,note:reason==="wrong_target"?"用户反馈：目标找错。请依据原始图片与终端证据重新定位，不能把粗框直接交给分割。":"用户反馈：边界不准确。检查局部目标覆盖并重新定位后再精修，不放宽几何安全阈值。",created_at:new Date().toISOString()};
    this.save(key,answer);
    const saved=await this.transport<HumanRequest>(`${this.taskRoot(task)}/human-requests/${esc(request.input.id)}/answer`,{method:"POST",body:JSON.stringify({answer})});
    if(saved.answer?.revision_id!==answer.revision_id)throw new Error("服务器未确认同一反馈版本");
    this.save(key,null);await this.reloadCurrent(task);
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
