import {useState} from "react";
import type {HumanRequest} from "../conversation-human-api";

/** A view of existing durable requests, not a second request queue or task state. */
export function ConversationHumanRequests({requests,taskId,activeId,ready,loadError,onRefresh,onOpen,onCancel,onRetry,onDefer,onInspect}:{
  requests:HumanRequest[];taskId?:string;activeId?:string;ready:boolean;loadError?:string;
  onRefresh:()=>void;onOpen:(request:HumanRequest)=>void;
  onCancel:(request:HumanRequest)=>Promise<void>;onRetry:(request:HumanRequest)=>Promise<void>;
  onDefer?:(request:HumanRequest)=>Promise<void>;
  onInspect:(request:HumanRequest)=>void;
}){
  const [pending,setPending]=useState<string>();
  const current=requests.filter(value=>value.input.task_id===taskId&&((value.status==="pending"&&!value.deferred)||value.status==="answered"||value.input.id===activeId));
  const currentIds=new Set(current.map(value=>value.input.id));
  const needsAction=(value:HumanRequest)=>(value.status==="pending"&&!value.deferred)||value.status==="answered";
  const history=requests.filter(value=>!currentIds.has(value.input.id)).sort((a,b)=>Number(needsAction(b))-Number(needsAction(a)));
  const otherPending=history.filter(value=>value.input.task_id!==taskId&&value.status==="pending").length;
  const deferredCount=history.filter(value=>value.input.task_id===taskId&&value.deferred).length;
  async function act(value:HumanRequest,action:(request:HumanRequest)=>Promise<void>){
    if(pending)return;
    setPending(value.input.id);
    try{await action(value);}finally{setPending(undefined);}
  }
  function card(value:HumanRequest){return <article key={value.input.id} className="conversation-consent" aria-label={`Request: ${value.input.question}`}>
    {value.input.task_id!==taskId&&<small>Another annotation goal · opens its saved task and image</small>}
    <p>{value.input.question}</p>
    <p role="status">{value.deferred ? "Deferred · not reviewed or completed" : value.resume_draft_id ? "Correction saved · revision Draft available" : value.status==="answered" ? "Correction saved · awaiting task continuation" : value.status}</p>
    {value.deferred&&<p className="muted">Reopen when ready to correct this sample. Reopening does not call a model or renew the budget.</p>}
    {value.resume_error&&<p role="alert">Correction saved, but Draft preparation failed: {value.resume_error}</p>}
    {value.status==="answered"&&<p>Retry prepares the saved revision and resumes its repair/sample test only if you already authorized that continuation. Original model, image, call and expiry limits still apply.</p>}
    {value.journey_resume?.error&&<p role="alert">Correction saved; automatic continuation needs attention: {value.journey_resume.error}. Review the saved repair authorization below before retrying.</p>}
    <div className="conversation-request-actions">
      <button disabled={Boolean(pending)} onClick={()=>onOpen(value)}>Open requested result</button>
      {value.status==="answered"&&<button disabled={Boolean(pending)} onClick={()=>void act(value,onRetry)}>Retry Draft preparation</button>}
      {value.resume_draft_id&&<button disabled={Boolean(pending)} onClick={()=>onInspect(value)}>Inspect revision Draft</button>}
      {value.status==="pending"&&<button disabled={Boolean(pending)} onClick={()=>void act(value,onCancel)}>Cancel request</button>}
      {value.status==="pending"&&onDefer&&<button disabled={Boolean(pending)} onClick={()=>void act(value,onDefer)}>{value.deferred ? "Reopen request" : "Do this later"}</button>}
    </div>
    {pending===value.input.id&&<p role="status">Saving request state…</p>}
  </article>;}
  const empty=ready&&!loadError&&current.length===0&&history.length===0;
  if (empty) return null;
  return <section aria-label="Human requests" className="conversation-human-requests" data-empty={empty}>
    {!ready ? loadError ? <p role="alert">Could not load saved requests: {loadError}. Refresh requests retries the read; it does not start inference.</p> : <p role="status">Loading saved requests…</p> : <>
      {current.length>0&&<><h3>Requests for your help</h3><p className="muted">For the current annotation goal. Opening a request does not call a model.</p>{current.map(card)}</>}
      {history.length>0&&<details><summary>Request history and other goals ({history.length}){otherPending>0 ? ` · ${otherPending} awaiting help in other goals` : ""}</summary>{history.map(card)}</details>}
      {current.length===0&&<p className="muted">{deferredCount ? `${deferredCount} deferred requests remain unfinished. Reopen them from request history when ready.` : taskId ? "No outstanding visual requests for this goal." : "Select an annotation goal to see its requests."}</p>}
    </>}
    <button disabled={(!ready&&!loadError)||Boolean(pending)} onClick={onRefresh}>Refresh requests</button>
  </section>;
}
