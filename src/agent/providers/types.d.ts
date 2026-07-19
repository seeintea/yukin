/** 当前应用支持的两种模型 API 协议。 */
export type ProviderApiFormat = "open-ai" | "anthropic";

/**
 * Provider 的连接信息。
 *
 * 示例：
 * {
 *   baseUrl: "https://api.deepseek.com/chat/completions",
 *   apiKey: "sk-...",
 *   format: "open-ai"
 * }
 */
export interface ProviderConnection {
  baseUrl: string;
  apiKey: string;
  format: ProviderApiFormat;
}

/** 所有 Provider 都具备的基础推理配置。 */
export interface ProviderOptions<TModel extends string = string> {
  model: TModel;
  stream: boolean;
}

/** Runtime 提供给模型的工具描述。 */
export interface ProviderToolSpec {
  name: string;
  description: string;
  inputSchema: Record<string, unknown>;
}

/** 模型生成的一次工具调用。 */
export interface ProviderToolCall {
  id: string;
  name: string;
  input: Record<string, unknown>;
}

/** Tool Registry 执行工具后返回给模型的结果。 */
export interface ProviderToolResult {
  toolCallId: string;
  output: unknown;
  isError: boolean;
}

/**
 * Provider 使用的统一消息内容。
 *
 * user/system 通常只有 text；assistant 可以同时包含 text 和 tool-call；
 * tool 消息使用 tool-result，并通过 toolCallId 与调用关联。
 */
export type ProviderMessageContent =
  | { type: "text"; text: string }
  | { type: "reasoning"; text: string }
  | { type: "tool-call"; call: ProviderToolCall }
  | { type: "tool-result"; result: ProviderToolResult };

export interface ProviderMessage {
  role: "system" | "user" | "assistant" | "tool";
  content: ProviderMessageContent[];
}

/** 每次调用 Provider 时变化的数据，不属于 Provider 的长期配置。 */
export interface ProviderChatInput {
  messages: ProviderMessage[];
  tools?: ProviderToolSpec[];
  signal?: AbortSignal;
}

/** 不同供应商的停止原因最终都映射到这组内部语义。 */
export type ProviderFinishReason =
  | "stop"
  | "tool-calls"
  | "length"
  | "content-filter"
  | "insufficient-resources";

/**
 * Provider 交给 Agent Runtime 的统一事件。
 *
 * 流式响应会产生多个 text-delta；非流式响应产生一个包含完整文本的
 * text-delta。两种模式最终都会产生 finish。
 */
export type ProviderEvent =
  | { type: "text-delta"; delta: string }
  | { type: "reasoning-delta"; delta: string }
  | { type: "tool-call"; call: ProviderToolCall }
  | { type: "finish"; reason: ProviderFinishReason };
