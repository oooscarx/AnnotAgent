import type {DeliveryPackageInput} from "./deliveryService";

/** Browser intent recovery only; never authoritative authorization or proof of admission. */
export function readPendingDelivery(storage: Pick<Storage,"getItem">, key:string):DeliveryPackageInput|undefined {
  const raw=storage.getItem(key);if(raw===null)return;
  let value:unknown;
  try{value=JSON.parse(raw);}catch{throw new Error("打包请求恢复记录损坏；不会重新生成命令或自动发送。");}
  const p=value as Partial<DeliveryPackageInput>|null;
  if(!p||typeof p!=="object"||typeof p.command_id!=="string"||!p.command_id||!Number.isSafeInteger(p.intent_revision)||(p.intent_revision||0)<1||typeof p.intent_sha256!=="string"||!p.intent_sha256||p.confirmed!==true||!p.image_reviews||typeof p.image_reviews!=="object"||Array.isArray(p.image_reviews)||Object.entries(p.image_reviews).some(([id,revision])=>!id||!Number.isSafeInteger(revision)||revision<1)||Object.keys(p).some(k=>!["command_id","intent_revision","intent_sha256","image_reviews","confirmed"].includes(k)))throw new Error("打包请求恢复记录无效；不会扩大范围或创建替代命令。");
  return p as DeliveryPackageInput;
}

export function rememberPendingDelivery(storage:Pick<Storage,"getItem"|"setItem">,key:string,input:DeliveryPackageInput) {
  const existing=readPendingDelivery(storage,key);
  if(existing&&JSON.stringify(existing)!==JSON.stringify(input))throw new Error("此任务还有待核实的打包请求，请先读取其服务器状态或按原请求重试。");
  storage.setItem(key,JSON.stringify(input));
  if(JSON.stringify(readPendingDelivery(storage,key))!==JSON.stringify(input))throw new Error("无法持久保存打包命令；没有发送请求。");
}

export function clearPendingDelivery(storage:Pick<Storage,"getItem"|"removeItem">,key:string,id:string) {
  if(readPendingDelivery(storage,key)?.command_id===id)storage.removeItem(key);
}
