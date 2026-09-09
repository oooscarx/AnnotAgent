import { useState } from "react";
import type { Task, WorkspaceAdapter } from "./adapter";

export function SampleRepairControls({task,image,adapter,onError}:{task:Task;image:string;adapter:WorkspaceAdapter;onError:(message:string)=>void}) {
  const [busy,setBusy]=useState(false);
  const request=task.repairRequests?.find(r=>r.image===image&&r.sample===task.sample?.id);
  if(adapter.kind!=="http"||!request||!adapter.reportSampleIssue||!adapter.prepareAction)return null;
  const blocked=busy||!!task.approval||["running","planning","stopping","outcome_unknown"].includes(task.phase);
  const run=async(reason?:"wrong_target"|"poor_boundary")=>{
    setBusy(true);
    const c={id:crypto.randomUUID(),project:task.project,task:task.id,revision:task.revision,selection:{image,candidate:"",revision:task.resultRevision || task.revision}};
    try {
      if(reason)await adapter.reportSampleIssue!(c,image,reason);
      else await adapter.prepareAction!(c,"repair");
    } catch(error){onError((error as Error).message);}finally{setBusy(false);}
  };
  return <section className="sample-repair-controls" aria-label="样例问题反馈">
    <strong>{request.status==="pending"?"这个结果有问题？":"问题反馈已保存"}</strong>
    <div>{request.status==="pending"?<>
      <button disabled={blocked} onClick={()=>void run("wrong_target")}>目标找错</button>
      <button disabled={blocked} onClick={()=>void run("poor_boundary")}>边界不准确</button>
    </>:<button disabled={blocked} onClick={()=>void run()}>让 Agent 改进方案…</button>}</div>
    <small role="status">{busy?"正在读取服务器回执…":request.status==="pending"?"只保存此样例的问题，不会把当前框当作人工正确答案，也不会调用模型。":"下一步先查看模型、图片范围与费用授权；改进不保证成功，不会自动接受标注。"}</small>
  </section>;
}
