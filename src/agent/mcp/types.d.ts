import type { ProviderToolSpec } from "../providers/types";

export interface McpCallToolRequest {
  name: string;
  arguments: Record<string, unknown>;
}

export interface McpCallToolResult {
  content: unknown;
  isError: boolean;
}

/**
 * 本项目暂时使用的最小 MCP Client 边界。
 *
 * 真实 MCP Client 以后负责 initialize、JSON-RPC 和传输；Runtime 只依赖
 * 工具发现 listTools 与工具执行 callTool。
 */
export interface McpClient {
  listTools(signal?: AbortSignal): Promise<ProviderToolSpec[]>;
  callTool(
    request: McpCallToolRequest,
    signal?: AbortSignal,
  ): Promise<McpCallToolResult>;
}
