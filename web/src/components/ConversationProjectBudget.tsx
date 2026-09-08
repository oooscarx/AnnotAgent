import {useEffect,useRef,useState} from "react";
import {api} from "../api";

/** A cumulative ceiling only; phase-specific model/data consent is still required. */
export function ConversationProjectBudget({project,onDirtyChange}:{project:string;onDirtyChange:(value:boolean)=>void}){
  const [saved,setSaved]=useState<Awaited<ReturnType<typeof api.projectCallLimit>>>();
  const [maximum,setMaximum]=useState("");
  const [confirmed,setConfirmed]=useState(false);
  const [busy,setBusy]=useState(false);
  const [loading,setLoading]=useState(true);
  const [error,setError]=useState("");
  const [reload,setReload]=useState(0);
  const frozen=useRef<Parameters<typeof api.setProjectCallLimit>[1]|undefined>(undefined);
  const alive=useRef(false);
  const dirty=Boolean(frozen.current)||Boolean(saved&&maximum!==String(saved.maximum_calls??""));
  useEffect(()=>{onDirtyChange(dirty);return()=>onDirtyChange(false);},[dirty,onDirtyChange]);
  useEffect(()=>{alive.current=true;return()=>{alive.current=false;};},[]);
  useEffect(()=>{
    const controller=new AbortController();setLoading(true);setError("");setConfirmed(false);
    void api.projectCallLimit(project,controller.signal).then(value=>{if(!controller.signal.aborted){setSaved(value);setMaximum(String(value.maximum_calls??""));}}).catch((failure:Error)=>{if(!controller.signal.aborted)setError(failure.message);}).finally(()=>{if(!controller.signal.aborted)setLoading(false);});
    return()=>controller.abort();
  },[project,reload]);
  async function save(){
    if(busy||loading||!saved)return;
    const value=Number(maximum);
    if(!frozen.current){
      if(!confirmed||maximum.trim()===""||!Number.isSafeInteger(value)||value<saved.reserved_calls)return;
      frozen.current={id:crypto.randomUUID(),expected_revision:saved.revision,maximum_calls:value};
    }
    setBusy(true);setError("");
    try {const value=await api.setProjectCallLimit(project,frozen.current);if(!alive.current)return;frozen.current=undefined;setSaved(value);setMaximum(String(value.maximum_calls??""));setConfirmed(false);}
    catch(failure){if(alive.current)setError((failure as Error).message);}
    finally{if(alive.current)setBusy(false);}
  }
  return <details className="conversation-project-budget"><summary>Project call limit</summary><section className="conversation-consent" aria-label="Project call limit">
    <p>This limit is shared by every conversation task and its confirmed dataset processing in this Project. It counts reserved calls, including failed or unknown outcomes—not tokens or money. Other legacy workflows are outside this conversation limit.</p>
    {saved&&!loading?<p role="status">{saved.reserved_calls} calls reserved · {saved.maximum_calls===null?"No Project conversation ceiling configured":`${saved.maximum_calls} cumulative maximum`} · Revision {saved.revision} · Saved snapshot; reload for current usage.</p>:<p role="status">Loading saved Project limit…</p>}
    <label>Cumulative maximum calls<input type="number" min={saved?.reserved_calls??0} step="1" value={maximum} disabled={!saved||busy||loading||Boolean(frozen.current)} onChange={event=>{setMaximum(event.target.value);setConfirmed(false);}}/></label>
    <label><input type="checkbox" checked={confirmed} disabled={busy||loading||Boolean(frozen.current)} onChange={event=>setConfirmed(event.target.checked)}/>I confirm this cumulative Project limit. Model, image and destination permissions still require their own consent.</label>
    {error&&<p role="alert">{error} No inference was requested. Retry keeps the same limit change; reload checks the latest saved revision.</p>}
    <div className="button-row"><button disabled={busy||loading||!saved||(!frozen.current&&(!confirmed||maximum.trim()===""||!Number.isSafeInteger(Number(maximum))||Number(maximum)<saved.reserved_calls))} onClick={()=>void save()}>{busy?"Saving Project limit…":frozen.current?"Retry saving Project limit":"Save Project limit"}</button><button disabled={busy||loading} onClick={()=>{if(dirty&&!window.confirm("Discard this unsaved limit input and load the current saved Project limit?"))return;frozen.current=undefined;setLoading(true);setReload(value=>value+1);}}>Reload saved limit</button></div>
  </section></details>;
}
