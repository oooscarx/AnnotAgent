import {expect,it} from "vitest";
import {readPendingDelivery,rememberPendingDelivery,clearPendingDelivery} from "./pendingDelivery";
const command={command_id:"same-command",intent_revision:3,intent_sha256:"frozen",image_reviews:{one:2,two:5},confirmed:true};
function memory(){const data=new Map<string,string>();return {data,getItem:(key:string)=>data.get(key)??null,setItem:(key:string,value:string)=>{data.set(key,value);},removeItem:(key:string)=>{data.delete(key);}};}
it("persists an exact pending scope across reloads without starting an operation",()=>{
  const storage=memory();rememberPendingDelivery(storage,"workspace/project/task",command);
  expect(readPendingDelivery(storage,"workspace/project/task")).toEqual(command);
  expect(readPendingDelivery(storage,"other/project/task")).toBeUndefined();
  rememberPendingDelivery(storage,"workspace/project/task",command);
  expect(()=>rememberPendingDelivery(storage,"workspace/project/task",{...command,command_id:"another"})).toThrow("待核实");
  expect(()=>rememberPendingDelivery(storage,"workspace/project/task",{...command,intent_revision:4})).toThrow("待核实");
  clearPendingDelivery(storage,"workspace/project/task","unrelated");expect(storage.data.size).toBe(1);
  clearPendingDelivery(storage,"workspace/project/task",command.command_id);expect(storage.data.size).toBe(0);
});
it("storage failures and corrupt untrusted recovery bodies cannot silently regenerate commands",()=>{
  const storage=memory();storage.setItem("pending","bad json");
  expect(()=>readPendingDelivery(storage,"pending")).toThrow("损坏");
  for(const input of [{...command,confirmed:false},{...command,image_reviews:{one:0}},{...command,extra:"not part of the frozen command"}]){
    storage.setItem("pending",JSON.stringify(input));expect(()=>readPendingDelivery(storage,"pending")).toThrow("无效");
  }
  expect(()=>rememberPendingDelivery({getItem:()=>null,setItem:()=>{throw new Error("TEST quota");}},"pending",command)).toThrow("quota");
});
