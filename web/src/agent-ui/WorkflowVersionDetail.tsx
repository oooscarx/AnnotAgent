import {useEffect,useState} from "react";
import type {api} from "../api";
import type {FrozenWorkflowVersion} from "../types";
import {Disclosure} from "./Disclosure";
import {agentPath} from "./navigationContract";
import {WorkflowComparison} from "./WorkflowComparison";
import {WorkflowClone,type WorkflowCloneService} from "./WorkflowClone";
import {GeometryEvidence,type GeometryEvidenceService} from "./GeometryEvidence";
import {GeometryCalibration,type GeometryCalibrationService} from "./GeometryCalibration";
export type WorkflowVersionService=Pick<typeof api,"frozenWorkflowVersion"> & WorkflowCloneService & GeometryEvidenceService & GeometryCalibrationService;
export function verifyFrozenVersion(value:FrozenWorkflowVersion,projectId:string,workflowId:string,version:number) {
  if(value.project_id!==projectId||value.workflow_id!==workflowId||value.version!==version||value.draft.project_id!==projectId||value.source_draft_id!==value.draft.id||!value.content_hash)throw new Error("冻结版本身份不匹配，没有用当前 Draft 替代。");
  return value;
}
export function parseVersion(value:string):number|undefined {return /^[1-9]\d*$/.test(value)&&Number.isSafeInteger(Number(value))?Number(value):undefined;}
export function versionLink(projectId:string,reference?:string):string|undefined {
  if(!reference)return undefined;const split=reference.lastIndexOf("@");if(split<1)return undefined;
  const version=reference.slice(split+1);if(!parseVersion(version))return undefined;
  try{return `${agentPath({kind:"detail",projectId,page:"pipelines",objectId:reference.slice(0,split)})}?version=${version}`;}catch{return undefined;}
}
export function WorkflowVersionDetail({service,projectId,workflowId,version,workspaceId}:{service:WorkflowVersionService;projectId:string;workflowId:string;version:string;workspaceId?:string}) {
  const [value,setValue]=useState<FrozenWorkflowVersion>();const [error,setError]=useState("");
  const [evidenceOpen,setEvidenceOpen]=useState(false);
  useEffect(()=>{const c=new AbortController();setValue(undefined);setError("");if(!parseVersion(version)){setError("无效的 Workflow 版本号，没有打开默认版本。");return;}
    void service.frozenWorkflowVersion(projectId,workflowId,Number(version),c.signal).then(result=>{if(!c.signal.aborted)setValue(verifyFrozenVersion(result,projectId,workflowId,Number(version)));}).catch(e=>{if(!c.signal.aborted)setError(e.message);});return()=>c.abort();
  },[service,projectId,workflowId,version]);
  return <section className="native-project-manager" aria-label="Workflow 版本详情"><h1>Workflow 版本</h1><a href={agentPath({kind:"work",projectId})}>返回 Agent</a>{error&&<p role="alert">{error}</p>}{!value&&!error&&<p role="status">读取指定版本…</p>}
    {value&&<><h2>{value.draft.name} · v{value.version}</h2><p>发布于 {value.published_at} · 只读</p><p>这是服务器保存的完整不可变版本，不是当前可编辑 Draft。查看不会修改发布内容或启动模型。</p>
      <p>来源草稿：{value.source_draft_id} · revision {value.draft.revision}</p><p>执行快照 Hash：{value.content_hash}</p>
      {!value.snapshot.draft&&<p role="status">此历史快照未保存完整执行 Draft，按原始数据展示，不补入当前版本。</p>}
      <h3>冻结的编排节点</h3>{value.draft.nodes.map(node=><Disclosure key={node.id} title={node.id}><pre>{JSON.stringify(node,null,2)}</pre></Disclosure>)}
      <Disclosure title="Label Pipeline 与共享阶段"><pre>{JSON.stringify(value.draft.label_pipeline??null,null,2)}</pre></Disclosure>
      <Disclosure title="完整冻结配置与模型资源"><pre>{JSON.stringify(value.snapshot,null,2)}</pre></Disclosure>
      <Disclosure title="完整发布对象（只读）"><pre>{JSON.stringify(value,null,2)}</pre></Disclosure>
      <WorkflowComparison key={`${value.workflow_id}@${value.version}`} source={value} service={service}/>
      <Disclosure title="项目几何策略与校准证据" onToggle={e=>setEvidenceOpen(e.currentTarget.open)}>{evidenceOpen&&<><GeometryEvidence key={projectId} service={service} projectId={projectId}/>{workspaceId&&<GeometryCalibration key={`${workspaceId}:${value.content_hash}`} service={service} source={value} workspaceId={workspaceId}/>}</>}</Disclosure>
      {workspaceId&&<WorkflowClone key={`${workspaceId}:${value.workflow_id}@${value.version}`} source={value} workspaceId={workspaceId} service={service}/>}
    </>}
  </section>;
}
