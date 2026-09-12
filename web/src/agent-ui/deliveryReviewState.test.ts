import {expect,it} from "vitest";
import {resolveFormalReview,resolveFormalReviewPage,type FormalReviewEvidence,type FormalRunEvidence} from "./deliveryReviewState";

const review=(overrides:Partial<FormalReviewEvidence>={}):FormalReviewEvidence=>({
  image_id:"image",confirmation_current:false,review_decision:null,unresolved_objects:3,
  execution_status:"awaiting_review",execution_error:"3 Artifact(s) require human review",...overrides,
});
const run=(overrides:Partial<FormalRunEvidence>={}):FormalRunEvidence=>({
  image_id:"image",child_run_id:"run",run_status:"completed_with_review",
  status:"awaiting_review",error:"3 Artifact(s) require human review",...overrides,
});

it("treats completed-with-review diagnostics as review work rather than execution failure",()=>{
  expect(resolveFormalReview(review(),run())).toEqual({
    state:"unresolved",blocks_package:true,diagnostic:"3 Artifact(s) require human review",
  });
  expect(resolveFormalReview(review({confirmation_current:true,review_decision:"positive_complete",unresolved_objects:0}),run())).toEqual({
    state:"positive_complete",blocks_package:false,diagnostic:"3 Artifact(s) require human review",
  });
});

it("keeps genuine child failures visible and blocking",()=>{
  expect(resolveFormalReview(review({execution_status:"failed",execution_error:"provider failed"}),run({run_status:"failed",error:"provider failed"}))).toEqual({
    state:"failed",blocks_package:true,diagnostic:"provider failed",
  });
});

it("accepts an explicit current exclusion without pretending a missing child Run succeeded",()=>{
  expect(resolveFormalReview(review({confirmation_current:true,review_decision:"excluded",unresolved_objects:0,execution_status:"pending",execution_error:null}),run({child_run_id:null,run_status:null,status:"pending",error:null}))).toEqual({
    state:"excluded",blocks_package:false,diagnostic:null,
  });
});

it("rejects inconsistent receipts and duplicate formal image evidence",()=>{
  expect(resolveFormalReview(review({confirmation_current:true,review_decision:"positive_complete",unresolved_objects:1}),run())).toEqual({
    state:"unresolved",blocks_package:true,diagnostic:"整图决定与当前对象状态不一致",
  });
  expect(()=>resolveFormalReviewPage([review()],[run(),run()])).toThrow("重复图片");
});
