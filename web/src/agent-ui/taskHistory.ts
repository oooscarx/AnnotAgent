import {request} from "../api";
import {createContextArchiveService,type ContextArchiveService} from "./contextArchive";
export type TaskTrace={project_id:string;conversation_id:string;task:{input:{id:string}};calls?:unknown[];builder_operations?:{items:unknown[]};sample_operations?:unknown[];processing_operations?:unknown[];human_requests?:unknown[];queue?:unknown[];[key:string]:unknown};
export type TraceMessage={id:string;task_id:string;conversation_id:string;message:{sequence?:number;input:{text:string;reference?:unknown}}};
export type TaskHistoryService={archives:ContextArchiveService;read:(project:string,conversation:string,task:string,signal:AbortSignal)=>Promise<{workspace:TaskTrace;messages:TraceMessage[]}>};
export function createTaskHistoryService(transport:typeof request=request):TaskHistoryService{return {archives:createContextArchiveService(transport),async read(project,conversation,task,signal){
  const root=`/api/projects/${encodeURIComponent(project)}/conversations/${encodeURIComponent(conversation)}/tasks/${encodeURIComponent(task)}`;
  const workspace=await transport<TaskTrace>(`${root}/workspace`,{signal});if(workspace.project_id!==project||workspace.conversation_id!==conversation||workspace.task.input.id!==task)throw new Error("任务上下文归属不匹配");
  const messages:TraceMessage[]=[];const cursors=new Set<string>();let cursor:string|number|null=null;
  do{const page:{items:TraceMessage[];next_cursor:string|number|null}=await transport(`${root}/thread?limit=100${cursor===null?"":`&cursor=${encodeURIComponent(cursor)}`}`,{signal});if(page.items.some(m=>m.task_id!==task||m.conversation_id!==conversation))throw new Error("轨迹消息归属不匹配");messages.push(...page.items);cursor=page.next_cursor;if(cursor!==null){if(cursors.has(String(cursor)))throw new Error("历史分页未前进，未将不完整历史当作完整上下文");cursors.add(String(cursor));}}while(cursor!==null);
  return {workspace,messages};
}};}
export const taskHistoryApi=createTaskHistoryService();
