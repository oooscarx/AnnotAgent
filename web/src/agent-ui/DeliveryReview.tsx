import { useEffect, useRef, useState } from "react";
import { AnnotationCanvas } from "../components/AnnotationCanvas";
import type { DeliveryImageView, DeliveryReviewInput, DeliveryService } from "./deliveryService";

type Image = {id:string;name:string;src?:string};
/** A whole-image receipt is separate from accepting individual objects or sample feedback. */
export function DeliveryReview({service,project,task,images,locked=false}:{service:DeliveryService;project:string;task:string;images:Image[];locked?:boolean}) {
  const restore=()=>{
    const query=new URL(location.href).searchParams;
    return {image:images.find(i=>i.id===query.get("delivery_image"))?.id || images[0]?.id || "",run:query.get("delivery_run") || ""};
  };
  const [selection,setSelection]=useState(restore);
  const [view,setView]=useState<DeliveryImageView>();
  const [error,setError]=useState("");
  const [busy,setBusy]=useState(false);
  const [reason,setReason]=useState("");
  const [selected,setSelected]=useState<string>();
  const [reload,setReload]=useState(0);
  const [message,setMessage]=useState("");
  const [readable,setReadable]=useState(false);
  const pending=useRef(false);
  const retry=useRef<{signature:string;input:DeliveryReviewInput} | undefined>(undefined);
  const image=images.find(i=>i.id===selection.image);
  useEffect(()=>{setReadable(false);if(!image?.src)return;let current=true;const probe=new Image();probe.onload=()=>{if(current)setReadable(probe.naturalWidth>0&&probe.naturalHeight>0);};probe.src=image.src;return()=>{current=false;};},[image?.src]);
  useEffect(()=>{const back=()=>{if(!pending.current)setSelection(restore());};window.addEventListener("popstate",back);return()=>window.removeEventListener("popstate",back);},[images]);
  useEffect(()=>{
    const ctrl=new AbortController();setView(undefined);setError("");setSelected(undefined);
    if(image)void service.image(project,task,image.id,selection.run||null,ctrl.signal).then(next=>{
      if(ctrl.signal.aborted)return;
      if(next.snapshot.image_id!==image.id || next.snapshot.source_run_id!==(selection.run||null))throw new Error("服务端图片或来源不匹配，未显示其他对象。");
      setView(next);
    }).catch(e=>{if(!ctrl.signal.aborted)setError(e.message);});
    return()=>ctrl.abort();
  },[service,project,task,selection.image,selection.run,reload,image?.id]);
  const choose=(next:typeof selection)=>{
    if(pending.current)return;
    setSelection(next);setReason("");setMessage("");
    const url=new URL(location.href);url.searchParams.set("delivery_image",next.image);
    if(next.run)url.searchParams.set("delivery_run",next.run);else url.searchParams.delete("delivery_run");
    history.pushState(history.state,"",url);
  };
  const confirm=async(decision:DeliveryReviewInput["decision"])=>{
    if(!view||!image||locked||pending.current||view.snapshot.image_id!==image.id||view.snapshot.source_run_id!==(selection.run||null))return;
    const scope={intent_revision:view.intent_revision,intent_sha256:view.intent_sha256,image_id:image.id,source_run_id:selection.run||null,expected_snapshot_sha256:view.snapshot.sha256,expected_review_revision:view.review?.revision||0,decision,reason:reason.trim()||null,confirmed:true};
    const signature=JSON.stringify(scope);
    if(retry.current?.signature!==signature)retry.current={signature,input:{...scope,command_id:crypto.randomUUID()}};
    pending.current=true;setBusy(true);setError("");setMessage("");
    try{await service.confirmImage(project,task,retry.current.input);retry.current=undefined;setMessage("整图决定已保存；未启动推理或打包。结构检查仍将在打包时执行。");setReload(v=>v+1);}
    catch(e){setError((e as Error).message);}
    finally{pending.current=false;setBusy(false);}
  };
  const positive=!!selection.run&&!!view&&view.accepted_objects>0&&view.unresolved_objects===0;
  const negative=!!view&&view.accepted_objects===0&&view.unresolved_objects===0;
  return <section className="delivery-review" aria-label="训练图片整图审核">
    <h3>逐张确认训练图片</h3>
    <p>这是正式标注的整图确认，不是样例测试。接受一个框、空检测或拒绝所有框，都不代表整张图片已检查完整。</p>
    <div className="delivery-review-controls">
      <label>图片<select aria-label="图片" value={selection.image} disabled={busy} onChange={e=>choose({image:e.target.value,run:""})}>{images.map((i,index)=><option key={i.id} value={i.id}>{index+1}/{images.length} · {i.name}</option>)}</select></label>
      <label>正式标注来源<select aria-label="正式标注来源" value={selection.run} disabled={busy||!view} onChange={e=>choose({...selection,run:e.target.value})}><option value="">仅检查原图／尚未选择来源</option>{view?.sources.map(s=><option key={s.run_id} value={s.run_id}>{s.model} · {s.created_at} · {s.run_id.slice(0,8)}</option>)}</select></label>
    </div>
    <p>来源列表仅包含本项目此图片的正式终态 Run，可能来自其他任务。请明确选择要复用的结果；不会自动使用最新 Run。</p>
    {error&&<p role="alert" className="error">{error}{selection.run&&!view&&<button disabled={busy} onClick={()=>choose({...selection,run:""})}>返回原图，重新选择来源</button>}</p>}{message&&<p role="status">{message}</p>}
    {!view&&!error&&<p role="status">读取正式标注与审核状态…</p>}
    {image&&<AnnotationCanvas imageUrl={image.src} annotations={selection.run?view?.snapshot.annotations.filter(a=>a.review_status!=="rejected")||[]:[]} selectedId={selected} onSelect={setSelected} onChange={()=>{}} readOnly compactList/>}
    {!readable&&<p role="status">原图尚未成功加载，不能确认整图完整或无目标。仍可填写原因明确排除。</p>}
    {view&&<>
      <p>{view.confirmation_current?`此快照已有整图决定：${view.review?.input.decision}`:"当前图片／来源尚未整图确认"} · 已接受对象 {view.accepted_objects} · 未解决对象 {view.unresolved_objects}</p>
      <p>发现漏标或边界错误时请保持未完成，不要确认完整。目前此面板只确认整图；对象编辑接线尚未完成。</p>
      <label>检查备注／排除原因<textarea value={reason} disabled={busy} onChange={e=>setReason(e.target.value)} placeholder="排除图片必须说明原因"/></label>
      <div className="actions">
        <button disabled={busy||locked||!positive||!readable} onClick={()=>void confirm("positive_complete")}>确认整张图标注完整</button>
        <button disabled={busy||locked||!negative||!readable} onClick={()=>void confirm("negative_confirmed")}>确认整张图没有目标</button>
        <button disabled={busy||locked||!reason.trim()} onClick={()=>void confirm("excluded")}>明确排除此图</button>
        <button disabled={busy} onClick={()=>setReload(v=>v+1)}>重新读取服务器状态</button>
      </div>
    </>}
  </section>;
}
