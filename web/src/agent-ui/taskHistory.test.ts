import {expect,it} from "vitest";
import {createTaskHistoryService} from "./taskHistory";
import type {request} from "../api";
const workspace={project_id:"p",conversation_id:"c",task:{input:{id:"t"}}};
it("reads all message pages without mutation and rejects stuck cursors",async()=>{
  const paths:string[]=[];const transport=(async(path:string)=>{paths.push(path);return path.endsWith("workspace")?workspace:{items:[{id:paths.length,task_id:"t",conversation_id:"c",message:{input:{text:"TEST"}}}],next_cursor:paths.length===2?"next":null};}) as typeof request;
  const result=await createTaskHistoryService(transport).read("p","c","t",new AbortController().signal);expect(result.messages).toHaveLength(2);expect(paths[2]).toContain("cursor=next");
  const stuck=(async(path:string)=>path.endsWith("workspace")?workspace:{items:[],next_cursor:"same"}) as typeof request;await expect(createTaskHistoryService(stuck).read("p","c","t",new AbortController().signal)).rejects.toThrow("分页未前进");
});
it("rejects wrong task ownership before displaying a trace",async()=>{const transport=(async()=>({...workspace,project_id:"other"})) as typeof request;await expect(createTaskHistoryService(transport).read("p","c","t",new AbortController().signal)).rejects.toThrow("归属不匹配");});
