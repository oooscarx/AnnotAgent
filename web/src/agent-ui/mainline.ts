import type {Annotation,ConversationFormalReference,ConversationMessageInput} from "../types";
import type {DeliverySampleResult} from "./deliveryVisualSelection";

export type IntakeSlot="dataset_scope"|"label_spec"|"training_target";
export type MainlineMessageKind="assistant_reply"|"clarification"|"operation"|"system_receipt";
export type MainlineMessage={
  id:string; task_id:string; sequence:number; kind:MainlineMessageKind;
  text:string; created_at:string; source:{kind:string;id:string};
};
export type MainlineStep={id:string;kind:string;title:string;status:"blocked"|"ready"|"awaiting_approval"|"running"|"waiting_for_human"|"completed"|"failed"|"outcome_unknown";detail?:string};
export type MainlineAction={id:string;state:"authorized"|"available"|"requires_confirmation"|"blocked";method:"GET"|"POST";url:string;execution_method?:"POST";execution_url?:string;requires_confirmation:boolean;reason:string|null;scope?:unknown;failure?:{stage?:string;category?:string;message?:string}|null};
export type MainlinePublicAction={id:string;kind:string;available:boolean;reason:string;requires_approval:boolean;scope_revision:string;method:"GET"|"POST";url:string};
export type MainlineTaskView={
  contract_version:"mainline-task-v1";project_id:string;project_owner_id:string;conversation_id:string;task_id:string;read_model_revision:string;
  revision?:string;intake?:{missing_slots:IntakeSlot[];dataset_scope:unknown;label_rules:unknown;training_target:unknown};
  delivery:unknown;schema:unknown;
  review_summary:{selected_images:number;saved_review_receipts:number;current_reviews:number;pending_reviews:number};
  package:{consents:unknown[];jobs:{id:string;phase:string;intent_revision:number;snapshot_sha256:string;error?:string|null}[]};
  available_actions:MainlineAction[];actions?:MainlinePublicAction[];blockers:(string|{code:string;message:string})[];
  completion:{model_request_completed:boolean;processing_completed:boolean;package_ready:boolean;task_completed:boolean;status?:"incomplete"|"package_ready";package_id?:string;download_url?:string};
  messages?:MainlineMessage[];steps?:MainlineStep[];active_operation_ids?:string[];review_work_item_id?:string;package_id?:string;formal_source?:unknown;capability_readiness?:unknown;
  links?:{self:string;thread:string;visual_selections:string;capability_readiness:string;review_work_items:string;package_consents:string;advance:string};
};
export type MainlineAdvanceInput={command_id:string;expected_read_model_revision:string;action_id:string};
export type MainlineAdvanceReceipt={command_id:string;action_id:string;replayed:boolean;result:unknown;workspace:MainlineTaskView};

/** Frozen at selection time. Display names and current canvas state are never identities. */
export type SampleVisualSelection={
  project_id:string;conversation_id:string;task_id:string;project_schema_revision:string;
  image:{image_id:string;sha256:string};
  sample:{draft_id:string;draft_revision:number;sample_test_id:string};
  candidate:{candidate_id:string;source_artifact_id:string};
  annotation:{kind:"bounding_box"|"classification"|"semantic_mask"|"instance_mask";label?:string};
  result_revision:string;
};
export type FormalVisualSelection={
  project_id:string;conversation_id:string;task_id:string;project_schema_revision:string;
  image:{image_id:string;sha256:string};reference:ConversationFormalReference;
  annotation:{kind:Annotation["value"]["kind"];label?:string};result_revision:string;
};
export type VisualSelection=SampleVisualSelection|FormalVisualSelection;

export type CapabilitySetupRequest={
  id:string;project_id:string;task_id:string;task_revision:string;registry_revision:string;
  role:string;required_capabilities:string[];compatible_model_ids:string[];
  status:"required"|"ready"|"cancelled"|"stale";return_path:string;
};
export type CanonicalVisualSelectionItem={
  project_id:string;conversation_id:string;task_id:string;project_schema_revision:string;
  draft_id:string;draft_revision:number;sample_test_id:string;sample_status:string;operation_status:string;result_available:boolean;
  images:{image_id:string;image_sha256:string;result_revision:string;candidates:{candidate_id:string;annotation_kind:string|null;label:string;source_artifact_id:string|null;feedback_available:boolean;selection:SampleVisualSelection|null}[]}[];
};
export type CanonicalVisualSelectionPage={project_id:string;conversation_id:string;task_id:string;items:CanonicalVisualSelectionItem[];next_cursor:string|null};
export type MainlineDomainSeams={
  review?:{open:(workItemId:string)=>void;currentSelection:()=>VisualSelection|undefined};
  package?:{open:(packageId:string)=>void};
  capabilitySetup?:{open:(request:CapabilitySetupRequest)=>void};
};
export interface MainlineTaskService {
  read(project:string,conversation:string,task:string,signal?:AbortSignal):Promise<MainlineTaskView>;
  advance(project:string,conversation:string,task:string,input:MainlineAdvanceInput):Promise<MainlineAdvanceReceipt>;
}

const present=(value:string)=>typeof value==="string"&&value.length>0;
export function assertVisualSelection(value:VisualSelection,project:string,task:string):VisualSelection{
  const base=value.project_id===project&&value.task_id===task&&[value.conversation_id,value.project_schema_revision,value.image.image_id,value.image.sha256,value.result_revision].every(present);
  if(!base)throw new Error("对象引用不完整或不属于当前任务；没有发送反馈");
  if("sample" in value){
    if(![value.sample.draft_id,value.sample.sample_test_id,value.candidate.candidate_id,value.candidate.source_artifact_id].every(present)||!Number.isSafeInteger(value.sample.draft_revision)||value.sample.draft_revision<1)throw new Error("样例候选引用不完整；没有发送反馈");
  }else{
    const reference=value.reference;
    if(reference.scope!=="formal_annotation"||reference.task_id!==task||reference.project_schema_revision!==value.project_schema_revision||![reference.intent_sha256,reference.processing_operation_id,reference.batch_id,reference.source_run_id,reference.annotation_id,reference.annotation_revision_id,reference.expected_snapshot_sha256].every(present)||!Number.isSafeInteger(reference.intent_revision)||reference.intent_revision<1)throw new Error("正式标注引用不完整；没有发送反馈");
  }
  return structuredClone(value);
}
export function selectedMessage(id:string,text:string,value:VisualSelection):ConversationMessageInput{
  const frozen=assertVisualSelection(value,value.project_id,value.task_id);
  if("reference" in frozen)return {id,text,image:{image_id:frozen.image.image_id,sha256:frozen.image.sha256},reference:frozen.reference};
  return {id,text,image:{image_id:frozen.image.image_id,sha256:frozen.image.sha256},reference:{scope:"sample_candidate",task_id:frozen.task_id,project_schema_revision:frozen.project_schema_revision,draft_id:frozen.sample.draft_id,draft_revision:frozen.sample.draft_revision,sample_test_id:frozen.sample.sample_test_id,candidate_id:frozen.candidate.candidate_id,source_artifact_id:frozen.candidate.source_artifact_id}};
}

export function formalVisualSelectionFromCanonical(input:{project_id:string;conversation_id:string;task_id:string;image_id:string;image_sha256:string;annotation:{annotation_id:string;label:string|null;value:Annotation["value"];annotation_revision_id:string|null;feedback_available:boolean;conversation_reference:ConversationFormalReference|null}}):FormalVisualSelection{
  const {annotation,project_id,conversation_id,task_id,image_id,image_sha256}=input;
  const reference=annotation.conversation_reference;
  if(!annotation.feedback_available||!reference||reference.annotation_id!==annotation.annotation_id||reference.annotation_revision_id!==annotation.annotation_revision_id||reference.task_id!==task_id)throw new Error("正式标注没有可验证的当前会话引用");
  return assertVisualSelection({project_id,conversation_id,task_id,project_schema_revision:reference.project_schema_revision,image:{image_id,sha256:image_sha256},reference,annotation:{kind:annotation.value.kind,...(annotation.label?{label:annotation.label}:{})},result_revision:reference.annotation_revision_id},project_id,task_id) as FormalVisualSelection;
}
export function taskIsComplete(view:MainlineTaskView){return view.completion.task_completed&&view.completion.package_ready&&view.package.jobs.some(job=>job.phase==="ready");}

/** Geometry remains in Sample Test results; all reference identities come from this canonical read model. */
export function deliverySampleResultFromCanonical(
  item:CanonicalVisualSelectionItem,
  annotationsByImage:Record<string,Annotation[]>,
):DeliverySampleResult{
  const supported=new Set(["bounding_box","classification","semantic_mask","instance_mask"]);
  const images=item.images.map(image=>{
    const seen=new Set<string>();
    const candidates:{candidate_id:string;selection:SampleVisualSelection|null}[]=[];
    for(const candidate of image.candidates){
      if(seen.has(candidate.candidate_id))throw new Error("Canonical SampleCandidate identity is duplicated; no reference was created");
      seen.add(candidate.candidate_id);
      const referenceAvailable=candidate.feedback_available&&candidate.source_artifact_id&&candidate.annotation_kind&&supported.has(candidate.annotation_kind);
      if(referenceAvailable&&!candidate.selection)throw new Error("Canonical SampleCandidate is missing its server-issued selection");
      const selection=referenceAvailable
        ? assertVisualSelection(candidate.selection!,item.project_id,item.task_id) as SampleVisualSelection
        : null;
      if(selection&&(selection.candidate.candidate_id!==candidate.candidate_id||selection.candidate.source_artifact_id!==candidate.source_artifact_id))throw new Error("Canonical SampleCandidate selection does not match its candidate");
      candidates.push({candidate_id:candidate.candidate_id,selection:selection?structuredClone(selection):null});
    }
    return {image_id:image.image_id,image_sha256:image.image_sha256,result_revision:image.result_revision,candidates,annotations:annotationsByImage[image.image_id]||[]};
  });
  return {project_id:item.project_id,conversation_id:item.conversation_id,task_id:item.task_id,project_schema_revision:item.project_schema_revision,draft_id:item.draft_id,draft_revision:item.draft_revision,sample_test_id:item.sample_test_id,images};
}
