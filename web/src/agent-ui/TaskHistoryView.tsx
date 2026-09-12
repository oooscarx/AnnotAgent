import {useEffect,useState} from "react";
import type {Task} from "./adapter";
import type {TaskHistoryService} from "./taskHistory";
import {Disclosure} from "./Disclosure";
import {ContextArchiveView} from "./ContextArchiveView";
import {TraceRecord} from "./TraceRecord";
export function TaskHistory({projectId,tasks,service}:{projectId:string;tasks:Task[];service:TaskHistoryService}){
  const initial=new URL(location.href).searchParams;
  const [search,setSearch]=useState("");
  const [selected,setSelected]=useState(()=>initial.get("task")||"");
  const [view,setView]=useState<"trace"|"save"|"load">(()=>{
    const value=initial.get("view");return value==="save"||value==="load"?value:"trace";
  });
  const rows=tasks.filter(t=>t.project===projectId&&!t.id.startsWith("new:"));const current=rows.find(t=>t.id===selected);
  const choose=(id:string)=>{setSelected(id);const u=new URL(location.href);u.searchParams.set("task",id);history.pushState(null,"",u);};
  useEffect(()=>{const change=()=>{const params=new URL(location.href).searchParams;setSelected(params.get("task")||"");const value=params.get("view");setView(value==="save"||value==="load"?value:"trace");};window.addEventListener("popstate",change);return()=>window.removeEventListener("popstate",change);},[]);
  const switchView=(next:typeof view)=>{setView(next);const u=new URL(location.href);u.searchParams.set("view",next);history.pushState(null,"",u);};
  const conversations=[...new Set(rows.flatMap(t=>t.conversationId?[t.conversationId]:[]))];
  return <section className="native-project-manager native-task-history" aria-label="Agent 任务历史">
    <h1>Agent 任务历史</h1>
    <p>查看多轮输入、模型调用、方案构建、样例测试和人工协助的实际服务器记录。查看、保存和加载都不会执行任务。</p>
    <nav className="native-management-tabs" aria-label="任务历史操作">
      <button aria-pressed={view==="trace"} onClick={()=>switchView("trace")}>执行轨迹</button>
      <button aria-pressed={view==="save"} onClick={()=>switchView("save")}>保存会话 JSON</button>
      <button aria-pressed={view==="load"} onClick={()=>switchView("load")}>加载会话 JSON</button>
    </nav>
    {view==="save"||view==="load"?<>
      <p role="status">{view==="save"?"选择下方保存按钮下载完整的已持久化会话。":"选择 JSON 文件后先预览；确认加载只创建只读历史，不恢复授权或执行。"}</p>
      <ContextArchiveView key={`${projectId}:${view}`} project={projectId} conversations={conversations} service={service.archives}/>
    </>:<>
      <label>搜索历史任务<input aria-label="搜索历史任务" value={search} onChange={e=>setSearch(e.target.value)}/></label>
      <p>{rows.length} 个已保存任务</p>
      <div className="settings-rows">{rows.filter(t=>`${t.title} ${t.id}`.toLowerCase().includes(search.toLowerCase())).map(t=><div className="settings-row" key={t.id}><div><strong>{t.title}</strong><p>{t.phase}</p></div><div className="actions"><button aria-pressed={selected===t.id} onClick={()=>choose(t.id)}>查看轨迹</button><a href={`/projects/${encodeURIComponent(projectId)}/work?task=${encodeURIComponent(t.id)}`}>打开原任务</a></div></div>)}</div>
      {selected&&!current&&<p role="alert">此任务不在当前项目已加载历史中，未打开其他任务。</p>}
      {current?.conversationId&&<Trace key={`${projectId}:${current.id}`} task={current} service={service}/>}
    </>}
  </section>;
}
function Trace({task,service}:{task:Task;service:TaskHistoryService}){
  const [data,setData]=useState<Awaited<ReturnType<TaskHistoryService["read"]>>>();const [error,setError]=useState("");const [refresh,setRefresh]=useState(0);
  useEffect(()=>{const c=new AbortController();setData(undefined);setError("");void service.read(task.project,task.conversationId!,task.id,c.signal).then(v=>{if(!c.signal.aborted)setData(v);}).catch(e=>{if(!c.signal.aborted)setError(e.message);});return()=>c.abort();},[task.project,task.conversationId,task.id,service,refresh]);
  const groups=data?[{title:"模型调用与决策回执",rows:data.workspace.calls||[]},{title:"方案构建与工具轨迹",rows:data.workspace.builder_operations?.items||[]},{title:"样例执行",rows:data.workspace.sample_operations||[]},{title:"全量处理",rows:data.workspace.processing_operations||[]},{title:"人工协助",rows:data.workspace.human_requests||[]},{title:"排队输入状态",rows:data.workspace.queue||[]}]:[];
  return <section aria-label="任务执行轨迹"><h2>{task.title}</h2><button onClick={()=>setRefresh(n=>n+1)}>刷新服务器轨迹</button>{error&&<p role="alert">{error}</p>}{!data&&!error&&<p role="status">读取完整分页消息与任务快照…</p>}{data&&<><p>这是可观察的输入、操作及结果，不是模型的隐藏思维链。不同类别记录不伪造为统一时间顺序。</p><h3>多轮对话 · {data.messages.length}</h3><ol>{data.messages.map(m=><li key={m.id}><p style={{whiteSpace:"pre-wrap"}}>{m.message.input.text}</p>{!!m.message.input.reference&&<Disclosure title="消息引用"><pre>{JSON.stringify(m.message.input.reference,null,2)}</pre></Disclosure>}</li>)}</ol>{groups.map(g=><Disclosure key={g.title} title={`${g.title} · ${g.rows.length}`}>{g.rows.length?g.rows.map((r,i)=><TraceRecord key={i} value={r} index={i}/>):<p>没有保存的记录。</p>}</Disclosure>)}<Disclosure title="完整任务工作快照（只读）"><pre>{JSON.stringify(data.workspace,null,2)}</pre></Disclosure></>}</section>;
}
