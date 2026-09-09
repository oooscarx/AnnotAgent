import {useEffect,useState} from "react";
import type {FrozenWorkflowVersion} from "../types";
import {Disclosure} from "./Disclosure";
import {parseVersion,verifyFrozenVersion,type WorkflowVersionService} from "./WorkflowVersionDetail";
function canonical(value:unknown):unknown {
  if(Array.isArray(value))return value.map(canonical);
  if(value&&typeof value==="object")return Object.fromEntries(Object.entries(value).sort(([a],[b])=>a.localeCompare(b)).map(([k,v])=>[k,canonical(v)]));
  return value;
}
export function frozenDifferences(left:FrozenWorkflowVersion,right:FrozenWorkflowVersion) {
  const result:{path:string;left:unknown;right:unknown}[]=[];
  for(const area of ["draft","snapshot"] as const){
    const a=left[area] as unknown as Record<string,unknown>,b=right[area] as unknown as Record<string,unknown>;
    for(const key of [...new Set([...Object.keys(a),...Object.keys(b)])].sort())if(JSON.stringify(canonical(a[key]))!==JSON.stringify(canonical(b[key])))result.push({path:`${area}.${key}`,left:a[key],right:b[key]});
  }
  return result;
}
function target() {const q=new URL(location.href).searchParams;return {workflow:q.get("compare_workflow")||"",version:q.get("compare_version")||""};}
export function WorkflowComparison({source,service}:{source:FrozenWorkflowVersion;service:WorkflowVersionService}) {
  const [selection,setSelection]=useState(target);
  const [workflow,setWorkflow]=useState(selection.workflow||source.workflow_id);
  const [version,setVersion]=useState(selection.version);
  const [value,setValue]=useState<FrozenWorkflowVersion>();const [error,setError]=useState("");const [busy,setBusy]=useState(false);
  useEffect(()=>{const pop=()=>{const next=target();setSelection(next);setWorkflow(next.workflow||source.workflow_id);setVersion(next.version);};window.addEventListener("popstate",pop);return()=>window.removeEventListener("popstate",pop);},[source.workflow_id]);
  useEffect(()=>{
    const c=new AbortController();setValue(undefined);setError("");setBusy(false);
    if(!selection.workflow&&!selection.version)return;
    const n=parseVersion(selection.version);if(!n||!selection.workflow||/[\/\\?#]/.test(selection.workflow)){setError("无效的比较对象，不读取默认版本。");return;}
    setBusy(true);void service.frozenWorkflowVersion(source.project_id,selection.workflow,n,c.signal).then(r=>{if(!c.signal.aborted)setValue(verifyFrozenVersion(r,source.project_id,selection.workflow,n));}).catch(e=>{if(!c.signal.aborted)setError(e.message);}).finally(()=>{if(!c.signal.aborted)setBusy(false);});return()=>c.abort();
  },[service,source.project_id,selection]);
  const compare=()=>{const url=new URL(location.href);url.searchParams.set("compare_workflow",workflow);url.searchParams.set("compare_version",version);history.pushState(null,"",url);setSelection({workflow,version});};
  const clear=()=>{const url=new URL(location.href);url.searchParams.delete("compare_workflow");url.searchParams.delete("compare_version");history.pushState(null,"",url);setSelection({workflow:"",version:""});};
  const changes=value?frozenDifferences(source,value):[];
  return <Disclosure title="比较另一个已发布版本" open={!!selection.version}><section aria-label="冻结版本比较"><p>只比较当前项目的两个不可变对象；差异不代表质量改进，不调用模型、不合并或发布。包括草稿元数据与完整执行快照字段。</p>
    <div className="actions"><label>比较 Workflow ID<input value={workflow} onChange={e=>setWorkflow(e.target.value)}/></label><label>比较版本<input inputMode="numeric" value={version} onChange={e=>setVersion(e.target.value)}/></label><button disabled={busy||!workflow.trim()||!parseVersion(version)} onClick={compare}>读取并比较</button><button onClick={clear}>关闭比较</button></div>
    {busy&&<p role="status">读取精确版本…</p>}{error&&<p role="alert">{error}</p>}
    {value&&<><p role="status">{source.workflow_id}@{source.version} 与 {value.workflow_id}@{value.version}：{changes.length} 个字段组不同。</p><p>左侧：当前版本；右侧：比较版本。</p>{changes.map(change=><Disclosure key={change.path} title={change.path}><div className="frozen-comparison"><section aria-label={`${change.path} 当前版本`}><h4>当前版本</h4><pre>{change.left===undefined?"未保存该字段":JSON.stringify(change.left,null,2)}</pre></section><section aria-label={`${change.path} 比较版本`}><h4>比较版本</h4><pre>{change.right===undefined?"未保存该字段":JSON.stringify(change.right,null,2)}</pre></section></div></Disclosure>)}</>}
  </section></Disclosure>;
}
