import { useEffect, useRef, useState } from "react";
import type { api } from "../api";
import type { Annotation, AnnotationRevision, ImageItem, ReviewItem, ReviewQueueProgress } from "../types";
import { AnnotationCanvas } from "../components/AnnotationCanvas";
import {reviewOffset,reviewPath} from "./reviewNavigation";
import {ReviewAttributes} from "./ReviewAttributes";
import {applyReviewEvidence,validEvidenceBox} from "./reviewEdits";
import {HumanAnnotation} from "./HumanAnnotation";
import { Disclosure } from "./Disclosure";
import {reviewReasonOptions,reviewDecisionReason,type ReviewReasonOption} from "./reviewReasons";
export type ReviewService = Pick<typeof api,"reviews"|"review"|"reviewNext"|"images"|"revise"|"decideAndNext"|"revisions"|"projectSummary"|"createAnnotation"|"skills">;
export function ReviewManagement({service,projectId,reviewId,workspaceId}:{service:ReviewService;projectId:string;reviewId?:string;workspaceId:string}) {
  const [attributesPending,setAttributesPending]=useState(false);
  const [adding,setAdding]=useState(false);
  const [revisions,setRevisions]=useState<AnnotationRevision[]>();
  const [items,setItems]=useState<ReviewItem[]>();
  const [item,setItem]=useState<ReviewItem>();
  const [image,setImage]=useState<ImageItem>();
  const [draft,setDraft]=useState<Annotation>();
  const [undo,setUndo]=useState<Annotation[]>([]);
  const [redo,setRedo]=useState<Annotation[]>([]);
  const [view,setView]=useState<"result"|"original"|"before">("result");
  const [reason,setReason]=useState("wrong_object");
  const [reasonOptions,setReasonOptions]=useState<ReviewReasonOption[]>(reviewReasonOptions([],[]));
  const [enabledSkills,setEnabledSkills]=useState<string[]>([]);
  const [note,setNote]=useState("");
  const [progress,setProgress]=useState<ReviewQueueProgress>();
  const [error,setError]=useState("");
  const [message,setMessage]=useState("");
  const [busy,setBusy]=useState(false);
  const pending=useRef(false);
  const progressEpoch=useRef(0);
  const leavingAfterSave=useRef(false);
  const [completed,setCompleted]=useState(false);
  const [offset,setOffset]=useState(()=>reviewOffset(new URL(location.href)));
  useEffect(()=>{const restore=()=>setOffset(reviewOffset(new URL(location.href)));window.addEventListener("popstate",restore);return()=>window.removeEventListener("popstate",restore);},[]);
  const selectOffset=(next:number)=>{setOffset(next);const url=new URL(location.href);url.searchParams.set("queue_offset",String(next));history.pushState(history.state,"",url);};
  const [nextOffset,setNextOffset]=useState<number|null>(null);
  const storageKey=`annotagent.review-edit.${workspaceId}.${projectId}.${reviewId}`;
  const dirty=!!(item&&draft&&JSON.stringify(item.annotation)!==JSON.stringify(draft));
  useEffect(()=>{
    const epoch=++progressEpoch.current;
    const ctrl=new AbortController();setError("");setItem(undefined);setDraft(undefined);setImage(undefined);setUndo([]);setRedo([]);setRevisions(undefined);setProgress(undefined);setNote("");setReason("wrong_object");setMessage("");setEnabledSkills([]);
    if(reviewId)void Promise.all([service.review(reviewId,ctrl.signal,projectId),service.images(projectId,ctrl.signal),service.projectSummary(projectId,ctrl.signal),service.skills(ctrl.signal)]).then(([review,images,summary,skills])=>{
      if(ctrl.signal.aborted)return;
      if(review.project_id!==projectId||review.review_id!==reviewId)throw new Error("审核对象与当前项目不匹配");
      if(summary.project.id!==projectId)throw new Error("审核原因配置与当前项目不匹配");
      const enabled=summary.project.enabled_skills.map(s=>s.id);const options=reviewReasonOptions(skills,enabled);setEnabledSkills(enabled);setReasonOptions(options);
      setItem(review);setDraft(review.annotation);setCompleted(["human_accepted","rejected"].includes(review.annotation.review_status));
      void service.reviewNext(reviewId,projectId).then(result=>{if(!ctrl.signal.aborted&&epoch===progressEpoch.current)setProgress(result.progress);}).catch(e=>{if(!ctrl.signal.aborted&&epoch===progressEpoch.current)setError(`审核进度读取失败：${e.message}`);});
      const original=images.images.find(i=>i.image_id===review.image_id&&i.project_id===projectId);setImage(original);
      if(!original)setError("找不到审核项对应的原图；不会使用其他图片替代。");
      try{const cached=JSON.parse(localStorage.getItem(storageKey)||"null");if(cached?.base===JSON.stringify(review.annotation)&&cached.draft?.id===review.annotation.id){setDraft(cached.draft);setNote(typeof cached.note==="string"?cached.note:"");setReason(typeof cached.reason==="string"?cached.reason:"wrong_object");setMessage("恢复了尚未提交的本地编辑。");}else if(cached)setMessage("服务器标注已变化，旧本地编辑没有覆盖服务器版本。");}catch{setMessage("本地编辑恢复记录无法读取，已保留服务器版本。");}
    }).catch(e=>{if(!ctrl.signal.aborted)setError(e.message);});
    else {setItems(undefined);setNextOffset(null);void service.reviews(projectId,ctrl.signal,offset).then(result=>{if(ctrl.signal.aborted)return;if(result.reviews.some(r=>r.project_id!==projectId))throw new Error("审核队列归属不匹配");setItems(result.reviews);setProgress(result.progress);setNextOffset(result.page.next_offset??null);}).catch(e=>{if(!ctrl.signal.aborted)setError(e.message);});}
    return()=>ctrl.abort();
  },[service,projectId,reviewId,offset,storageKey]);
  useEffect(()=>{const guard=(e:Event)=>{if(!leavingAfterSave.current&&(dirty||busy||!!note.trim())&&!window.confirm("有未保存编辑或操作尚未结束，仍要离开？"))e.preventDefault();};const unload=(e:BeforeUnloadEvent)=>{if(!leavingAfterSave.current&&(dirty||busy||!!note.trim())){e.preventDefault();e.returnValue="";}};window.addEventListener("ui-preview:before-navigate",guard);window.addEventListener("beforeunload",unload);return()=>{window.removeEventListener("ui-preview:before-navigate",guard);window.removeEventListener("beforeunload",unload);};},[dirty,busy,note]);
  const edit=(next:Annotation)=>{
    if(!item||!draft||busy||completed)return;
    try{localStorage.setItem(storageKey,JSON.stringify({base:JSON.stringify(item.annotation),draft:next,note,reason}));}
    catch{setError("本地编辑暂存失败；离开前请保存到服务器。");}
    setUndo(old=>[...old.slice(-49),draft]);setRedo([]);setDraft(next);setMessage("有未保存的编辑");
  };
  const moveEdit=(direction:"undo"|"redo")=>{
    const next=(direction==="undo"?undo:redo).at(-1);if(!next||!draft||!item||busy||completed)return;
    if(direction==="undo"){setUndo(old=>old.slice(0,-1));setRedo(old=>[...old,draft]);}else{setRedo(old=>old.slice(0,-1));setUndo(old=>[...old,draft]);}
    setDraft(next);
    try{localStorage.setItem(storageKey,JSON.stringify({base:JSON.stringify(item.annotation),draft:next,note,reason}));}catch{setError("本地编辑暂存失败；离开前请保存到服务器。");}
  };
  const feedback=(nextReason:string,nextNote:string)=>{
    setReason(nextReason);setNote(nextNote);
    if(item&&draft)try{localStorage.setItem(storageKey,JSON.stringify({base:JSON.stringify(item.annotation),draft,note:nextNote,reason:nextReason}));}catch{setError("本地备注暂存失败；离开页面会丢失未提交备注。");}
  };
  const run=async(action:()=>Promise<void>)=>{if(pending.current)return;pending.current=true;setBusy(true);setError("");try{await action();}catch(e){setError((e as Error).message);}finally{pending.current=false;setBusy(false);}};
  const decide=(decision:"accept"|"reject")=>void run(async()=>{
    if(!item||dirty||completed)return;
    const selectedReason=reviewDecisionReason(reasonOptions,reason,decision,item.source_skill_id||undefined,enabledSkills);
    const result=await service.decideAndNext(item.review_id,projectId,decision,selectedReason.code,note,selectedReason.skillId,projectId);
    progressEpoch.current++;
    setProgress(result.progress);
    if(result.next_review&&result.next_review.project_id!==projectId)throw new Error("下一审核项项目归属不匹配；没有自动跳转。");
    try{localStorage.removeItem(storageKey);}catch{/* A saved server decision must not become a failed decision due to local storage. */}
    if(result.next_review){leavingAfterSave.current=true;location.assign(reviewPath(projectId,result.next_review.review_id,offset));}
    else {setNote("");setItem({...item,annotation:result.annotation});setDraft(result.annotation);setUndo([]);setCompleted(true);setMessage("当前审核项已处理，队列已结束。可以返回图片或导出结果。");}
  });
  return <section className="native-project-manager native-review"><h1>{reviewId?"审核标注":"审核队列"}</h1><nav className="native-management-tabs"><a href={`/projects/${encodeURIComponent(projectId)}/work`}>返回 Agent</a>{reviewId&&<a href={reviewPath(projectId,undefined,offset)}>返回审核队列</a>}</nav>{error&&<p role="alert" className="error">{error}</p>}{message&&<p role="status">{message}</p>}{progress&&<p role="status">已审核 {progress.reviewed_count} / {progress.total_count} · 待审核 {progress.remaining_count}</p>}
    {!reviewId?<>{!items&&!error&&<p role="status">读取待审核项…</p>}{items?.length===0&&<p>当前没有待审核项。</p>}{items?.map(review=><article className="settings-row" key={review.review_id}><div><strong>{review.annotation.label||review.annotation.task_id}</strong><p>{review.review_explanation?.summary||review.review_reason}</p></div><a href={reviewPath(projectId,review.review_id,offset)}>检查标注</a></article>)}<div className="actions"><button disabled={offset===0} onClick={()=>selectOffset(Math.max(0,offset-50))}>上一页</button><button disabled={nextOffset===null} onClick={()=>{if(nextOffset!==null)selectOffset(nextOffset);}}>下一页</button></div></>:!item||!draft?(!error&&<p role="status">读取标注和原图…</p>):<>
      <button disabled={busy||attributesPending||dirty||!image} onClick={()=>{if(adding&&!window.dispatchEvent(new Event("ui-preview:before-navigate",{cancelable:true})))return;setAdding(!adding);}}>{adding?"返回审核编辑":"补充遗漏标注…"}</button>{adding&&image&&<HumanAnnotation service={service} projectId={projectId} runId={item.run_id} imageId={item.image_id} imageUrl={image.url} queueOffset={offset} workspaceId={workspaceId}/>} {!adding&&<><p>{item.review_explanation?.summary||item.review_reason}</p>{item.validation_issues.map((v,i)=><p className="notice" key={i}>{v}</p>)}
      <div className="actions" role="group" aria-label="审核图片视图"><button aria-pressed={view==="result"} onClick={()=>setView("result")}>当前编辑</button><button aria-pressed={view==="before"} onClick={()=>setView("before")}>服务器已保存标注</button><button aria-pressed={view==="original"} onClick={()=>setView("original")}>原图</button></div>
      <AnnotationCanvas imageUrl={image?.url} annotations={view==="original"?[]:[view==="before"?item.annotation:draft]} selectedId={view==="original"?undefined:draft.id} onSelect={()=>{}} onChange={edit} readOnly={busy||completed||!image||view!=="result"} compactList/>
      <label>标签<input value={draft.label||""} disabled={busy||completed} onChange={e=>edit({...draft,label:e.target.value,...(draft.value.kind==="classification"?{value:{...draft.value,labels:[e.target.value]}}:{})})}/></label>
      <Disclosure title="属性编辑"><ReviewAttributes value={draft.attributes} disabled={busy||completed} onPendingChange={setAttributesPending} onApply={attributes=>edit({...draft,attributes})}/></Disclosure><Disclosure title="审核问题与备注" className="review-feedback"><label>拒绝原因<select aria-label="拒绝原因" value={reason} disabled={busy||completed} onChange={e=>feedback(e.target.value,note)}>{!reasonOptions.some(option=>option.key===reason)&&<option value={reason} disabled>原原因已不可用，请重新选择</option>}{reasonOptions.map(option=><option key={option.key} value={option.key}>{option.label}</option>)}</select></label><label>审核备注<textarea aria-label="审核备注" value={note} disabled={busy||completed} onChange={e=>feedback(reason,e.target.value)}/></label><p>原因和备注随显式审核决定提交；这不会自动重跑模型或新增遗漏目标。</p></Disclosure><div className="actions"><button disabled={busy||completed||!undo.length} onClick={()=>moveEdit("undo")}>撤销编辑</button><button disabled={busy||completed||!redo.length} onClick={()=>moveEdit("redo")}>重做编辑</button><button disabled={busy||attributesPending||!dirty} onClick={()=>void run(async()=>{const current=await service.review(item.review_id,undefined,projectId);if(JSON.stringify(current.annotation)!==JSON.stringify(item.annotation))throw new Error("服务器标注已变化；本地编辑已保留，请重新读取并核对。");await service.revise(draft,"Human geometry/label correction in Agent UI");try{if(note.trim())localStorage.setItem(storageKey,JSON.stringify({base:JSON.stringify(draft),draft,note,reason}));else localStorage.removeItem(storageKey);}catch{/* Server success is independent of cache cleanup. */}setItem({...item,annotation:draft});setUndo([]);setRedo([]);setMessage("编辑已保存到服务器；尚未接受此标注。");})}>保存编辑</button><button disabled={busy||attributesPending||adding||completed||dirty||!image||!reasonOptions.some(option=>option.key===reason)} onClick={()=>decide("reject")}>拒绝并下一项</button><button className="primary" disabled={busy||attributesPending||adding||completed||dirty||!image} onClick={()=>decide("accept")}>接受这个对象并下一项</button></div>
      <Disclosure title="来源与审核证据"><p>原图：{image?.name||item.image_id}</p><p>Run：<a href={`/projects/${encodeURIComponent(projectId)}/manage/runs/${encodeURIComponent(item.run_id)}?source_review=${encodeURIComponent(item.review_id)}&queue_offset=${offset}&annotation=${encodeURIComponent(item.annotation.id)}`}>查看来源运行</a> · Workflow v{item.workflow_version}</p><p>节点：{item.source_node||"未记录"}</p><p>语义分数：{item.confidence??"未记录"}，不等同于几何准确率。</p>{item.detection_evidence.length>0&&<section aria-label="来源模型框"><h2>来源模型框</h2><p>选择只替换当前编辑的几何，不代表模型分数是准确率，也不会自动保存或接受。</p>{item.detection_evidence.map((e,i)=><article className="settings-row" key={`${e.source_artifact_id}:${i}`}><div><strong>{e.source_model_display_name||e.source_model_id}</strong><p>{e.source_capability} · {e.score.semantics}</p><pre>{JSON.stringify({bbox:e.bbox,score:e.score,artifact:e.source_artifact_id},null,2)}</pre></div><button disabled={busy||attributesPending||completed||draft.value.kind!=="bounding_box"||!validEvidenceBox(e.bbox)} onClick={()=>{edit(applyReviewEvidence(draft,e));setView("result");}}>使用来源框 {i+1}</button></article>)}</section>}<button onClick={()=>void service.revisions(item.review_id,projectId).then(r=>setRevisions(r.revisions)).catch(e=>setError(e.message))}>读取修订记录</button>{revisions?.map(revision=><article className="settings-row" key={revision.revision_id}><div><strong>{revision.actor} · {revision.created_at}</strong><p>{revision.reason||"未记录修订原因"}</p><Disclosure title="比较本次修改"><pre>{JSON.stringify({before:revision.before,after:revision.after},null,2)}</pre></Disclosure></div></article>)}</Disclosure></>}
    </>}
  </section>;
}
