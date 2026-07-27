import type { ProviderToolCall, ProviderToolSpec } from "../providers/types";

/** Runtime 认识的统一 Tool，不关心能力来自 Tauri、MCP 还是普通函数。 */
export interface AgentTool {
  spec: ProviderToolSpec;
  execute(
    input: Record<string, unknown>,
    signal?: AbortSignal,
  ): Promise<unknown>;
}

export interface ToolExecutor {
  readonly specs: ProviderToolSpec[];
  execute(call: ProviderToolCall, signal?: AbortSignal): Promise<unknown>;
}
