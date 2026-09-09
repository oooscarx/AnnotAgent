import {useState} from "react";
import {comparisonBoxes,type GeometrySample} from "./geometryComparison";
import {labelColor} from "../annotationVisuals";
const groups=[{id:"coarse",name:"整图粗定位",dash:"8 4"},{id:"relocalized",name:"局部重定位",dash:"3 3"},{id:"refined",name:"精修候选",dash:""},{id:"retained",name:"终端保留结果",dash:"12 3 2 3"}];
export function GeometryStageView({sample,imageUrl}:{sample:GeometrySample;imageUrl:string}){
  const [visible,setVisible]=useState(groups.map(g=>g.id));const boxes=comparisonBoxes(sample);
  return <section aria-label="几何阶段对比"><p>只读诊断。精修候选可能已被拒绝；显示不等于接受。终端结果不包含未选中的中间检测，也不覆盖人工修改。</p><div className="actions">{groups.map(g=><label key={g.id}><input type="checkbox" checked={visible.includes(g.id)} onChange={e=>setVisible(v=>e.target.checked?[...v,g.id]:v.filter(id=>id!==g.id))}/>{g.name} · {boxes.filter(b=>b.group===g.id).length}</label>)}</div>
    {sample.width>0&&sample.height>0&&<svg viewBox={`0 0 ${sample.width} ${sample.height}`} style={{width:"100%",display:"block"}} role="img" aria-label="原图上的只读阶段框"><image href={imageUrl} width={sample.width} height={sample.height}/>{boxes.map((b,i)=>{if(!visible.includes(b.group))return null;const g=groups.find(g=>g.id===b.group)!;return <rect key={i} x={b.rect[0]*sample.width} y={b.rect[1]*sample.height} width={b.rect[2]*sample.width} height={b.rect[3]*sample.height} fill="none" stroke={labelColor(b.label)} strokeWidth={1.5} strokeDasharray={g.dash} vectorEffect="non-scaling-stroke"><title>{i+1}. {g.name} · {b.label} · {b.detail}</title></rect>;})}</svg>}
    {!boxes.length&&<p>没有可在原图坐标系显示的阶段框。</p>}<ol>{boxes.map((b,i)=>visible.includes(b.group)&&<li key={i} value={i+1}>{groups.find(g=>g.id===b.group)?.name} · {b.label} · {b.rect.map((v,j)=>Math.round(v*(j%2?sample.height:sample.width))).join(", ")} px · {b.detail}</li>)}</ol><p>这里显示边界框，不把框伪装成分割掩码。</p>
  </section>;
}
