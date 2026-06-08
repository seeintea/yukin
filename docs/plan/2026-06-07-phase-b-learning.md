# Phase B — Rust Foundation · Learning Track

> 创建日期: 2026-06-07
> 配套: [phase-b-rust-foundation](./2026-06-06-phase-b-rust-foundation.md)(架构与交付物定义,**不在此重复**)
> 用途: **学习导向**地推进 Phase B 的迭代日志 + 进度跟踪。可在任意设备/任意代理上接力继续。

---

## Handoff(给接手的代理读)

接手时请先做这 3 件事:

1. **读两份文档**:

   - 本文件:**你正在落地的内容、节奏、当前进度**
   - 同目录 `2026-06-06-phase-b-rust-foundation.md`:**Phase B 的架构定义与最终交付物**(本文件不重复列依赖、文件、CSP 等内容,需要时回去看)
2. **核对进度**:扫下面"子步骤进度"的 checkbox,找到第一个未打钩的步骤即下一步入口。看那一步的"如有疑问/卡点"小节,确认是否已经被解决。
3. **遵守教学契约**(下一节)。如果用户没改变意图,**不要主动用 Edit/Write 写 Rust 代码到 `src-tauri/`**,除非用户明说"这段你来写"或要求 review 时改某行。

---

## 教学契约(与用户约定)

- 用户的首要目标是**学 Rust**,不是尽快出空骨架。
- 每一步的循环:**概念讲解(代理)→ 文档/示例指路(代理)→ 代码实现(用户)→ Review + 追问(代理)**。
- 代理职责:讲清"为什么这样写",指 crate doc / Tauri example 的具体章节,review 用户提交的代码,抓住可教学的点继续讲。
- 代理**不**主动写 `src-tauri/` 下的 Rust 代码,除非用户明确要求。
- 用户钦定的学习重点:
  1. **错误处理 + `?` 运算符**(`thiserror` / `From` / `AppResult`)
  2. **async / tokio / 并发**(`RwLock` / `mpsc` / `CancellationToken` / `Send + 'static`)
  3. 其它(ownership / trait / 模块系统 / Tauri 集成宏)同等深入
- 不另开 `examples/` —— 每一行代码都是 Phase B 真实需要的代码。

---

## 子步骤进度

每步状态: `[ ]` 未开始 / `[~]` 进行中 / `[x]` 完成。
完成一步要做的:打钩、填"完成日期"、把"实际收获 / 踩坑"补在本节末。

### B1 — Cargo 依赖 + 第一次 `cargo check`(机械步,热身)

- 状态: `[x]`
- 完成日期: ——
- 目标:`src-tauri/Cargo.toml` 按 phase-b doc 第 1 节追加 19 个依赖;`cd src-tauri && cargo check` 通过。
- 教学点:Cargo features 语义、`default-features = false`、为什么 `reqwest` 选 `rustls-tls` 而不是 `native-tls`、`tokio` 的 `features = ["full"]` 实际开了什么。
- 指路:[Cargo Book — Features](https://doc.rust-lang.org/cargo/reference/features.html)、[reqwest doc — TLS backends](https://docs.rs/reqwest/latest/reqwest/#optional-features)。

### B2 — `error.rs`:`AppError` + `?` + `From` trait(**学习重头戏 1**)

- 状态: `[x]`
- 完成日期: 2026-06-08
- **概念课文档**:[learning-notes/B2-error-handling.md](./learning-notes/B2-error-handling.md) — 任务、自检、易错点都在里面
- 目标:`src-tauri/src/error.rs` 落地 `AppError` 全部变体 + 手写 `impl Serialize` 让前端拿 `{code, message}`;在临时 `_test` 函数里验证 `?` 能自动转换 `io::Error`。
- 教学点:
  - `#[derive(thiserror::Error)]` 宏展开后等价于什么(对照手写 `impl std::error::Error + Display`)
  - `#[from]` 自动生成 `From<X> for AppError` 的机制
  - `?` 脱糖:`x?` ≡ `match x { Ok(v) => v, Err(e) => return Err(From::from(e)) }` —— 这就是 `?` 能跨类型工作的原因
  - `AppResult<T>` type alias 的取舍
  - 为什么要手写 `Serialize`:Tauri 命令 `Err` 返回值序列化到前端,默认实现不友好
- 指路:[thiserror doc](https://docs.rs/thiserror/latest/thiserror/)、[Rust by Example — `?`](https://doc.rust-lang.org/rust-by-example/error/result/enter_question_mark.html)、[serde — custom Serialize](https://serde.rs/impl-serialize.html)。

### B3 — 模块系统 + `commands/*` 骨架(机械 + ownership 入门)

- 状态: `[x]`
- 完成日期: 2026-06-08
- **概念课文档**:[learning-notes/B3-modules-and-skeleton.md](./learning-notes/B3-modules-and-skeleton.md)
- 目标:按 phase-b doc 第 5 节建全部 `commands/`、`llm/`、`tools/`、`agent/`、`path_safety.rs` 文件;每个 `#[tauri::command]` 函数体写 `Err(AppError::Other("todo".into()))`;`cargo check` 通过。
- 教学点:
  - `mod` 声明 vs `use` 引入;`mod.rs` 模式 vs `xxx.rs + xxx/` 模式(Rust 2018+ 两种都行)
  - `pub` / `pub(crate)` / `pub(super)` 可见性分级
  - 参数 `&str` vs `String` vs `&String`:借用 vs 拥有的肌肉记忆从这里开始
  - `.into()` 是 `From` 的反向调用 —— 跟 B2 串起来
- 指路:[Rust Book Ch.7 — 模块系统](https://doc.rust-lang.org/book/ch07-00-managing-growing-projects-with-packages-crates-and-modules.html)。

### B4 — `state.rs`:`AppState` + `Arc<RwLock>` + `Send + Sync`(**学习重头戏 2 · 并发模型**)

- 状态: `[x]`
- 完成日期: 2026-06-08
- **概念课文档**:[learning-notes/B4-state-and-concurrency.md](./learning-notes/B4-state-and-concurrency.md) — 任务、自检、决策记录都在里面
- 目标:`src-tauri/src/state.rs` 定义 `AppState` 结构 + `impl AppState { pub async fn new(...) -> AppResult<Self> { unimplemented!() } }`;`cargo check` 过;能用 `app.manage(state)` 注册不报 `Send/Sync` 错误。
- 决策点(必须想清楚再写):**`db: sqlx::SqlitePool` 字段 Phase B 不真初始化**,选一种处理方式:
  - (a) 用 `Option<SqlitePool>`(B5 后续直接 manage,字段 None)
  - (b) `db` 字段先不加,Phase C 再补
  - (c) 用 `OnceCell<SqlitePool>` lazy 初始化
  - 各自代价?选哪个?把你的选择记到本步骤"实际收获"里。
  - **决定**:选 (a) `Option<SqlitePool>`,理由:调用噪音(`.as_ref().ok_or(...)?`)可接受;Phase C 改动局部化(只改 `AppState::new` 内部),不影响 struct 定义和 commands 调用。
- 教学点:
  - 为什么 Tauri `State<T>` 要求 `T: Send + Sync + 'static`
  - `tokio::sync::RwLock` vs `std::sync::RwLock`:跨 `.await` 持锁会发生什么(死锁路径)
  - `RwLock<Option<PathBuf>>` 这种"可选可变全局状态"的常见 pattern
  - `HashMap<String, CancellationToken>` per-run_id 跟踪:为什么这个映射要放进锁里
- 指路:[Tokio tutorial — Shared State](https://tokio.rs/tokio/tutorial/shared-state)、[Tauri — Managed State](https://tauri.app/develop/state-management/)。

### B5 — `lib.rs`:Tauri setup hook + `tracing` + plugin 注册(**学习重头戏 3 · async/Tauri 接缝**)

- 状态: `[x]`
- 完成日期: 2026-06-08
- **概念课文档**:[learning-notes/B5-tauri-setup-and-tracing.md](./learning-notes/B5-tauri-setup-and-tracing.md) — 任务、自检、决策记录都在里面
- 目标:`src-tauri/src/lib.rs` 删 `greet`;初始化 `tracing_subscriber`;注册 `opener` + `dialog` + `sql` 三个 plugin;setup hook 用 `tauri::async_runtime::block_on` 调 `AppState::new` 后 `app.manage`;`generate_handler![...]` 列出 B3 全部 todo 命令;`pnpm tauri dev` 启动主窗口不崩。
- 教学点:
  - `tracing_subscriber::fmt().with_env_filter(...).init()`:`RUST_LOG=yukin=debug` 怎么生效
  - `Builder::setup(|app| { ... })` closure 签名为什么返回 `Result<_, Box<dyn Error>>` 而不是 `AppResult`
  - sync setup 里 `block_on` 的代价 / 有没有更优雅做法
  - `app.manage(state)` 的内部存储(TypeMap)是怎么按类型查找的
  - `generate_handler![...]` 宏展开扫一眼(`cargo expand` 看真相)
  - plugin 注册顺序有没有讲究
- 指路:[Tauri — Lifecycle Hooks](https://tauri.app/develop/calling-rust/#lifecycle-hooks)、[tracing-subscriber EnvFilter doc](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html)。
- **实际收获 / 踩坑**:
  - **AppState::new 选了"选项 B"**(最简 Ok 实现):`workspace=None / db=None / http=Client::new() / runs=HashMap::new()`。理由:窗口能起来 = 整条链路打通,B6 也能继续验证 CSP;Phase C 只改 `AppState::new` 内部把 `db` 从 `None` 改 `Some(pool)`,不影响 struct 定义和调用方。
  - **commands 命名对齐了一遍**:B3 阶段命名跟 phase B 架构文档不一致(`fs_read_text_file` vs `fs_read`、`pick_workspace` vs `select_workspace`、`keychain_get` vs `key_get` 等)。骨架阶段统一改成架构文档命名,后续阶段不再返工。
  - **Tauri 2 `__TAURI__` 全局默认不注入**:devtools console 调试要在 `tauri.conf.json` 加 `app.withGlobalTauri: true`(本次只用一下,B6 之前回退;后续看是否长期保留)。或者用 `const { invoke } = await import('@tauri-apps/api/core')`。
  - **验证通过**:`pnpm tauri dev` 启动 → 终端看到 `INFO yukin: yukin setup complete` → devtools console `invoke('get_workspace')` 返回 `{code:"other", message:"todo"}`。

### B6 — 配置收尾:capabilities + CSP

- 状态: `[ ]`
- 完成日期: ——
- 目标:`src-tauri/capabilities/default.json` 加 `dialog:allow-open`;`src-tauri/tauri.conf.json` `security.csp` 从 `null` 改为 phase-b doc 第 8 节的锁死 CSP;devtools Console 无 CSP 违规;`await __TAURI__.core.invoke('get_workspace')` 返回 `{code:"other", message:"todo"}`。
- 教学点:
  - Tauri v1 `allowlist` → v2 capability 系统的演进
  - CSP 各 directive 含义;`ipc: https://ipc.localhost` 是 Tauri 的特殊 origin
  - 为什么**不**需要 `connect-src https://api.anthropic.com`:架构核心决策,前端不直接调外网
- 指路:[Tauri — Capabilities](https://tauri.app/security/capabilities/)、[MDN — CSP directives](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Security-Policy)。

---

## 验证清单(全部完成后)

与 phase-b 原文档"验证"一致:

- [ ] `cd src-tauri && cargo check` 通过(B1 起每步都要保持通过)
- [ ] `pnpm tauri dev` 启动成功(B5 后)
- [ ] devtools: `await __TAURI__.core.invoke('get_workspace')` 返回 `{code:"other", message:"todo"}`
- [ ] devtools Network 看 CSP header,无 `api.anthropic.com` 字样

---

## 跨设备/跨代理使用提示

- 进度状态本质是这个 markdown 的 checkbox。**每完成一步,改 checkbox + 填完成日期 + 在该步骤末尾追加"实际收获 / 踩坑"小节**(便于以后回看,也便于其它代理理解你的思路偏好)。
- 切换设备时:`git pull` 拿最新进度;接手代理读 Handoff 节就知道在哪。
- 如果某步骤实际拆得不够细 / 顺序需要调整,**直接改本文件**,不要藏在对话里 —— 对话不跨设备,文件跨设备。
- 不要把架构定义抄到这里(那是 `2026-06-06-phase-b-rust-foundation.md` 的职责)。这里只装"学习节奏 + 进度 + 个人收获"。
