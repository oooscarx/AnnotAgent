import { describe, expect, it } from "vitest";
import { imageClassTokens, replaceImageClassToken, sameImageClass, initialImageClassLocal, parseImageClassLocal, makeImageClassAnswer, imageClassOverlay, mergeImageClassReview, imageClassEditValue, imageClassLocalNeedsGuard, imageClassDisplayOrigin, sameImageClassInput, imageClassAnswerConflicts } from "./conversation-image-class";
import { sampleAnnotations } from "./sampleAnnotations";
import type { ImageClassReview } from "./conversation-image-class-api";
import type { SampleTestOutcomeRecord } from "./types";

const box: SampleTestOutcomeRecord = { id: "box", label: "cup", status: "needs_review", value: { kind: "bounding_box", rect: [0.1, 0.2, 0.3, 0.4] } };
const classification: SampleTestOutcomeRecord = { id: "classification", label: "display label is not a class", status: "needs_review", value: { kind: "classification", labels: ["cup", "indoor", "cupboard"] } };

describe("same-image class review semantics", () => {
  it("uses saved bbox labels and actual classification tokens, never display text or substring matches", () => {
    expect(imageClassTokens(box)).toEqual(["cup"]);
    expect(imageClassTokens(classification)).toEqual(["cup", "indoor", "cupboard"]);
    expect(sameImageClass(box, "cup")).toBe(true);
    expect(sameImageClass({ ...box, label: "cupboard" }, "cup")).toBe(false);
    expect(sameImageClass(classification, "cup")).toBe(true);
    expect(sameImageClass(classification, "display label is not a class")).toBe(false);
    expect(sameImageClass(classification, "cup, indoor, cupboard")).toBe(false);
  });
  it("replaces or excludes only the explicitly selected original classification token", () => {
    const original = ["cup", "indoor", "cupboard"];
    expect(replaceImageClassToken(original, "cup", "bottle")).toEqual(["bottle", "indoor", "cupboard"]);
    expect(replaceImageClassToken(original, "cup", null)).toEqual(["indoor", "cupboard"]);
    expect(replaceImageClassToken(["cup"], "cup", null)).toEqual([]);
    expect(original).toEqual(["cup", "indoor", "cupboard"]);
  });
  it("does not silently reselect a missing token or collapse another token during replacement", () => {
    expect(() => replaceImageClassToken(["indoor"], "cup", null)).toThrow();
    expect(() => replaceImageClassToken(["cup", "indoor"], "cup", "indoor")).toThrow();
    expect(() => replaceImageClassToken(["cup"], "cup", "")).toThrow();
    expect(replaceImageClassToken(["cup", "indoor"], "cup", "cup")).toEqual(["cup", "indoor"]);
  });
});

const review = { id: "review", task_id: "task", conversation_id: "conversation", status: "pending", scope_digest: "digest", scope: { kind: "bounding_box", target_label: "cup", image_id: "image", sample_test_id: "test", baseline_feedback: [], members: [{ source_artifact_id: "artifact", outcome: box }] }, answer: null, revisions: [], repair_draft_id: null } as unknown as ImageClassReview;
describe("frozen image-class batch editing",()=>{
  it("recognizes an exact saved batch through the Rust f32 bbox wire representation and reload",()=>{
    const local=initialImageClassLocal(review);
    const rect:[number,number,number,number]=[0.143456789,0.2,0.123456789,0.4];
    const action={action:"edit" as const,outcome_id:"box",source_artifact_id:"artifact",corrected_label:"cup",corrected_value:{kind:"bounding_box" as const,rect}};
    local.actions=[action];local.frozen=makeImageClassAnswer(review,local,"precise-command");
    // Literal decimals reproduce serde's shortest round-trip representation of f32.
    const wire={...local.frozen,actions:[{...action,corrected_value:{kind:"bounding_box" as const,rect:[0.14345679,0.2,0.12345679,0.4] as [number,number,number,number]}}]};
    expect(sameImageClassInput(local.frozen,wire)).toBe(true);
    const saved={...review,status:"applied" as const,answer:wire};
    const restored=parseImageClassLocal(JSON.stringify(local),saved)!;
    expect(restored.frozen).toEqual(local.frozen);
    expect(imageClassAnswerConflicts(restored,saved)).toBe(false);
    expect(imageClassLocalNeedsGuard(restored,saved)).toBe(false);
    expect(sameImageClassInput(local.frozen,{...wire,command_id:"another"})).toBe(false);
    expect(sameImageClassInput(local.frozen,{...wire,actions:[{...wire.actions[0],source_artifact_id:"another"}]})).toBe(false);
    expect(sameImageClassInput(local.frozen,{...wire,actions:[{...wire.actions[0],corrected_value:{kind:"bounding_box",rect:[0.14345679,0.2,0.1234568,0.4]}}]})).toBe(false);
    expect(sameImageClassInput({budget:0.123456789},{budget:0.12345679})).toBe(false);
    expect(rect).toEqual([0.143456789,0.2,0.123456789,0.4]);
  });
  it("validates bbox dimensions and summed bounds in Core f32 space before freezing",()=>{
    const local=initialImageClassLocal(review);
    const action={action:"edit" as const,outcome_id:"box",source_artifact_id:"artifact",corrected_label:"cup",corrected_value:{kind:"bounding_box" as const,rect:[0.1,0.2,0.3,0.4] as [number,number,number,number]}};
    for(const rect of [[0.1,0.2,1e-50,0.4],[0.8,0.2,0.2000003,0.4],[0.1,0.9,0.2,0.1000003]] as [number,number,number,number][]){
      const edited={...local,actions:[{...action,corrected_value:{kind:"bounding_box" as const,rect}}]};
      expect(parseImageClassLocal(JSON.stringify(edited),review)).toEqual(edited);
      expect(()=>makeImageClassAnswer(review,edited,"invalid")).toThrow();
      expect(edited.frozen).toBeUndefined();
    }
    local.actions=[{...action,corrected_value:{kind:"bounding_box",rect:[0.8,0.2,0.2000001,0.4]}}];
    expect(()=>makeImageClassAnswer(review,local,"epsilon-boundary")).not.toThrow();
  });
  it("retains exact local selection/actions/command on refresh, rejects a different scope",()=>{
    const local=initialImageClassLocal(review);
    local.actions[0]={action:"exclude",outcome_id:"box",source_artifact_id:"artifact"};
    local.frozen=makeImageClassAnswer(review,local,"command");
    expect(parseImageClassLocal(JSON.stringify(local),review)).toEqual(local);
    expect(parseImageClassLocal(JSON.stringify(local),{...review,scope_digest:"other"})).toBeUndefined();
    expect(makeImageClassAnswer(review,local,"new-command")).toEqual(local.frozen);
  });
  it("rejects omitted, duplicate, outside or wrong-Artifact actions before submission",()=>{
    const local=initialImageClassLocal(review), action=local.actions[0];
    for(const actions of [[],[action,action],[{...action,outcome_id:"outside"}],[{...action,source_artifact_id:"other"}]])expect(()=>makeImageClassAnswer(review,{...local,actions},"command")).toThrow();
  });
  it("keeps incomplete human fields through refresh without authorizing an invalid batch",()=>{
    const local=initialImageClassLocal(review);
    local.actions[0]={action:"edit",outcome_id:"box",source_artifact_id:"artifact",corrected_label:"",corrected_value:{kind:"bounding_box",rect:[0.1,0.2,0,0.4]}};
    expect(parseImageClassLocal(JSON.stringify(local),review)).toEqual(local);
    expect(()=>makeImageClassAnswer(review,local,"command")).toThrow();
  });
  it("edits the retained local geometry instead of replacing an incomplete field with baseline",()=>{
    const annotation=sampleAnnotations([box],"image","test")[0];
    const action={action:"edit" as const,outcome_id:"box",source_artifact_id:"artifact",corrected_label:"",corrected_value:{kind:"bounding_box" as const,rect:[0.9,0.2,0.3,0.4] as [number,number,number,number]}};
    expect(imageClassEditValue(annotation,action)).toMatchObject({label:"",value:action.corrected_value});
    expect(annotation.value).toEqual(box.value);
  });
  it("keeps a leave guard for local edits even when another tab has answered",()=>{
    const local=initialImageClassLocal(review);
    local.actions[0]={...local.actions[0],action:"exclude"};
    const answered={...review,status:"applied" as const,answer:makeImageClassAnswer(review,initialImageClassLocal(review),"other")};
    expect(imageClassLocalNeedsGuard(local,answered)).toBe(true);
    expect(imageClassDisplayOrigin(local,answered,false)).toBe("local");
    expect(imageClassDisplayOrigin(local,answered,true)).toBe("saved");
    expect(imageClassDisplayOrigin(initialImageClassLocal(answered),answered,false)).toBe("saved");
    expect(imageClassLocalNeedsGuard(initialImageClassLocal(answered),answered)).toBe(false);
  });
  it("keeps baseline geometry and cannot mutate another class during local editing",()=>{
    const original=[box,{...box,id:"outside",label:"bottle"}];
    const local=initialImageClassLocal(review);local.actions[0]={...local.actions[0],action:"exclude"};
    const overlay=imageClassOverlay(review.scope,local.actions,original);
    expect(overlay.annotations.map(item=>item.id)).toEqual(["outside"]);
    expect(overlay.excluded[0].annotation.id).toBe("box");
    expect(original[0]).toEqual(box);
  });
  it("never replaces saved batch completion or cancellation with a slow pending read",()=>{
    const applied={...review,status:"applied" as const,answer:makeImageClassAnswer(review,initialImageClassLocal(review),"saved"),repair_draft_id:"repair"};
    expect(mergeImageClassReview(applied,review)).toEqual(applied);
    expect(mergeImageClassReview({...review,status:"cancelled"},review).status).toBe("cancelled");
  });
});
