import { parseSse } from "#/agent/transport/sse";
import type {
  ProviderChatInput,
  ProviderEvent,
  ProviderFinishReason,
  ProviderMessage,
  ProviderToolCall,
  ProviderToolResult,
} from "../types";
import type {
  ChatProtocol,
  ProtocolErrorCode,
  ProtocolRequestOptions,
} from "./types";

export class ProtocolError extends Error {
  constructor(
    readonly code: ProtocolErrorCode,
    message: string,
  ) {
    super(message);
    this.name = "ProtocolError";
  }
}

interface OpenAIToolCallDelta {
  index: number;
  id?: string;
  function?: {
    name?: string;
    arguments?: string;
  };
}

interface OpenAIChoice {
  delta?: {
    content?: string | null;
    reasoning_content?: string | null;
    tool_calls?: OpenAIToolCallDelta[];
  };
  message?: {
    content?: string | null;
    reasoning_content?: string | null;
    tool_calls?: Array<{
      id?: string;
      function?: {
        name?: string;
        arguments?: string;
      };
    }>;
  };
  finish_reason?: string | null;
}

interface OpenAIResponse {
  choices?: OpenAIChoice[];
  error?: {
    message?: string;
  };
}

interface PendingToolCall {
  id: string;
  name: string;
  arguments: string;
}

interface OpenAITextMessage {
  role: "system" | "user";
  content: string;
}

interface OpenAIAssistantMessage {
  role: "assistant";
  content: string | null;
  reasoning_content?: string;
  tool_calls?: Array<{
    id: string;
    type: "function";
    function: {
      name: string;
      arguments: string;
    };
  }>;
}

interface OpenAIToolMessage {
  role: "tool";
  tool_call_id: string;
  content: string;
}

type OpenAIRequestMessage =
  | OpenAITextMessage
  | OpenAIAssistantMessage
  | OpenAIToolMessage;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function invalidData(message: string): never {
  throw new ProtocolError("INVALID_RESPONSE_DATA", message);
}

function parsePayload(payload: unknown): OpenAIResponse {
  if (!isRecord(payload)) invalidData("模型返回的数据不是对象");

  if ("error" in payload) {
    if (!isRecord(payload.error)) {
      invalidData("模型返回的 error 结构无效");
    }

    const message = payload.error.message;
    if (message !== undefined && typeof message !== "string") {
      invalidData("模型返回的 error.message 不是字符串");
    }

    return { error: { message } };
  }

  if (!Array.isArray(payload.choices)) {
    invalidData("模型返回的 choices 不是数组");
  }

  return payload as OpenAIResponse;
}

function parseSsePayload(data: string): OpenAIResponse {
  try {
    return parsePayload(JSON.parse(data));
  } catch (error) {
    if (error instanceof ProtocolError) throw error;
    invalidData("模型返回的 SSE data 不是有效 JSON");
  }
}

function textFromMessage(message: ProviderMessage) {
  return message.content
    .filter((block) => block.type === "text")
    .map((block) => block.text)
    .join("");
}

function reasoningFromMessage(message: ProviderMessage) {
  return message.content
    .filter((block) => block.type === "reasoning")
    .map((block) => block.text)
    .join("");
}

function serializeToolResult(result: ProviderToolResult) {
  if (typeof result.output === "string" && !result.isError) {
    return result.output;
  }

  return JSON.stringify({
    isError: result.isError,
    output: result.output,
  });
}

function toOpenAIMessages(messages: ProviderMessage[]): OpenAIRequestMessage[] {
  const result: OpenAIRequestMessage[] = [];

  for (const message of messages) {
    if (message.role === "tool") {
      result.push(
        ...message.content
          .filter((block) => block.type === "tool-result")
          .map((block) => ({
            role: "tool" as const,
            tool_call_id: block.result.toolCallId,
            content: serializeToolResult(block.result),
          })),
      );
      continue;
    }

    const text = textFromMessage(message);
    if (message.role !== "assistant") {
      result.push({ role: message.role, content: text });
      continue;
    }

    const reasoningContent = reasoningFromMessage(message);
    const toolCalls = message.content
      .filter((block) => block.type === "tool-call")
      .map((block) => ({
        id: block.call.id,
        type: "function" as const,
        function: {
          name: block.call.name,
          arguments: JSON.stringify(block.call.input),
        },
      }));

    result.push({
      role: "assistant",
      content: text || null,
      ...(reasoningContent ? { reasoning_content: reasoningContent } : {}),
      ...(toolCalls.length > 0 ? { tool_calls: toolCalls } : {}),
    });
  }

  return result;
}

function parseToolInput(rawInput: string): Record<string, unknown> {
  let input: unknown;

  try {
    input = JSON.parse(rawInput || "{}");
  } catch {
    invalidData("模型返回的 Tool Call 参数不是有效 JSON");
  }

  if (!isRecord(input)) {
    invalidData("模型返回的 Tool Call 参数不是对象");
  }

  return input;
}

function completeToolCall(call: PendingToolCall): ProviderToolCall {
  if (!call.id || !call.name) {
    invalidData("模型返回的 Tool Call 缺少 id 或 name");
  }

  return {
    id: call.id,
    name: call.name,
    input: parseToolInput(call.arguments),
  };
}

export function mapOpenAIFinishReason(reason: string): ProviderFinishReason {
  switch (reason) {
    case "stop":
      return "stop";
    case "tool_calls":
      return "tool-calls";
    case "length":
      return "length";
    case "content_filter":
      return "content-filter";
    case "insufficient_system_resource":
      return "insufficient-resources";
    default:
      invalidData(`模型返回了未知的 finish_reason：${reason}`);
  }
}

export class OpenAIChatProtocol implements ChatProtocol {
  createRequestBody(
    input: ProviderChatInput,
    options: ProtocolRequestOptions,
  ): Record<string, unknown> {
    return {
      model: options.model,
      messages: toOpenAIMessages(input.messages),
      stream: options.stream,
      ...(input.tools?.length
        ? {
            tools: input.tools.map((tool) => ({
              type: "function",
              function: {
                name: tool.name,
                description: tool.description,
                parameters: tool.inputSchema,
              },
            })),
          }
        : {}),
      ...options.extensions,
    };
  }

  async *readResponse(
    response: Response,
    stream: boolean,
  ): AsyncIterable<ProviderEvent> {
    if (stream) {
      yield* this.readStream(response);
      return;
    }

    yield* this.readJson(response);
  }

  private async *readStream(response: Response): AsyncIterable<ProviderEvent> {
    if (!response.body) {
      throw new ProtocolError("EMPTY_RESPONSE_BODY", "模型没有返回响应流");
    }

    const pendingToolCalls = new Map<number, PendingToolCall>();
    let finishReason: ProviderFinishReason | null = null;
    let receivedDone = false;

    try {
      for await (const data of parseSse(response.body)) {
        if (data === "[DONE]") {
          receivedDone = true;
          break;
        }

        const payload = parseSsePayload(data);
        if (payload.error) {
          throw new ProtocolError(
            "API_ERROR",
            payload.error.message ?? "模型返回了未知 API 错误",
          );
        }

        const choice = payload.choices?.[0];
        if (!choice) continue;

        if (choice.delta?.reasoning_content) {
          yield {
            type: "reasoning-delta",
            delta: choice.delta.reasoning_content,
          };
        }

        if (choice.delta?.content) {
          yield { type: "text-delta", delta: choice.delta.content };
        }

        for (const toolCall of choice.delta?.tool_calls ?? []) {
          const pending = pendingToolCalls.get(toolCall.index) ?? {
            id: "",
            name: "",
            arguments: "",
          };

          if (toolCall.id) pending.id = toolCall.id;
          if (toolCall.function?.name) pending.name = toolCall.function.name;
          if (toolCall.function?.arguments) {
            pending.arguments += toolCall.function.arguments;
          }

          pendingToolCalls.set(toolCall.index, pending);
        }

        if (choice.finish_reason) {
          finishReason = mapOpenAIFinishReason(choice.finish_reason);
        }
      }
    } catch (error) {
      if (error instanceof ProtocolError) throw error;
      throw new ProtocolError("RESPONSE_READ_FAILED", "模型响应流读取失败");
    }

    if (!receivedDone) {
      throw new ProtocolError("INCOMPLETE_STREAM", "模型响应流提前结束");
    }

    if (!finishReason) {
      throw new ProtocolError("MISSING_FINISH_REASON", "模型响应缺少结束原因");
    }

    for (const pending of [...pendingToolCalls.entries()]
      .sort(([left], [right]) => left - right)
      .map(([, call]) => call)) {
      yield { type: "tool-call", call: completeToolCall(pending) };
    }

    yield { type: "finish", reason: finishReason };
  }

  private async *readJson(response: Response): AsyncIterable<ProviderEvent> {
    let rawPayload: unknown;

    try {
      rawPayload = await response.json();
    } catch {
      invalidData("模型返回的数据不是有效 JSON");
    }

    const payload = parsePayload(rawPayload);
    if (payload.error) {
      throw new ProtocolError(
        "API_ERROR",
        payload.error.message ?? "模型返回了未知 API 错误",
      );
    }

    const choice = payload.choices?.[0];
    if (!choice) invalidData("模型响应缺少 choice");

    if (choice.message?.reasoning_content) {
      yield {
        type: "reasoning-delta",
        delta: choice.message.reasoning_content,
      };
    }

    if (choice.message?.content) {
      yield { type: "text-delta", delta: choice.message.content };
    }

    for (const toolCall of choice.message?.tool_calls ?? []) {
      yield {
        type: "tool-call",
        call: completeToolCall({
          id: toolCall.id ?? "",
          name: toolCall.function?.name ?? "",
          arguments: toolCall.function?.arguments ?? "",
        }),
      };
    }

    if (!choice.finish_reason) {
      throw new ProtocolError("MISSING_FINISH_REASON", "模型响应缺少结束原因");
    }

    yield {
      type: "finish",
      reason: mapOpenAIFinishReason(choice.finish_reason),
    };
  }
}
