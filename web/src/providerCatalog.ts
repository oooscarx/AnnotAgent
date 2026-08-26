export const CUSTOM_MODEL = "__custom_model__";

export interface ProviderModel {
  id: string;
  label: string;
  hint: string;
  recommended?: boolean;
}

export interface ProviderPreset {
  id: string;
  label: string;
  shortLabel: string;
  description: string;
  endpoint?: string;
  apiKeyEnv?: string;
  docsUrl?: string;
  models: ProviderModel[];
  offline?: boolean;
  custom?: boolean;
  reasoningMode?: string;
}

export interface CompatibleProviderSettings {
  endpoint?: string;
  api_key_env?: string;
  model?: string;
  protocol?: string;
  request_timeout_seconds?: number;
  max_output_tokens?: number;
  temperature?: number;
  reasoning_mode?: string | null;
  supports_tool_calls?: boolean;
  supports_json_schema?: boolean;
  custom_headers?: Record<string, string>;
  extra_request_fields?: Record<string, unknown>;
  max_retries?: number;
  [key: string]: unknown;
}

export interface CatalogSettings {
  default_provider?: string;
  provider?: CompatibleProviderSettings;
  [key: string]: unknown;
}

export const PROVIDER_PRESETS: ProviderPreset[] = [
  {
    id: "mock",
    label: "Mock · Offline test",
    shortLabel: "Mock",
    description: "Runs the deterministic demo locally. No account or API key required.",
    models: [],
    offline: true,
  },
  {
    id: "dashscope",
    label: "Alibaba Cloud · Qwen",
    shortLabel: "DashScope",
    description: "Best fit for the existing Qwen RoboCup workflow and mainland China access.",
    endpoint: "https://dashscope.aliyuncs.com/compatible-mode/v1",
    apiKeyEnv: "DASHSCOPE_API_KEY",
    docsUrl: "https://help.aliyun.com/zh/model-studio/compatibility-of-openai-with-dashscope",
    reasoningMode: "medium",
    models: [
      { id: "qwen3.7-flash", label: "Qwen 3.7 Flash", hint: "Fast · Recommended", recommended: true },
      { id: "qwen3.7-plus", label: "Qwen 3.7 Plus", hint: "Higher quality" },
      { id: "qwen3.8-max", label: "Qwen 3.8 Max", hint: "Maximum capability" },
    ],
  },
  {
    id: "openai",
    label: "OpenAI · GPT",
    shortLabel: "OpenAI",
    description: "Current GPT vision models with function calling through Chat Completions.",
    endpoint: "https://api.openai.com/v1",
    apiKeyEnv: "OPENAI_API_KEY",
    docsUrl: "https://developers.openai.com/api/docs/models",
    reasoningMode: "none",
    models: [
      { id: "gpt-5.6-luna", label: "GPT-5.6 Luna", hint: "Lowest cost · Recommended", recommended: true },
      { id: "gpt-5.6-terra", label: "GPT-5.6 Terra", hint: "Balanced" },
      { id: "gpt-5.6-sol", label: "GPT-5.6 Sol", hint: "Highest capability" },
      { id: "gpt-5.4-mini", label: "GPT-5.4 mini", hint: "Fast previous generation" },
    ],
  },
  {
    id: "gemini",
    label: "Google AI · Gemini",
    shortLabel: "Gemini",
    description: "Native multimodal Gemini models through Google's OpenAI compatibility layer.",
    endpoint: "https://generativelanguage.googleapis.com/v1beta/openai",
    apiKeyEnv: "GEMINI_API_KEY",
    docsUrl: "https://ai.google.dev/gemini-api/docs/openai",
    reasoningMode: undefined,
    models: [
      { id: "gemini-3.7-flash", label: "Gemini 3.7 Flash", hint: "Fast · Recommended", recommended: true },
      { id: "gemini-3.6-flash", label: "Gemini 3.6 Flash", hint: "Stable previous generation" },
      { id: "gemini-3.5-flash-lite", label: "Gemini 3.5 Flash-Lite", hint: "High-volume, low cost" },
    ],
  },
  {
    id: "openrouter",
    label: "OpenRouter · Multi-provider",
    shortLabel: "OpenRouter",
    description: "One key for popular vision models from OpenAI, Google, Anthropic, xAI, and others.",
    endpoint: "https://openrouter.ai/api/v1",
    apiKeyEnv: "OPENROUTER_API_KEY",
    docsUrl: "https://openrouter.ai/docs/guides/overview/multimodal/image-understanding",
    reasoningMode: undefined,
    models: [
      { id: "openrouter/auto", label: "Auto Router", hint: "Automatic selection · Recommended", recommended: true },
      { id: "openai/gpt-5.6-luna", label: "OpenAI · GPT-5.6 Luna", hint: "Cost efficient" },
      { id: "google/gemini-3.7-flash", label: "Google · Gemini 3.7 Flash", hint: "Fast multimodal" },
      { id: "anthropic/claude-opus-5", label: "Anthropic · Claude Opus 5", hint: "Premium visual reasoning" },
      { id: "x-ai/grok-4.20", label: "xAI · Grok 4.20", hint: "Multimodal reasoning" },
    ],
  },
  {
    id: "custom",
    label: "Custom · OpenAI-compatible",
    shortLabel: "Custom",
    description: "Use a private gateway or another service that implements Chat Completions.",
    models: [],
    custom: true,
  },
];

export function getProviderPreset(id: string): ProviderPreset {
  return PROVIDER_PRESETS.find((preset) => preset.id === id)
    ?? PROVIDER_PRESETS[PROVIDER_PRESETS.length - 1];
}

function normalizedEndpoint(endpoint: string | undefined): string {
  return (endpoint ?? "").replace(/\/+$/, "").toLowerCase();
}

export function inferProviderPreset(settings: CatalogSettings): ProviderPreset {
  if ((settings.default_provider ?? "mock") === "mock") return getProviderPreset("mock");
  return inferConfiguredProviderPreset(settings);
}

export function inferConfiguredProviderPreset(settings: CatalogSettings): ProviderPreset {
  const endpoint = normalizedEndpoint(settings.provider?.endpoint);
  return PROVIDER_PRESETS.find((preset) =>
    preset.endpoint && normalizedEndpoint(preset.endpoint) === endpoint,
  ) ?? getProviderPreset("custom");
}

export function applyProviderPreset(settings: CatalogSettings, id: string): CatalogSettings {
  const preset = getProviderPreset(id);
  if (preset.offline) {
    return { ...settings, default_provider: "mock" };
  }
  if (preset.custom) {
    return { ...settings, default_provider: "openai_compatible" };
  }
  const current = settings.provider ?? {};
  return {
    ...settings,
    default_provider: "openai_compatible",
    provider: {
      ...current,
      endpoint: preset.endpoint,
      api_key_env: preset.apiKeyEnv,
      model: preset.models.find((model) => model.recommended)?.id ?? preset.models[0]?.id ?? "",
      protocol: "chat_completions",
      reasoning_mode: preset.reasoningMode,
      supports_tool_calls: true,
    },
  };
}

export function isCatalogModel(preset: ProviderPreset, model: string | undefined): boolean {
  return preset.models.some((candidate) => candidate.id === model);
}
