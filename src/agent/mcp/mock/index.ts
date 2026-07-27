import { createMcpToolRegistry } from "../tool-adapter";
import { MockMcpClient } from "./client";

/** 模拟 MCP 连接和 listTools，返回可交给 Agent loop 的 Tool Registry。 */
export function createMockMcpToolRegistry(signal?: AbortSignal) {
  return createMcpToolRegistry(new MockMcpClient(), signal);
}
