// Typed IPC wrappers over Tauri's invoke().
// Group by domain (workspace / fs / key / memory / session) to mirror src-tauri/src/commands/*.

import { invoke } from "@tauri-apps/api/core";

import {
  FsReadResult,
  DirEntry,
  MemoryKind,
  MemoryRow,
  MemorySaveInput,
  MemoryUpdatePatch,
  SessionCreateInput,
  SessionRow,
  SessionUpdatePatch,
  MessageAppendInput,
  MessageRow,
} from "./data";

export const tauri = {
  workspace: {
    get: () => invoke<string | null>("get_workspace"),
    select: () => invoke<string>("select_workspace"),
    set: (path: string) => invoke<string>("set_workspace", { path }),
  },

  fs: {
    read: (path: string) => invoke<FsReadResult>("fs_read", { path }),
    write: (path: string, content: string, createDirs?: boolean) =>
      invoke<void>("fs_write", { path, content, createDirs }),
    edit: (path: string, search: string, replace: string, all?: boolean) =>
      invoke<void>("fs_edit", { path, search, replace, all }),
    listDir: (path: string) => invoke<DirEntry[]>("fs_list_dir", { path }),
    glob: (pattern: string) => invoke<string[]>("fs_glob", { pattern }),
    exists: (path: string) => invoke<boolean>("fs_exists", { path }),
  },

  key: {
    set: (provider: string, key: string) =>
      invoke<void>("key_set", { provider, key }),
    exists: (provider: string) => invoke<boolean>("key_exists", { provider }),
    delete: (provider: string) => invoke<void>("key_delete", { provider }),
    listProviders: () => invoke<string[]>("key_list_providers"),
  },

  memory: {
    save: (input: MemorySaveInput) =>
      invoke<MemoryRow>("memory_save", { input }),
    recall: (query: string, limit?: number, kind?: MemoryKind) =>
      invoke<MemoryRow[]>("memory_recall", { query, limit, kind }),
    list: (kind?: MemoryKind) => invoke<MemoryRow[]>("memory_list", { kind }),
    delete: (id: string) => invoke<void>("memory_delete", { id }),
    update: (id: string, patch: MemoryUpdatePatch) =>
      invoke<MemoryRow>("memory_update", { id, patch }),
  },

  session: {
    create: (input: SessionCreateInput) =>
      invoke<SessionRow>("session_create", { input }),
    list: () => invoke<SessionRow[]>("session_list"),
    update: (id: string, patch: SessionUpdatePatch) =>
      invoke<SessionRow>("session_update", { id, patch }),
    delete: (id: string) => invoke<void>("session_delete", { id }),
    appendMessage: (input: MessageAppendInput) =>
      invoke<MessageRow>("session_append_message", { input }),
    loadMessages: (sessionId: string) =>
      invoke<MessageRow[]>("session_load_messages", { sessionId }),
  },
};

export * from "./data";
