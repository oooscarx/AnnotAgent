import {useEffect,useState} from "react";
import type {api} from "../api";
import type {ProviderProbeUsage} from "../types";
import {Disclosure} from "./Disclosure";
type Service=Pick<typeof api,"modelProfiles"|"modelProfileUsage">;
export function probeCost(record:Pick<ProviderProbeUsage,"cost"|"currency">) {
  const value=Number(record.cost);
  return record.cost.trim()&&Number.isFinite(value)&&value>0?`${record.currency} ${record.cost}（服务器估算，非账单）`:"费用未核实（零记录不代表免费）";
}
export function ProbeUsage({service}:{service:Service}) {
  const [open,setOpen]=useState(false);
  return <Disclosure title="模型探测用量记录" onToggle={e=>setOpen(e.currentTarget.open)}>{open&&<Records service={service}/>}</Disclosure>;
}
function Records({service}:{service:Service}) {
  const [rows,setRows]=useState<(ProviderProbeUsage&{modelName:string})[]>();
  const [errors,setErrors]=useState<string[]>([]);
  const [reload,setReload]=useState(0);
  const [offset,setOffset]=useState(0);
  const [query,setQuery]=useState("");
  useEffect(()=>{
    let current=true;setRows(undefined);setErrors([]);setOffset(0);
    void (async()=>{
      const {models}=await service.modelProfiles();
      const results:(ProviderProbeUsage&{modelName:string})[]=[];const failures:string[]=[];
      // Bounded reads; closing the disclosure prevents dispatch of subsequent groups.
      for(let start=0;current&&start<models.length;start+=4){
        await Promise.all(models.slice(start,start+4).map(async model=>{
          try {const r=await service.modelProfileUsage(model.id);if(r.model_profile_id!==model.id||r.active_probes.some(p=>p.model_profile_id!==model.id||p.provider_id!==model.provider_id))throw new Error("记录身份不匹配");results.push(...r.active_probes.map(p=>({...p,modelName:model.display_name})));}
          catch(e){failures.push(`${model.display_name}：${(e as Error).message}`);}
        }));
      }
      if(current){setRows(results.sort((a,b)=>b.created_at.localeCompare(a.created_at)||a.id.localeCompare(b.id)));setErrors(failures);}
    })().catch(e=>{if(current)setErrors([(e as Error).message]);});
    return()=>{current=false;};
  },[service,reload]);
  const filtered=rows?.filter(r=>`${r.modelName} ${r.model_profile_id}`.toLowerCase().includes(query.toLowerCase()));
  return <section aria-label="模型探测用量"><p>仅当前 Registry 模型的显式生成探测记录，不包含普通 Run、Task 或已删除模型，不是全系统费用或供应商账单。读取不会探测模型。</p>
    <div className="actions"><label>搜索模型<input value={query} onChange={e=>{setQuery(e.target.value);setOffset(0);}}/></label><button onClick={()=>setReload(n=>n+1)}>刷新探测记录</button></div>
    {errors.length>0&&<div role="alert"><p>记录不完整；未将读取失败计算为零。</p>{errors.map((e,i)=><p key={i}>{e}</p>)}</div>}
    {!rows&&!errors.length&&<p role="status">读取探测记录…</p>}
    {filtered&&<><p>当前匹配 {filtered.length} 条探测记录。</p>{filtered.slice(offset,offset+25).map(r=><article className="settings-row" key={r.id}><div><strong>{r.modelName} · r{r.model_profile_revision}</strong><p>{r.created_at} · {r.succeeded?"成功":"失败"}</p><p>{r.safe_message}</p></div><div><p>Tokens：{r.total_tokens??"未知"}</p><p>{probeCost(r)}</p><p>{r.duration_ms} ms</p></div></article>)}<div className="actions"><button disabled={offset===0} onClick={()=>setOffset(Math.max(0,offset-25))}>上一页</button><button disabled={offset+25>=filtered.length} onClick={()=>setOffset(offset+25)}>下一页</button></div></>}
  </section>;
}
