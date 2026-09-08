import type { ConversationBuilderConsent, ConversationBuilderPreview } from "./types";

export type BuilderRepairContext = { id:string;draft:string;kind:"human_request"|"image_class_review" };
export const builderConsent = (preview:ConversationBuilderPreview):ConversationBuilderConsent => ({selection:preview.selection,repair:preview.repair,image_class_repair:preview.image_class_repair,scope_hash:preview.scope_hash,previous_grant_id:preview.previous_grant_id,expires_at:preview.expires_at,allow_unknown_cost:true});
const object=(value:unknown):value is Record<string,unknown>=>value!==null && typeof value==="object" && !Array.isArray(value);
const hash=(value:unknown)=>typeof value==="string" && /^[a-f0-9]{64}$/i.test(value);
const same=(a:unknown,b:unknown)=>JSON.stringify(a,(_key,value)=>object(value)?Object.fromEntries(Object.entries(value).sort(([a],[b])=>a.localeCompare(b))):value)===JSON.stringify(b,(_key,value)=>object(value)?Object.fromEntries(Object.entries(value).sort(([a],[b])=>a.localeCompare(b))):value);

/** A local pending envelope is only an exact retry aid, never an authorization source. */
export function restoreBuilderPending(raw:string,schema:{id:string;revision:number},source?:BuilderRepairContext):{preview:ConversationBuilderPreview;consent:ConversationBuilderConsent}|undefined {
  try {
    if(raw.length>32768)return;
    const saved:unknown=JSON.parse(raw);
    if(!object(saved)||!object(saved.preview)||!object(saved.consent))return;
    const p=saved.preview,s=p.selection;
    if(!object(s)||s.schema_id!==schema.id||s.schema_revision!==schema.revision||typeof s.operation_id!=="string"||!s.operation_id||typeof s.model_id!=="string"||!s.model_id)return;
    if(!hash(p.scope_hash)||typeof p.expires_at!=="string"||!Number.isFinite(Date.parse(p.expires_at))||(p.previous_grant_id!==null&&typeof p.previous_grant_id!=="string"))return;
    if(!["model_name","remote_model","destination","data_scope","operation"].every(key=>typeof p[key]==="string"))return;
    if(!["maximum_builder_calls","maximum_calls","used_calls"].every(key=>Number.isSafeInteger(p[key])&&Number(p[key])>=0)||p.image_count!==0||p.estimated_cost!==null)return;
    if(source?.kind==="image_class_review") {
      const r=p.image_class_repair;
      if(s.image_class_review_id!==source.id||s.repair_request_id!=null||p.repair!=null||!object(r)||r.review_id!==source.id||r.draft_id!==source.draft||r.schema_id!==schema.id||r.schema_revision!==schema.revision||!Number.isSafeInteger(r.revision)||Number(r.revision)<1||!hash(r.content_hash)||!hash(r.scope_digest)||!hash(r.feedback_digest))return;
    } else if(source) {
      const r=p.repair;
      if(s.repair_request_id!==source.id||s.image_class_review_id!=null||p.image_class_repair!=null||!object(r)||r.request_id!==source.id||r.draft_id!==source.draft||!Number.isSafeInteger(r.revision)||Number(r.revision)<1||!hash(r.content_hash))return;
    } else if(s.repair_request_id!=null||s.image_class_review_id!=null||p.repair!=null||p.image_class_repair!=null)return;
    const preview=p as unknown as ConversationBuilderPreview;
    if(!same(saved.consent,builderConsent(preview)))return;
    return {preview,consent:saved.consent as unknown as ConversationBuilderConsent};
  } catch { return; }
}
