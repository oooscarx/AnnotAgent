import {expect,it} from "vitest";
import type {request} from "../api";
import {createContextArchiveService,parseContextArchive,type ContextArchive} from "./contextArchive";
const archive={format:"annotagent.context",version:1,archive_hash:"hash",payload:{records:[]}} as unknown as ContextArchive;
it("rejects unsupported or partial archive shapes",()=>{expect(()=>parseContextArchive('{"format":"other"}')).toThrow("归档 v1");expect(parseContextArchive(JSON.stringify(archive))).toEqual(archive);});
it("previews separately and retries exact import command without changing scope",async()=>{
  const calls:{path:string;init?:RequestInit}[]=[];const service=createContextArchiveService((async(path:string,init?:RequestInit)=>{calls.push({path,init});return {};}) as typeof request);
  await service.preview("p",archive);await service.import("p",archive,"preview","same-command");await service.import("p",archive,"preview","same-command");await service.recover("p","same-command");
  expect(calls[0].path).toBe("/api/projects/p/context-imports/preview");expect(calls[1]).toEqual(calls[2]);expect(JSON.parse(String(calls[1].init?.body))).toEqual({archive,preview_hash:"preview",command_id:"same-command",confirm_archive_only:true});expect(calls[3].init?.method).toBeUndefined();
});
it("lists every archived context page and rejects foreign ownership",async()=>{
  let pages=0;const service=createContextArchiveService((async()=>({items:[{project_id:"p"}],next_cursor:++pages===1?1:null})) as typeof request);expect(await service.list("p",new AbortController().signal)).toHaveLength(2);
  const wrong=createContextArchiveService((async()=>({items:[{project_id:"other"}],next_cursor:null})) as typeof request);await expect(wrong.list("p",new AbortController().signal)).rejects.toThrow("项目不匹配");
});
