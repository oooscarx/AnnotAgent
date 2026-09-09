import {useEffect,useState} from "react";
import type {api} from "../api";
import type {WorkflowVersion} from "../types";
import {Disclosure} from "./Disclosure";
import {agentPath} from "./navigationContract";
export type WorkflowVersionService=Pick<typeof api,"projectSummary">;
export function parseVersion(value:string):number|undefined {return /^[1-9]\d*$/.test(value)&&Number.isSafeInteger(Number(value))?Number(value):undefined;}
export function versionLink(projectId:string,reference?:string):string|undefined {
  if(!reference)return undefined;const split=reference.lastIndexOf("@");if(split<1)return undefined;
  const version=reference.slice(split+1);if(!parseVersion(version))return undefined;
  try{return `${agentPath({kind:"detail",projectId,page:"pipelines",objectId:reference.slice(0,split)})}?version=${version}`;}catch{return undefined;}
}
export function WorkflowVersionDetail({service,projectId,workflowId,version}:{service:WorkflowVersionService;projectId:string;workflowId:string;version:string}) {
  const [value,setValue]=useState<WorkflowVersion>();const [error,setError]=useState("");
  useEffect(()=>{const c=new AbortController();setValue(undefined);setError("");if(!parseVersion(version)){setError("无效的 Workflow 版本号，没有打开默认版本。");return;}
    void service.projectSummary(projectId,c.signal).then(result=>{if(c.signal.aborted)return;if(result.project.id!==projectId)throw new Error("项目归属不匹配");const found=result.project.available_workflow_versions.find(v=>v.workflow_id===workflowId&&String(v.version)===version);if(!found)throw new Error("此项目中未找到指定版本，可能已删除。没有自动打开另一个版本。");setValue(found);}).catch(e=>{if(!c.signal.aborted)setError(e.message);});return()=>c.abort();
  },[service,projectId,workflowId,version]);
  return <section className="native-project-manager" aria-label="Workflow 版本详情"><h1>Workflow 版本</h1><a href={agentPath({kind:"work",projectId})}>返回 Agent</a>{error&&<p role="alert">{error}</p>}{!value&&!error&&<p role="status">读取指定版本…</p>}
    {value&&<><h2>{value.name} · v{value.version}</h2><p>{value.status} · {value.validation_status}{value.is_default?" · 项目默认版本":""}</p><p>这是指定版本的服务器摘要，不是当前可编辑 Draft。查看不会修改发布内容或启动模型。</p>
      <p>来源：{value.source} · {value.workflow_id}</p><h3>已发布节点摘要</h3>{value.nodes.map(node=><Disclosure key={node.id} title={`${node.node_type} · ${node.id}`}><dl><dt>模型绑定</dt><dd>{node.model_binding||"未声明直接绑定"}</dd><dt>上游节点</dt><dd>{node.depends_on.join(" · ")||"无"}</dd><dt>验证器</dt><dd>{node.validators.join(" · ")||"无"}</dd><dt>精修器</dt><dd>{node.refiners.join(" · ")||"无"}</dd><dt>人工审核</dt><dd>{node.human_review_gate?"要求":"未声明"}</dd><dt>Fallback</dt><dd>{node.fallback||"未声明"}</dd></dl></Disclosure>)}
      <p>完整冻结配置、版本复制与比较仍在迁移；这里不以最新 Draft 代替历史版本。</p>
    </>}
  </section>;
}
