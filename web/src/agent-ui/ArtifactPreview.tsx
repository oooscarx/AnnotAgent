import {useState} from "react";
import type {PipelineArtifact} from "../types";
import {artifactCropMarks,artifactDetectionMarks,geometrySemanticsLabel,scoreSemanticsLabel} from "../pipelinePresentation";

export function artifactIdentity(value:PipelineArtifact):string|undefined {
  const ref=value.artifact.reference;
  if(!ref||typeof ref!=="object")return undefined;
  const id=(ref as Record<string,unknown>).artifact_id;
  return typeof id==="string"?id:undefined;
}
export function artifactImageMatches(value:PipelineArtifact,imageId:string):boolean {
  return value.artifact.image_id===imageId && value.artifact.root_region==null;
}

/** Read-only inspection: never turns intermediate geometry into formal annotations. */
export function ArtifactPreview({artifact,projectId,imageId}:{artifact:PipelineArtifact;projectId:string;imageId?:string}) {
  const [original,setOriginal]=useState(false);const [selected,setSelected]=useState("");
  const [size,setSize]=useState<{width:number;height:number}>();const [error,setError]=useState(false);
  if(!imageId||!artifactImageMatches(artifact,imageId))return <p>产物缺少匹配的原图引用。仅显示保存的数据，不猜测图片或坐标。</p>;
  const imageUrl=`/api/projects/${encodeURIComponent(projectId)}/images/${encodeURIComponent(imageId)}/content`;
  const detections=artifactDetectionMarks([artifact]);const crops=artifactCropMarks([artifact],[]);
  const marks=[...detections,...crops];const active=marks.find(m=>m.id===selected);
  return <section className="native-artifact-preview" aria-label="中间产物预览">
    <p>中间产物 · {artifact.kind}。这些框不是额外的最终目标；此处查看不会保存或重跑。</p>
    <div className="actions"><button aria-pressed={original} onClick={()=>setOriginal(v=>!v)}>{original?"显示产物叠加":"只看原图"}</button></div>
    {error?<p role="alert">原图读取失败，未使用其他图片替代。</p>:<div className="native-artifact-image"><img src={imageUrl} alt="此产物的原始输入图片" onLoad={e=>setSize({width:e.currentTarget.naturalWidth,height:e.currentTarget.naturalHeight})} onError={()=>setError(true)}/>
      {size&&!original&&<svg viewBox={`0 0 ${size.width} ${size.height}`} aria-hidden="true">{marks.map((m,i)=><rect key={`${m.id}:${i}`} x={m.x*size.width} y={m.y*size.height} width={m.width*size.width} height={m.height*size.height} fill="none" stroke={m.color} strokeWidth={m.id===selected?2:1.5} vectorEffect="non-scaling-stroke"/>)}</svg>}
    </div>}
    {!marks.length&&<p>此产物没有可显示的检测框或 Crop；分类、Mask 及其他结构请查看下方原始数据。没有生成替代框。</p>}
    {!!marks.length&&<ul aria-label="产物对象列表">{marks.map((m,i)=><li key={`${m.id}:${i}`}><button aria-pressed={selected===m.id} onClick={()=>setSelected(m.id)}>{i+1} · {m.label}</button></li>)}</ul>}
    {active&&<div><p>{active.label} · {scoreSemanticsLabel(active.scoreSemantics)}：{active.confidence??"未提供"} · 几何：{geometrySemanticsLabel(active.geometrySemantics)}</p><p>对象 {active.id}{active.parentId?` · Parent ${active.parentId}`:""}{active.parentArtifact?` · 来源产物 ${active.parentArtifact}`:""}</p></div>}
    {size&&!!crops.length&&<div className="native-artifact-crops">{crops.map((crop,i)=><figure key={`${crop.id}:${i}`}><svg viewBox={`${crop.x*size.width} ${crop.y*size.height} ${crop.width*size.width} ${crop.height*size.height}`} role="img" aria-label={`Crop ${i+1}`}><image href={imageUrl} width={size.width} height={size.height}/></svg><figcaption>Crop {i+1} · Parent {crop.parentId||"未提供"}</figcaption></figure>)}</div>}
  </section>;
}
