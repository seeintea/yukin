# Phase B — Rust Foundation (依赖 + 错误 + 状态 + 模块骨架)

> 创建日期: 2026-06-06
> 目标: 装齐 Rust 依赖,建好 `AppError`、`AppState`、`commands/*` 模块、`llm/*`、`tools/*`、`agent/*` 骨架。每个命令返回占位,但 `cargo check` 通过、`pnpm tauri dev` 可启动。

## 前置
- Phase A 完成

## 步骤

1. **追加 Cargo 依赖** (`src-tauri/Cargo.toml`):
   ```toml
   tauri-plugin-dialog = "2"
   tauri-plugin-sql    = { version = "2", features = ["sqlite"] }
   tokio       = { version = "1", features = ["full"] }
   tokio-util  = "0.7"                           # CancellationToken
   sqlx        = { version = "0.8", features = ["runtime-tokio","sqlite","macros","chrono","json"] }
   keyring     = "3"
   reqwest     = { version = "0.12", features = ["json","rustls-tls","stream"], default-features = false }
   eventsource-stream = "0.2"                    # SSE 解析
   futures     = "0.3"
   async-trait = "0.1"
   schemars    = { version = "0.8", features = ["chrono"] }
   serde       = { version = "1", features = ["derive"] }
   serde_json  = "1"
   anyhow      = "1"
   thiserror   = "1"
   tracing     = "0.1"
   tracing-subscriber = { version = "0.3", features = ["env-filter"] }
   chrono      = { version = "0.4", features = ["serde"] }
   uuid        = { version = "1", features = ["v4","serde"] }
   glob        = "0.3"
   path-clean  = "1"
   once_cell   = "1"
   ```

2. **`src-tauri/src/error.rs`**:
   ```rust
   #[derive(thiserror::Error, Debug)]
   pub enum AppError {
       #[error("workspace not set")] NoWorkspace,
       #[error("path escapes workspace: {0}")] PathEscape(String),
       #[error("io: {0}")] Io(#[from] std::io::Error),
       #[error("db: {0}")] Db(#[from] sqlx::Error),
       #[error("keyring: {0}")] Keyring(#[from] keyring::Error),
       #[error("dialog cancelled")] DialogCancelled,
       #[error("shell: {0}")] Shell(String),
       #[error("http: {0}")] Http(#[from] reqwest::Error),
       #[error("llm: {0}")] Llm(String),
       #[error("cancelled")] Cancelled,
       #[error("{0}")] Other(String),
   }
   impl serde::Serialize for AppError { /* { code, message } */ }
   pub type AppResult<T> = std::result::Result<T, AppError>;
   ```

3. **`src-tauri/src/state.rs`**:
   ```rust
   pub struct AppState {
       pub workspace: tokio::sync::RwLock<Option<PathBuf>>,
       pub db: sqlx::SqlitePool,                                   // Phase C 真初始化
       pub http: reqwest::Client,
       pub runs: tokio::sync::RwLock<HashMap<String, CancellationToken>>,  // run_id → token
   }
   impl AppState {
       pub async fn new(app: &AppHandle) -> AppResult<Self> { /* Phase C 实现 */ }
   }
   ```

4. **`src-tauri/src/path_safety.rs`**: 函数签名 + `unimplemented!()`,Phase D 实现+测试

5. **模块骨架(每文件就 mod 声明 + 占位)**:
   ```
   src-tauri/src/
   ├── lib.rs
   ├── error.rs / state.rs / path_safety.rs
   ├── commands/
   │   ├── mod.rs
   │   ├── workspace.rs / fs.rs / keychain.rs / memory.rs
   │   ├── session.rs / agent.rs
   ├── llm/
   │   ├── mod.rs           (LlmProvider trait + ChatMessage / LlmEvent types)
   │   ├── anthropic.rs     (impl LlmProvider — Phase F)
   ├── tools/
   │   ├── mod.rs           (Tool trait + ToolRegistry)
   │   ├── fs_tool.rs / memory_tool.rs / shell_tool.rs / http_tool.rs
   ├── agent/
   │   ├── mod.rs
   │   ├── loop.rs          (run_agent — Phase G)
   │   ├── events.rs        (AgentEvent enum)
   ```

   每个 `#[tauri::command]` 返回 `Err(AppError::Other("todo".into()))`。

6. **`lib.rs`** 整理:
   - 删 `greet`
   - 初始化 `tracing_subscriber`
   - plugins: `tauri_plugin_opener::init()`, `tauri_plugin_dialog::init()`, `tauri_plugin_sql::Builder::default().build()`
   - setup hook: `tauri::async_runtime::block_on(async { let state = AppState::new(&handle).await?; app.manage(state); })`
   - `tauri::generate_handler![ … ]` 列出所有命令

7. **capabilities** (`src-tauri/capabilities/default.json`):
   ```json
   { "permissions": ["core:default","opener:default","dialog:allow-open"] }
   ```
   不需要 `sql:default` —— SQL 全在 Rust。

8. **CSP** (`src-tauri/tauri.conf.json` `security.csp`) — **全锁死**(关键差异!):
   ```
   default-src 'self' tauri: https://tauri.localhost;
   connect-src 'self' ipc: https://ipc.localhost;
   img-src 'self' data: https:;
   style-src 'self' 'unsafe-inline';
   script-src 'self';
   font-src 'self' data:
   ```
   不需要 `api.anthropic.com` 等 host —— 因为前端不调外网。

## 关键文件

- `src-tauri/Cargo.toml`(改)
- `src-tauri/src/{error,state,path_safety}.rs`(新)
- `src-tauri/src/commands/{mod,workspace,fs,keychain,memory,session,agent}.rs`(新)
- `src-tauri/src/llm/{mod,anthropic}.rs`(新,骨架)
- `src-tauri/src/tools/{mod,fs_tool,memory_tool,shell_tool,http_tool}.rs`(新,骨架)
- `src-tauri/src/agent/{mod,loop,events}.rs`(新,骨架)
- `src-tauri/src/lib.rs`(改)
- `src-tauri/capabilities/default.json`(改)
- `src-tauri/tauri.conf.json`(改 CSP)

## 验证
- [ ] `cd src-tauri && cargo check` 通过
- [ ] `pnpm tauri dev` 启动成功
- [ ] devtools: `await __TAURI__.core.invoke('get_workspace')` 返回 `null` 或 `{code:"other", message:"todo"}`
- [ ] devtools Network 看 CSP header,无 `api.anthropic.com` 字样

## 风险/陷阱
- 第一次 `cargo build` 拉依赖 5-10 分钟
- `tokio-util` 的 `CancellationToken` 在 trait 边界要 `Send + 'static`,设计时注意
- `eventsource-stream` 替代品: `reqwest-eventsource`,看哪个 stream API 更顺手