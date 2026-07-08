export type MemoryKind = "user" | "feedback" | "project" | "reference";

export interface MemoryRow {
  id: string;
  name: string;
  kind: string;
  description: string | null;
  content: string;
  metadata: string;
  workspace: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface MemorySaveInput {
  name: string;
  kind: MemoryKind;
  content: string;
  description?: string;
  metadata?: unknown;
  workspace?: string;
}

export interface MemoryUpdatePatch {
  name?: string;
  description?: string;
  content?: string;
  metadata?: unknown;
}

export interface SessionRow {
  id: string;
  title: string;
  workspacePath: string | null;
  provider: string | null;
  model: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface SessionCreateInput {
  title: string;
  workspacePath?: string;
  provider?: string;
  model?: string;
}

export interface SessionUpdatePatch {
  title?: string;
  workspacePath?: string;
  provider?: string;
  model?: string;
}

export interface MessageRow {
  id: string;
  sessionId: string;
  role: "system" | "user" | "assistant" | "tool";
  content: string;
  toolCalls: string | null;
  toolResults: string | null;
  stepIndex: number | null;
  createdAt: string;
}

export interface MessageAppendInput {
  sessionId: string;
  role: "system" | "user" | "assistant" | "tool";
  content: string;
  toolCalls?: string;
  toolResults?: string;
  stepIndex?: number;
}

export interface FsReadResult {
  content: string;
  truncated: boolean;
  originalSize: number;
}

export interface DirEntry {
  name: string;
  path: string;
  isDir: boolean;
  isFile: boolean;
  size: number | null;
}

export interface AppError {
  code: string;
  message: string;
}
