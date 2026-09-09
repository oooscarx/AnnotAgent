import {useEffect,useState} from "react";
import type {api} from "../api";
import type {FrozenWorkflowVersion} from "../types";
import {Disclosure} from "./Disclosure";
import {agentPath} from "./navigationContract";
export type WorkflowVersionService=Pick<typeof api,"frozenWorkflowVersion">;
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
export function WorkflowVersionDetail({service,projectId,workflowId,version}:{service:WorkflowVersionService;projectId:string;workflowId:string;version:string}) {
  const [value,setValue]=useState<FrozenWorkflowVersion>();const [error,setError]=useState("");
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
      <p>版本复制与比较仍在迁移；不会自动创建草稿或重新发布。</p>
    </>}
  </section>;
}
