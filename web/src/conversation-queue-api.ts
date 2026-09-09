import {request} from "./api";
import type {ConversationCallReceipt} from "./types";
export type QueueConsent={call_id:string;model_id:string;scope_hash:string;request_hash:string;previous_grant_id:string|null;maximum_calls:number;expires_at:string;allow_unknown_cost:boolean};
export type QueuePreview=Omit<QueueConsent,"call_id"|"allow_unknown_cost"> & {model_name:string;destination:string;used_calls:number;new_request_limit:number;image_count:number;estimated_cost:null;data_scope:string;operation:string};
const path=(project:string,conversation:string,task:string,message:string)=>`/api/projects/${encodeURIComponent(project)}/conversations/${encodeURIComponent(conversation)}/tasks/${encodeURIComponent(task)}/message-queue/${encodeURIComponent(message)}`;
export const queuePlanningApi={
  preview:(p:string,c:string,t:string,m:string,signal?:AbortSignal)=>request<QueuePreview>(`${path(p,c,t,m)}/schema-preview`,{signal}),
  authorization:(p:string,c:string,t:string,m:string,signal?:AbortSignal)=>request<QueueConsent|null>(`${path(p,c,t,m)}/schema-authorization`,{signal}),
  propose:(p:string,c:string,t:string,m:string,consent:QueueConsent)=>request<ConversationCallReceipt>(`${path(p,c,t,m)}/schema-proposals`,{method:"POST",body:JSON.stringify(consent)}),
  receipt:(p:string,c:string,t:string,call:string,signal?:AbortSignal)=>request<ConversationCallReceipt>(`/api/projects/${encodeURIComponent(p)}/conversations/${encodeURIComponent(c)}/tasks/${encodeURIComponent(t)}/calls/${encodeURIComponent(call)}`,{signal}),
};
export function parseQueueConsent(raw:string|null):QueueConsent|undefined {
  if(!raw||raw.length>4096)return;
  try{const v=JSON.parse(raw),uuid=(x:unknown)=>typeof x==="string"&&/^[a-f\d]{8}(-[a-f\d]{4}){3}-[a-f\d]{12}$/i.test(x),hash=(x:unknown)=>typeof x==="string"&&/^[a-f\d]{64}$/i.test(x);
    if(Object.keys(v).some(key=>!["call_id","model_id","scope_hash","request_hash","previous_grant_id","maximum_calls","expires_at","allow_unknown_cost"].includes(key)))return;
    if(uuid(v.call_id)&&uuid(v.model_id)&&hash(v.scope_hash)&&hash(v.request_hash)&&(v.previous_grant_id===null||uuid(v.previous_grant_id))&&Number.isSafeInteger(v.maximum_calls)&&v.maximum_calls>0&&v.maximum_calls<=0xffff_ffff&&typeof v.expires_at==="string"&&Number.isFinite(Date.parse(v.expires_at))&&v.allow_unknown_cost===true)return v;
  }catch{/* Invalid browser data grants nothing. */}
}
