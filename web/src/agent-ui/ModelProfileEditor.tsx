import { useState } from "react";
import type {
  InputModality,
  ModelCapability,
  ProviderProfile,
  RegistryModelProfile,
} from "../types";

export const modelCapabilities: [ModelCapability, string][] = [
  ["text_generation", "文本生成"],
  ["vision_language", "视觉语言"],
  ["image_classification", "图片分类"],
  ["object_detection", "目标检测"],
  ["open_vocabulary_detection", "开放词汇检测"],
  ["phrase_grounding", "短语定位"],
  ["semantic_segmentation", "语义分割"],
  ["prompted_segmentation", "提示分割"],
  ["instance_segmentation", "实例分割"],
  ["keypoint_detection", "关键点检测"],
];

const protocols: [keyof RegistryModelProfile["protocol_features"], string][] = [
  ["tool_calls", "工具调用"],
  ["parallel_tool_calls", "并行工具调用"],
  ["structured_output", "结构化输出"],
  ["json_schema", "JSON Schema"],
  ["usage_reporting", "用量报告"],
  ["streaming", "流式响应"],
  ["reasoning_controls", "支持推理参数（能力声明）"],
];

const prices = [
  ["input_per_million_tokens", "输入 / 百万 tokens"],
  ["output_per_million_tokens", "输出 / 百万 tokens"],
  ["cached_input_per_million_tokens", "缓存输入 / 百万 tokens"],
  ["per_image", "每张图片"],
  ["per_request", "每次请求"],
] as const;

export type ModelRuntimeOptions = {
  model_profile_id: string;
  model_profile_revision: number;
  status: "verified" | "unknown";
  supported_reasoning_modes: string[];
  source: string;
  maximum_output_tokens?: number | null;
};

export function runtimeOptionsForModel(
  model: RegistryModelProfile | undefined,
  runtimeOptions: ModelRuntimeOptions | undefined,
) {
  return !!model
    && runtimeOptions?.model_profile_id === model.id
    && runtimeOptions.model_profile_revision === model.revision
    ? runtimeOptions
    : undefined;
}

export type EditableModel = Pick<
  RegistryModelProfile,
  | "provider_id"
  | "display_name"
  | "remote_model_id"
  | "input_modalities"
  | "task_capabilities"
  | "protocol_features"
  | "limits"
  | "generation_defaults"
  | "pricing"
> & { enabled?: boolean };

export function modelEditorValue(model?: RegistryModelProfile): EditableModel {
  return model
    ? structuredClone({
        provider_id: model.provider_id,
        display_name: model.display_name,
        remote_model_id: model.remote_model_id,
        input_modalities: model.input_modalities,
        task_capabilities: model.task_capabilities,
        protocol_features: model.protocol_features,
        limits: model.limits,
        generation_defaults: model.generation_defaults,
        pricing: model.pricing,
        enabled: model.enabled,
      })
    : {
        provider_id: "",
        display_name: "",
        remote_model_id: "",
        input_modalities: ["text"],
        task_capabilities: ["text_generation"],
        protocol_features: {
          tool_calls: false,
          parallel_tool_calls: false,
          structured_output: false,
          json_schema: false,
          usage_reporting: false,
          streaming: false,
          reasoning_controls: false,
        },
        limits: {},
        generation_defaults: {},
        pricing: { currency: "USD", source: "unknown" },
      };
}

function positiveInteger(value: unknown, label: string) {
  if (value == null || value === "") return undefined;
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed <= 0)
    throw new Error(`${label}必须是大于零的整数。`);
  return parsed;
}

function decimal(
  value: unknown,
  label: string,
  minimum: number,
  maximum: number,
  minimumExclusive = false,
) {
  if (value == null || value === "") return undefined;
  const parsed = Number(value);
  if (
    !Number.isFinite(parsed) ||
    (minimumExclusive ? parsed <= minimum : parsed < minimum) ||
    parsed > maximum
  )
    throw new Error(`${label}必须在${minimumExclusive ? "(" : "["}${minimum}, ${maximum}]范围内。`);
  return parsed;
}

export function validateModelEditor(
  value: EditableModel,
  runtimeOptions?: ModelRuntimeOptions,
): EditableModel {
  if (!value.provider_id || !value.display_name.trim() || !value.remote_model_id.trim())
    throw new Error("请选择 Provider，并填写显示名称和精确模型 ID。");
  if (!value.input_modalities.length || !value.task_capabilities.length)
    throw new Error("至少选择一种输入和一种任务能力。");
  for (const [key] of prices)
    if (value.pricing[key] != null && !/^\d+(\.\d+)?$/.test(value.pricing[key]!))
      throw new Error("价格必须是非负十进制数；未知价格请留空。");
  if (!/^[A-Z]{3}$/.test(value.pricing.currency))
    throw new Error("币种必须是三个大写字母，例如 USD 或 CNY。");

  const limits = {
    ...value.limits,
    context_tokens: positiveInteger(value.limits.context_tokens, "上下文窗口"),
    maximum_output_tokens: positiveInteger(value.limits.maximum_output_tokens, "模型最大输出"),
    maximum_images_per_request: positiveInteger(value.limits.maximum_images_per_request, "单次最大图片数"),
    maximum_image_pixels: positiveInteger(value.limits.maximum_image_pixels, "单张最大像素"),
  };
  const generationMaximum = positiveInteger(
    value.generation_defaults.maximum_output_tokens,
    "默认最大输出",
  );
  if (limits.maximum_output_tokens && generationMaximum && generationMaximum > limits.maximum_output_tokens)
    throw new Error("默认最大输出不能超过模型最大输出限制。");

  const reasoningMode = value.generation_defaults.reasoning_mode;
  if (
    reasoningMode &&
    runtimeOptions?.status === "verified" &&
    !runtimeOptions.supported_reasoning_modes.includes(String(reasoningMode))
  )
    throw new Error("所选思考模式不在 Provider 已验证支持的模式中。");

  return {
    ...value,
    display_name: value.display_name.trim(),
    remote_model_id: value.remote_model_id.trim(),
    limits,
    generation_defaults: {
      ...value.generation_defaults,
      temperature: decimal(value.generation_defaults.temperature, "Temperature", 0, 2),
      top_p: decimal(value.generation_defaults.top_p, "Top P", 0, 1, true),
      maximum_output_tokens: generationMaximum,
    },
  };
}

function optionalNumber(value: string) {
  return value === "" ? undefined : Number(value);
}

export function ModelProfileEditor({
  model,
  providers,
  busy,
  runtimeOptions,
  save,
  cancel,
}: {
  model?: RegistryModelProfile;
  providers: ProviderProfile[];
  busy: boolean;
  runtimeOptions?: ModelRuntimeOptions;
  save: (value: EditableModel) => void;
  cancel: () => void;
}) {
  const [value, setValue] = useState(() => modelEditorValue(model));
  const [error, setError] = useState("");
  const toggle = <T extends string>(items: T[], item: T) =>
    items.includes(item) ? items.filter((candidate) => candidate !== item) : [...items, item];
  const reasoningMode = String(value.generation_defaults.reasoning_mode ?? "");
  const currentRuntimeOptions = runtimeOptionsForModel(model, runtimeOptions);
  const verifiedModes = currentRuntimeOptions?.status === "verified"
    ? currentRuntimeOptions.supported_reasoning_modes
    : [];
  const unsupportedSavedMode = reasoningMode && !verifiedModes.includes(reasoningMode);

  return (
    <form
      className="model-profile-editor"
      onSubmit={(event) => {
        event.preventDefault();
        if (busy) return;
        try {
          setError("");
          save(validateModelEditor(value, runtimeOptions));
        } catch (reason) {
          setError((reason as Error).message);
        }
      }}
    >
      <label>
        Provider
        <select required disabled={busy || !!model} value={value.provider_id} onChange={(event) => setValue({ ...value, provider_id: event.target.value })}>
          <option value="">选择连接</option>
          {providers.map((provider) => <option value={provider.id} key={provider.id}>{provider.display_name}</option>)}
        </select>
      </label>
      <label>显示名称<input required disabled={busy} value={value.display_name} onChange={(event) => setValue({ ...value, display_name: event.target.value })} /></label>
      <label>远程模型 ID<input required disabled={busy} value={value.remote_model_id} onChange={(event) => setValue({ ...value, remote_model_id: event.target.value })} /></label>
      {model && <label className="model-check"><input type="checkbox" disabled={busy} checked={value.enabled} onChange={(event) => setValue({ ...value, enabled: event.target.checked })} />启用模型</label>}

      <fieldset disabled={busy}>
        <legend>输入类型</legend>
        {(["text", "image", "video"] as InputModality[]).map((item) => <label className="model-check" key={item}><input type="checkbox" checked={value.input_modalities.includes(item)} onChange={() => setValue({ ...value, input_modalities: toggle(value.input_modalities, item) })} />{item}</label>)}
      </fieldset>
      <fieldset disabled={busy}>
        <legend>任务能力</legend>
        {modelCapabilities.map(([id, label]) => <label className="model-check" key={id}><input type="checkbox" checked={value.task_capabilities.includes(id)} onChange={() => setValue({ ...value, task_capabilities: toggle(value.task_capabilities, id) })} />{label}</label>)}
      </fieldset>
      <fieldset disabled={busy}>
        <legend>协议能力声明</legend>
        <p><code>reasoning_controls</code> 只声明协议是否接受推理参数，不代表用户已经选择某个思考模式。</p>
        {protocols.map(([id, label]) => <label className="model-check" key={id}><input type="checkbox" checked={value.protocol_features[id]} onChange={() => setValue({ ...value, protocol_features: { ...value.protocol_features, [id]: !value.protocol_features[id] } })} />{label}</label>)}
      </fieldset>

      <fieldset disabled={busy}>
        <legend>上下文与输出限制</legend>
        <p>限制描述模型容量；默认值描述后续请求采用的配置。保存后仍需实际测试回执证明 Provider 接受并应用。</p>
        <label>上下文窗口（tokens）<input inputMode="numeric" type="number" min="1" value={value.limits.context_tokens ?? ""} placeholder="未知" onChange={(event) => setValue({ ...value, limits: { ...value.limits, context_tokens: optionalNumber(event.target.value) } })} /></label>
        <label>模型最大输出（tokens）<input inputMode="numeric" type="number" min="1" value={value.limits.maximum_output_tokens ?? ""} placeholder="未知" onChange={(event) => setValue({ ...value, limits: { ...value.limits, maximum_output_tokens: optionalNumber(event.target.value) } })} /></label>
        <label>默认最大输出（tokens）<input inputMode="numeric" type="number" min="1" value={value.generation_defaults.maximum_output_tokens ?? ""} placeholder="使用模型限制或服务默认" onChange={(event) => setValue({ ...value, generation_defaults: { ...value.generation_defaults, maximum_output_tokens: optionalNumber(event.target.value) } })} /></label>
        <label>Temperature<input inputMode="decimal" type="number" min="0" max="2" step="0.01" value={value.generation_defaults.temperature ?? ""} placeholder="Provider 默认" onChange={(event) => setValue({ ...value, generation_defaults: { ...value.generation_defaults, temperature: optionalNumber(event.target.value) } })} /></label>
        <label>Top P<input inputMode="decimal" type="number" min="0.01" max="1" step="0.01" value={value.generation_defaults.top_p ?? ""} placeholder="Provider 默认" onChange={(event) => setValue({ ...value, generation_defaults: { ...value.generation_defaults, top_p: optionalNumber(event.target.value) } })} /></label>
        <label>
          思考模式
          <select
            disabled={busy || currentRuntimeOptions?.status !== "verified" || verifiedModes.length === 0}
            value={reasoningMode}
            onChange={(event) => setValue({ ...value, generation_defaults: { ...value.generation_defaults, reasoning_mode: event.target.value || undefined } })}
          >
            <option value="">使用 Provider 默认</option>
            {unsupportedSavedMode && <option value={reasoningMode}>当前保存值（未在已验证清单）</option>}
            {verifiedModes.map((mode) => <option key={mode} value={mode}>{mode}</option>)}
          </select>
        </label>
        {currentRuntimeOptions?.status === "verified"
          ? <p>Provider 已报告：{verifiedModes.length ? verifiedModes.join("、") : "不提供可选思考模式"} · 来源 {currentRuntimeOptions.source}</p>
          : <p>实际支持的思考模式尚未验证，因此不能凭 <code>reasoning_controls</code> 猜测选项；现有值会保持不变。</p>}
      </fieldset>

      <fieldset disabled={busy}>
        <legend>价格与币种</legend>
        <p>留空表示未知，不会作为零费用。价格随 Model Profile revision 保存，历史调用使用自己的价格快照。</p>
        <label>币种<input maxLength={3} value={value.pricing.currency} onChange={(event) => setValue({ ...value, pricing: { ...value.pricing, currency: event.target.value.toUpperCase(), source: "user_configured" } })} /></label>
        {prices.map(([id, label]) => <label key={id}>{label}<input inputMode="decimal" value={value.pricing[id] ?? ""} placeholder="未知" onChange={(event) => setValue({ ...value, pricing: { ...value.pricing, [id]: event.target.value || undefined, source: "user_configured" } })} /></label>)}
      </fieldset>
      <p>手工声明的能力不等于已经验证。保存不会更改在途请求或 Published Version，也不会运行收费测试。</p>
      {error && <p role="alert">{error}</p>}
      <div className="actions"><button type="button" disabled={busy} onClick={cancel}>取消</button><button disabled={busy}>{busy ? "保存中…" : "保存配置"}</button></div>
    </form>
  );
}
