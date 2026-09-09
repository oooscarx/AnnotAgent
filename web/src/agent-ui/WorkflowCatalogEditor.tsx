import {useEffect,useState} from "react";
import type {api} from "../api";
import type {WorkflowCatalog,WorkflowDraft,PipelineStep,WorkflowNodePort} from "../types";
import {pipelineNodeKind} from "../pipelinePresentation";
import {Disclosure} from "./Disclosure";
export type WorkflowCatalogService=Pick<typeof api,"workflowCatalog">;
export function appendCatalogNode(draft:WorkflowDraft,catalog:WorkflowCatalog,descriptorId:string,target:string,id:string):WorkflowDraft {
  if(catalog.project_id!==draft.project_id)throw new Error("节点目录归属不同");
  const descriptor=catalog.node_catalog.find(d=>d.id===descriptorId);if(!descriptor)throw new Error("节点不在 Registry 目录中");
  const defaults=Object.fromEntries(Object.entries((descriptor.config_schema.properties??{}) as Record<string,{default?:unknown}>).filter(([,v])=>v&&v.default!==undefined).map(([k,v])=>[k,v.default]));
  const next=structuredClone(draft);
  if(next.label_pipeline){
    const groups=[...next.label_pipeline.shared_stages.map(g=>({key:`shared:${g.id}`,steps:g.steps})),...next.label_pipeline.label_pipelines.map(g=>({key:`label:${g.id}`,steps:g.steps}))];
    const group=groups.find(g=>g.key===target);if(!group)throw new Error("请选择真实的共享阶段或 Label Pipeline");
    if(groups.some(g=>g.steps.some(s=>s.id===id)))throw new Error("步骤 ID 已存在");
    const step:PipelineStep={id,node_type:descriptor.id,kind:pipelineNodeKind(descriptor.id),inputs:{},outputs:Object.fromEntries(descriptor.output_ports.map(p=>[p.name,p.artifact_type])) as PipelineStep["outputs"],parameters:defaults,validators:[],refiners:[],retry_policy:{max_attempts:1},review_gate:{required:false,allow_manual_override:false},resources:{}};
    group.steps.push(step);
  }else{
    if(next.nodes.some(n=>n.id===id))throw new Error("节点 ID 已存在");
    const ports=(items:typeof descriptor.input_ports)=>items.map(p=>({id:p.name,artifact_type:p.artifact_type,required:p.required,multiple:p.cardinality==="many"})) as WorkflowNodePort[];
    next.nodes.push({id,node_type:descriptor.id,kind:pipelineNodeKind(descriptor.id),depends_on:[],inputs:ports(descriptor.input_ports),outputs:ports(descriptor.output_ports),validators:[],refiners:[],max_retries:0,review_gate:false,parameters:defaults});
  }
  return next;
}
export function WorkflowCatalogEditor({service,draft,disabled,onChange}:{service:WorkflowCatalogService;draft:WorkflowDraft;disabled:boolean;onChange:(draft:WorkflowDraft)=>void}) {
  const [catalog,setCatalog]=useState<WorkflowCatalog>();const [error,setError]=useState("");const [node,setNode]=useState("");const [target,setTarget]=useState("");
  useEffect(()=>{let current=true;void service.workflowCatalog(draft.project_id).then(value=>{if(!current)return;if(value.project_id!==draft.project_id)throw new Error("目录不属于当前项目");setCatalog(value);}).catch(e=>{if(current)setError(e.message);});return()=>{current=false;};},[service,draft.project_id]);
  const descriptor=catalog?.node_catalog.find(d=>d.id===node);
  return <section aria-label="真实节点目录"><h2>步骤与节点目录</h2>{error&&<p role="alert">{error}</p>}
    {draft.label_pipeline&&<><h3>共享阶段</h3>{draft.label_pipeline.shared_stages.map(group=><Disclosure key={group.id} title={group.name}>{group.steps.map(step=><p key={step.id}>{step.node_type} · {step.id} · 模型 {step.model_binding?.model_id||"未绑定模型"}</p>)}</Disclosure>)}<h3>Label Pipelines</h3>{draft.label_pipeline.label_pipelines.map(group=><Disclosure key={group.id} title={`${group.target_task_id} · ${group.target_label}`}>{group.steps.map(step=><p key={step.id}>{step.node_type} · {step.id} · 模型 {step.model_binding?.model_id||"未绑定模型"}</p>)}</Disclosure>)}</>}
    {!catalog?<p role="status">读取 Registry 节点目录…</p>:<><label>添加 Registry 节点<select disabled={disabled} value={node} onChange={e=>setNode(e.target.value)}><option value="">选择节点</option>{catalog.node_catalog.map(item=><option key={item.id} value={item.id}>{item.display_name} · {item.id}</option>)}</select></label>
      {draft.label_pipeline&&<label>添加到哪个阶段<select disabled={disabled} value={target} onChange={e=>setTarget(e.target.value)}><option value="">选择阶段</option>{draft.label_pipeline.shared_stages.map(g=><option value={`shared:${g.id}`} key={`s:${g.id}`}>共享 · {g.name}</option>)}{draft.label_pipeline.label_pipelines.map(g=><option value={`label:${g.id}`} key={`l:${g.id}`}>{g.target_task_id} · {g.target_label}</option>)}</select></label>}
      {descriptor&&<p>输入：{descriptor.input_ports.map(p=>`${p.name}: ${p.artifact_type}${p.required?"（必需）":""}`).join(" · ")||"无"}；输出：{descriptor.output_ports.map(p=>`${p.name}: ${p.artifact_type}`).join(" · ")}。模型能力：{descriptor.required_model_capability||"无要求"}。</p>}
      <button type="button" disabled={disabled||!descriptor||!!draft.label_pipeline&&!target} onClick={()=>{try{onChange(appendCatalogNode(draft,catalog,node,target,`step-${crypto.randomUUID()}`));setError("");}catch(e){setError((e as Error).message);}}}>加入未连接的步骤</button><p>只添加到当前未保存草稿。不会猜测连线或替你选择收费模型；请在高级配置中填写输入、绑定和参数，保存后仍需校验。</p>
    </>}
  </section>;
}
