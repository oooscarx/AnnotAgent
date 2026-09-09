import { expect, it } from "vitest";
import { modelEditorValue, validateModelEditor } from "./ModelProfileEditor";
it("new model declarations preserve unknown prices and do not claim verified capabilities",()=>{
  const value=modelEditorValue();
  expect(value.pricing).toEqual({currency:"USD",source:"unknown"});
  expect("status" in value).toBe(false);
  expect(Object.values(value.protocol_features).every(v=>!v)).toBe(true);
  expect(()=>validateModelEditor(value)).toThrow("Provider");
});
it("validates declaration inputs and uses exact decimal price strings",()=>{
  const value={...modelEditorValue(),provider_id:"p",display_name:" Model ",remote_model_id:" remote "};
  expect(validateModelEditor(value).remote_model_id).toBe("remote");
  expect(()=>validateModelEditor(JSON.parse(JSON.stringify({...value,pricing:{...value.pricing,per_request:null}})))) .not.toThrow();
  expect(validateModelEditor({...value,pricing:{...value.pricing,per_request:"0.000001"}}).pricing.per_request).toBe("0.000001");
  for(const bad of ["-1","NaN","Infinity","1e3",""])expect(()=>validateModelEditor({...value,pricing:{...value.pricing,per_request:bad}})).toThrow("价格");
  expect(()=>validateModelEditor({...value,task_capabilities:[]})).toThrow("至少");
});
