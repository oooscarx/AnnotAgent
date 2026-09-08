import { api } from "../api";
import { useState, useEffect, useRef } from "react";
import { useRouteQuery } from "../useRouteQuery";

/** The requesting task is explicit; the actual export remains project-wide. */
export function ConversationExports({project,conversation,task}:{project:string;conversation:string;task:string}) {
  const [cursors,setCursors]=useState<string[]>([]);
  const before=cursors.at(-1);
  const query=useRouteQuery(`conversation-exports:${project}:${conversation}:${task}:${before ?? "first"}`,signal=>api.conversationExports(project,conversation,task,signal,before,20));
  if(!before && !query.error && !query.data?.length)return null;
  return <section className="conversation-consent conversation-exports" aria-label="Saved export deliveries">
    <h3>Export deliveries</h3>
    <p>Project-wide exports requested from this goal. These may include confirmed results from other tasks.</p>
    {(before || query.data?.length===20) && <nav className="button-row" aria-label="Export history pages">
      <button disabled={!before || query.loading} onClick={()=>setCursors(value=>value.slice(0,-1))}>Newer exports</button>
      <span aria-live="polite">Page {cursors.length+1}</span>
      <button disabled={query.loading || query.data?.length!==20} onClick={()=>{const last=query.data?.at(-1);if(last)setCursors(value=>[...value,last.id]);}}>Older exports</button>
    </nav>}
    {before && !query.loading && query.data?.length===0 && <p>No older exports. Use Newer exports to return.</p>}
    {query.error && <p role="alert">{query.error.message}</p>}
    <button onClick={()=>void query.retry().catch(()=>undefined)}>Refresh export status</button>
    {query.data?.map(receipt=><article key={receipt.id}>
      <h4>{receipt.format} · {receipt.result ? "Export complete" : receipt.error ? "Export failed" : "Export requested"}</h4>
      <small>Export {receipt.id} · {receipt.created_at}</small>
      {receipt.error && <p role="alert">{receipt.error}</p>}
      {!receipt.result && !receipt.error && <ExportProgress project={project} conversation={conversation} task={task} id={receipt.id} onSettled={()=>void query.retry().catch(()=>undefined)} />}
      {receipt.result && <><p>{receipt.result.report.exported_count} annotations exported · {receipt.result.report.skipped_count} skipped</p>
        {receipt.result.delivery && <div className="button-row"><a className="button primary" download href={`/api/projects/${encodeURIComponent(project)}/exports/${receipt.result.delivery.id}/download`}>Download annotation archive</a></div>}
        <details><summary>Compatibility report</summary>{receipt.result.report.warnings.length ? <ul>{receipt.result.report.warnings.map((warning,index)=><li key={index}>{warning}</li>)}</ul> : <p>No export warnings were reported. This is not a model accuracy claim.</p>}</details>
      </>}
    </article>)}
  </section>;
}

function ExportProgress({project,conversation,task,id,onSettled}:{project:string;conversation:string;task:string;id:string;onSettled:()=>void}){
  const query=useRouteQuery(`export-job:${project}:${conversation}:${task}:${id}`,signal=>api.conversationExportStatus(project,conversation,task,id,signal));
  const settled=useRef(false);const notify=useRef(onSettled);notify.current=onSettled;
  useEffect(()=>{
    if(query.data?.job.result || query.data?.job.error){if(!settled.current){settled.current=true;notify.current();}return;}
    if(query.error || !query.data?.active)return;
    const timer=setTimeout(()=>void query.retry().catch(()=>undefined),2000);
    return ()=>clearTimeout(timer);
  },[query.data,query.error,query.retry]);
  return <div role="status">
    <p>{query.error ? query.error.message : query.data?.active ? "Export is running on the server. You can leave this page." : query.loading ? "Checking saved export…" : "Completion is unconfirmed; no active worker is reported. Checking status will not restart the export."}</p>
    {(query.error || !query.data?.active) && <button onClick={()=>void query.retry().catch(()=>undefined)}>Check export job</button>}
  </div>;
}
