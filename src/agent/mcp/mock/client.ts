import { invoke } from "@tauri-apps/api/core";
import type {
  McpCallToolRequest,
  McpCallToolResult,
  McpClient,
} from "../types";

interface OpenChromeResult {
  application: string;
  opened: boolean;
}

function throwIfAborted(signal?: AbortSignal) {
  if (signal?.aborted) {
    throw new DOMException("Mock MCP 请求已取消", "AbortError");
  }
}

function getErrorMessage(cause: unknown) {
  if (cause instanceof Error) return cause.message;

  if (
    typeof cause === "object" &&
    cause !== null &&
    "message" in cause &&
    typeof cause.message === "string"
  ) {
    return cause.message;
  }

  return String(cause);
}

/**
 * 用 Tauri invoke 模拟 MCP Client 的工具发现和工具调用。
 * 后续真实 MCP Client 可以直接替换此类，不影响 Agent loop。
 */
export class MockMcpClient implements McpClient {
  async listTools(signal?: AbortSignal) {
    throwIfAborted(signal);

    return [
      {
        name: "open_chrome",
        description: "打开用户电脑上的 Google Chrome 应用",
        inputSchema: {
          type: "object",
          properties: {},
          additionalProperties: false,
        },
      },
    ];
  }

  async callTool(
    request: McpCallToolRequest,
    signal?: AbortSignal,
  ): Promise<McpCallToolResult> {
    throwIfAborted(signal);

    if (request.name !== "open_chrome") {
      return {
        content: `Mock MCP 不存在 Tool：${request.name}`,
        isError: true,
      };
    }

    if (Object.keys(request.arguments).length > 0) {
      return {
        content: "open_chrome 不接受参数",
        isError: true,
      };
    }

    try {
      const result = await invoke<OpenChromeResult>("open_chrome");
      throwIfAborted(signal);
      return { content: result, isError: false };
    } catch (cause) {
      if (signal?.aborted) throw cause;
      return { content: getErrorMessage(cause), isError: true };
    }
  }
}
