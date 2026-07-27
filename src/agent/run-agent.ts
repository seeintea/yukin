import type { BaseProvider } from "./providers/provider";
import type {
  ProviderChatInput,
  ProviderEvent,
  ProviderFinishReason,
  ProviderMessage,
  ProviderMessageContent,
  ProviderToolCall,
  ProviderToolResult,
} from "./providers/types";
import type { ToolExecutor } from "./tools/types";

export interface RunAgentInput {
  provider: BaseProvider;
  messages: ProviderMessage[];
  tools: ToolExecutor;
  signal?: AbortSignal;
  maxSteps?: number;
}

function getErrorMessage(cause: unknown) {
  return cause instanceof Error ? cause.message : "Tool 执行失败";
}

async function executeTool(
  tools: ToolExecutor,
  call: ProviderToolCall,
  signal?: AbortSignal,
): Promise<ProviderToolResult> {
  try {
    return {
      toolCallId: call.id,
      output: await tools.execute(call, signal),
      isError: false,
    };
  } catch (cause) {
    if (signal?.aborted) throw cause;

    return {
      toolCallId: call.id,
      output: getErrorMessage(cause),
      isError: true,
    };
  }
}

/** 最小 Agent loop：model → tool → model，直到模型给出最终回答。 */
export async function* runAgent({
  provider,
  messages,
  tools,
  signal,
  maxSteps = 5,
}: RunAgentInput): AsyncIterable<ProviderEvent> {
  const transcript = [...messages];

  for (let step = 1; step <= maxSteps; step += 1) {
    const assistantContent: ProviderMessageContent[] = [];
    const toolCalls: ProviderToolCall[] = [];
    let finishReason: ProviderFinishReason | null = null;

    const input: ProviderChatInput = {
      messages: transcript,
      tools: tools.specs,
      signal,
    };

    for await (const event of provider.chat(input)) {
      switch (event.type) {
        case "text-delta":
          assistantContent.push({ type: "text", text: event.delta });
          yield event;
          break;

        case "reasoning-delta":
          assistantContent.push({ type: "reasoning", text: event.delta });
          yield event;
          break;

        case "tool-call":
          toolCalls.push(event.call);
          assistantContent.push({ type: "tool-call", call: event.call });
          yield event;
          break;

        case "finish":
          finishReason = event.reason;
          break;
      }
    }

    if (!finishReason) throw new Error("Provider 没有返回 finish 事件");

    transcript.push({ role: "assistant", content: assistantContent });

    if (toolCalls.length === 0) {
      yield { type: "finish", reason: finishReason };
      return;
    }

    if (finishReason !== "tool-calls") {
      throw new Error(`模型返回 Tool Call，但结束原因为 ${finishReason}`);
    }

    const results: ProviderToolResult[] = [];
    for (const call of toolCalls) {
      results.push(await executeTool(tools, call, signal));
    }

    transcript.push({
      role: "tool",
      content: results.map((result) => ({ type: "tool-result", result })),
    });
  }

  throw new Error(`Agent 超过最大执行步数：${maxSteps}`);
}
