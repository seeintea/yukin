import type { RecordMetadata } from "#/protocol/common";

export type McpServerType = "node" | "python" | "binary" | "uv";

export interface DeclaredTool {
  name: string;
  description: string;
}

export interface McpConfigField {
  name: string;
  title: string;
  description: string;
  fieldType: string;
  required: boolean;
  sensitive: boolean;
}

export interface McpServer extends RecordMetadata {
  id: string;
  name: string;
  displayName: string | null;
  version: string;
  description: string;
  authorName: string;
  serverType: McpServerType;
  enabled: boolean;
  declaredTools: DeclaredTool[];
  configFields: McpConfigField[];
}

export interface SetEnabledRequest {
  id: string;
  enabled: boolean;
}

export interface DeleteRequest {
  id: string;
}
