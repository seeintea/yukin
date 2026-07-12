import { parseSse } from "../../transport/sse";
import { DeepSeekError } from "./error";
import type { DeepSeekFinishReason, DeepSeekStreamEvent } from "./types";

interface DeepSeekChoice {
  delta?: {
    content?: string | null;
  };
  finish_reason?: DeepSeekFinishReason | null;
}

interface DeepSeekChunk {
  choices?: DeepSeekChoice[];
  error?: {
    message?: string;
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isFinishReason(value: unknown): value is DeepSeekFinishReason {
  return (
    value === "stop" ||
    value === "length" ||
    value === "content_filter" ||
    value === "insufficient_system_resource"
  );
}

function invalidStreamData(message: string): never {
  throw new DeepSeekError("INVALID_STREAM_DATA", message);
}

function parseChunk(data: string): DeepSeekChunk {
  let payload: unknown;

  try {
    payload = JSON.parse(data);
  } catch {
    invalidStreamData("DeepSeek 返回的数据不是有效 JSON");
  }

  if (!isRecord(payload)) {
    invalidStreamData("DeepSeek 返回的数据不是对象");
  }

  if ("error" in payload) {
    if (!isRecord(payload.error)) {
      invalidStreamData("DeepSeek 返回的 error 结构无效");
    }

    const message = payload.error.message;
    if (message !== undefined && typeof message !== "string") {
      invalidStreamData("DeepSeek 返回的 error.message 不是字符串");
    }

    return { error: { message } };
  }

  if (!Array.isArray(payload.choices)) {
    invalidStreamData("DeepSeek 返回的 choices 不是数组");
  }

  if (payload.choices.length === 0) {
    return { choices: [] };
  }

  const choice = payload.choices[0];
  if (!isRecord(choice)) {
    invalidStreamData("DeepSeek 返回的 choice 不是对象");
  }

  let delta: DeepSeekChoice["delta"];

  if (choice.delta !== undefined) {
    if (!isRecord(choice.delta)) {
      invalidStreamData("DeepSeek 返回的 delta 不是对象");
    }

    const content = choice.delta.content;
    if (
      content !== undefined &&
      content !== null &&
      typeof content !== "string"
    ) {
      invalidStreamData("DeepSeek 返回的 delta.content 不是字符串或 null");
    }

    delta = { content };
  }

  const finishReason = choice.finish_reason;
  if (
    finishReason !== undefined &&
    finishReason !== null &&
    !isFinishReason(finishReason)
  ) {
    invalidStreamData("DeepSeek 返回了未知的 finish_reason");
  }

  return {
    choices: [{ delta, finish_reason: finishReason }],
  };
}

export async function* streamDeepSeek(
  url: string,
  key: string,
  messages: { role: string; content: string }[],
): AsyncGenerator<DeepSeekStreamEvent> {
  let response: Response;

  try {
    response = await fetch(`${url}/chat/completions`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${key}`,
      },
      body: JSON.stringify({
        messages,
        model: "deepseek-v4-pro",
        // thinking: { type: "enabled" },
        thinking: { type: "disabled" }, // 暂时关闭思考模式
        reasoning_effort: "high",
        stream: true,
      }),
    });
  } catch {
    throw new DeepSeekError("REQUEST_FAILED", "DeepSeek 请求发送失败");
  }

  if (!response.ok) {
    const message = await response.text();
    throw new DeepSeekError(
      "HTTP_ERROR",
      `DeepSeek 请求失败：${response.status} ${message}`,
    );
  }

  if (!response.body) {
    throw new DeepSeekError("EMPTY_RESPONSE_BODY", "DeepSeek 没有返回响应流");
  }

  try {
    let finishReason: DeepSeekFinishReason | null = null;
    let receivedDone = false;

    for await (const data of parseSse(response.body)) {
      if (data === "[DONE]") {
        receivedDone = true;
        break;
      }

      const payload = parseChunk(data);

      if (payload.error) {
        throw new DeepSeekError(
          "API_ERROR",
          payload.error.message ?? "DeepSeek 返回了未知 API 错误",
        );
      }
      const choice = payload.choices?.[0];

      if (choice?.delta?.content) {
        yield {
          type: "content",
          content: choice.delta.content,
        };
      }

      if (choice?.finish_reason) {
        finishReason = choice.finish_reason;
      }
    }

    if (!receivedDone) {
      throw new DeepSeekError("INCOMPLETE_STREAM", "DeepSeek 响应流提前结束");
    }

    if (!finishReason) {
      throw new DeepSeekError(
        "MISSING_FINISH_REASON",
        "DeepSeek 响应缺少结束原因",
      );
    }

    yield {
      type: "finish",
      reason: finishReason,
    };
  } catch (error) {
    if (error instanceof DeepSeekError) throw error;

    throw new DeepSeekError("STREAM_READ_FAILED", "DeepSeek 响应流读取失败");
  }
}

export { DeepSeekError } from "./error";
