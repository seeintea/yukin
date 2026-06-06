# Phase H — Tools 完整实现 (fs / memory / shell / http)

> 创建日期: 2026-06-06
> 目标: 把所有 v1 tool 实装到 `ToolRegistry`,删除 dummy。端到端 agent 可读写文件、记记忆、跑 shell、抓 URL。

## 前置
- Phase G 完成(loop + dummy tool 跑通)

## 步骤

1. **`src/tools/fs_tool.rs`** — 5 个文件工具,每个对应一个 struct + JsonSchema input:
   - `FsReadTool` → 调 `internal_fs_read`
   - `FsWriteTool` → 调 `internal_fs_write`(input: path/content/create_dirs)
   - `FsEditTool` → 调 `internal_fs_edit`(input: path/search/replace/all)
   - `FsListDirTool` → 调 `internal_fs_list_dir`
   - `FsGlobTool` → 调 `internal_fs_glob`
   - Description 要明确说"path is relative to workspace"、"use `fs_edit` for surgical edits, prefer over `fs_write` when only changing part of the file"

   `internal_*` 函数: Phase D 的 `#[tauri::command]` 抽出共享内核 + 让命令调内核 + 让 tool 也调内核。

2. **`src/tools/memory_tool.rs`** — 3 件:
   - `MemorySaveTool`:
     description: "Persist a fact for future sessions. Use kind='user' for user preferences, 'feedback' for guidance about how to work, 'project' for ongoing work, 'reference' for external pointers (URLs, docs)."
   - `MemoryRecallTool`:
     description: "Full-text search persistent memory by query. Use when answering would benefit from prior facts about the user / project / preferences."
   - `MemoryListTool`(可选,kind 可选过滤)

3. **`src/tools/shell_tool.rs`**:
   ```rust
   #[derive(Deserialize, JsonSchema)]
   struct ShellInput { cmd: String, timeout_ms: Option<u64> }

   pub struct ShellTool;
   #[async_trait]
   impl Tool for ShellTool {
       fn name(&self) -> &str { "shell_exec" }
       fn description(&self) -> &str {
           "Run a shell command in the workspace. Returns stdout/stderr/exit code. Time-limited (default 30s, max 300s). Use for build/test/git commands."
       }
       fn input_schema(&self) -> serde_json::Value { serde_json::to_value(schema_for!(ShellInput)).unwrap() }
       async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> AppResult<serde_json::Value> {
           let i: ShellInput = serde_json::from_value(input)?;
           let root = ctx.state.workspace.read().await.clone().ok_or(AppError::NoWorkspace)?;
           let timeout = Duration::from_millis(i.timeout_ms.unwrap_or(30_000).min(300_000));

           #[cfg(target_os = "windows")]
           let (program, arg) = ("cmd", "/C");
           #[cfg(not(target_os = "windows"))]
           let (program, arg) = ("sh", "-c");

           let fut = tokio::process::Command::new(program)
               .args([arg, &i.cmd]).current_dir(&root)
               .stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?
               .wait_with_output();

           // 同时监听取消
           let result = tokio::select! {
               r = tokio::time::timeout(timeout, fut) => r,
               _ = ctx.cancel.cancelled() => return Err(AppError::Cancelled),
           };

           let (stdout, stderr, code, timed_out) = match result {
               Ok(Ok(o)) => (truncate(o.stdout, 200_000), truncate(o.stderr, 50_000), o.status.code(), false),
               Ok(Err(e)) => return Err(AppError::Shell(e.to_string())),
               Err(_) => (String::new(), "[timeout]".into(), None, true),
           };
           Ok(json!({ "stdout": stdout, "stderr": stderr, "code": code, "timed_out": timed_out }))
       }
   }
   ```

4. **`src/tools/http_tool.rs`**:
   ```rust
   #[derive(Deserialize, JsonSchema)]
   struct HttpInput {
       url: String,
       method: Option<String>,
       headers: Option<HashMap<String, String>>,
       body: Option<String>,
   }
   // 实现要点:
   // - 检查 url 不是 file://
   // - 阻断内网地址(localhost、127.0.0.0/8、10/8、172.16/12、192.168/16),除非 ctx 提示允许
   // - body 大小限制 5MB,超出截断
   // - 默认 method=GET
   // - 返回 { status, headers, body }
   ```

5. **`build_tool_registry()`** 注册全部:
   ```rust
   pub fn build_tool_registry() -> ToolRegistry {
       let mut r = ToolRegistry::new();
       r.register(Box::new(FsReadTool));
       r.register(Box::new(FsWriteTool));
       r.register(Box::new(FsEditTool));
       r.register(Box::new(FsListDirTool));
       r.register(Box::new(FsGlobTool));
       r.register(Box::new(MemorySaveTool));
       r.register(Box::new(MemoryRecallTool));
       r.register(Box::new(ShellTool));
       r.register(Box::new(HttpTool));
       r
   }
   ```

6. **前端 `ToolCallView.tsx`** 完整 UI:
   - 折叠头: 工具 icon + name + 状态 dot(spinner/check/x)+ args 一行 preview
   - 展开: args JSON + result(代码块或 markdown 渲染)
   - 截断提示: 若 `truncated: true` 或 `timed_out: true` 显示警告条
   - 错误状态: `is_error: true` 红色高亮

7. **System prompt 优化** (`build_system_prompt`):
   ```
   You are Yukin, an autonomous coding/assistant agent running locally on the user's machine.

   ## Workspace
   Working directory: {workspace_path or "<not set — ask user to select via Settings>"}

   ## Available tools
   - fs_read, fs_write, fs_edit, fs_list_dir, fs_glob (workspace-scoped)
   - memory_save, memory_recall (cross-session persistent memory)
   - shell_exec (run shell commands, time-limited)
   - http_fetch (make HTTP requests)

   ## Guidelines
   - Reference files as `path:line` (clickable).
   - For surgical edits prefer `fs_edit` (give a unique substring) over `fs_write` (overwrites entire file).
   - Save lasting facts about the user / project with `memory_save`.
   - Before destructive operations (rm, force push), confirm with the user.
   - Be concise; the user sees your text in a terminal-like UI.
   ```

## 关键文件
- `src-tauri/src/tools/{fs_tool,memory_tool,shell_tool,http_tool}.rs`(实装)
- `src-tauri/src/commands/{fs,memory}.rs`(把内核函数抽 `pub(crate) async fn internal_*`)
- `src-tauri/src/agent/loop.rs`(改 `build_tool_registry` 调用)
- `src-tauri/src/agent/prompts.rs`(新,`build_system_prompt`)
- `src/components/chat/ToolCallView.tsx`(完整 UI)

## 端到端验证(每条都该一次成功)
- [ ] "list files in the workspace root" → fs_list_dir 卡片 + 文件列表
- [ ] "read package.json and tell me dependencies" → fs_read + 摘要
- [ ] "create hello.txt with 'Hello yukin'" → fs_write,文件出现在磁盘
- [ ] "in hello.txt replace Hello with Hi" → fs_edit,文件变 "Hi yukin"
- [ ] "find all .ts files" → fs_glob 返回数组
- [ ] "remember that I prefer pnpm" → memory_save 写入,DB Browser 看到行
- [ ] "what package manager do I prefer?" → memory_recall 命中(同会话也能)
- [ ] "run `ls -la` and summarize" → shell_exec(注意 sandbox 第一次 macOS 可能需授权)
- [ ] "fetch https://api.github.com/repos/tauri-apps/tauri and tell me stargazers" → http_fetch + 解析
- [ ] 上述任一长输出(如大目录 ls -R)被截断,UI 显示 "[truncated]"
- [ ] Stop 按钮在 shell_exec 进行中按下,Rust 端 select! 立刻丢弃任务

## 风险/陷阱
- `internal_fs_*` 内核函数共享给命令(direct invoke)和工具(via Tool trait)— 一处实现两处用,避免重复
- shell_exec 在 cancel 时 `kill` 子进程: 用 `Command::spawn()` 返回的 `Child` 持有,select! cancel 分支调 `child.kill().await`(避免孤儿进程)
- http_fetch 默认 deny 内网防 SSRF;后续 v2 加 `allow_local` 参数
- 工具描述要足够具体,模型才能选对工具 —— 描述里要含使用边界、典型用法、参数语义