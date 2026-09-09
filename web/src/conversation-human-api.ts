import type { SampleFeedbackRevision } from "./types";

export type HumanRequest = {
  authorized_journey_id?:string;
  journey_resume?:{consent_id:string;error?:string;status?:unknown};
  input: {id:string;task_id:string;conversation_id:string;sample_test_id:string;image_id:string;content_hash:string;outcome_id?:string;addition_id?:string;expected_feedback_sequence:number;reason_code:string;question:string;resume_checkpoint_ref:string};
  status:"pending"|"answered"|"applied"|"cancelled"|"stale";
  answer?:SampleFeedbackRevision|null;
  resume_draft_id?:string|null;
  resume_error?:string|null;
  deferred?:boolean;
  deferral_revision?:number;
};
