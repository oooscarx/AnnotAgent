import {useState} from "react";
import type {ExcludedSampleCandidate} from "../sampleFeedbackOverlay";
import {AnnotationCanvas} from "../components/AnnotationCanvas";
import {Disclosure} from "./Disclosure";
export function ExcludedCandidates({excluded,imageUrl}:{excluded:ExcludedSampleCandidate[];imageUrl:string}){
  const [open,setOpen]=useState(false);const [selected,setSelected]=useState("");const current=excluded.find(c=>c.annotation.id===selected)??excluded[0];
  if(!current)return null;
  return <Disclosure title={`已排除的样例候选 · ${excluded.length}`} onToggle={e=>setOpen(e.currentTarget.open)}>{open&&<section aria-label="已排除候选检查"><p>仅从当前样例视图排除；原始预测与正式标注未被删除。这里只读查看，不重新接受候选。</p><label>查看候选<select aria-label="查看已排除候选" value={current.annotation.id} onChange={e=>setSelected(e.target.value)}>{excluded.map(c=><option key={c.annotation.id} value={c.annotation.id}>{c.annotation.label} · {c.annotation.id}</option>)}</select></label><p>排除原因：{current.revision.reason} · {current.revision.note||"未填写补充说明"}</p><AnnotationCanvas compactList readOnly imageUrl={imageUrl} annotations={[current.annotation]} selectedId={current.annotation.id} onSelect={()=>{}} onChange={()=>{}}/><Disclosure title="排除记录与来源"><pre>{JSON.stringify(current.revision,null,2)}</pre></Disclosure></section>}</Disclosure>;
}
