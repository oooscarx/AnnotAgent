import { useEffect, useState } from "react";
import type { RegistryModelProfile } from "../types";
import { Disclosure } from "./Disclosure";
import type { ModelRuntimeOptions } from "./ModelProfileEditor";
import "./model-request-evidence.css";

type NullablePrice = string | null;

export type EffectiveModelPricing = {
  currency: string;
  input_per_million_tokens: NullablePrice;
  output_per_million_tokens: NullablePrice;
  cached_input_per_million_tokens: NullablePrice;
  per_image: NullablePrice;
  per_request: NullablePrice;
  source: "user_configured" | "provider_discovered" | "preset" | "unknown";
  updated_at: string | null;
};

export type EffectiveModelRequest = {
  model_profile_id: string;
  model_profile_revision: number;
  provider_id: string;
  provider_adapter: string;
  endpoint_summary: string;
  remote_model_id: string;
  context_tokens: number | null;
  requested_maximum_output_tokens: number | null;
  effective_maximum_output_tokens: number;
  maximum_input_context_tokens: number | null;
  temperature: number;
  top_p: string | number | null;
  structured_output_mode: string | null;
  image_detail: string | null;
  system_prompt_version: string | null;
  reasoning: {
    requested_mode: string | null;
    supported_modes: string[];
    support_known: boolean;
    wire_parameter: "reasoning_effort" | "enable_thinking" | null;
    wire_value: string | boolean | null;
  };
  pricing_snapshot: {
    model_profile_id: string;
    model_profile_revision: number;
    pricing: EffectiveModelPricing;
    captured_at: string;
  };
  snapshot_sha256: string;
};

export type ModelRuntimeEvidenceService = {
  getEffectiveModelRequest?: (
    modelProfileId: string,
    signal?: AbortSignal,
  ) => Promise<EffectiveModelRequest>;
};

export function validateEffectiveModelRequest(
  model: RegistryModelProfile,
  value: EffectiveModelRequest,
): ModelRuntimeOptions {
  if (
    value.model_profile_id !== model.id
    || value.model_profile_revision !== model.revision
    || value.provider_id !== model.provider_id
    || value.pricing_snapshot.model_profile_id !== model.id
    || value.pricing_snapshot.model_profile_revision !== model.revision
  ) {
    throw new Error("有效请求预览不属于当前 Model Profile revision");
  }
  return {
    model_profile_id: value.model_profile_id,
    model_profile_revision: value.model_profile_revision,
    status: value.reasoning.support_known ? "declared" : "unknown",
    supported_reasoning_modes: value.reasoning.supported_modes,
    source: "effective_request",
    maximum_output_tokens: value.effective_maximum_output_tokens,
  };
}

function shown(value: string | number | boolean | null, fallback = "未知") {
  return value == null || value === "" ? fallback : String(value);
}

function price(value: NullablePrice, currency: string) {
  return value == null ? "未知（不是 0）" : `${currency} ${value}`;
}

export function EffectiveModelRequestView({ value }: { value: EffectiveModelRequest }) {
  const pricing = value.pricing_snapshot.pricing;
  return (
    <div className="model-request-evidence-view">
      <div className="model-request-evidence-summary">
        <div><span>上下文窗口</span><strong>{shown(value.context_tokens)} tokens</strong></div>
        <div><span>最大输入预算</span><strong>{shown(value.maximum_input_context_tokens)} tokens</strong></div>
        <div><span>请求最大输出</span><strong>{shown(value.requested_maximum_output_tokens, "使用默认")}</strong></div>
        <div><span>实际最大输出</span><strong>{shown(value.effective_maximum_output_tokens)} tokens</strong></div>
      </div>
      <p>
        Temperature {shown(value.temperature)} · Top P {shown(value.top_p, "Provider 默认")}
      </p>
      <div className="model-request-reasoning">
        <strong>思考模式</strong>
        <p>
          当前选择：{shown(value.reasoning.requested_mode, "Provider 默认")} · 映射：
          {value.reasoning.wire_parameter
            ? `${value.reasoning.wire_parameter}=${shown(value.reasoning.wire_value)}`
            : "未发送参数"}
        </p>
        <p>
          {value.reasoning.support_known
            ? `当前 revision 配置的可选模式：${value.reasoning.supported_modes.join("、") || "无"}`
            : "实际可选模式未知；reasoning_controls 声明不会生成模式清单。"}
        </p>
      </div>
      <div className="model-request-pricing">
        <strong>价格快照 · Model Profile r{value.pricing_snapshot.model_profile_revision}</strong>
        <p>
          输入 / 百万 tokens：{price(pricing.input_per_million_tokens, pricing.currency)} · 输出 / 百万 tokens：
          {price(pricing.output_per_million_tokens, pricing.currency)}
        </p>
        <p>币种 {pricing.currency} · 来源 {pricing.source} · 捕获于 {value.pricing_snapshot.captured_at}</p>
      </div>
      <Disclosure title="请求映射证据">
        <pre>{JSON.stringify({
          provider_adapter: value.provider_adapter,
          endpoint_summary: value.endpoint_summary,
          remote_model_id: value.remote_model_id,
          structured_output_mode: value.structured_output_mode,
          image_detail: value.image_detail,
          system_prompt_version: value.system_prompt_version,
          snapshot_sha256: value.snapshot_sha256,
        }, null, 2)}</pre>
      </Disclosure>
      <p className="model-request-evidence-caveat">
        这是服务器对当前 revision 的被动映射，不会发送模型请求。只有显式测试回执才能证明远端接受这些字段。
      </p>
    </div>
  );
}

export function ModelRequestEvidence({
  model,
  service,
  onRuntimeOptions,
}: {
  model: RegistryModelProfile;
  service: ModelRuntimeEvidenceService;
  onRuntimeOptions?: (value: ModelRuntimeOptions | undefined) => void;
}) {
  const [value, setValue] = useState<EffectiveModelRequest>();
  const [error, setError] = useState("");
  const [reload, setReload] = useState(0);

  useEffect(() => {
    const read = service.getEffectiveModelRequest;
    if (!read) {
      setValue(undefined);
      onRuntimeOptions?.(undefined);
      return;
    }
    const controller = new AbortController();
    setValue(undefined);
    setError("");
    onRuntimeOptions?.(undefined);
    void read(model.id, controller.signal)
      .then((result) => {
        if (controller.signal.aborted) return;
        const options = validateEffectiveModelRequest(model, result);
        setValue(result);
        onRuntimeOptions?.(options);
      })
      .catch((reason) => {
        if (controller.signal.aborted) return;
        setError((reason as Error).message);
        onRuntimeOptions?.(undefined);
      });
    return () => controller.abort();
  }, [model, onRuntimeOptions, reload, service]);

  return (
    <section className="model-request-evidence" aria-label="有效模型请求">
      <div className="model-request-evidence-heading">
        <div><h3>下一次请求会怎样执行</h3><p>Model Profile r{model.revision}</p></div>
        <button type="button" disabled={!service.getEffectiveModelRequest} onClick={() => setReload((value) => value + 1)}>重新读取</button>
      </div>
      {!service.getEffectiveModelRequest && <p>当前 Adapter 尚未接入有效请求预览；不能把已保存字段当作已生效证据。</p>}
      {service.getEffectiveModelRequest && !value && !error && <p role="status">读取服务器的有效请求映射…</p>}
      {error && <p role="alert">{error}。未发送模型请求，也未把配置标记为已验证。</p>}
      {value && <EffectiveModelRequestView value={value} />}
    </section>
  );
}
