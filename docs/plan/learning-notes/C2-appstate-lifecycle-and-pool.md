# C2 — `AppState::new` 真实现:连 db / 跑 migration / 回填 workspace(概念课)

> 创建日期: 2026-06-09
> 配套: [phase C 学习总入口](../2026-06-08-phase-c-learning.md) / [phase C 架构定义](../2026-06-06-phase-c-sqlite-keychain-session.md)
> 用途: C2 的概念讲解 + 任务清单 + 自检步骤。学完后回主入口打钩。

---

C1 写了 schema,但 schema 只是磁盘上一份 `.sql` 文件。C2 要让 Rust 进程**真正打开 db 文件、跑迁移、回填 workspace 状态**,让 `AppState` 从"占位"变成"真用得起来"。

这一步是 **Phase C 的承重墙** —— C3/C4/C5 全部依赖 `AppState::db` 是个真的 `SqlitePool`。

分 8 节讲:

1. C2 已做出的两个决策(开工前必须看)
2. `app.path().app_data_dir()` 跨平台行为
3. `SqlitePoolOptions` 与 SQLite 的并发模型
4. **SQLite 连接 URL 的两个坑**(`?mode=rwc` + Windows 路径)
5. `sqlx::migrate!()` 编译期魔法 + 路径相对 `Cargo.toml`
6. **三条必加的 `PRAGMA`**(架构 doc 漏写)
7. `?` 在 3 种错误源之间穿透 (io / sqlx / MigrateError)
8. 拆 3 个私有助手 vs 一坨长函数

然后:任务、验证、卡点。

---

## 1. C2 已做出的两个决策(已 ok)

### 决策 (i):`db` 字段 — `Option<SqlitePool>` → 裸 `SqlitePool`

**B4 当时选 `Option<SqlitePool>`** 的理由是"延后 db 初始化的决定空间"。C2 决定**反悔**:

- `AppState::new` 在 setup hook 同步初始化 db
- db 起不来 = setup 失败 = app 起不来
- 既然 `AppState` 存在的前提就是"db 已经就位",`Option` 是个**谎言类型**

**改:**

```rust
pub struct AppState {
    pub workspace: RwLock<Option<PathBuf>>,
    pub db: sqlx::SqlitePool,            // ← 裸,不是 Option<...>
    pub http: reqwest::Client,
    pub runs: RwLock<HashMap<String, CancellationToken>>,
}
```

**收益**:

- C3/C4/C5 写命令时直接 `&state.db`,不再写 `state.db.as_ref().ok_or(...)?`
- 0 处调用方 → C2 是改这事的零成本窗口期
- 类型反映真实语义(创建即就绪)

**代价**:几乎为 0。理论上未来要 lazy init 再改回 `Option` 需要改 N 处调用方,但当前没有这个需求,YAGNI。

### 决策 (ii):`AppState::new` 拆 3 个私有助手

不写成一坨长函数,拆成:

```rust
impl AppState {
    pub async fn new(app: &AppHandle) -> AppResult<Self> {
        let pool = Self::open_db(app).await?;
        Self::run_migrations(&pool).await?;
        let workspace = Self::load_workspace(&pool).await?;

        Ok(Self {
            workspace: RwLock::new(workspace),
            db: pool,
            http: reqwest::Client::new(),
            runs: RwLock::new(HashMap::new()),
        })
    }

    async fn open_db(app: &AppHandle) -> AppResult<sqlx::SqlitePool> { ... }
    async fn run_migrations(pool: &sqlx::SqlitePool) -> AppResult<()> { ... }
    async fn load_workspace(pool: &sqlx::SqlitePool) -> AppResult<Option<PathBuf>> { ... }
}
```

**收益**:

- `new` 主函数像一份目录,5 行业务流程清晰
- 报错栈直接告诉你 "open_db 失败" / "run_migrations 失败",不只是 "?在 line N"
- C3 写 `#[sqlx::test]` 时,可以单独测 `load_workspace`(它是个纯函数:pool 进 → Option<PathBuf> 出)
- 未来在 setup 加东西(metrics / health check / 读 last_session_id …)往对应助手里加,主函数不臃肿

**结构性开销**:多 3 个 `async fn` 签名,函数体三五行。**没有抽象成本,只有定义成本**。

---

## 2. `app.path().app_data_dir()` 跨平台行为

Tauri 提供 `PathResolver`(通过 `app.path()`)统一三大平台的"应用数据目录":

| 平台 | `app_data_dir()` 返回 |
|------|---------------------|
| macOS | `~/Library/Application Support/<bundle_identifier>/` |
| Linux | `~/.local/share/<bundle_identifier>/` |
| Windows | `C:\Users\<user>\AppData\Roaming\<bundle_identifier>\` |

`<bundle_identifier>` 是 `tauri.conf.json` 的 `identifier`,我们是 `xyz.yukin.agent`。

```rust
// 用法
use tauri::Manager;     // ← B7 你已经在 state.rs 加过这个 import 了
let data_dir = app.path().app_data_dir()?;
// macOS: /Users/yukkuri/Library/Application Support/xyz.yukin.agent
```

### 关键事实

- **目录可能不存在** —— 第一次启动 app 时 Tauri 不保证已经建好。**你必须自己 `std::fs::create_dir_all(&data_dir)?`**
- **返回 `Result<PathBuf, tauri::Error>`** —— 因为某些极端环境(如沙箱里禁了 HOME)可能拿不到。`?` 借 `From<tauri::Error> for AppError` 透传
  - **当前 `AppError` 还没有 `Tauri` 变体** —— 这是个**新错误源**,你需要在 `error.rs` 加一个变体(下面"任务"节再讲具体怎么加)
- **不要硬编码路径** —— 平台无关性是 Tauri 的核心价值,任何 `~/Library/...` 字面量都是耦合

### 为什么不直接用 `dirs` crate?

[dirs](https://crates.io/crates/dirs) 也能给你 "data dir",但**不考虑 bundle identifier**,需要你自己拼。Tauri 的 `PathResolver` 帮你拼好了。**用 Tauri 的**,跟 app 配置一致。

---

## 3. `SqlitePoolOptions` 与 SQLite 并发模型

### `sqlx::SqlitePool` 是什么

一个**连接池**,内部维护若干 SQLite 连接。`sqlx::query!(...).fetch_all(&pool)` 自动借一个连接、用完归还。pool 本身是 `Send + Sync + Clone`(内部 `Arc`),可以随便 `clone()` 给各任务用。

### `max_connections(1)` 为什么这么小?

SQLite 在写操作时**有进程级互斥锁**:
- 多个连接可以**同时读**
- **只能一个连接同时写** —— SQLite 用 `BEGIN IMMEDIATE` 锁住数据库文件,其它写连接阻塞等

我们项目里:

```rust
SqlitePoolOptions::new()
    .max_connections(1)
    .connect(&db_url)
    .await?
```

`max_connections(1)` 意思:**整个池子就一个连接**。所有 query 串行排队走这一个连接。

**为什么?**

- 我们是单用户桌面应用,并发压力小
- SQLite 写锁本来就限制并发,与其让 5 个连接互相抢锁,不如 1 个连接顺序排队(更可预测)
- 避免 "database is locked" 错误的复杂处理
- WAL 模式(下一节讲)能在写的时候允许并发读,但我们就 1 连接也无所谓

如果你想要更高并发(比如 SaaS 场景),可以 `max_connections(N)` + WAL 模式 + 处理 lock 重试。**桌面 agent 用不着**。

### `?mode=rwc` 是什么

完整 db url:
```
sqlite:/Users/yukkuri/Library/Application Support/xyz.yukin.agent/yukin.db?mode=rwc
```

`?mode=rwc` 三个字母分别是:

| 字母 | 含义 |
|------|------|
| `r` | read |
| `w` | write |
| `c` | **create if not exists** |

**默认是 `rw`**(读写但不创建)。第一次启动时 db 文件不存在,默认模式会报 `unable to open database file`。**必须加 `?mode=rwc`** 才会自动创建。

C1 你用 `sqlx database create` CLI 手动建了 `dev.db`,所以可能不需要 `?mode=rwc` 也能跑。但**生产用户机器上**第一次启动一定没有 db 文件,所以 url 必须带 `?mode=rwc`。

---

## 4. SQLite 连接 URL 的两个坑

### 坑 1:`?mode=rwc`(上一节已讲)

### 坑 2:Windows 路径有反斜杠和空格

`PathBuf::display()` 在 Windows 上输出 `C:\Users\xxx`(反斜杠)。直接拼:

```rust
let db_url = format!("sqlite:{}/yukin.db?mode=rwc", data_dir.display());
// Windows 实际: sqlite:C:\Users\xxx/yukin.db?mode=rwc
//                       ↑↑ 混用 / 和 \, sqlx 不喜欢
```

**还有空格** —— `AppData\Roaming` 是 OK 的,但用户名带空格(`John Doe`)就报 url 解析错。

### 解法

不要手拼字符串,用 `sqlx::sqlite::SqliteConnectOptions`:

```rust
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;

let db_path = data_dir.join("yukin.db");

let options = SqliteConnectOptions::new()
    .filename(&db_path)
    .create_if_missing(true);

let pool = SqlitePoolOptions::new()
    .max_connections(1)
    .connect_with(options)
    .await?;
```

`SqliteConnectOptions::filename()` 接 `PathBuf`,内部处理所有路径细节。`create_if_missing(true)` 等价于 `?mode=rwc`。**跨平台干净,推荐用这种**。

mac 下用字符串 url 也能跑(路径都是 `/`),但 Windows 用户跑你的代码会爆炸。**学一次 idiomatic 写法,以后都不用想。**

---

## 5. `sqlx::migrate!()` 编译期魔法

### 一行代码做的事

```rust
sqlx::migrate!("./migrations").run(&pool).await?;
```

`sqlx::migrate!(path)` 是**宏**,**编译期**把 `path` 目录下所有 `.sql` 文件读进来,生成一段静态 `Migrator` 数据。

效果:

- 发布二进制时**不需要带 migrations 目录** —— 所有 SQL 已经编进 binary
- 运行时 `Migrator::run` 检查 db 里的 `_sqlx_migrations` 表,只跑还没跑过的 migration
- 文件被 forward-only 强校验 — 已跑过的 migration 改了,会触发 checksum mismatch 错误,启动失败

### 路径相对哪里?

`./migrations` 是相对 **`CARGO_MANIFEST_DIR`**(`Cargo.toml` 所在目录),不是相对 `state.rs` 这个源文件。

所以 `sqlx::migrate!("./migrations")` 查找路径是:
```
<repo>/src-tauri/migrations/0001_init.sql
```

跟 C1 你 `sqlx migrate run` 时 CLI 找的是同一个目录。CLI 默认也是 cwd 相对(C1 你撞过 `canonicalizing path migrations` 这个坑)。

### 返回的错误是 `MigrateError`(新错误源)

```rust
sqlx::migrate!("./migrations").run(&pool).await?;
//                                       ^ 返回 Result<(), sqlx::migrate::MigrateError>
```

**`MigrateError` 不是 `sqlx::Error`**,是另一个独立类型!`AppError` 里 `Db(#[from] sqlx::Error)` 接不住 `MigrateError`。

你需要给 `AppError` 加一个变体:

```rust
#[error("migrate: {0}")]
Migrate(#[from] sqlx::migrate::MigrateError),
```

加完后 `?` 自动从 `MigrateError` 转 `AppError::Migrate`(这是 B2 学的 `From` trait 工作机制 — 你已经熟了)。

---

## 6. 三条必加的 `PRAGMA`

### `PRAGMA foreign_keys = ON;`

**最关键的一条**。SQLite 默认外键关闭(C1 已经讲过)。我们的 schema:

```sql
CREATE TABLE messages (
  ...
  session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE
);
```

**没开 `foreign_keys`,这个 `CASCADE` 是空话**。删 `sessions` 行,`messages` 表里对应行**纹丝不动** —— 这就是 C5 的 `delete_cascades_messages` 测试会爆炸的原因。

**注意**:`PRAGMA foreign_keys` 是**每个连接独立设置**的,不是数据库全局。所以:

- ❌ `sqlite3 dev.db "PRAGMA foreign_keys=ON;"` —— 只对那一次 CLI session 有效
- ✅ Rust 代码里**每次开新连接都要执行** —— 我们 pool 只有 1 连接,在创建后执行一次即可

### `PRAGMA journal_mode = WAL;`

SQLite 写日志模式:

- **默认 `DELETE`**:每次写都生成 `-journal` 文件,旧值备份在里面;事务结束删除
- **`WAL`(Write-Ahead Log)**:写操作 append 到 `-wal` 文件,**读操作完全不阻塞**(看 db 主文件的快照),checkpoint 时把 wal 合并回主文件

**WAL 优势**:

- 桌面 app 可能频繁读 + 偶尔写,WAL 让读永不被写阻塞
- 写性能更好(append vs random write)
- 数据库文件(crash-resistant)

**WAL 副作用**:

- 多 2 个文件:`yukin.db-wal`、`yukin.db-shm`(对你透明,可一起备份)
- 不适合网络文件系统(NFS / SMB),但我们存本地,无所谓

**`journal_mode` 是数据库全局属性**,设一次永久生效 —— 不像 `foreign_keys` 每连接独立。

### `PRAGMA busy_timeout = N;`(可选)

SQLite 默认遇到锁立即报错 `database is locked`。设个 `busy_timeout=5000`(毫秒),让连接等 5 秒再放弃:

```rust
sqlx::query("PRAGMA busy_timeout = 5000").execute(&pool).await?;
```

我们 `max_connections(1)` + 应用层只有 1 个 task 写 db 的话,**理论上永远不会撞锁**。**可加可不加**,加了是个安全网。

### 用 `connect_with` + `pragma()` 更优雅

`SqliteConnectOptions` 支持把 PRAGMA 内嵌进连接配置:

```rust
let options = SqliteConnectOptions::new()
    .filename(&db_path)
    .create_if_missing(true)
    .foreign_keys(true)                          // ← built-in 方法
    .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
    .busy_timeout(std::time::Duration::from_secs(5));

let pool = SqlitePoolOptions::new()
    .max_connections(1)
    .connect_with(options)
    .await?;
```

**这种方式比手动 `sqlx::query("PRAGMA ...")` 更好**:

- 每次新连接自动应用(虽然我们只有 1 连接,但语义更对)
- 编译期检查名字拼写
- 一行表达"连接的所有属性",可读性高

---

## 7. `?` 在 3 种错误源之间穿透

C2 的代码会用到 4 种错误:

| 错误类型 | 来源 | 当前 `AppError` 接得住吗 |
|---------|------|--------------------------|
| `std::io::Error` | `std::fs::create_dir_all` | ✅ 已有 `Io(#[from] std::io::Error)` |
| `sqlx::Error` | `connect_with`, `PRAGMA query` | ✅ 已有 `Db(#[from] sqlx::Error)` |
| `sqlx::migrate::MigrateError` | `migrate!().run` | ❌ **要加变体** |
| `tauri::Error` | `app.path().app_data_dir()` | ❌ **要加变体** |

所以 C2 之前,你要在 `error.rs` 加两个变体:

```rust
#[derive(thiserror::Error, Debug)]
pub enum AppError {
    // ... 现有变体 ...
    #[error("migrate: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("tauri: {0}")]
    Tauri(#[from] tauri::Error),
    // ... Other ...
}
```

同时记得在 `impl Serialize for AppError` 的 `code` match 里加:

```rust
AppError::Migrate(_) => "migrate",
AppError::Tauri(_) => "tauri",
```

否则编译报 "non-exhaustive patterns"。

这套**"撞到新错误源 → 回 error.rs 加变体 + serialize code"**的流程会反复出现在 C3/C4/C5。**这就是 Rust 错误处理 idiom 的肌肉记忆训练**。

加完后,`AppState::new` 里写:

```rust
pub async fn new(app: &AppHandle) -> AppResult<Self> {
    let pool = Self::open_db(app).await?;          // tauri::Error / sqlx::Error / io::Error 都能 ? 透传
    Self::run_migrations(&pool).await?;             // MigrateError ? 透传
    let workspace = Self::load_workspace(&pool).await?;
    ...
}
```

4 种错误源在同一个函数链上靠 `?` 穿透,无需任何 `.map_err`。这就是 `From` trait + `?` 脱糖 的威力。

---

## 8. 拆 3 个助手的具体形状

### `open_db`

```rust
async fn open_db(app: &AppHandle) -> AppResult<sqlx::SqlitePool> {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteJournalMode};

    let data_dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&data_dir)?;
    let db_path = data_dir.join("yukin.db");
    tracing::info!(?db_path, "opening db");

    let options = SqliteConnectOptions::new()
        .filename(&db_path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(std::time::Duration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await?;

    Ok(pool)
}
```

### `run_migrations`

```rust
async fn run_migrations(pool: &sqlx::SqlitePool) -> AppResult<()> {
    sqlx::migrate!("./migrations").run(pool).await?;
    tracing::info!("migrations applied");
    Ok(())
}
```

就一行实质代码 — 这个助手存在的价值是**给报错一个清晰名字**,不是"减少代码量"。

### `load_workspace`

```rust
async fn load_workspace(pool: &sqlx::SqlitePool) -> AppResult<Option<PathBuf>> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM settings WHERE key = 'workspace_path'"
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|(v,)| PathBuf::from(v)))
}
```

- **`query_as` 而不是 `query!`** —— 这里我们用动态查询(因为没必要编译期校验这条),C3 才正式启用 `query!` 派
- **`fetch_optional`** 比 `fetch_one` 安全 —— 第一次启动时 `settings` 表是空的,`fetch_one` 会报 `RowNotFound`
- **元组解构 `(String,)`** —— sqlx 把单列结果装成 1 元素元组,要带尾随逗号区分跟普通括号
- **早期 `tracing::debug!` 把命中结果打出来** 也行,便于将来 debug

---

## 9. 任务

### 9.1 改 `error.rs`(加两个错误变体)

按第 7 节加 `Migrate` 和 `Tauri` 变体,同步更新 `impl Serialize` 的 `code` match。`cargo check` 不应报警。

### 9.2 改 `state.rs`

按第 1 节(裸 SqlitePool) + 第 8 节(拆 3 个助手)写 `AppState::new`。所有依赖的 import:

```rust
use std::{collections::HashMap, path::PathBuf};

use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions},
    Row,
};
use tauri::{AppHandle, Manager};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use crate::AppResult;
```

(`Row` 如果你用元组解构可以不要,`sqlx::Row` trait 是用 `row.get::<T, _>("col")` 方式时才需要)

### 9.3 改 `lib.rs` 一处

`AppState::new` 的报错现在是 `AppError`,Tauri setup hook 签名是 `Result<(), Box<dyn Error>>`,`AppError` 满足 `: Error`(thiserror 帮你 derive 了),所以 `?` 自动装箱。**lib.rs 那一行 `block_on(async move { AppState::new(...).await })?` 不用改**。

但如果你之前用 `Option<SqlitePool>` 留了个 fallback closure,改成裸 SqlitePool 后那个 closure 不再需要。检查一下。

---

## 10. 验证

### 10.1 `cargo check`

应通过。可能有 unused warning 在 `state.rs::AppState::new` 的拆分函数里 —— 都是 `pub async fn` 的私有 self 方法,**不会触发 unused warning**(self 方法默认不算 dead code)。

### 10.2 启动 app

```bash
pnpm tauri dev
```

终端应该看到:

```
INFO yukin::state: opening db db_path=/Users/yukkuri/Library/Application Support/xyz.yukin.agent/yukin.db
INFO yukin::state: migrations applied
INFO yukin: yukin setup complete
```

(第一行是 `open_db` 里的 `tracing::info!(?db_path, ...)`,第二行是 `run_migrations`)

### 10.3 确认 db 文件创建

```bash
ls -la "$HOME/Library/Application Support/xyz.yukin.agent/"
# 应该看到:
# yukin.db
# yukin.db-wal     ← WAL 模式产物
# yukin.db-shm     ← 同上
```

### 10.4 schema 真跑进去了

```bash
sqlite3 "$HOME/Library/Application Support/xyz.yukin.agent/yukin.db" ".tables"
# 期望含:
# _sqlx_migrations  memory  memory_fts  messages  providers  sessions  settings
```

### 10.5 PRAGMA 真生效

```bash
sqlite3 "$HOME/Library/Application Support/xyz.yukin.agent/yukin.db" "PRAGMA foreign_keys; PRAGMA journal_mode;"
```

期望:

```
0           ← 注意!这是 CLI 单独开的新连接,默认 OFF
wal         ← journal_mode 是数据库全局,看到 wal 就对了
```

`foreign_keys` 在 sqlite3 CLI 这边是 0 不奇怪 —— 它是 **per-connection 设置**,我们 Rust 端连接是 ON。要从 Rust 端验证,Phase C 后续 `delete_cascades_messages` 测试自然能验。

---

## 11. 卡点 / 易错点提醒

### 编译错

- **加 `Migrate` / `Tauri` 变体但忘了改 `Serialize`** → "non-exhaustive patterns in match" 错。回 `error.rs` 在 `code` match 里加对应分支。
- **`sqlx::migrate::MigrateError` 路径** —— 在 `sqlx::migrate` 模块下,不是 `sqlx::Error::Migrate`(那是不存在的)。完整路径 `sqlx::migrate::MigrateError`。
- **`use tauri::Manager`** —— B7 你已经在 `state.rs` 加过这个 import 了。如果忘了,`app.path()` 报 "method not found"。

### 运行时错

- **第一次启动 db 文件没创建** → 检查 `create_if_missing(true)`
- **`AppData/Roaming/xyz.yukin.agent/` 创建失败** → 检查 `create_dir_all`,可能是权限问题(罕见,mac/win 用户家目录应该都有写权限)
- **"migration checksum mismatch"** → 你改了 `0001_init.sql` 但本地 dev.db 已经记录了旧 checksum。删 dev.db 重跑(forward-only 不允许改老 migration);production 用户机器上要加新 migration 文件 0002_*.sql

### 业务陷阱

- **`load_workspace` 用 `fetch_one` 会爆** → 用 `fetch_optional`。第一次启动 settings 表是空的。
- **`PRAGMA foreign_keys` 想在 CLI 验证是 ON** → 验不了。CLI 是另一个连接。要么相信 sqlx 的 `foreign_keys(true)`,要么用 sqlx 写个临时查询验证。

---

## 12. 写完贴给我 review 时,我会重点看

- `error.rs` 是否加了 `Migrate` + `Tauri` 两个变体,`Serialize` `code` match 是否同步更新
- `state.rs::AppState::db` 是否改成裸 `SqlitePool`(不是 `Option<...>`)
- `AppState::new` 是否拆 3 个助手 (`open_db` / `run_migrations` / `load_workspace`)
- `open_db` 是否用 `SqliteConnectOptions::filename()`(跨平台)而不是字符串拼 URL
- `open_db` 的 PRAGMA 是用 `.foreign_keys(true).journal_mode(Wal)` 链式配置(更佳)还是后续 `sqlx::query("PRAGMA ...")`(可接受但啰嗦)
- `load_workspace` 是用 `fetch_optional`(对)还是 `fetch_one`(会爆)
- `tracing::info!` 有没有打 db 路径(便于以后 debug)
- `pnpm tauri dev` 启动后终端日志、`Application Support/xyz.yukin.agent/` 目录、`yukin.db` 文件是否都到位
- `lib.rs` 没有冗余的 fallback 代码(以前给 `Option` 准备的)

---

## 13. 决策记录(填进主进度文档"实际收获"小节)

```
- 决策 (i): db 字段改裸 SqlitePool。理由:db 起不来 app 就不该起来,Option<...> 是谎言类型;C2 是改这事的零成本窗口期。
- 决策 (ii): AppState::new 拆 open_db / run_migrations / load_workspace 三助手。理由:错误定位、可测性、未来 setup 加东西不臃肿。
```
