import { useEffect, useRef, useState, type ReactNode } from "react";
import { Dialog } from "./Dialog";
import { ApiRequestError } from "../api";
import { historyCommand, validateHistoryScope, type HistoryScope, type HistoryPreview, type HistoryCommand, type historyScopeApi } from "./historyScope";

/** Explicit, immutable server boundary; local storage holds only an unresolved command, never membership. */
export function HistoryBoundary({workspaceId, service, children}: {
  workspaceId:string; service:typeof historyScopeApi; children:(scope:HistoryScope)=>ReactNode;
}) {
  const key=`annotagent.history-boundary.${workspaceId}`;
  const [scope,setScope]=useState<HistoryScope|null>();
  const [preview,setPreview]=useState<HistoryPreview>();
  const [pending,setPending]=useState<HistoryCommand>();
  const [error,setError]=useState("");
  const [busy,setBusy]=useState(false);
  const [reload,setReload]=useState(0);
  const lock=useRef(false);
  const generation=useRef(0);
  useEffect(()=>{
    const current=++generation.current;
    const controller=new AbortController();
    setScope(undefined);setError("");setPending(undefined);setPreview(undefined);
    try {const raw=localStorage.getItem(key);if(raw)setPending(JSON.parse(raw));}
    catch {setError("未能读取待确认记录；只会查询服务器，不会重发请求。");}
    void service.read(controller.signal).then(result=>{
      if(current!==generation.current)return;
      const found=result.scope?validateHistoryScope(result.scope):null;
      setScope(found);
      if(found){localStorage.removeItem(key);setPending(undefined);}
    }).catch(e=>{if(current===generation.current&&!controller.signal.aborted)setError(e.message);});
    return()=>{generation.current++;controller.abort();};
  },[key,service,reload]);
  const inspect=async()=>{
    if(lock.current||pending)return;
    lock.current=true;setBusy(true);setError("");const current=generation.current;
    try {const value=await service.preview();historyCommand(value,crypto.randomUUID());if(current===generation.current)setPreview(value);}
    catch(e){if(current===generation.current)setError((e as Error).message);}
    finally{lock.current=false;if(current===generation.current)setBusy(false);}
  };
  const confirm=async()=>{
    if(lock.current||!preview||pending)return;
    lock.current=true;setBusy(true);setError("");const current=generation.current;
    try {
      const exact=historyCommand(preview,crypto.randomUUID());
      localStorage.setItem(key,JSON.stringify(exact));setPending(exact);setPreview(undefined);
      const result=validateHistoryScope((await service.establish(exact)).scope);
      if(result.establishment_command_id!==exact.command_id)throw new Error("回执与本次操作不匹配；请读取服务器范围核实。");
      if(current===generation.current){setScope(result);setPending(undefined);localStorage.removeItem(key);}
    }catch(e){if(current===generation.current){
      // Only an explicit server non-admission permits a fresh preview. Network failures stay pending.
      if(e instanceof ApiRequestError && e.code === "history_scope_snapshot_changed") {localStorage.removeItem(key);setPending(undefined);}
      setError(`${(e as Error).message} 未自动重试，也未读取全部旧历史。请刷新服务器状态核实。`);
    }}
    finally{lock.current=false;if(current===generation.current)setBusy(false);}
  };
  if(scope)return <>{children(scope)}</>;
  return <section className="native-project-manager"><h1>启用新的历史列表</h1>
    <p>首次确认会固定整个工作区当前的历史记录范围。此前的 Pipeline、Run、Batch 和回收站内容不进入新列表，但不会删除；当前任务引用、已发布版本、标注和原图保持不变。这个边界不能重置。</p>
    {error&&<p role="alert">{error}</p>}
    {scope===undefined&&!error&&<p role="status">读取服务器历史范围…</p>}
    {pending&&<p role="status">已有一次提交等待核实。这里只查询服务器，不会重复建立范围。</p>}
    <div className="actions"><button disabled={busy} onClick={()=>setReload(n=>n+1)}>刷新服务器状态</button>{scope===null&&!pending&&<button disabled={busy} onClick={()=>void inspect()}>预览历史范围…</button>}</div>
    {preview&&<Dialog title="确认工作区历史范围" onClose={()=>{if(!busy)setPreview(undefined);}}>
      <p>以下已有记录不会进入新历史列表，数据仍保留。此操作不运行模型，不生成收费请求。</p>
      <dl>{Object.entries(preview.excluded_counts).map(([kind,count])=><div key={kind}><dt>{kind}</dt><dd>{count}</dd></div>)}</dl>
      <p>预览后历史有变化时，服务器会拒绝本次确认，不会悄悄扩大范围。</p>
      <div className="actions"><button disabled={busy} onClick={()=>setPreview(undefined)}>取消</button><button disabled={busy} onClick={()=>void confirm()}>确认建立工作区范围</button></div>
    </Dialog>}
  </section>;
}
