# Phase C — SQLite + Keychain + Session · Learning Track

> 创建日期: 2026-06-08
> 配套: [phase-c-sqlite-keychain-session](./2026-06-06-phase-c-sqlite-keychain-session.md)(架构与交付物定义,**不在此重复**)
> 前置: Phase B 完成(error.rs / state.rs / commands 骨架 / Tauri setup / CSP 全部 done)
> 用途: **学习导向**地推进 Phase C 的迭代日志 + 进度跟踪。可在任意设备/任意代理上接力继续。

---

## Handoff(给接手的代理读)

接手时请先做这 3 件事:

1. **读两份文档**:
   - 本文件:**你正在落地的内容、节奏、当前进度**
   - 同目录 `2026-06-06-phase-c-sqlite-keychain-session.md`:**Phase C 的架构定义与最终交付物**(本文件不重复列 SQL schema、命令签名等内容,需要时回去看)
2. **核对进度**:扫下面"子步骤进度"的 checkbox,找到第一个未打钩的步骤即下一步入口。看那一步的"实际收获 / 踩坑"小节,确认有没有遗留问题。
3. **遵守教学契约**(下一节)。Phase C **节奏是 Phase B 的 1.5–2 倍**,不要催用户。

---

## 教学契约(与 Phase B 同,Phase C 追加 3 条)

复用 Phase B 的所有契约(用户主导写 Rust 代码、代理讲概念 + 指路 + review、不主动写 `src-tauri/` 下代码除非用户明说)。

**Phase C 专属追加**:

- **决策点必须先答再写代码**:C1 / C2 / C3 / C4 / C5 各有显式决策点(详见各步)。**在开工前,先在该步骤"实际收获"小节写下你的选择 + 一两句理由**。
- **错误变体补充流程化**:Phase C 会暴露 `AppError` 当前缺 `serde_json::Error` / 可能缺 `JoinError`。流程是:`?` 报错 `From<X> not implemented for AppError` → 回 `error.rs` 加变体 + `#[from]` → 加 serialize code → 回原文件。这套流程 B2 已讲过,Phase C 是反复巩固。
- **测试写在 `db/` 私有模块里,命令是薄壳**:`commands/memory.rs` 只做 DTO 转换 + 调 `db::memory::save(...)`;测试针对 `db::memory::*`,不 mock `State<AppState>`。这是 C3 早期定下的小架构决策。

---

## 学习重点(钦定)

1. **sqlx hands-on**:`query!` / `query_as` / `query` 三件套 · `#[derive(FromRow)]` · pool 语义 · 离线模式(`.sqlx` 缓存)
2. **sync ↔ async 边界**:`spawn_blocking` · `JoinHandle` · `JoinError` · `Send + 'static` 在这里的真实作用(回看 B4)
3. **serde DTO 设计**:input 与 row 分离 · `Option<T>` · `#[serde(rename_all = "camelCase")]` · `#[serde(default)]`
4. **真实 IO 链路的错误传播**:`?` 在 io / sqlx / keyring / serde_json / JoinError 之间穿透,回看 B2 的 `From`

---

## 全局决策(Phase C 开工前已定)

- **sqlx API 选型** = `query!` 派(编译期校验 SQL + 列名)。代价:需要 `DATABASE_URL` 或 `.sqlx` 离线缓存,Windows 装 `sqlx-cli` 要带特定 feature flag。详见 C1 卡点。
- **测试代码位置** = 单文件内联(`#[cfg(test)] mod tests { ... }` 在 `db/memory.rs` 末尾)。能访问私有 API,文件不散。
- **`db: Option<SqlitePool>`** = 保留 B4 决策,C3 用一个 `fn db(state: &AppState) -> AppResult<&SqlitePool>` 助手减少调用噪音。

---

## 子步骤进度

每步状态: `[ ]` 未开始 / `[~]` 进行中 / `[x]` 完成。
完成一步要做的:打钩、填"完成日期"、把"实际收获 / 踩坑"补在本节末。

### C1 — `migrations/0001_init.sql`:schema 设计 + FTS5 trigger

- 状态: `[x]`
- 完成日期: 2026-06-09
- **概念课文档**:[learning-notes/C1-sqlite-schema-and-fts5.md](./learning-notes/C1-sqlite-schema-and-fts5.md)
- 目标:新建 `src-tauri/migrations/0001_init.sql`,按 phase-c doc 第 11–64 行写 5 张表 + FTS5 虚拟表 + **3 个同步 trigger**(doc 里只写了注释占位,你来补)+ 2 个 index。sqlite CLI 手动跑通 schema。
- 决策点:无(全局决策已在 C1 开工前定)
- 教学点:
  - SQLite 5 个小怪癖(type affinity / `BOOLEAN` 用 `INT` / datetime 字符串 vs epoch / `CHECK` / `foreign_keys` 默认关)
  - FTS5 三件套(虚拟表 + content='memory' 外部内容模式 + tokenizer)
  - FTS5 同步 trigger 完整模板(INSERT / UPDATE / DELETE)
  - sqlx migrations 工作流(`sqlx::migrate!` 宏 / `_sqlx_migrations` 元表 / forward-only)
  - 外键 cascade 需要 `PRAGMA foreign_keys=ON`(C2 加)
- 指路:[SQLite FTS5 docs](https://www.sqlite.org/fts5.html)、[sqlx-cli README](https://github.com/launchbadge/sqlx/tree/main/sqlx-cli)。
- 预估时长:2–3h
- **实际收获 / 踩坑**:
  - **选择偷懒,由代理直接写 schema** —— 跳过手写 SQL 学习,直接拿成品。C2 起回归用户主导节奏。
  - **mac 环境配 sqlx-cli**:`export DATABASE_URL="sqlite:./dev.db"`(不是 PowerShell 的 `$env:DATABASE_URL = "..."`,文档原本是 Win 视角)
  - **`sqlx migrate run` 必须在 `src-tauri/` 下跑** —— 它默认从 cwd 找 `migrations/` 目录,不在则报 `error canonicalizing path migrations`。可显式 `--source ./migrations` 兜底
  - **验证全通过**:`.tables` 看到 6 张表 (`memory/memory_fts/messages/providers/sessions/settings/_sqlx_migrations`),`type='trigger'` 看到 `memory_ai/ad/au`,3 个 index 齐全

### C2 — `AppState::new` 真实现:连 db / 跑 migration / 回填 workspace

- 状态: `[x]`
- 完成日期: 2026-06-09
- **概念课文档**:[learning-notes/C2-appstate-lifecycle-and-pool.md](./learning-notes/C2-appstate-lifecycle-and-pool.md)
- 目标:把 `state.rs` 里 `AppState::new` 从 `unimplemented!()` 改成真实现。`pnpm tauri dev` 启动后 db 文件存在,schema 已建。
- 决策点(已定 2026-06-09):
  - (i) **`db` 字段改裸 `SqlitePool`** —— 不是 `Option<SqlitePool>`。理由:db 起不来 app 就不该起来,Option 是谎言类型;C2 是改这事的零成本窗口期(0 处调用方)。
  - (ii) **`AppState::new` 拆 `open_db / run_migrations / load_workspace` 三个私有助手** —— 理由:错误定位清晰、可单测、未来 setup 加东西不臃肿。
- 教学点:
  - `app.path().app_data_dir()` 跨平台行为(mac / win / linux 各到哪)
  - `SqlitePoolOptions::max_connections(1)` 与 SQLite 写者全局锁
  - **Windows 连接 URL 路径坑**(反斜杠 / 空格 / `?mode=rwc`)→ 用 `SqliteConnectOptions::filename()` 绕开
  - `sqlx::migrate!("./migrations")` 路径相对 `Cargo.toml` 不是源文件
  - **三条必加 PRAGMA**:`foreign_keys=ON`(per-connection!)、`journal_mode=WAL`、可选 `busy_timeout`
  - 第一次 4 种 IO 错误源 (`io::Error` / `sqlx::Error` / `MigrateError` / `tauri::Error`) 在同一函数链上靠 `?` 穿透 → 需补 `AppError::Migrate` + `AppError::Tauri` 变体
- 指路:[sqlx::pool::PoolOptions](https://docs.rs/sqlx/latest/sqlx/pool/struct.PoolOptions.html)、[Tauri PathResolver](https://docs.rs/tauri/latest/tauri/path/struct.PathResolver.html)。
- 预估时长:2–4h
- **实际收获 / 踩坑**:
  - **两个决策按 C2 概念课定稿**:`db` 裸 `SqlitePool` + `new` 拆 3 助手 (`open_db` / `run_migrations` / `load_workspace`)
  - **`AppError` 加 2 个 `#[from]` 变体** (`Migrate` + `Tauri`) + `Serialize` `code` match 同步更新
  - **跨平台路径用 `SqliteConnectOptions::filename()`** 而非字符串拼 URL,规避 Win 反斜杠/空格坑
  - **PRAGMA 全链式**:`foreign_keys(true).journal_mode(Wal).busy_timeout(5s)`,一行表达连接属性
  - **`load_workspace` 用 `fetch_optional`**,settings 表空时返回 `Ok(None)`,不会 `RowNotFound`
  - **5 个错误源 (`io / sqlx / Migrate / Tauri / Other`) 全靠 `?` 自动透传**,无任何 `.map_err`
  - **验证全过**:`pnpm tauri dev` 启动看到 `opening db` + `migrations applied` 日志,`~/Library/Application Support/xyz.yukin.agent/` 下出现 `yukin.db` + `-wal` + `-shm` 三件套

### C3 — `commands/memory.rs` + DTO + `#[sqlx::test]` 集成测试(**🔥 学习重头戏 1**)

- 状态: `[~]`
- 完成日期: ——
- **概念课文档**:[learning-notes/C3-sqlx-and-dto-and-tests.md](./learning-notes/C3-sqlx-and-dto-and-tests.md)
- 目标:实现 `memory_save / memory_recall / memory_list / memory_delete / memory_update` 5 个命令;建 `db/memory.rs` 放纯 SQL;DTO `MemorySaveInput` / `MemoryUpdate` / `MemoryRow`;补 `AppError::Json` 变体;写至少 3 个 `#[sqlx::test]` 通过。
- 决策点(已定 2026-06-09):
  - (i) **`MemoryKind` 用 enum** + `as_str()` + `#[serde(rename_all = "snake_case")]`。类型安全 + 学 serde rename_all idiom;sqlx 端用 `as_str()` 转字符串,`MemoryRow.kind` 用 `String` 保留(避免 sqlx 反向 decode 复杂度)
  - (ii) **DTO 三个独立类型**:`MemorySaveInput` / `MemoryUpdate` / `MemoryRow`。类型即契约,patch 语义只能用全 Option + `COALESCE` 表达
  - (iii) **FTS5 用户输入原样传 + TODO 注释**。调用方是 agent (LLM),v1 MVP 不需要防御性 sanitize
- 教学点:
  - sqlx 三件套对比表 + 为什么我们选 `query!` 派
  - `#[derive(sqlx::FromRow)]` 工作机制
  - bind 参数 vs SQL 注入(永远不要 `format!` SQL)
  - 4 种 fetch 模式(`execute` / `fetch_one` / `fetch_all` / `fetch_optional`)
  - `.sqlx` 离线缓存(`cargo sqlx prepare` + 入库 git)
  - DTO 设计三原则(input ≠ row / `Option` / `rename_all camelCase`)
  - `#[sqlx::test]` 工作机制(独立临时 sqlite per test,自动跑 migrations)
  - **第一次 `?` 跨 4 种错误源**:`io / sqlx / serde_json / 业务`
  - **chrono ↔ SQLite datetime 格式不兼容**(`datetime('now')` 输出无 T 无时区,默认 RFC3339 解析失败)→ 三种解法选一个
- 指路:[sqlx-macros docs](https://docs.rs/sqlx/latest/sqlx/macro.query.html)、[sqlx Test docs](https://docs.rs/sqlx/latest/sqlx/attr.test.html)。
- 预估时长:**6–10h(分 2–3 个学习时段)。不要中断**
- **细分时段建议**(types 完成后追加):
  - **段 1**(1.5–2.5h):`error.rs` 加 `Json` 变体 + 写 `save` + `fetch` 两个函数。`save` 是最慢的(第一次撞 sqlx Encode / fetch 选型 / 多步 INSERT-then-select),后续函数会快
  - **段 2**(1.5–2.5h):`recall` + `list` + `delete` + `update` 四个函数。`recall` 撞 FTS5 表 sqlx 不识别问题(预期回退 `query_as` runtime 校验)
  - **段 3**(1.5–2.5h):3 个 `#[sqlx::test]` + commands 薄壳 + `lib.rs` 加 `memory_update`
  - **段 4**(1–2h):devtools 端到端联调 5 个命令链路 + 撞坑修
- **节奏建议**:
  - 第一个函数 (`save`) 不追求完美,能跑通就行 —— 后面函数会让你看清更好的写法,回头重构
  - 撞错误立刻加 `AppError` 变体,不要 `.map_err()` —— 这是肌肉记忆训练
  - 测试先过最简的 `save_then_recall`,再写 `delete_then_recall` / `update_content`
- **当前进度**(2026-06-09):
  - ✅ `db/mod.rs` + `db/memory.rs` types 部分(`MemoryKind` / `MemorySaveInput` / `MemoryUpdate` / `MemoryRow`)
  - ✅ `error.rs` 加 `Json(#[from] serde_json::Error)` 变体 + Serialize code match
  - ✅ `lib.rs` 加 `mod db;`
  - ⏳ 段 1-4 待完成

### C4 — `commands/keychain.rs` + `spawn_blocking`(**🔥 学习重头戏 2**)

- 状态: `[ ]`
- 完成日期: ——
- **概念课文档**:_待写,C4 开工前再起_(路径 `learning-notes/C4-spawn-blocking-and-keychain.md`)
- 目标:实现 `key_set / key_get / key_delete / key_list_providers` 4 个命令;关键技巧 `tokio::task::spawn_blocking`;`providers` 表 upsert。**注意:当前 stub 是 `key_exists`,C4 要改 `lib.rs::generate_handler!`**。
- 决策点:
  - (i) 给 `AppError` 加 `JoinError(#[from] tokio::task::JoinError)` 变体,还是 `.map_err(|e| AppError::Other(...))`?
  - (ii) `keyring::Error::NoEntry` 是错误还是 `Ok(None)`?(语义上是后者)
- 教学点:
  - tokio runtime 两类线程(worker vs blocking pool)
  - 什么算"阻塞"(同步 IO / 系统调用 / CPU 密集 / `std::thread::sleep`)
  - `spawn_blocking` 签名拆解(`FnOnce + Send + 'static` / `R: Send + 'static`)**← 回看 B4**
  - `JoinHandle<R>` / `JoinError` / **双层 Result 三种拆法**
  - 平台差异表(macOS Keychain / Windows Credential Manager / Linux libsecret)
  - SQLite `ON CONFLICT DO UPDATE` upsert 语法(跟 PG/MySQL 不同)
  - 为什么 `key_list_providers` 查 db 而不是问 keyring(keyring 没列举 API)
- 指路:[tokio::task::spawn_blocking](https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html)、[keyring crate](https://docs.rs/keyring/latest/keyring/)。
- 预估时长:3–5h

### C5 — `commands/session.rs` + JSON content + 1:N cascade

- 状态: `[ ]`
- 完成日期: ——
- **概念课文档**:_待写,C5 开工前再起_(路径 `learning-notes/C5-sessions-and-json-content.md`)
- 目标:实现 6 个 session 命令(`session_create / session_list / session_update / session_delete / session_append_message / session_load_messages`);新增 4 个 DTO;补 2 个 `#[sqlx::test]`(`create_then_list` / `delete_cascades_messages`)。
- 决策点:
  - (i) `messages.content` 用 `String` 还是 `serde_json::Value`?(推介 `Value` + `#[sqlx(json)]`)
  - (ii) `session_update` 全 None 时直接 `Ok(())` 还是 `Err`?
- 教学点:
  - 1:N + cascade 真实测试(`delete_cascades_messages` 验证 C2 的 PRAGMA 真生效)
  - `String` vs `serde_json::Value` 字段取舍
  - patch 语义(`Option<String>` + `COALESCE(?1, title)` pattern)
  - `tool_calls` / `tool_results` 字段 Phase G 才填,`Option<Value>` + `skip_serializing_if`
- 指路:[sqlx json feature docs](https://docs.rs/sqlx/latest/sqlx/types/struct.Json.html)。
- 预估时长:1.5–3h

---

## 验证清单(全部完成后)

与 phase-c 原文档"验证"一致:

- [ ] `~/Library/Application Support/xyz.yukin.agent/yukin.db`(mac)/ `%APPDATA%/xyz.yukin.agent/yukin.db`(win) 启动后存在
- [ ] devtools: `invoke('memory_save', {input:{name:"t",kind:"user",content:"hello",metadata:{}}})` 返回 id
- [ ] devtools: `invoke('memory_recall', {query:"hello"})` 返回数组
- [ ] DB Browser 打开 yukin.db 验证 memory 行
- [ ] `invoke('key_set', {provider:"anthropic", key:"sk-ant-test"})` 无错
- [ ] Windows Credential Manager / mac Keychain Access 搜 "xyz.yukin.agent" 显示条目 "anthropic"
- [ ] `invoke('key_get', {provider:"anthropic"})` 返回 "sk-ant-test"
- [ ] `invoke('session_create', {title:"test"})` 返回 session 对象
- [ ] `cargo test` 全绿(至少 5 个 `#[sqlx::test]`)

---

## 预估总时长

Phase C 总时长 ≈ Phase B × 1.8。建议分散到 **4–5 个学习时段**。
**C3 必须独占一个完整时段**,中途插断容易丢线。

---

## 跨设备/跨代理使用提示

(同 Phase B,参考 `2026-06-07-phase-b-learning.md` 末尾段。)
