import {useState} from "react";
import type {PipelineStep,PipelineSource,WorkflowCatalog,WorkflowDraft} from "../types";
import {Dialog} from "./Dialog";
export type StepLocation={kind:"shared"|"label";group:string;step:string};
export function findPipelineStep(draft:WorkflowDraft,location:StepLocation){const groups=location.kind==="shared"?draft.label_pipeline?.shared_stages:draft.label_pipeline?.label_pipelines;return groups?.find(g=>g.id===location.group)?.steps.find(s=>s.id===location.step);}
export function replacePipelineStep(draft:WorkflowDraft,location:StepLocation,base:PipelineStep,next:PipelineStep){
  const copy=structuredClone(draft);const found=findPipelineStep(copy,location);
  if(!found||JSON.stringify(found)!==JSON.stringify(base)||next.id!==base.id||next.node_type!==base.node_type)throw new Error("步骤已变化或身份不匹配，请重新打开编辑器");
  Object.assign(found,next);return copy;
}
export function stepSources(draft:WorkflowDraft,location:StepLocation,type:string):{label:string;source:PipelineSource}[]{
  const options:{label:string;source:PipelineSource}[]=type==="image"?[{label:"项目原图",source:{source:"image"}}]:[];
  const add=(steps:PipelineStep[],stage?:string)=>{for(const step of steps){if(step.id===location.step)continue;for(const [port,artifact] of Object.entries(step.outputs)){if(artifact===type)options.push({label:`${stage?`共享 ${stage} / `:""}${step.id} · ${port}`,source:stage?{source:"shared_stage",stage_id:stage,step_id:step.id,port,artifact_type:artifact}:{source:"step",step_id:step.id,port,artifact_type:artifact}});}}};
  for(const stage of draft.label_pipeline?.shared_stages??[])add(stage.steps,stage.id);
  if(location.kind==="label")add(draft.label_pipeline?.label_pipelines.find(g=>g.id===location.group)?.steps??[]);
  return options;
}
export function PipelineStepEditor({draft,location,catalog,onApply,onClose}:{draft:WorkflowDraft;location:StepLocation;catalog:WorkflowCatalog;onApply:(draft:WorkflowDraft)=>void;onClose:()=>void}) {
  const [base]=useState(()=>structuredClone(findPipelineStep(draft,location)!));const [next,setNext]=useState(()=>structuredClone(base));const [params,setParams]=useState(()=>JSON.stringify(base.parameters,null,2));const [error,setError]=useState("");
  const descriptor=catalog.node_catalog.find(n=>n.id===base.node_type);
  const capability=descriptor?.required_model_capability;
  const normalize=(s:string)=>s==="image_classification"?"classification":s;
  const models=[...catalog.model_registry,...catalog.expert_models.filter(m=>m.availability==="available"&&!catalog.model_registry.some(r=>r.id===m.model_id)).map(m=>({id:m.model_id,display_name:m.display_name,capabilities:m.capabilities}))].filter(m=>!capability||m.capabilities.some(c=>normalize(c)===normalize(capability)));
  const changed=JSON.stringify(base)!==JSON.stringify(next)||params!==JSON.stringify(base.parameters,null,2);
  const close=()=>{if(!changed||window.confirm("放弃此步骤尚未应用的修改？"))onClose();};
  return <Dialog title="编辑 Pipeline 步骤" onClose={close}><p>{base.node_type} · {base.id}</p>{error&&<p role="alert">{error}</p>}
    {!descriptor&&<p>此节点已不在当前目录。保留原配置；新增输入/模型能力需先核实 Registry。</p>}
    {(capability||base.model_binding)&&<label>步骤模型<select value={next.model_binding?.model_id||""} onChange={e=>{const id=e.target.value;setNext({...next,model_binding:id?{model_id:id,capability:base.model_binding?.capability||normalize(capability!),configuration:next.model_binding?.configuration||{}}:undefined});}}><option value="">不绑定模型</option>{next.model_binding&&!models.some(m=>m.id===next.model_binding!.model_id)&&<option value={next.model_binding.model_id}>当前绑定不可在目录验证 · {next.model_binding.model_id}</option>}{models.map(m=><option key={m.id} value={m.id}>{m.display_name}</option>)}</select></label>}
    <p>选择只修改此步骤，不探测或调用模型。保存后仍需服务端兼容性校验。</p>
    {descriptor?.input_ports.map(port=>{const options=stepSources(draft,location,port.artifact_type);const value=next.inputs[port.name]?JSON.stringify(next.inputs[port.name]):"";return <label key={port.name}>{port.name} · {port.artifact_type}{port.required?"（必需）":""}<select value={value} onChange={e=>{const inputs={...next.inputs};if(!e.target.value)delete inputs[port.name];else inputs[port.name]=JSON.parse(e.target.value) as PipelineSource;setNext({...next,inputs});}}><option value="">尚未连接</option>{value&&!options.some(o=>JSON.stringify(o.source)===value)&&<option value={value}>保留现有来源（分支/组合或需核实）</option>}{options.map(o=><option key={JSON.stringify(o.source)} value={JSON.stringify(o.source)}>{o.label}</option>)}</select></label>;})}
    <label className="pipeline-step-parameters">步骤参数 JSON<textarea rows={10} spellCheck={false} value={params} onChange={e=>setParams(e.target.value)}/></label>
    <label className="pipeline-review-toggle"><input type="checkbox" checked={next.review_gate.required} onChange={e=>setNext({...next,review_gate:{...next.review_gate,required:e.target.checked}})}/>要求人工审核</label>
    <p>此处来源候选按 Artifact 类型筛选，不保证无环或完整；未连接必需输入仍须校验，不能因此发布。</p>
    <div className="actions"><button type="button" onClick={close}>取消</button><button type="button" disabled={!changed} onClick={()=>{try{const parameters:unknown=JSON.parse(params);if(!parameters||Array.isArray(parameters)||typeof parameters!=="object")throw new Error("参数必须为 JSON 对象");onApply(replacePipelineStep(draft,location,base,{...next,parameters:parameters as Record<string,unknown>}));onClose();}catch(e){setError((e as Error).message);}}}>应用到未保存草稿</button></div>
  </Dialog>;
}
