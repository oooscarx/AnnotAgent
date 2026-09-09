import {expect,it} from "vitest";
import {calibrationRuns,calibrationNodes} from "./GeometryCalibration";
import type {FrozenWorkflowVersion} from "../types";
it("requires explicit bounded run references without guessing or duplicates",()=>{const id="12345678-1234-1234-1234-123456789abc";expect(calibrationRuns(`${id},\n${id}`)).toEqual([id]);for(const input of ["","latest","wrong owner","1"])expect(()=>calibrationRuns(input)).toThrow();});
it("does not offer classifiers or unfrozen legacy bindings as geometry calibration nodes",()=>{
  const node={id:"geometry",model_profile_binding:{model_profile_id:"model",locked:true},outputs:[{artifact_type:"detection_set"}]};
  const source={draft:{nodes:[node,{...node,id:"classifier",outputs:[{artifact_type:"classification_set"}]},{...node,id:"missing",model_profile_binding:{model_profile_id:"absent"}},{...node,id:"legacy",model_profile_binding:undefined,model_binding:"default-vision"}]},snapshot:{model_profiles:[{model_profile_id:"model"}]}} as unknown as FrozenWorkflowVersion;
  expect(calibrationNodes(source).map(n=>n.id)).toEqual(["geometry"]);
});
