import type { ProviderToolCall, ProviderToolSpec } from "../providers/types";
import type { AgentTool, ToolExecutor } from "./types";

function throwIfAborted(signal?: AbortSignal) {
  if (signal?.aborted) {
    throw new DOMException("Tool 执行已取消", "AbortError");
  }
}

export class ToolRegistry implements ToolExecutor {
  private readonly tools = new Map<string, AgentTool>();

  constructor(tools: AgentTool[] = []) {
    for (const tool of tools) this.register(tool);
  }

  get specs(): ProviderToolSpec[] {
    return [...this.tools.values()].map((tool) => tool.spec);
  }

  register(tool: AgentTool) {
    if (this.tools.has(tool.spec.name)) {
      throw new Error(`Tool 已注册：${tool.spec.name}`);
    }

    this.tools.set(tool.spec.name, tool);
  }

  async execute(call: ProviderToolCall, signal?: AbortSignal) {
    throwIfAborted(signal);

    const tool = this.tools.get(call.name);
    if (!tool) throw new Error(`未找到 Tool：${call.name}`);

    const output = await tool.execute(call.input, signal);
    throwIfAborted(signal);
    return output;
  }
}
