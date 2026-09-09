import { useEffect, useRef, useState } from "react";
import { AnnotationCanvas } from "../components/AnnotationCanvas";
import type { DeliveryImageView, DeliveryReviewInput, DeliveryService } from "./deliveryService";
import type { Annotation } from "../types";

type Image = {id:string;name:string;src?:string};
/** A whole-image receipt is separate from accepting individual objects or sample feedback. */
export function DeliveryReview({service,project,task,images,labels=[],locked=false,onEditingState}:{service:DeliveryService;project:string;task:string;images:Image[];labels?:{stable_id:string;display_name:string}[];locked?:boolean;onEditingState?:(active:boolean)=>void}) {
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
  const [draft,setDraft]=useState<Annotation>();
  const original=view?.snapshot.annotations.find(a=>a.id===selected);
  const object=draft||original;
  const dirty=!!draft&&JSON.stringify(draft)!==JSON.stringify(original);
  useEffect(()=>{onEditingState?.(dirty||busy);return()=>onEditingState?.(false);},[dirty,busy,onEditingState]);
  const editRetry=useRef<{signature:string;input:import("./deliveryService").DeliveryObjectEdit}|undefined>(undefined);
  const pending=useRef(false);
  const retry=useRef<{signature:string;input:DeliveryReviewInput} | undefined>(undefined);
  const image=images.find(i=>i.id===selection.image);
  useEffect(()=>{setReadable(false);if(!image?.src)return;let current=true;const probe=new Image();probe.onload=()=>{if(current)setReadable(probe.naturalWidth>0&&probe.naturalHeight>0);};probe.src=image.src;return()=>{current=false;};},[image?.src]);
  useEffect(()=>{const back=()=>queueMicrotask(()=>{if(!pending.current){const next=restore();if(next.image!==selection.image||next.run!==selection.run){setDraft(undefined);setSelection(next);}}});window.addEventListener("popstate",back);return()=>window.removeEventListener("popstate",back);},[images,selection.image,selection.run]);
  useEffect(()=>{
    const ctrl=new AbortController();setView(undefined);setError("");
    if(image)void service.image(project,task,image.id,selection.run||null,ctrl.signal).then(next=>{
      if(ctrl.signal.aborted)return;
      if(next.snapshot.image_id!==image.id || next.snapshot.source_run_id!==(selection.run||null))throw new Error("服务端图片或来源不匹配，未显示其他对象。");
      setView(next);
      setSelected(current=>next.snapshot.annotations.some(a=>a.id===current)?current:undefined);
    }).catch(e=>{if(!ctrl.signal.aborted)setError(e.message);});
    return()=>ctrl.abort();
  },[service,project,task,selection.image,selection.run,reload,image?.id]);
  useEffect(()=>{
    const guard=(e:Event)=>{if((dirty||busy)&&!window.confirm("有未保存的对象编辑或正在保存。仍要离开？"))e.preventDefault();};
    const unload=(e:BeforeUnloadEvent)=>{if(dirty||busy){e.preventDefault();e.returnValue="";}};
    window.addEventListener("ui-preview:before-navigate",guard);window.addEventListener("beforeunload",unload);
    return()=>{window.removeEventListener("ui-preview:before-navigate",guard);window.removeEventListener("beforeunload",unload);};
  },[dirty,busy]);
  const choose=(next:typeof selection)=>{
    if(pending.current||(dirty&&!window.confirm("放弃当前尚未保存的对象修改？")))return;
    setDraft(undefined);setSelected(undefined);
    setSelection(next);setReason("");setMessage("");
    const url=new URL(location.href);url.searchParams.set("delivery_image",next.image);
    if(next.run)url.searchParams.set("delivery_run",next.run);else url.searchParams.delete("delivery_run");
    history.pushState(history.state,"",url);
  };
  const confirm=async(decision:DeliveryReviewInput["decision"])=>{
    if(!view||!image||locked||dirty||pending.current||view.snapshot.image_id!==image.id||view.snapshot.source_run_id!==(selection.run||null))return;
    const scope={intent_revision:view.intent_revision,intent_sha256:view.intent_sha256,image_id:image.id,source_run_id:selection.run||null,expected_snapshot_sha256:view.snapshot.sha256,expected_review_revision:view.review?.revision||0,decision,reason:reason.trim()||null,confirmed:true};
    const signature=JSON.stringify(scope);
    if(retry.current?.signature!==signature)retry.current={signature,input:{...scope,command_id:crypto.randomUUID()}};
    pending.current=true;setBusy(true);setError("");setMessage("");
    try{await service.confirmImage(project,task,retry.current.input);retry.current=undefined;setMessage("整图决定已保存；未启动推理或打包。结构检查仍将在打包时执行。");setReload(v=>v+1);}
    catch(e){setError((e as Error).message);}
    finally{pending.current=false;setBusy(false);}
  };
  const saveObject=async(status:"needs_review"|"human_accepted"|"rejected")=>{
    if(!object||!view||!image||!selection.run||pending.current||locked||!readable)return;
    const body={intent_revision:view.intent_revision,intent_sha256:view.intent_sha256,source_run_id:selection.run,annotation_id:object.id,expected_snapshot_sha256:view.snapshot.sha256,label:object.label||"",value:object.value,review_status:status,reason:reason.trim()||`Human object ${status} in delivery review`};
    const signature=JSON.stringify(body);if(editRetry.current?.signature!==signature)editRetry.current={signature,input:{...body,command_id:crypto.randomUUID()}};
    pending.current=true;setBusy(true);setError("");
    try{await service.editObject(project,task,image.id,editRetry.current.input);setDraft(undefined);editRetry.current=undefined;setMessage("对象决定已保存；这不表示整张图已标注完整。请继续检查其余对象和漏标。");setReload(v=>v+1);}
    catch(e){setError((e as Error).message);}finally{pending.current=false;setBusy(false);}
  };
  const positive=!!selection.run&&!!view&&view.accepted_objects>0&&view.unresolved_objects===0;
  const negative=!!view&&view.accepted_objects===0&&view.unresolved_objects===0;
  const selectObject=(id:string)=>{if(id===selected||busy||(dirty&&!window.confirm("放弃当前对象的未保存修改？")))return;setDraft(undefined);setSelected(id);};
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
    {image&&<AnnotationCanvas imageUrl={image.src} annotations={selection.run?view?.snapshot.annotations.filter(a=>a.review_status!=="rejected").map(a=>draft?.id===a.id?draft:a)||[]:[]} selectedId={selected} onSelect={selectObject} onChange={next=>{if(!busy&&!locked){setSelected(next.id);setDraft(next);}}} readOnly={busy||locked||!selection.run||!service.editObject} compactList/>}
    {object&&selection.run&&<div className="delivery-object-editor"><p>选中对象 · {object.review_status} {dirty?"· 尚未保存":""}</p><label>对象类别<select aria-label="对象类别" value={object.label||""} disabled={busy||locked} onChange={e=>setDraft({...object,label:e.target.value})}>{labels.length?labels.map(l=><option key={l.stable_id} value={l.stable_id}>{l.display_name}</option>):<option value={object.label||""}>{object.label}</option>}</select></label><div className="actions"><button disabled={busy||!dirty} onClick={()=>setDraft(undefined)}>撤销对象修改</button><button disabled={busy||locked||!dirty} onClick={()=>void saveObject("needs_review")}>保存对象修改</button><button disabled={busy||locked||!readable||object.review_status==="human_accepted"&&!dirty} onClick={()=>void saveObject("human_accepted")}>接受这个对象</button><button disabled={busy||locked||!readable} onClick={()=>void saveObject("rejected")}>拒绝这个对象</button></div></div>}
    {!readable&&<p role="status">原图尚未成功加载，不能确认整图完整或无目标。仍可填写原因明确排除。</p>}
    {view&&<>
      <p>{view.confirmation_current?`此快照已有整图决定：${view.review?.input.decision}`:"当前图片／来源尚未整图确认"} · 已接受对象 {view.accepted_objects} · 未解决对象 {view.unresolved_objects}</p>
      <p>可直接选框修正边界或类别，并逐个接受／拒绝。发现遗漏但尚未补齐时保持未完成；此面板尚不支持新增漏框。</p>
      <label>检查备注／排除原因<textarea value={reason} disabled={busy} onChange={e=>setReason(e.target.value)} placeholder="排除图片必须说明原因"/></label>
      <div className="actions">
        <button disabled={busy||locked||dirty||!positive||!readable} onClick={()=>void confirm("positive_complete")}>确认整张图标注完整</button>
        <button disabled={busy||locked||dirty||!negative||!readable} onClick={()=>void confirm("negative_confirmed")}>确认整张图没有目标</button>
        <button disabled={busy||locked||dirty||!reason.trim()} onClick={()=>void confirm("excluded")}>明确排除此图</button>
        <button disabled={busy||dirty} onClick={()=>setReload(v=>v+1)}>重新读取服务器状态</button>
      </div>
    </>}
  </section>;
}
