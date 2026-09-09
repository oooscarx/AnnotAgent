import {parseStopSelection,stopSelectionConflicts,isStopMessage} from "../conversation-control";
import type {StopRequestRecord} from "../conversation-stop-api";
export function ownedStopSelection(record:StopRequestRecord,conversation:string,message:string,raw:string|null){
  if(record.message.conversation_id!==conversation||record.message.input.id!==message||!isStopMessage(record.message.input))throw new Error("停止回执不属于当前会话与原请求");
  const pending=parseStopSelection(raw,message);
  if(raw&&raw!=="null"&&!pending)throw new Error("停止目标恢复记录无效，未替换目标");
  if(stopSelectionConflicts(pending,record))throw new Error("服务器已保存不同停止目标；未应用浏览器选择");
  return pending;
}
