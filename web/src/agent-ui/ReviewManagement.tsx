import { useEffect, useRef, useState } from "react";
import type { api } from "../api";
import type { Annotation, AnnotationRevision, ImageItem, ReviewItem } from "../types";
import { AnnotationCanvas } from "../components/AnnotationCanvas";
import { Disclosure } from "./Disclosure";
export type ReviewService = Pick<typeof api,"reviews"|"review"|"images"|"revise"|"decideAndNext"|"revisions">;
export function ReviewManagement({service,projectId,reviewId,workspaceId}:{service:ReviewService;projectId:string;reviewId?:string;workspaceId:string}) {
  const [revisions,setRevisions]=useState<AnnotationRevision[]>();
  const [items,setItems]=useState<ReviewItem[]>();
  const [item,setItem]=useState<ReviewItem>();
  const [image,setImage]=useState<ImageItem>();
  const [draft,setDraft]=useState<Annotation>();
  const [undo,setUndo]=useState<Annotation[]>([]);
  const [error,setError]=useState("");
  const [message,setMessage]=useState("");
  const [busy,setBusy]=useState(false);
  const pending=useRef(false);
  const leavingAfterSave=useRef(false);
  const [completed,setCompleted]=useState(false);
  const [offset,setOffset]=useState(0);
  const [more,setMore]=useState(false);
  const [version,setVersion]=useState(0);
  const storageKey=`annotagent.review-edit.${workspaceId}.${projectId}.${reviewId}`;
  const dirty=!!(item&&draft&&JSON.stringify(item.annotation)!==JSON.stringify(draft));
  useEffect(()=>{
    const ctrl=new AbortController();setError("");setItem(undefined);setDraft(undefined);setImage(undefined);setUndo([]);
    if(reviewId)void Promise.all([service.review(reviewId,ctrl.signal,projectId),service.images(projectId,ctrl.signal)]).then(([review,images])=>{
      if(ctrl.signal.aborted)return;
      if(review.project_id!==projectId||review.review_id!==reviewId)throw new Error("审核对象与当前项目不匹配");
      setItem(review);setDraft(review.annotation);setCompleted(["human_accepted","rejected"].includes(review.annotation.review_status));
      const original=images.images.find(i=>i.image_id===review.image_id&&i.project_id===projectId);setImage(original);
      if(!original)setError("找不到审核项对应的原图；不会使用其他图片替代。");
      try{const cached=JSON.parse(localStorage.getItem(storageKey)||"null");if(cached?.base===JSON.stringify(review.annotation)&&cached.draft?.id===review.annotation.id){setDraft(cached.draft);setMessage("恢复了尚未提交的本地编辑。");}else if(cached)setMessage("服务器标注已变化，旧本地编辑没有覆盖服务器版本。");}catch{setMessage("本地编辑恢复记录无法读取，已保留服务器版本。");}
    }).catch(e=>{if(!ctrl.signal.aborted)setError(e.message);});
    else {setItems(undefined);void service.reviews(projectId,ctrl.signal,offset).then(result=>{if(ctrl.signal.aborted)return;if(result.reviews.some(r=>r.project_id!==projectId))throw new Error("审核队列归属不匹配");setItems(result.reviews);setMore(result.reviews.length===50);}).catch(e=>{if(!ctrl.signal.aborted)setError(e.message);});}
    return()=>ctrl.abort();
  },[service,projectId,reviewId,offset,version,storageKey]);
  useEffect(()=>{const guard=(e:Event)=>{if(!leavingAfterSave.current&&(dirty||busy)&&!window.confirm("有未保存编辑或操作尚未结束，仍要离开？"))e.preventDefault();};const unload=(e:BeforeUnloadEvent)=>{if(!leavingAfterSave.current&&(dirty||busy)){e.preventDefault();e.returnValue="";}};window.addEventListener("ui-preview:before-navigate",guard);window.addEventListener("beforeunload",unload);return()=>{window.removeEventListener("ui-preview:before-navigate",guard);window.removeEventListener("beforeunload",unload);};},[dirty,busy]);
  const edit=(next:Annotation)=>{
    if(!item||!draft||busy||completed)return;
    try{localStorage.setItem(storageKey,JSON.stringify({base:JSON.stringify(item.annotation),draft:next}));}
    catch{setError("本地编辑暂存失败；离开前请保存到服务器。");}
    setUndo(old=>[...old.slice(-49),draft]);setDraft(next);setMessage("有未保存的编辑");
  };
  const run=async(action:()=>Promise<void>)=>{if(pending.current)return;pending.current=true;setBusy(true);setError("");try{await action();}catch(e){setError((e as Error).message);}finally{pending.current=false;setBusy(false);}};
  const decide=(decision:"accept"|"reject")=>void run(async()=>{
    if(!item||dirty||completed)return;
    const result=await service.decideAndNext(item.review_id,projectId,decision,decision==="accept"?"accepted_as_is":"wrong_object","Human decision in Agent UI",undefined,projectId);
    if(result.next_review&&result.next_review.project_id!==projectId)throw new Error("下一审核项项目归属不匹配；没有自动跳转。");
    try{localStorage.removeItem(storageKey);}catch{/* A saved server decision must not become a failed decision due to local storage. */}
    if(result.next_review){leavingAfterSave.current=true;location.assign(`/projects/${encodeURIComponent(projectId)}/manage/review/${encodeURIComponent(result.next_review.review_id)}`);}
    else {setItem({...item,annotation:result.annotation});setDraft(result.annotation);setUndo([]);setCompleted(true);setMessage("当前审核项已处理，队列已结束。可以返回图片或导出结果。");}
  });
  return <section className="native-project-manager native-review"><h1>{reviewId?"审核标注":"审核队列"}</h1><nav className="native-management-tabs"><a href={`/projects/${encodeURIComponent(projectId)}/work`}>返回 Agent</a>{reviewId&&<a href={`/projects/${encodeURIComponent(projectId)}/manage/review`}>返回审核队列</a>}</nav>{error&&<p role="alert" className="error">{error}</p>}{message&&<p role="status">{message}</p>}
    {!reviewId?<>{!items&&!error&&<p role="status">读取待审核项…</p>}{items?.length===0&&<p>当前没有待审核项。</p>}{items?.map(review=><article className="settings-row" key={review.review_id}><div><strong>{review.annotation.label||review.annotation.task_id}</strong><p>{review.review_explanation?.summary||review.review_reason}</p></div><a href={`/projects/${encodeURIComponent(projectId)}/manage/review/${encodeURIComponent(review.review_id)}`}>检查标注</a></article>)}<div className="actions"><button disabled={offset===0} onClick={()=>setOffset(n=>Math.max(0,n-50))}>上一页</button><button disabled={!more} onClick={()=>setOffset(n=>n+50)}>下一页</button></div></>:!item||!draft?(!error&&<p role="status">读取标注和原图…</p>):<>
      <p>{item.review_explanation?.summary||item.review_reason}</p>{item.validation_issues.map((v,i)=><p className="notice" key={i}>{v}</p>)}
      <AnnotationCanvas imageUrl={image?.url} annotations={[draft]} selectedId={draft.id} onSelect={()=>{}} onChange={edit} readOnly={busy||completed||!image} compactList/>
      <label>标签<input value={draft.label||""} disabled={busy||completed} onChange={e=>edit({...draft,label:e.target.value,...(draft.value.kind==="classification"?{value:{...draft.value,labels:[e.target.value]}}:{})})}/></label>
      <div className="actions"><button disabled={busy||!undo.length} onClick={()=>{const previous=undo.at(-1)!;setUndo(old=>old.slice(0,-1));setDraft(previous);localStorage.setItem(storageKey,JSON.stringify({base:JSON.stringify(item.annotation),draft:previous}));}}>撤销编辑</button><button disabled={busy||!dirty} onClick={()=>void run(async()=>{const current=await service.review(item.review_id,undefined,projectId);if(JSON.stringify(current.annotation)!==JSON.stringify(item.annotation))throw new Error("服务器标注已变化；本地编辑已保留，请重新读取并核对。");await service.revise(draft,"Human geometry/label correction in Agent UI");localStorage.removeItem(storageKey);setMessage("编辑已保存到服务器；尚未接受此标注。");setVersion(v=>v+1);})}>保存编辑</button><button disabled={busy||completed||dirty||!image} onClick={()=>decide("reject")}>拒绝并下一项</button><button className="primary" disabled={busy||completed||dirty||!image} onClick={()=>decide("accept")}>接受这个对象并下一项</button></div>
      <Disclosure title="来源与审核证据"><p>原图：{image?.name||item.image_id}</p><p>Run：{item.run_id} · Workflow v{item.workflow_version}</p><p>节点：{item.source_node||"未记录"}</p><p>语义分数：{item.confidence??"未记录"}，不等同于几何准确率。</p><button onClick={()=>void service.revisions(item.review_id,projectId).then(r=>setRevisions(r.revisions)).catch(e=>setError(e.message))}>读取修订记录</button>{revisions?.map(revision=><article className="settings-row" key={revision.revision_id}><div><strong>{revision.actor} · {revision.created_at}</strong><p>{revision.reason||"未记录修订原因"}</p><Disclosure title="比较本次修改"><pre>{JSON.stringify({before:revision.before,after:revision.after},null,2)}</pre></Disclosure></div></article>)}</Disclosure>
    </>}
  </section>;
}
