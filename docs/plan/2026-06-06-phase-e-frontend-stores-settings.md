# Phase E — 前端 IPC 包装 + zustand + Settings 页

> 创建日期: 2026-06-06
> 目标: 前端有 typed `tauri.*` API、4 个 zustand stores、Settings 页可完成 workspace + Anthropic key 配置。

## 前置
- Phase D 完成(workspace + fs 可用)

## 步骤

1. **`src/lib/types.ts`** — Rust struct ↔ TS 类型对齐:
   ```ts
   export type MemoryKind = "user" | "feedback" | "project" | "reference";
   export type Provider = "anthropic"; // v1 仅 anthropic,future 扩展

   export interface MemoryRow { id: string; name: string; kind: MemoryKind; description?: string; content: string; metadata: Record<string, unknown>; workspace?: string; createdAt: string; updatedAt: string; }
   export interface FsReadResult { content: string; truncated: boolean; originalSize: number; }
   export interface DirEntry { name: string; path: string; isDir: boolean; isFile: boolean; size?: number; modified?: string; }
   export interface EditReport { replacements: number; beforeExcerpt: string; afterExcerpt: string; }
   export interface ShellResult { stdout: string; stderr: string; code: number | null; timedOut: boolean; }
   export interface Session { id: string; title: string; workspacePath?: string; provider?: string; model?: string; createdAt: string; updatedAt: string; }
   export interface PersistedMessage { id: string; sessionId: string; role: "system"|"user"|"assistant"|"tool"; content: unknown; toolCalls?: unknown; toolResults?: unknown; stepIndex?: number; createdAt: string; }
   export interface AppError { code: string; message: string; }

   // Agent event (Phase G 详细定义,这里先列)
   export type AgentEvent =
     | { type: "TextDelta"; delta: string }
     | { type: "TextDone" }
     | { type: "ToolCall"; id: string; name: string; input: unknown }
     | { type: "ToolResult"; id: string; result: unknown }
     | { type: "Error"; message: string }
     | { type: "Finish"; usage: unknown };
   ```

2. **`src/lib/tauri.ts`** — typed 包装:
   ```ts
   import { invoke } from "@tauri-apps/api/core";
   import { Channel } from "@tauri-apps/api/core";

   export const tauri = {
     workspace: {
       select: () => invoke<string>("select_workspace"),
       get:    () => invoke<string | null>("get_workspace"),
       set:    (path: string) => invoke<string>("set_workspace", { path }),
     },
     fs: {
       read:    (path: string) => invoke<FsReadResult>("fs_read", { path }),
       write:   (path: string, content: string, createDirs = true) => invoke<void>("fs_write", { path, content, createDirs }),
       edit:    (path: string, search: string, replace: string, all = false) => invoke<EditReport>("fs_edit", { path, search, replace, all }),
       listDir: (path: string) => invoke<DirEntry[]>("fs_list_dir", { path }),
       glob:    (pattern: string) => invoke<string[]>("fs_glob", { pattern }),
       exists:  (path: string) => invoke<boolean>("fs_exists", { path }),
     },
     key: {
       set:            (provider: string, key: string) => invoke<void>("key_set", { provider, key }),
       exists:         (provider: string) => invoke<boolean>("key_exists", { provider }),  // 不返回 key 本身!
       delete:         (provider: string) => invoke<void>("key_delete", { provider }),
       listProviders:  () => invoke<string[]>("key_list_providers"),
     },
     memory: {
       save:   (input: MemorySaveInput) => invoke<MemoryRow>("memory_save", { input }),
       recall: (query: string, limit = 8, kind?: MemoryKind) => invoke<MemoryRow[]>("memory_recall", { query, limit, kind }),
       list:   (kind?: MemoryKind) => invoke<MemoryRow[]>("memory_list", { kind }),
       delete: (id: string) => invoke<void>("memory_delete", { id }),
     },
     session: {
       create:        (title: string) => invoke<Session>("session_create", { title }),
       list:          () => invoke<Session[]>("session_list"),
       update:        (id: string, patch: Partial<Session>) => invoke<Session>("session_update", { id, patch }),
       delete:        (id: string) => invoke<void>("session_delete", { id }),
       loadMessages:  (id: string) => invoke<PersistedMessage[]>("session_load_messages", { sessionId: id }),
     },
     agent: {
       send: (sessionId: string, content: string, onEvent: (e: AgentEvent) => void) => {
         const channel = new Channel<AgentEvent>();
         channel.onmessage = onEvent;
         return invoke<string>("chat_send", { sessionId, content, channel });  // returns run_id
       },
       abort: (runId: string) => invoke<void>("chat_abort", { runId }),
     },
   };
   ```

   **关键**: `key.get` 改成 `key.exists`(返回 boolean),前端永远不直接拿到 key 字符串。Key 只在 Rust 内部的 `chat_send` 流程里用 `keyring::Entry::get_password()` 取出。

3. **zustand stores** (`src/lib/store/`):
   - `workspace.ts`: `{ path, loading, select(), refresh() }`
   - `settings.ts`: `{ provider, model, anthropicKeyExists, setProvider, setModel, refreshKeyStatus }`
   - `sessions.ts`: `{ list, currentId, messages, create, switch, appendLocal, loadMessages, deleteSession }`
   - `ui.ts`: `{ sidebarOpen, toasts, currentRunId, setRunId }`

   **不持久化到 localStorage** — 全部从 SQLite hydrate。

4. **`SettingsPage.tsx`** + 子组件:
   - `WorkspaceSelector`(Phase D 已建)
   - `ProviderPicker`(v1 只有 anthropic,UI 上可显示但 disable 其他)
   - `ApiKeyForm`:
     - 输入框 `<input type="password">`
     - 上方显示 "Key configured" / "Not configured"(基于 `tauri.key.exists("anthropic")`)
     - "Save" → `tauri.key.set("anthropic", key)` → 刷新 exists
     - "Delete" → `tauri.key.delete("anthropic")` → 刷新

5. **`App.tsx`** 增加 tab 切换:
   - sidebar 顶部 Chat / Settings
   - 中部 Sessions 列表(Phase I 完整)
   - 底部 workspace 指示器

## 关键文件
- `src/lib/types.ts`(新)
- `src/lib/tauri.ts`(新)
- `src/lib/store/{workspace,settings,sessions,ui}.ts`(新)
- `src/pages/SettingsPage.tsx`(新)
- `src/components/settings/{ProviderPicker,ApiKeyForm}.tsx`(新)
- `src/App.tsx`(改:加 tab 切换)
- `src-tauri/src/commands/keychain.rs`(改:把 `key_get` 限制内部用,新加 `key_exists` 命令)

## 验证
- [ ] Sidebar 切换 Chat / Settings 流畅
- [ ] ProviderPicker 显示 "Anthropic"(其他灰)
- [ ] ApiKeyForm 输入 → Save → 显示 "Key configured"
- [ ] 重启 app,workspace + 显示 "Key configured" 仍在
- [ ] devtools: `invoke('key_get', {provider:"anthropic"})` **不应被前端调用**(检查代码 grep 一下)

## 风险/陷阱
- `key_get` 仍然存在(Rust 内部用),但**不要在 `tauri.ts` 包装**,也不要在 `generate_handler!` 暴露,可以改成 `pub(crate)` 普通函数。`tauri::generate_handler!` 只列 `key_set`/`key_exists`/`key_delete`/`key_list_providers`。
- zustand store 的 `refresh()` 注意依赖管理,避免无限重渲染。
- Tauri 2 的 `Channel<T>` 类型:确保 `@tauri-apps/api` ≥ 2.x