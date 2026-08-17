import type { RecordMetadata } from "./common";

export type ApiFormat = "openai" | "anthropic";

export type ReasoningEffort = "none" | "minimal" | "low" | "medium" | "high" | "xhigh" | "max";

export interface ModelPreset {
  modelId: string;
  displayName: string;
  supportsThinking: boolean;
  reasoningEfforts: ReasoningEffort[];
}

export interface ConnectionPreset {
  apiFormat: ApiFormat;
  baseUrl: string;
  models: ModelPreset[];
}

export interface ModelProviderPreset {
  providerKey: string;
  displayName: string;
  connections: ConnectionPreset[];
}

export interface ModelProvider extends RecordMetadata {
  id: string;
  providerKey: string;
  apiFormat: ApiFormat;
  baseUrl: string;
  providerAlias: string;
}

export interface CreateRequest {
  providerKey: string;
  apiFormat: ApiFormat;
  baseUrl: string;
  providerAlias: string;
  apiKey: string;
}

export interface FindRequest {
  id: string;
}

export interface UpdateRequest {
  id: string;
  apiFormat?: ApiFormat;
  baseUrl?: string;
  providerAlias?: string;
}

export interface ReplaceCredentialRequest {
  id: string;
  apiKey: string;
}

export interface DeleteRequest {
  id: string;
}
