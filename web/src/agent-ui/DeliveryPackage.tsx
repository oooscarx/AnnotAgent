import { useEffect, useRef, useState } from "react";
import { Disclosure } from "./Disclosure";
import type { DeliveryPackageInput, DeliveryPackageRead, DeliveryService } from "./deliveryService";

type Scope={revision:number;content_sha256:string;image_ids:string[]};
const phases={preparing:"准备中",exporting:"正在打包原图和标签",validating:"正在独立校验",ready:"数据集已打包",failed:"打包失败",cancelled:"已取消"};
export function DeliveryPackage({service,project,task,scope,locked=false,onInspect}:{service:DeliveryService;project:string;task:string;scope:Scope;locked?:boolean;onInspect:(id:string)=>void}) {
  const [history,setHistory]=useState<{id:string;created_at:string}[]>([]);
  const [cursor,setCursor]=useState<string|null>(null);
  const [id,setId]=useState(()=>new URL(location.href).searchParams.get("delivery_package")||"");
  const [job,setJob]=useState<DeliveryPackageRead>();
  const [error,setError]=useState("");
  const [busy,setBusy]=useState(false);
  const [reviews,setReviews]=useState<Record<string,number>>();
  const [missing,setMissing]=useState<string[]>();
  const [refresh,setRefresh]=useState(0);
  const [recovery,setRecovery]=useState<DeliveryPackageInput>();
  const pending=useRef(false);
  const retry=useRef<DeliveryPackageInput | undefined>(undefined);
  const select=(next:string)=>{setId(next);const url=new URL(location.href);url.searchParams.set("delivery_package",next);window.history.pushState(window.history.state,"",url);};
  useEffect(()=>{const back=()=>setId(new URL(location.href).searchParams.get("delivery_package")||"");window.addEventListener("popstate",back);return()=>window.removeEventListener("popstate",back);},[]);
  useEffect(()=>{setReviews(undefined);setMissing(undefined);retry.current=undefined;},[scope.revision,scope.content_sha256]);
  useEffect(()=>{try{const saved=service.pendingPackage?.(project,task);setRecovery(saved);if(saved)setId(current=>current||saved.command_id);}catch(e){setError((e as Error).message);}},[service,project,task,refresh]);
  useEffect(()=>{const ctrl=new AbortController();void service.history(project,task,undefined,ctrl.signal).then(page=>{if(!ctrl.signal.aborted){setHistory(page.items);setCursor(page.next_cursor);}}).catch(e=>{if(!ctrl.signal.aborted)setError(e.message);});return()=>ctrl.abort();},[service,project,task,refresh]);
  useEffect(()=>{
    const ctrl=new AbortController();let timer:ReturnType<typeof setTimeout>|undefined;setJob(undefined);
    const read=async()=>{try{const value=await service.packageStatus(project,task,id,ctrl.signal);if(ctrl.signal.aborted)return;setJob(value);setRecovery(current=>current?.command_id===id?undefined:current);if(value.active&&!value.interrupted)timer=setTimeout(()=>void read(),1500);}catch(e){if(!ctrl.signal.aborted)setError((e as Error).message);}};
    if(id&&!busy)void read();return()=>{ctrl.abort();clearTimeout(timer);};
  },[service,project,task,id,refresh,busy]);
  const act=async(fn:()=>Promise<void>)=>{if(pending.current)return;pending.current=true;setBusy(true);setError("");try{await fn();}catch(e){setError((e as Error).message);}finally{pending.current=false;setBusy(false);}};
  const check=()=>void act(async()=>{
    setReviews(undefined);setMissing(undefined);const map:Record<string,number>={};const unresolved:string[]=[];
    // Bounded sequential reads. Never starts inference or infers completion from empty results.
    for(const image of scope.image_ids){let state=await service.image(project,task,image,null);if(state.review?.input.source_run_id)state=await service.image(project,task,image,state.review.input.source_run_id);
      if(state.intent_revision!==scope.revision||state.intent_sha256!==scope.content_sha256)throw new Error("交付范围已变化，请重新读取会话信息。");
      if(state.confirmation_current&&state.review)map[image]=state.review.revision;else unresolved.push(image);
    }
    setMissing(unresolved);if(!unresolved.length)setReviews(map);
  });
  const start=()=>void act(async()=>{
    if(!reviews||locked||recovery)return;
    const body={intent_revision:scope.revision,intent_sha256:scope.content_sha256,image_reviews:reviews,confirmed:true};
    if(!retry.current||JSON.stringify({...retry.current,command_id:undefined})!==JSON.stringify(body))retry.current={...body,command_id:crypto.randomUUID()};
    const input=retry.current;select(input.command_id);
    const receipt=await service.startPackage(project,task,input);
    setError("");setJob({...receipt,interrupted:false});setRefresh(v=>v+1);
  });
  const receipt=job?.job.result;
  return <section className="delivery-package" aria-label="训练数据包交付">
    <h3>交付训练数据包</h3><p>当前范围 {scope.image_ids.length} 张。完整正样本、明确负样本与排除原因都需要当前版本的整图决定。此处仅本地打包，不新增模型调用。</p>
    {recovery&&<div className="notice"><p>恢复了一个尚未核实回执的打包命令：交付 revision {recovery.intent_revision}，{Object.keys(recovery.image_reviews).length} 张图片。仅恢复请求，不代表服务器已接收。</p><button disabled={busy} onClick={()=>{select(recovery.command_id);setRefresh(v=>v+1);}}>核实原打包请求</button><button disabled={busy||locked} onClick={()=>void act(async()=>{const original=recovery;select(original.command_id);const result=await service.startPackage(project,task,original);setJob({...result,interrupted:false});setRecovery(undefined);setRefresh(v=>v+1);})}>按原范围和命令重试</button></div>}
    <button disabled={busy||locked} onClick={check}>检查当前打包范围</button>
    {missing&&<p role="status">{missing.length?`还有 ${missing.length} 张未完成当前整图确认。`:"整图决定齐全；划分、类别和原图仍需打包器独立校验。"}</p>}
    {!!missing?.length&&<Disclosure title="查看未完成图片">{missing.map((image,index)=><button key={image} disabled={busy} onClick={()=>onInspect(image)}>检查图片 {index+1}</button>)}</Disclosure>}
    {reviews&&<div className="notice"><p>确认按交付 revision {scope.revision} 的全部图片及整图决定打包，包含明确排除记录。旧包不会被覆盖。</p><button disabled={busy||locked||!!recovery} onClick={start}>确认并生成训练数据包</button></div>}
    {error&&<p role="alert" className="error">{error} 不会自动重试或显示完成。</p>}
    {!!history.length&&<label>本任务已保存的数据包<select aria-label="本任务已保存的数据包" value={id} onChange={e=>select(e.target.value)}><option value="">选择数据包</option>{history.map(item=><option key={item.id} value={item.id}>{item.created_at} · {item.id.slice(0,8)}</option>)}</select></label>}
    {cursor&&<button disabled={busy} onClick={()=>void act(async()=>{const page=await service.history(project,task,cursor);setHistory(old=>[...old,...page.items.filter(p=>!old.some(o=>o.id===p.id))]);setCursor(page.next_cursor);})}>更早的数据包</button>}
    {id&&<><button disabled={busy} onClick={()=>{setError("");setRefresh(v=>v+1);}}>读取打包状态</button>{!job&&!error&&<p role="status">读取服务器记录…</p>}</>}
    {job&&<article className="delivery-package-receipt">
      <strong role="status">{job.interrupted?"执行已中断或远端状态未知":phases[job.job.phase]}</strong>
      {job.interrupted&&<p>服务器没有活动 Worker。尚未确认成功，不会自动重新生成；可取消此记录后明确创建新包。</p>}
      {job.job.error&&<p role="alert">{job.job.error}</p>}
      {!["ready","failed","cancelled"].includes(job.job.phase)&&<button disabled={busy} onClick={()=>void act(async()=>{await service.cancelPackage(project,task,id);setRefresh(v=>v+1);})}>取消这个打包任务</button>}
      {job.job.phase==="ready"&&receipt&&<>
        <p>Ultralytics YOLO · 目标检测 · 冻结交付 revision {job.job.intent_revision}</p>
        <p>类别：{receipt.summary?.labels.join("、")||"旧回执未记录，请查看包内清单"}</p>
        <p>训练图片 {receipt.summary?.splits.train??"未记录"} · 验证图片 {receipt.summary?.splits.val??"未记录"} · 确认负样本 {receipt.negatives} · 排除 {receipt.excluded}</p>
        <p>已纳入 {receipt.images} 张原图、{receipt.objects} 个正式对象 · {receipt.bytes.toLocaleString()} bytes</p>
        <p>结构检查通过；不代表模型精度或漏检检查通过。完整性依据为保存的人工整图确认。</p>
        <a href={service.downloadUrl(project,task,id)} download>下载数据集 ZIP</a>
        <Disclosure title="查看检查摘要"><p>SHA-256：{receipt.sha256}</p><p>完整报告、原图哈希、来源与划分位于 ZIP 的 annotagent 目录。官方加载器 smoke 未执行。</p>{receipt.summary?.warnings.map((warning,index)=><p key={index}>{warning}</p>)}{Object.entries(receipt.summary?.exclusions||{}).map(([image,reason])=><p key={image}>排除 {image}：{reason}</p>)}</Disclosure>
      </>}
    </article>}
  </section>;
}
