import type {ConversationMessageInput} from "../types";

export type IntakeSlot="dataset_scope"|"label_rules"|"training_target";
export type MainlineMessageKind="assistant_reply"|"clarification"|"operation"|"system_receipt";
export type MainlineMessage={
  id:string; task_id:string; sequence:number; kind:MainlineMessageKind;
  text:string; created_at:string; source:{kind:string;id:string};
};
export type MainlineStep={id:string;kind:string;title:string;status:"blocked"|"ready"|"awaiting_approval"|"running"|"waiting_for_human"|"completed"|"failed"|"outcome_unknown";detail?:string};
export type MainlineAction={id:string;kind:string;available:boolean;reason:string;requires_approval:boolean;scope_revision:string};
export type MainlineTaskView={
  project_id:string;project_owner_id:string;conversation_id:string;task_id:string;revision:string;
  intake:{missing_slots:IntakeSlot[];dataset_scope?:unknown;label_rules?:unknown;training_target?:unknown};
  messages:MainlineMessage[];steps:MainlineStep[];blockers:{code:string;message:string;setup_request_id?:string}[];
  actions:MainlineAction[];active_operation_ids:string[];review_work_item_id?:string;package_id?:string;
  completion:{status:"incomplete"|"package_ready";package_id?:string;download_url?:string};
};

/** Frozen at selection time. Display names and current canvas state are never identities. */
export type VisualSelection={
  project_id:string;conversation_id:string;task_id:string;project_schema_revision:string;
  image:{image_id:string;sha256:string};
  sample:{draft_id:string;draft_revision:number;sample_test_id:string};
  candidate:{candidate_id:string;source_artifact_id:string};
  annotation:{kind:"bounding_box"|"classification"|"semantic_mask"|"instance_mask";label?:string};
  result_revision:string;
};

export type CapabilitySetupRequest={
  id:string;project_id:string;task_id:string;task_revision:string;registry_revision:string;
  role:string;required_capabilities:string[];compatible_model_ids:string[];
  status:"required"|"ready"|"cancelled"|"stale";return_path:string;
};
export type MainlineDomainSeams={
  review?:{open:(workItemId:string)=>void;currentSelection:()=>VisualSelection|undefined};
  package?:{open:(packageId:string)=>void};
  capabilitySetup?:{open:(request:CapabilitySetupRequest)=>void};
};
export interface MainlineTaskService {read(project:string,conversation:string,task:string,signal:AbortSignal):Promise<MainlineTaskView>}

const present=(value:string)=>typeof value==="string"&&value.length>0;
export function assertVisualSelection(value:VisualSelection,project:string,task:string):VisualSelection{
  if(value.project_id!==project||value.task_id!==task||![value.conversation_id,value.project_schema_revision,value.image.image_id,value.image.sha256,value.sample.draft_id,value.sample.sample_test_id,value.candidate.candidate_id,value.candidate.source_artifact_id,value.result_revision].every(present)||!Number.isSafeInteger(value.sample.draft_revision)||value.sample.draft_revision<1)throw new Error("候选引用不完整或不属于当前任务；没有发送反馈");
  return structuredClone(value);
}
export function selectedMessage(id:string,text:string,value:VisualSelection):ConversationMessageInput{
  const frozen=assertVisualSelection(value,value.project_id,value.task_id);
  return {id,text,image:{image_id:frozen.image.image_id,sha256:frozen.image.sha256},reference:{scope:"sample_candidate",task_id:frozen.task_id,project_schema_revision:frozen.project_schema_revision,draft_id:frozen.sample.draft_id,draft_revision:frozen.sample.draft_revision,sample_test_id:frozen.sample.sample_test_id,candidate_id:frozen.candidate.candidate_id,source_artifact_id:frozen.candidate.source_artifact_id}};
}
export function taskIsComplete(view:MainlineTaskView){return view.completion.status==="package_ready"&&!!view.completion.package_id&&!!view.completion.download_url;}
