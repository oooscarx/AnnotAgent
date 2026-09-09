import {useEffect,useRef,useState} from "react";
import {ApiRequestError,type api} from "../api";
import type {ConversationSchemaDraft,ConversationCallReceipt} from "../types";
import {SchemaProposal,ownedProposalDraft} from "./SchemaProposal";
import {SchemaClarifications} from "./SchemaClarifications";
import {Disclosure} from "./Disclosure";
import {CreateTaskSchema} from "./CreateTaskSchema";
import {SchemaBuilder,type SchemaBuilderService} from "./SchemaBuilder";
export type TaskSchemaService=SchemaBuilderService&Pick<typeof api,"conversationSchemaClarification"|"cancelSchemaClarification"|"humanConversationSchemas"|"saveConversationSchemaDraft"|"saveHumanConversationSchema"|"conversationSchemaCalls"|"conversationSchemaDraftForCall"|"conversationSchemaDraft"|"editConversationSchemaDraft">;
export async function readTaskSchemaState(service:TaskSchemaService,project:string,conversation:string,task:string,signal:AbortSignal){
  const [human,calls]=await Promise.all([service.humanConversationSchemas(project,conversation,task,signal),service.conversationSchemaCalls(project,conversation,task,signal)]);
  if(calls.some(c=>c.task_id!==task))throw new Error("Schema 调用不属于当前任务");
  const proposals=calls.filter(c=>c.status==="completed"&&c.evidence?.decision?.Ok?.decision==="draft");
  const generated=await Promise.all(proposals.map(async c=>{const d=await service.conversationSchemaDraftForCall(project,conversation,task,c.id,signal);return d?ownedProposalDraft(d,task,c.id):null;}));
  const drafts=[...human,...generated.filter((d):d is ConversationSchemaDraft=>!!d)];if(drafts.some(d=>d.task_id!==task))throw new Error("Schema 草稿不属于当前任务");
  return {drafts:[...new Map(drafts.map(d=>[d.id,d])).values()],proposals:proposals.filter((_,i)=>!generated[i])};
}
export async function readTaskSchemas(service:TaskSchemaService,project:string,conversation:string,task:string,signal:AbortSignal){return (await readTaskSchemaState(service,project,conversation,task,signal)).drafts;}
export function TaskSchemaDrafts({service,project,conversation,task,workspace}:{service:TaskSchemaService;project:string;conversation:string;task:string;workspace:string}){
  const [opened,setOpened]=useState(false);
  return <Disclosure title="标签与边界规则草稿" onToggle={e=>{if(e.currentTarget.open)setOpened(true);}}>{opened&&<SchemaList key={`${workspace}:${project}:${conversation}:${task}`} {...{service,project,conversation,task,workspace}}/>}</Disclosure>;
}
function SchemaList({service,project,conversation,task,workspace}:{service:TaskSchemaService;project:string;conversation:string;task:string;workspace:string}){
  const [rows,setRows]=useState<ConversationSchemaDraft[]>();const [proposals,setProposals]=useState<ConversationCallReceipt[]>([]);const [error,setError]=useState("");
  useEffect(()=>{const c=new AbortController();void readTaskSchemaState(service,project,conversation,task,c.signal).then(v=>{if(!c.signal.aborted){setRows(v.drafts);setProposals(v.proposals);}}).catch(e=>{if(!c.signal.aborted)setError((e as Error).message);});return()=>c.abort();},[service,project,conversation,task]);
return <section aria-label="任务语义草稿"><p>仅编辑此任务的标签语义草稿；不修改 Project Schema、Published Version 或正式标注，不调用模型。已有方案不会自动采用新 revision。</p>{error&&<p role="alert">{error}</p>}{!rows&&!error&&<p role="status">读取已保存草稿…</p>}{rows?.length===0&&<p>当前任务没有已保存的语义草稿。</p>}{rows&&<SchemaClarifications {...{service,project,conversation,task,workspace}} onSaved={saved=>setRows(previous=>[saved,...(previous||[]).filter(d=>d.id!==saved.id)])}/>} {rows&&<CreateTaskSchema {...{service,project,conversation,task,workspace}} onSaved={saved=>setRows(previous=>[saved,...(previous||[]).filter(d=>d.id!==saved.id)])}/>} {proposals.map(call=><SchemaProposal key={call.id} {...{service,project,conversation,task,call}} onSaved={saved=>{setRows(previous=>[saved,...(previous||[]).filter(d=>d.id!==saved.id)]);setProposals(previous=>previous.filter(c=>c.id!==call.id));}}/>)} {rows?.map(d=><SchemaEdit key={d.id} initial={d} project={project} conversation={conversation} service={service} storageKey={`annotagent.schema-edit.${workspace}.${project}.${task}.${d.id}`}/>)}</section>;
}
type Edit=Parameters<TaskSchemaService["editConversationSchemaDraft"]>[2];
function SchemaEdit({initial,project,conversation,service,storageKey}:{initial:ConversationSchemaDraft;project:string;conversation:string;service:TaskSchemaService;storageKey:string}){
  const [builderOpen,setBuilderOpen]=useState(false);
  const [builderActive,setBuilderActive]=useState(false);
  const [invalidRecovery,setInvalidRecovery]=useState(false);
  const [draft,setDraft]=useState(initial);const [labels,setLabels]=useState(initial.definition.task.labels.join("\n"));const [rules,setRules]=useState(initial.definition.boundary_rules.join("\n"));const [pending,setPending]=useState<Edit>();const [error,setError]=useState("");const [busy,setBusy]=useState(false);const lock=useRef(false);const alive=useRef(true);
  const dirty=labels!==draft.definition.task.labels.join("\n")||rules!==draft.definition.boundary_rules.join("\n");
  useEffect(()=>{alive.current=true;try{const raw=localStorage.getItem(storageKey);if(raw){const v=JSON.parse(raw) as Edit;const decision=v.decision as {labels?:unknown;boundary_rules?:unknown};if(typeof v.request_id!=="string"||!Number.isSafeInteger(v.expected_revision)||!Array.isArray(decision?.labels)||!Array.isArray(decision?.boundary_rules))throw new Error("保存请求记录无效；未自动执行");setPending(v);}}catch(e){setInvalidRecovery(true);setError((e as Error).message);}return()=>{alive.current=false;};},[storageKey]);
  useEffect(()=>{const guard=(e:Event)=>{if((dirty||pending||busy)&&!window.confirm("有未保存编辑或待核实保存请求，仍要离开？"))e.preventDefault();};const unload=(e:BeforeUnloadEvent)=>{if(dirty||pending||busy){e.preventDefault();e.returnValue="";}};window.addEventListener("ui-preview:before-navigate",guard);window.addEventListener("beforeunload",unload);return()=>{window.removeEventListener("ui-preview:before-navigate",guard);window.removeEventListener("beforeunload",unload);};},[dirty,pending,busy]);
  const save=async()=>{
    if(lock.current||invalidRecovery||builderActive)return;lock.current=true;setBusy(true);setError("");
    try{const input=pending||{request_id:crypto.randomUUID(),expected_revision:draft.revision,decision:{decision:"draft",kind:draft.definition.task.kind,labels:labels.split("\n").map(s=>s.trim()).filter(Boolean),multi_label:draft.definition.task.multi_label,attributes:draft.definition.task.attributes,boundary_rules:rules.split("\n").map(s=>s.trim()).filter(Boolean),rationale:"User-edited annotation semantics"}};
      localStorage.setItem(storageKey,JSON.stringify(input));setPending(input);const saved=await service.editConversationSchemaDraft(project,draft.id,input);
      if(saved.id!==draft.id||saved.task_id!==draft.task_id)throw new Error("保存回执归属不匹配");localStorage.removeItem(storageKey);if(alive.current){setDraft(saved);setLabels(saved.definition.task.labels.join("\n"));setRules(saved.definition.boundary_rules.join("\n"));setPending(undefined);}
    }catch(e){if(alive.current){setError((e as Error).message);if(e instanceof ApiRequestError&&[400,401,403,404,409,422].includes(e.status)){localStorage.removeItem(storageKey);setPending(undefined);}}}finally{lock.current=false;if(alive.current)setBusy(false);}
  };
  return <Disclosure title={`${draft.definition.task.labels.join("、")} · revision ${draft.revision}`}><p>{draft.definition.goal}</p><p role="status">{busy?"保存中…":pending?"保存结果待核实；只重试原请求":dirty?"有未保存修改":"已读取服务器草稿"}</p>
<label>标签（每行一个）<textarea value={labels} disabled={builderActive||busy||!!pending} onChange={e=>setLabels(e.target.value)}/></label><label>边界规则（每行一条）<textarea value={rules} disabled={builderActive||busy||!!pending} onChange={e=>setRules(e.target.value)}/></label>
    {error&&<p role="alert">{error}。输入保留；未自动重新执行。</p>}<div className="actions"><button disabled={builderActive||busy||!!pending||!dirty} onClick={()=>{setLabels(draft.definition.task.labels.join("\n"));setRules(draft.definition.boundary_rules.join("\n"));}}>取消修改</button><button disabled={builderActive||invalidRecovery||busy||(!pending&&(!dirty||!labels.trim()))} onClick={()=>void save()}>{pending?"核实原保存请求":"保存语义草稿"}</button><button disabled={builderActive||busy||!!pending||invalidRecovery} onClick={async()=>{if(dirty&&!window.confirm("丢弃当前编辑并读取最新草稿？"))return;setBusy(true);try{const v=await service.conversationSchemaDraft(project,draft.id);if(v.id!==draft.id||v.task_id!==draft.task_id)throw new Error("草稿归属不匹配");if(alive.current){setDraft(v);setLabels(v.definition.task.labels.join("\n"));setRules(v.definition.boundary_rules.join("\n"));setError("");}}catch(e){if(alive.current)setError((e as Error).message);}finally{if(alive.current)setBusy(false);}}}>读取最新草稿</button></div>
    {!builderOpen&&<button disabled={busy||!!pending||dirty||invalidRecovery} onClick={()=>setBuilderOpen(true)}>从此草稿构建方案</button>}
    {builderOpen&&<SchemaBuilder key={`${draft.id}:${draft.revision}`} project={project} conversation={conversation} task={draft.task_id} schema={draft} storageKey={storageKey} disabled={busy||!!pending||dirty||invalidRecovery} service={service} onActive={setBuilderActive}/>}
  </Disclosure>;
}
