import { useState } from "react";
import type { InputModality, ModelCapability, ProviderProfile, RegistryModelProfile } from "../types";

export const modelCapabilities: [ModelCapability, string][] = [
  ["text_generation", "文本生成"], ["vision_language", "视觉语言"],
  ["image_classification", "图片分类"], ["object_detection", "目标检测"],
  ["open_vocabulary_detection", "开放词汇检测"], ["phrase_grounding", "短语定位"],
  ["semantic_segmentation", "语义分割"], ["prompted_segmentation", "提示分割"],
  ["instance_segmentation", "实例分割"], ["keypoint_detection", "关键点检测"],
];
const protocols: [keyof RegistryModelProfile["protocol_features"], string][] = [
  ["tool_calls", "工具调用"], ["parallel_tool_calls", "并行工具调用"],
  ["structured_output", "结构化输出"], ["json_schema", "JSON Schema"],
  ["usage_reporting", "用量报告"], ["streaming", "流式响应"], ["reasoning_controls", "推理参数"],
];
const prices = [
  ["input_per_million_tokens", "输入 / 百万 tokens"],
  ["output_per_million_tokens", "输出 / 百万 tokens"],
  ["cached_input_per_million_tokens", "缓存输入 / 百万 tokens"],
  ["per_image", "每张图片"], ["per_request", "每次请求"],
] as const;
export type EditableModel = Pick<RegistryModelProfile, "provider_id" | "display_name" | "remote_model_id" | "input_modalities" | "task_capabilities" | "protocol_features" | "pricing"> & {enabled?:boolean};
export function modelEditorValue(model?: RegistryModelProfile): EditableModel {
  return model ? structuredClone({provider_id:model.provider_id,display_name:model.display_name,remote_model_id:model.remote_model_id,input_modalities:model.input_modalities,task_capabilities:model.task_capabilities,protocol_features:model.protocol_features,pricing:model.pricing,enabled:model.enabled}) : {
    provider_id: "", display_name: "", remote_model_id: "", input_modalities: ["text"],
    task_capabilities: ["text_generation"],
    protocol_features: { tool_calls: false, parallel_tool_calls: false, structured_output: false, json_schema: false, usage_reporting: false, streaming: false, reasoning_controls: false },
    pricing: { currency: "USD", source: "unknown" },
  };
}
export function validateModelEditor(value: EditableModel): EditableModel {
  if (!value.provider_id || !value.display_name.trim() || !value.remote_model_id.trim()) throw new Error("请选择 Provider，并填写显示名称和精确模型 ID。");
  if (!value.input_modalities.length || !value.task_capabilities.length) throw new Error("至少选择一种输入和一种任务能力。");
  // Rust Option pricing fields are returned as null by the HTTP API.
  for (const [key] of prices) if (value.pricing[key] != null && !/^\d+(\.\d+)?$/.test(value.pricing[key]!)) throw new Error("价格必须是非负十进制数；未知价格请留空。");
  return { ...value, display_name: value.display_name.trim(), remote_model_id: value.remote_model_id.trim() };
}

export function ModelProfileEditor({model, providers, busy, save, cancel}: {
  model?: RegistryModelProfile; providers: ProviderProfile[]; busy: boolean;
  save: (value: EditableModel) => void; cancel: () => void;
}) {
  const [value, setValue] = useState(() => modelEditorValue(model));
  const [error, setError] = useState("");
  const toggle = <T extends string,>(items:T[], item:T) => items.includes(item) ? items.filter(i=>i!==item) : [...items,item];
  return <form className="model-profile-editor" onSubmit={event=>{
    event.preventDefault(); if(busy)return;
    try { setError(""); save(validateModelEditor(value)); } catch(reason) { setError((reason as Error).message); }
  }}>
    <label>Provider<select required disabled={busy || !!model} value={value.provider_id} onChange={e=>setValue({...value,provider_id:e.target.value})}>
      <option value="">选择连接</option>{providers.map(p=><option value={p.id} key={p.id}>{p.display_name}</option>)}
    </select></label>
    <label>显示名称<input required disabled={busy} value={value.display_name} onChange={e=>setValue({...value,display_name:e.target.value})}/></label>
    <label>远程模型 ID<input required disabled={busy} value={value.remote_model_id} onChange={e=>setValue({...value,remote_model_id:e.target.value})}/></label>
    {model&&<label className="model-check"><input type="checkbox" disabled={busy} checked={value.enabled} onChange={e=>setValue({...value,enabled:e.target.checked})}/>启用模型</label>}
    <fieldset disabled={busy}><legend>输入类型</legend>{(["text","image","video"] as InputModality[]).map(item=><label className="model-check" key={item}><input type="checkbox" checked={value.input_modalities.includes(item)} onChange={()=>setValue({...value,input_modalities:toggle(value.input_modalities,item)})}/>{item}</label>)}</fieldset>
    <fieldset disabled={busy}><legend>任务能力</legend>{modelCapabilities.map(([id,label])=><label className="model-check" key={id}><input type="checkbox" checked={value.task_capabilities.includes(id)} onChange={()=>setValue({...value,task_capabilities:toggle(value.task_capabilities,id)})}/>{label}</label>)}</fieldset>
    <fieldset disabled={busy}><legend>协议特性</legend>{protocols.map(([id,label])=><label className="model-check" key={id}><input type="checkbox" checked={value.protocol_features[id]} onChange={()=>setValue({...value,protocol_features:{...value.protocol_features,[id]:!value.protocol_features[id]}})}/>{label}</label>)}</fieldset>
    <fieldset disabled={busy}><legend>价格（{value.pricing.currency}）</legend><p>留空表示未知，不会作为零费用。这里只保存配置，不探测价格。</p>{prices.map(([id,label])=><label key={id}>{label}<input inputMode="decimal" value={value.pricing[id]??""} placeholder="未知" onChange={e=>setValue({...value,pricing:{...value.pricing,[id]:e.target.value||undefined,source:"user_configured"}})}/></label>)}</fieldset>
    <p>手工声明的能力不等于已经验证。保存不会更改在途请求或 Published Version。</p>
    {error&&<p role="alert">{error}</p>}
    <div className="actions"><button type="button" disabled={busy} onClick={cancel}>取消</button><button disabled={busy}>{busy?"保存中…":"保存配置"}</button></div>
  </form>;
}
