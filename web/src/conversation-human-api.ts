import type { SampleFeedbackRevision } from "./types";

export type HumanRequest = {
  input: {id:string;task_id:string;conversation_id:string;sample_test_id:string;image_id:string;content_hash:string;outcome_id:string;expected_feedback_sequence:number;reason_code:string;question:string;resume_checkpoint_ref:string};
  status:"pending"|"answered"|"applied"|"cancelled"|"stale";
  answer?:SampleFeedbackRevision|null;
};
