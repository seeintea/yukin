export const DEEP_SEEK_MODELS = [
  "deepseek-v4-flash",
  "deepseek-v4-pro",
] as const;

export const DEFAULT_DEEP_SEEK_OPTIONS = {
  model: "deepseek-v4-pro",
  stream: true,
  thinking: false,
  reasoningEffort: "high",
} as const;
