import { expect, it } from "vitest";
import { validateImpact } from "./LifecycleOperation";
import type { ManagementPreview, ManagementRequest } from "../types";
const request:ManagementRequest={project_id:"p",action:"restore",objects:[{kind:"workflow_draft",id:"d",expected_revision:2}],idempotency_key:"client-key"};
const preview={project_id:"p",action:"restore",objects:request.objects} as ManagementPreview;
it("requires the exact project, action, object IDs and revisions in impact previews",()=>{
  expect(validateImpact(request,preview)).toBe(preview);
  for(const wrong of [{...preview,project_id:"other"},{...preview,action:"purge" as const},{...preview,objects:[{...request.objects[0],expected_revision:3}]},{...preview,objects:[]}])expect(()=>validateImpact(request,wrong)).toThrow("不匹配");
});
