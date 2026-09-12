import { expect, it } from "vitest";
import { modelEditorValue, runtimeOptionsForModel, validateModelEditor } from "./ModelProfileEditor";
import type { RegistryModelProfile } from "../types";
it("new model declarations preserve unknown prices and do not claim verified capabilities",()=>{
  const value=modelEditorValue();
  expect(value.pricing).toEqual({currency:"USD",source:"unknown"});
  expect(value.limits).toEqual({});
  expect(value.generation_defaults).toEqual({});
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
it("validates ModelLimits and GenerationDefaults independently of capability declarations",()=>{
  const value={
    ...modelEditorValue(),provider_id:"p",display_name:"Model",remote_model_id:"remote",
    limits:{context_tokens:32768,maximum_output_tokens:2048},
    generation_defaults:{maximum_output_tokens:1024,temperature:0.1,top_p:0.9,reasoning_mode:"medium"},
  };
  const options={model_profile_id:"model",model_profile_revision:7,status:"verified" as const,supported_reasoning_modes:["low","medium","high"],source:"provider_test"};
  expect(validateModelEditor(value,options)).toMatchObject({limits:{context_tokens:32768,maximum_output_tokens:2048},generation_defaults:{maximum_output_tokens:1024,reasoning_mode:"medium"}});
  expect(()=>validateModelEditor({...value,generation_defaults:{...value.generation_defaults,maximum_output_tokens:4096}},options)).toThrow("不能超过");
  expect(()=>validateModelEditor({...value,generation_defaults:{...value.generation_defaults,reasoning_mode:"ultra"}},options)).toThrow("已验证支持");
  expect(()=>validateModelEditor({...value,limits:{...value.limits,context_tokens:0}},options)).toThrow("大于零");
});
it("does not turn reasoning_controls into an invented mode list",()=>{
  const value={...modelEditorValue(),provider_id:"p",display_name:"Model",remote_model_id:"remote",protocol_features:{...modelEditorValue().protocol_features,reasoning_controls:true}};
  expect(validateModelEditor(value).generation_defaults.reasoning_mode).toBeUndefined();
});
it("ignores stale verified modes after a Model Profile revision changes",()=>{
  const model={id:"model",revision:7} as RegistryModelProfile;
  const stale={model_profile_id:"model",model_profile_revision:6,status:"verified" as const,supported_reasoning_modes:["medium"],source:"provider_test"};
  expect(runtimeOptionsForModel(model,stale)).toBeUndefined();
  expect(runtimeOptionsForModel(model,{...stale,model_profile_revision:7})).toEqual(expect.objectContaining({supported_reasoning_modes:["medium"]}));
});
