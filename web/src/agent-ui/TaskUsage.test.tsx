import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { TaskUsageView, type TaskUsageAttempt, type TaskUsagePage } from "./TaskUsage";

const attempt = (overrides: Partial<TaskUsageAttempt> = {}): TaskUsageAttempt => ({
  attempt_id: "attempt-1",
  call_id: "call-1",
  attempt_number: 1,
  kind: "task",
  status: "succeeded",
  model_profile_id: "model-1",
  model_profile_revision: 7,
  model_name: "Qwen Vision",
  provider_id: "provider-1",
  provider_name: "Remote Provider",
  request_id: "request-1",
  input_tokens: 1500,
  cached_input_tokens: 0,
  output_tokens: 500,
  image_count: 0,
  usage_source: "actual",
  cost: "0.007",
  currency: "USD",
  pricing_snapshot: {
    model_profile_id: "model-1",
    model_profile_revision: 7,
    pricing: {
      currency: "USD",
      input_per_million_tokens: "2",
      output_per_million_tokens: "8",
      cached_input_per_million_tokens: null,
      per_image: null,
      per_request: null,
      source: "user_configured",
      updated_at: "2026-09-12T00:00:00Z",
    },
    captured_at: "2026-09-12T00:00:00Z",
  },
  started_at: "2026-09-12T00:00:00Z",
  completed_at: "2026-09-12T00:00:01Z",
  duration_ms: 4,
  failure: null,
  effective_request: { effective_maximum_output_tokens: 1024, runtime_request: { maximum_output_tokens: 1024 } },
  ...overrides,
});

const page = (overrides: Partial<TaskUsagePage> = {}): TaskUsagePage => ({
  scope: { project_id: "project", conversation_id: "conversation", task_id: "task" },
  state: "complete",
  summary: {
    attempt_count: 1,
    known_cost: "0.007",
    currency: "USD",
    costs_by_currency: [{ currency: "USD", cost: "0.007" }],
    input_tokens: 1500,
    cached_input_tokens: 0,
    output_tokens: 500,
    token_unknown_attempt_count: 0,
    unknown_cost_attempt_count: 0,
  },
  attempts: { items: [attempt()], next_cursor: null },
  ...overrides,
});

describe("TaskUsage", () => {
  it("shows preset mode as no request without fabricated token or cost", () => {
    const html = renderToStaticMarkup(<TaskUsageView value={page({
      state: "no_model_requests",
      summary: { attempt_count: 0, known_cost: null, currency: null, costs_by_currency: [], input_tokens: 0, cached_input_tokens: 0, output_tokens: 0, token_unknown_attempt_count: 0, unknown_cost_attempt_count: 0 },
      attempts: { items: [], next_cursor: null },
    })} />);
    expect(html).toContain("无本次模型请求");
    expect(html).toContain("Preset 候选不会伪造 Token 或费用");
    expect(html).not.toContain("USD 0");
  });

  it("renders immutable per-attempt model, provider, tokens, source, price revision and decimal cost", () => {
    const html = renderToStaticMarkup(<TaskUsageView value={page()} />);
    for (const text of ["Qwen Vision", "Remote Provider", "1,500", "500", "actual", "Model Profile r7", "USD 0.007", "价格 revision r7", "effective_maximum_output_tokens"])
      expect(html).toContain(text);
  });

  it("keeps unknown cost and probes distinct from ordinary task attempts", () => {
    const html = renderToStaticMarkup(<TaskUsageView value={page({
      state: "partial",
      summary: { attempt_count: 2, known_cost: null, currency: null, costs_by_currency: [{ currency: "USD", cost: "0.007" }], input_tokens: null, cached_input_tokens: null, output_tokens: null, token_unknown_attempt_count: 1, unknown_cost_attempt_count: 1 },
      attempts: { items: [attempt({ cost: null, currency: null, input_tokens: null, status: "in_doubt" }), attempt({ attempt_id: "probe", kind: "probe" })], next_cursor: null },
    })} />);
    expect(html).toContain("费用未知（不是 0）");
    expect(html).toContain("输入 未知（不是 0）");
    expect(html).toContain("独立的模型探测记录");
    expect(html).toContain("探测不是普通 Task 推理");
  });

  it("lists mixed currencies separately instead of adding them", () => {
    const html = renderToStaticMarkup(<TaskUsageView value={page({
      summary: { attempt_count: 2, known_cost: null, currency: null, costs_by_currency: [{ currency: "USD", cost: "0.007" }, { currency: "CNY", cost: "0.050" }], input_tokens: 20, cached_input_tokens: 0, output_tokens: 5, token_unknown_attempt_count: 0, unknown_cost_attempt_count: 0 },
      attempts: { items: [attempt(), attempt({ attempt_id: "attempt-2", cost: "0.050", currency: "CNY" })], next_cursor: null },
    })} />);
    expect(html).toContain("USD 0.007");
    expect(html).toContain("CNY 0.050");
    expect(html).not.toContain("0.057");
  });
});
