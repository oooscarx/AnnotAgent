import {useEffect,useRef,useState} from "react";
import type {api} from "../api";
import {projectBudgetAvailability} from "../projectBudget";
import type {ProjectCallLimitSnapshot} from "../types";
import {Dialog} from "./Dialog";
export type ProjectCallLimitService=Pick<typeof api,"projectCallLimit"|"setProjectCallLimit">;
type LimitCommand=Parameters<ProjectCallLimitService["setProjectCallLimit"]>[1];
export function parseLimitMaximum(text:string,reserved:number){
  if(!/^\d+$/.test(text.trim()))throw new Error("请输入累计调用次数整数；空白不代表无限额。");
  const value=Number(text);if(!Number.isSafeInteger(value)||value<reserved)throw new Error("累计上限不能低于已预留调用次数，也不能超过安全整数范围。");return value;
}
export function restoreLimitCommand(raw:string):LimitCommand {
  const value=JSON.parse(raw) as LimitCommand;
  if(!value||typeof value.id!=="string"||!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(value.id)||!Number.isSafeInteger(value.expected_revision)||value.expected_revision<0||!Number.isSafeInteger(value.maximum_calls)||value.maximum_calls<0)throw new Error("额度修改恢复记录无效；没有发送请求。");return value;
}
export function ProjectCallLimit({projectId,workspaceId,service}:{projectId:string;workspaceId:string;service:ProjectCallLimitService}){
  const key=`annotagent.project-call-limit.${workspaceId}.${projectId}`;
  const [saved,setSaved]=useState<ProjectCallLimitSnapshot>();const [text,setText]=useState("");const [pending,setPending]=useState<LimitCommand>();const [confirm,setConfirm]=useState<LimitCommand>();const [blocked,setBlocked]=useState(false);const [busy,setBusy]=useState(false);const [error,setError]=useState("");const [message,setMessage]=useState("");const [epoch,setEpoch]=useState(0);const lock=useRef(false);const alive=useRef(false);
  const dirty=!!pending||!!confirm||!!saved&&text!==String(saved.maximum_calls??"");
  useEffect(()=>{alive.current=true;const ctrl=new AbortController();setSaved(undefined);setError("");
    try{const raw=localStorage.getItem(key);if(raw)setPending(restoreLimitCommand(raw));}catch(e){setBlocked(true);setError((e as Error).message);}
    void service.projectCallLimit(projectId,ctrl.signal).then(value=>{if(ctrl.signal.aborted)return;if(!projectBudgetAvailability(value).known)throw new Error("服务器额度快照无效");setSaved(value);setText(String(value.maximum_calls??""));}).catch(e=>{if(!ctrl.signal.aborted)setError(e.message);});
    return()=>{alive.current=false;ctrl.abort();};
  },[key,projectId,service,epoch]);
  useEffect(()=>{const guard=(e:Event)=>{if((dirty||busy)&&!window.confirm("有尚未确认的额度修改，仍要离开？已提交的请求不会因此取消。"))e.preventDefault();};const unload=(e:BeforeUnloadEvent)=>{if(dirty||busy){e.preventDefault();e.returnValue="";}};window.addEventListener("ui-preview:before-navigate",guard);window.addEventListener("beforeunload",unload);return()=>{window.removeEventListener("ui-preview:before-navigate",guard);window.removeEventListener("beforeunload",unload);};},[dirty,busy]);
  const send=async(command:LimitCommand)=>{
    if(lock.current)return;lock.current=true;setBusy(true);setError("");
    try{localStorage.setItem(key,JSON.stringify(command));setPending(command);setConfirm(undefined);const value=await service.setProjectCallLimit(projectId,command);if(!projectBudgetAvailability(value).known||value.revision<=command.expected_revision)throw new Error("额度保存回执无效");try{localStorage.removeItem(key);}catch{/* Receipt is authoritative. */}if(alive.current){setSaved(value);setText(String(value.maximum_calls??""));setPending(undefined);setMessage("原修改请求已确认；显示服务器当前额度，可能包含随后其他人的修改。没有启动模型调用。");}}
    catch(e){if(alive.current)setError(`${(e as Error).message} 未确认的请求保留原命令，不会自动重试。`);}finally{lock.current=false;if(alive.current)setBusy(false);}
  };
  const reload=()=>{if((dirty||blocked)&&!window.confirm("放下这次编辑并读取最新额度？这不会撤销已到达服务器的请求。"))return;try{localStorage.removeItem(key);}catch(e){setError((e as Error).message);return;}setPending(undefined);setConfirm(undefined);setBlocked(false);setMessage("");setEpoch(v=>v+1);};
  return <section aria-label="项目调用额度"><h2>项目调用额度</h2><p>仅累计本项目对话任务及其已授权的数据集处理调用，包含失败和结果未知的预留调用。不是 token 或金额预算，也不代表全系统用量。其他工作流不在此额度范围内。</p><p>调整上限不会授权模型、图片外传或开始处理；各操作仍需单独确认。</p>{error&&<p role="alert">{error}</p>}{message&&<p role="status">{message}</p>}
    {saved?<p role="status">已预留 {saved.reserved_calls} 次 · {saved.maximum_calls===null?"尚未配置累计上限":`累计上限 ${saved.maximum_calls} 次`} · 当前读取快照</p>:!error&&<p role="status">读取项目额度…</p>}
    {pending?<><p>原请求：累计 {pending.maximum_calls} 次，基于 revision {pending.expected_revision}。刷新不会重新发送。</p><button disabled={busy||blocked} onClick={()=>void send(pending)}>核实原额度修改</button></>:<><label>累计调用次数上限<input aria-label="累计调用次数上限" inputMode="numeric" value={text} disabled={!saved||busy||blocked} onChange={e=>{setText(e.target.value);setMessage("");}}/></label><button disabled={!saved||busy||blocked||text===String(saved?.maximum_calls??"")} onClick={()=>{try{setConfirm({id:crypto.randomUUID(),expected_revision:saved!.revision,maximum_calls:parseLimitMaximum(text,saved!.reserved_calls)});setError("");}catch(e){setError((e as Error).message);}}}>检查额度修改…</button></>}
    <button disabled={busy} onClick={reload}>读取最新额度</button>
    {confirm&&<Dialog title="确认项目累计调用上限" onClose={()=>setConfirm(undefined)}><p>项目：{projectId}</p><p>累计上限设为 {confirm.maximum_calls} 次；已预留次数不会清零。不是美元预算，不保证这些调用免费。</p><p>只修改此项目额度；不启动、续跑或授权任何推理。</p><div className="actions"><button onClick={()=>setConfirm(undefined)}>取消</button><button disabled={busy} onClick={()=>void send(confirm)}>保存累计上限</button></div></Dialog>}
  </section>;
}
