import { ToolRegistry } from "../tools/registry";
import type { AgentTool } from "../tools/types";
import type { McpClient } from "./types";

function getToolError(content: unknown) {
  return typeof content === "string"
    ? content
    : `MCP Tool 执行失败：${JSON.stringify(content)}`;
}

/** 把任意 MCP Client 发现的工具适配成 Agent Runtime 的统一 Tool。 */
export async function createMcpToolRegistry(
  client: McpClient,
  signal?: AbortSignal,
) {
  const specs = await client.listTools(signal);
  const tools: AgentTool[] = specs.map((spec) => ({
    spec,
    async execute(input, executeSignal) {
      const result = await client.callTool(
        { name: spec.name, arguments: input },
        executeSignal,
      );

      if (result.isError) throw new Error(getToolError(result.content));
      return result.content;
    },
  }));

  return new ToolRegistry(tools);
}
