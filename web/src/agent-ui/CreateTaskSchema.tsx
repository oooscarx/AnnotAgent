import {useEffect,useRef,useState} from "react";
import {ApiRequestError,type api} from "../api";
import type {ConversationSchemaDraft} from "../types";
import {Disclosure} from "./Disclosure";
type Decision={decision:"draft";kind:"classification"|"bounding_box";labels:string[];multi_label:false;attributes:Record<string,never>;boundary_rules:string[];rationale:string};
export type ClarificationReference={call_id:string;expected_schema_revision:string};
type Input={request_id:string;decision:Decision;clarification?:ClarificationReference};
export function readClarificationCommand(raw:string,reference:ClarificationReference):Input{
  const value=JSON.parse(raw) as Input;
  if(!value||Object.keys(value).sort().join(",")!=="clarification,decision,request_id"||!value.clarification||Object.keys(value.clarification).sort().join(",")!=="call_id,expected_schema_revision"||value.clarification.call_id!==reference.call_id||value.clarification.expected_schema_revision!==reference.expected_schema_revision)throw new Error("澄清回答恢复引用不匹配；未提交");
  readManualSchemaCommand(JSON.stringify({request_id:value.request_id,decision:value.decision}));return value;
}
export function readManualSchemaCommand(raw:string):Input{
  const value=JSON.parse(raw) as Input;const d=value?.decision;
  if(!value||Object.keys(value).sort().join(",")!=="decision,request_id"||typeof value.request_id!=="string"||!/^[0-9a-f-]{36}$/i.test(value.request_id)||!d||Object.keys(d).sort().join(",")!=="attributes,boundary_rules,decision,kind,labels,multi_label,rationale"||d.decision!=="draft"||!["classification","bounding_box"].includes(d.kind)||d.multi_label!==false||!d.attributes||Object.keys(d.attributes).length||!Array.isArray(d.labels)||!d.labels.length||!d.labels.every(s=>typeof s==="string"&&s.trim())||!Array.isArray(d.boundary_rules)||!d.boundary_rules.every(s=>typeof s==="string")||typeof d.rationale!=="string")throw new Error("手工草稿恢复记录无效；不会覆盖或执行它");
  return value;
}
export function CreateTaskSchema({service,project,conversation,task,workspace,onSaved,clarification,onActive}:{service:Pick<typeof api,"saveHumanConversationSchema">;project:string;conversation:string;task:string;workspace:string;onSaved:(draft:ConversationSchemaDraft)=>void;clarification?:ClarificationReference;onActive?:(active:boolean)=>void}){
  const key=`annotagent.manual-schema.${workspace}.${project}.${conversation}.${task}${clarification?`.clarification.${clarification.call_id}.${clarification.expected_schema_revision}`:""}`;
  const parse=(raw:string)=>clarification?readClarificationCommand(raw,clarification):readManualSchemaCommand(raw);
  const [kind,setKind]=useState<Decision["kind"]>("bounding_box");const [labels,setLabels]=useState("");const [rules,setRules]=useState("");const [pending,setPending]=useState<Input>();const [invalid,setInvalid]=useState(false);const [busy,setBusy]=useState(false);const [error,setError]=useState("");const [message,setMessage]=useState("");const alive=useRef(true);const lock=useRef(false);
  useEffect(()=>{alive.current=true;try{const raw=localStorage.getItem(key);if(raw){const value=parse(raw);setPending(value);setKind(value.decision.kind);setLabels(value.decision.labels.join("\n"));setRules(value.decision.boundary_rules.join("\n"));}}catch(e){setInvalid(true);setError((e as Error).message);}return()=>{alive.current=false;};},[key]);
  const dirty=!!labels||!!rules||kind!=="bounding_box";
  useEffect(()=>{onActive?.(dirty||!!pending||busy||invalid);},[dirty,pending,busy,invalid,onActive]);
  useEffect(()=>{const guard=(e:Event)=>{if((dirty||pending||busy)&&!window.confirm("手工语义草稿未保存或结果待核实，仍要离开？"))e.preventDefault();};const unload=(e:BeforeUnloadEvent)=>{if(dirty||pending||busy){e.preventDefault();e.returnValue="";}};window.addEventListener("ui-preview:before-navigate",guard);window.addEventListener("beforeunload",unload);return()=>{window.removeEventListener("ui-preview:before-navigate",guard);window.removeEventListener("beforeunload",unload);};},[dirty,pending,busy]);
  const save=async()=>{
    if(lock.current||invalid||(!pending&&!labels.trim()))return;lock.current=true;setBusy(true);setError("");setMessage("");
    try{const input=pending||{request_id:crypto.randomUUID(),decision:{decision:"draft" as const,kind,labels:labels.split("\n").map(s=>s.trim()).filter(Boolean),multi_label:false as const,attributes:{},boundary_rules:rules.split("\n").map(s=>s.trim()).filter(Boolean),rationale:"Human-authored annotation semantics"},...(clarification?{clarification}:{})};
      parse(JSON.stringify(input));localStorage.setItem(key,JSON.stringify(input));setPending(input);const saved=await service.saveHumanConversationSchema(project,conversation,task,input);
      if(saved.task_id!==task||saved.source_request_id!==input.request_id)throw new Error("手工草稿保存回执不属于当前请求");
      localStorage.removeItem(key);if(alive.current){setPending(undefined);setLabels("");setRules("");setKind("bounding_box");setMessage("手工草稿已保存；没有调用模型或开始处理。");onSaved(saved);}
    }catch(e){if(alive.current){setError((e as Error).message);if(e instanceof ApiRequestError&&[400,401,403,404,409,422].includes(e.status)){localStorage.removeItem(key);setPending(undefined);}}}finally{lock.current=false;if(alive.current)setBusy(false);}
  };
  return <Disclosure title={clarification?"回答这个澄清问题":"无需模型，手工定义标签"}><section aria-label={clarification?"澄清回答":"手工定义语义草稿"}><p>{clarification?"保存本问题的标签和边界规则；不发送 Journey 续跑授权，不自动调用模型。":"只创建当前任务的语义草稿，不回答已有澄清问题、不自动续跑。后续方案和样本仍需单独授权。"}</p>
<label>输出类型<select aria-label="输出类型" value={kind} disabled={busy||!!pending||invalid} onChange={e=>setKind(e.target.value as Decision["kind"])}><option value="bounding_box">框出目标</option><option value="classification">给整图分类</option></select></label>
    <p>{kind==="bounding_box"?"例如：框出每个杯子，标签为杯子。":"例如：将整张图片分为室内或室外。"}</p>
<label>新草稿标签（每行一个）<textarea aria-label="新草稿标签（每行一个）" value={labels} disabled={busy||!!pending||invalid} onChange={e=>setLabels(e.target.value)}/></label><label>新草稿边界规则（可选）<textarea aria-label="新草稿边界规则（可选）" value={rules} disabled={busy||!!pending||invalid} onChange={e=>setRules(e.target.value)}/></label>
    <p role="status">{busy?"保存中…":pending?"结果待核实；刷新不会重复创建":message||"尚未保存为服务器草稿"}</p>{error&&<p role="alert">{error}</p>}
    <div className="actions"><button disabled={busy||!!pending||invalid||!dirty} onClick={()=>{setLabels("");setRules("");setKind("bounding_box");setError("");}}>取消手工输入</button><button disabled={busy||invalid||(!pending&&!labels.trim())} onClick={()=>void save()}>{pending?"核实原创建请求":clarification?"保存澄清回答":"保存手工草稿"}</button></div>
  </section></Disclosure>;
}
