import {useEffect,useState} from "react";
import type {api} from "../api";
import type {GeometryCalibrationView,ProjectGeometryPolicy} from "../types";
import {Disclosure} from "./Disclosure";
export type GeometryEvidenceService=Pick<typeof api,"geometryPolicy"|"geometryCalibrations">;
export function checkedGeometryEvidence(project:string,policies:ProjectGeometryPolicy[],calibrations:GeometryCalibrationView[]){
  // These project-scoped endpoints return Core UUIDs, not the URL's project slug.
  // Ownership is resolved by the server from the requested route; never guess a UUID from a name.
  const ids=[...policies.map(p=>p.project_id),...calibrations.map(c=>c.report.key.project_id)];
  if(!project||ids.some(id=>!id)||new Set(ids).size>1)throw new Error("几何证据的项目归属不一致，未展示混合项目记录");
  return {policies,calibrations};
}
export function geometryMeasure(value:number|undefined,percent=false):string {
  return value===undefined||!Number.isFinite(value)?"未测量":percent?`${(value*100).toFixed(1)}%`:value.toFixed(3);
}
export function GeometryEvidence({service,projectId}:{service:GeometryEvidenceService;projectId:string}){
  const [data,setData]=useState<ReturnType<typeof checkedGeometryEvidence>>();const [error,setError]=useState("");const [generation,setGeneration]=useState(0);
  useEffect(()=>{let current=true;setData(undefined);setError("");void Promise.all([service.geometryPolicy(projectId),service.geometryCalibrations(projectId)]).then(([p,c])=>{const result=checkedGeometryEvidence(projectId,p.policies,c.calibrations);if(current)setData(result);}).catch(e=>{if(current)setError(e.message);});return()=>{current=false;};},[service,projectId,generation]);
  return <section aria-label="项目几何质量证据"><p>项目级校准记录，不代表当前版本已校准。模型修订、节点配置、预处理和数据范围必须匹配；语义置信度不能替代边界质量。</p><button onClick={()=>setGeneration(n=>n+1)}>刷新几何证据</button>{error&&<p role="alert">{error}</p>}{!data&&!error&&<p role="status">读取服务器几何证据…</p>}
    {data&&<><h3>几何质量要求</h3>{!data.policies.length&&<p>服务器尚未保存项目几何策略。</p>}{data.policies.map((p,i)=><Disclosure key={i} title={`${p.task_kind} · ${p.required_quality}`}><p>自动接受策略：{p.auto_accept_policy}</p><pre>{JSON.stringify(p.calibration_thresholds,null,2)}</pre></Disclosure>)}
      <h3>校准记录</h3>{!data.calibrations.length&&<p>尚无校准记录，不能声称框或掩码已达到训练质量。</p>}{data.calibrations.map(c=><Disclosure key={c.report.id} title={`${c.report.key.task_id}${c.report.key.label_id?` · ${c.report.key.label_id}`:""} · ${c.effective_status}`}><p>记录时间：{c.report.created_at} · 原始状态：{c.report.status}</p><dl><dt>样本数 / 小目标样本数</dt><dd>{c.report.sample_count} / {c.report.small_object_sample_count}</dd><dt>Median IoU / P10 IoU</dt><dd>{geometryMeasure(c.report.median_iou)} / {geometryMeasure(c.report.p10_iou)}</dd><dt>人工调整率</dt><dd>{geometryMeasure(c.report.manual_adjustment_rate,true)}</dd></dl>{c.staleness_reasons.length>0&&<p role="status">过期原因：{c.staleness_reasons.join("；")}</p>}<Disclosure title="实际校准范围与证据引用"><pre>{JSON.stringify(c.report,null,2)}</pre></Disclosure></Disclosure>)}</>}
  </section>;
}
