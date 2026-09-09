import {request} from "../api";
export type ContextArchive={format:"annotagent.context";version:1;archive_hash:string;payload:{source:{project_id:string;conversation_id:string};captured_at:string;records:{kind:string;id:string;data:unknown}[];resources:unknown[];integrity:Record<string,unknown>}};
export type ArchiveReceipt={command_id:string;context_id:string;project_id:string;archive_hash:string;mode:"archive_only";created_at:string;continuation:{can_resume:false;reasons:string[]}};
export type ArchivePreview={valid:true;preview_hash:string;archive_hash:string;target_project_id:string;mode:"archive_only";counts:unknown;integrity:unknown;continuation:unknown};
export type LoadedContext={receipt:ArchiveReceipt;archive:ContextArchive;objects:unknown[];trust:"untrusted_import"};
export function createContextArchiveService(transport:typeof request=request){
  const root=(p:string)=>`/api/projects/${encodeURIComponent(p)}`;
  const post=<T,>(path:string,body:unknown)=>transport<T>(path,{method:"POST",body:JSON.stringify(body)});
  return {
    export:(p:string,c:string)=>transport<ContextArchive>(`${root(p)}/conversations/${encodeURIComponent(c)}/context-archive`),
    preview:(p:string,archive:ContextArchive)=>post<ArchivePreview>(`${root(p)}/context-imports/preview`,{archive}),
    import:(p:string,archive:ContextArchive,preview_hash:string,command_id:string)=>post<ArchiveReceipt>(`${root(p)}/context-imports`,{archive,preview_hash,command_id,confirm_archive_only:true}),
    recover:(p:string,command:string,signal?:AbortSignal)=>transport<ArchiveReceipt>(`${root(p)}/context-imports/${encodeURIComponent(command)}`,{signal}),
    load:(p:string,id:string,signal?:AbortSignal)=>transport<LoadedContext>(`${root(p)}/archived-contexts/${encodeURIComponent(id)}`,{signal}),
    list:async(p:string,signal:AbortSignal)=>{const items:ArchiveReceipt[]=[];const seen=new Set<number>();let after=0;for(;;){const page=await transport<{items:ArchiveReceipt[];next_cursor:number|null}>(`${root(p)}/archived-contexts?after=${after}&limit=50`,{signal});if(page.items.some(r=>r.project_id!==p))throw new Error("归档项目不匹配");items.push(...page.items);if(page.next_cursor===null)return items;if(seen.has(page.next_cursor))throw new Error("归档分页未前进");seen.add(page.next_cursor);after=page.next_cursor;}},
  };
}
export type ContextArchiveService=ReturnType<typeof createContextArchiveService>;
export function parseContextArchive(text:string):ContextArchive{const value=JSON.parse(text) as ContextArchive;if(value?.format!=="annotagent.context"||value.version!==1||!Array.isArray(value.payload?.records)||!value.archive_hash)throw new Error("不是支持的 AnnotAgent 会话归档 v1");return value;}
export function downloadContextArchive(archive:ContextArchive){const url=URL.createObjectURL(new Blob([JSON.stringify(archive,null,2)],{type:"application/json"}));const a=document.createElement("a");a.href=url;a.download=`annotagent-context-${archive.archive_hash.slice(0,12)}.json`;a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);}
