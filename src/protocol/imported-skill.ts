import type { RecordMetadata } from "#/protocol/common";

export interface ImportedSkill extends RecordMetadata {
  id: string;
  name: string;
  description: string;
  sourceKind: "directory" | "archive";
  contentDigest: string;
  enabled: boolean;
}

export interface SetEnabledRequest {
  id: string;
  enabled: boolean;
}

export interface DeleteRequest {
  id: string;
}
