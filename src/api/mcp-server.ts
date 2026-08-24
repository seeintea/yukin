import { invoke } from "@tauri-apps/api/core";

import type { DeleteRequest, McpServer, SetEnabledRequest } from "#/protocol/mcp-server";

export function mcpServerImport(): Promise<McpServer | null> {
  return invoke("mcp_server_import");
}

export function mcpServerList(): Promise<McpServer[]> {
  return invoke("mcp_server_list");
}

export function mcpServerSetEnabled(request: SetEnabledRequest): Promise<McpServer> {
  return invoke("mcp_server_set_enabled", { request });
}

export async function mcpServerDelete(request: DeleteRequest): Promise<void> {
  await invoke("mcp_server_delete", { request });
}
