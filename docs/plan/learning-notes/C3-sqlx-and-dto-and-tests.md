# C3 — `commands/memory.rs` + DTO + `#[sqlx::test]`(概念课)

> 创建日期: 2026-06-09
> 配套: [phase C 学习总入口](../2026-06-08-phase-c-learning.md) / [phase C 架构定义](../2026-06-06-phase-c-sqlite-keychain-session.md)
> 用途: C3 的概念讲解 + 任务清单 + 自检步骤。学完后回主入口打钩。

---

## 节标记图例

- 📚 **知识点** —— 概念、原理、对比,不直接写代码
- 🔧 **操作** —— 你要动手写的内容(代码、命令、文件改动)
- 📌 **背景** —— 决策记录、参考信息

---

## 📌 0. 阶段地位

C3 是 **Phase C 重头戏 1**,也是**整个项目第一次写真业务逻辑**。前面 B/C1/C2 都是脚手架,C3 起每行代码都直接服务于真用户场景。

学完 C3,你会真正掌握:
- sqlx 的核心使用模式(三件套 / FromRow / fetch 四式)
- Rust 错误处理在多源 IO 链路上的肌肉记忆
- DTO 分层设计(input ≠ row ≠ patch)
- `#[sqlx::test]` 集成测试(每测试独立临时 db,自动跑迁移)

**预估时长 6-10 小时,建议独占一个完整时段**。中途插断容易丢线 —— sqlx 类型系统跟错误传播链很容易在脑子里拼不起来。

---

## 📌 1. C3 决策点(已定 2026-06-09)

| # | 决策 | 推荐方案 | 简短理由 |
|---|------|---------|---------|
| (i) | `MemoryKind` 类型 | **enum + `as_str()` + serde `rename_all = "snake_case"`** | 类型安全 + 学 serde rename_all idiom |
| (ii) | DTO 形状 | **3 个独立类型**:`MemorySaveInput` / `MemoryUpdate` / `MemoryRow` | 类型即契约,patch 语义只有分离能正确表达 |
| (iii) | FTS5 用户输入 | **原样传 + TODO 注释** | 调用方是 agent (LLM),不是人类;v1 MVP 不需要防御 |

详细对比见 [对话中的决策讨论](../2026-06-08-phase-c-learning.md#c3-决策点-2026-06-09-定稿)(由开发对话记录,这里不复述)。

---

## 📚 2. sqlx 三件套:`query` / `query!` / `query_as`

sqlx 提供 3 种构造 SQL 的方式,功能重叠但语义和编译期保证差异巨大。

### 2.1 `query()` —— 运行时 SQL(最弱保障)

```rust
let row = sqlx::query("SELECT id, name FROM memory WHERE id = ?")
    .bind(&id)
    .fetch_one(&pool)
    .await?;

let name: String = row.get("name");      // 运行时类型检查
let name: String = row.get(1);            // 也可以按索引
```

- ✅ 灵活(SQL 是字符串,可以拼)
- ❌ SQL 语法错 / 列名错 / 类型不匹配 **都是运行时才发现**
- ❌ 返回 `SqliteRow`,要手动 `.get` 取列

**何时用**:动态生成 SQL(列名/表名运行时决定 — 罕见),或者迁移代码、临时调试。

### 2.2 `query!()` —— 编译期校验(最强保障)⭐

```rust
let id = "abc-123";
let row = sqlx::query!(
    "SELECT id, name, kind FROM memory WHERE id = ?",
    id
)
.fetch_one(&pool)
.await?;

let name: String = row.name;             // 直接字段访问,类型已知
```

- ✅ **编译期**连真 db 校验 SQL:语法、列名、表名、类型映射全部 check
- ✅ 自动生成 struct(`row.name` 这种访问)
- ✅ 拼写错误编译就拒
- ❌ 需要 `DATABASE_URL` 环境变量或 `.sqlx` 离线缓存(C1 已配)
- ❌ SQL 必须是字符串字面量(不能传变量)

**何时用**:**几乎所有静态 SQL**。这是我们 C3 选的派别。

### 2.3 `query_as()` —— 映射到自定义 struct

```rust
#[derive(sqlx::FromRow)]
struct MemoryRow {
    id: String,
    name: String,
}

let rows = sqlx::query_as::<_, MemoryRow>(
    "SELECT id, name FROM memory WHERE kind = ?"
)
.bind("user")
.fetch_all(&pool)
.await?;
```

- ✅ 返回你的 struct 直接用,不需要手动 `.get` 取列
- ✅ SQL 可以是变量(因为是 runtime 校验)
- ❌ SQL **不**编译期校验
- ❌ `#[derive(FromRow)]` 自动从列名映射,列名拼错运行时才发现

**何时用**:动态 SQL 但要 struct 返回(列名固定但 WHERE 条件动态)。我们 C3 的 `load_workspace` 已经用过这种。

### 2.4 三者对比

| 维度 | `query` | `query!` ⭐ | `query_as` |
|------|---------|-------------|-----------|
| 编译期 SQL 校验 | ❌ | ✅ | ❌ |
| 返回类型 | `SqliteRow` | 自动生成匿名 struct | 你定义的 struct |
| SQL 必须字面量 | 否 | **是** | 否 |
| `.bind()` 类型校验 | 运行时 | 编译期 | 运行时 |
| 需要 db 连接(编译期) | 否 | **是**(或离线缓存) | 否 |

### 2.5 我们 C3 怎么选?

**主用 `query!`**,因为它给最强保障。**两个例外**:

1. **`memory_recall` 用 `query_as!`** —— `query!` 自动 struct 字段名是匿名 + DB 列名,我们需要的是 `MemoryRow` 强类型。`query_as!` 是 `query!` + 自定义 struct,**编译期校验 + 强类型返回**两全
   ```rust
   sqlx::query_as!(MemoryRow, "SELECT * FROM memory WHERE id = ?", id)
   ```
2. **`memory_recall` SQL 涉及 FTS5 表** —— sqlx 不认识 FTS5 虚拟表的"列",`query!`/`query_as!` 可能解析失败。这种情况退回到 `query_as::<_, MemoryRow>("SELECT ...")` 的运行时校验。具体用哪种到时候试

---

## 📚 3. `#[derive(sqlx::FromRow)]` 工作机制

```rust
#[derive(sqlx::FromRow)]
pub struct MemoryRow {
    pub id: String,
    pub name: String,
    pub kind: String,             // 注意先用 String,后面 §4.3 讲为什么不用 MemoryKind
    pub description: Option<String>,
    pub content: String,
    pub metadata: String,         // JSON 字符串
    pub workspace: Option<String>,
    pub created_at: String,       // 注意是 String,§9 讲为什么不用 chrono::DateTime
    pub updated_at: String,
}
```

derive 宏自动实现 `FromRow for MemoryRow`,工作机制:

1. 拿到 `SqliteRow`(sqlx 内部的列+值结构)
2. 对每个字段 `f`,调 `row.try_get::<FieldType, _>("f")`
3. 任一 `try_get` 失败 → `FromRow::from_row` 返回 `Err(sqlx::Error::ColumnDecode)`
4. 全成功 → 装好 struct,返回 `Ok(MemoryRow { ... })`

### 字段名要求

**默认**:struct 字段名 = DB 列名(精确匹配)。我们 schema 用 snake_case 列名,Rust struct 也 snake_case,自然对齐。

**重命名**:如果 DB 列叫 `user_id` 但你想 struct 字段叫 `userId`:
```rust
#[sqlx(rename = "user_id")]
pub user_id_db: String,
```

我们不需要(列名跟字段名一致)。

### `Option<T>` 字段语义

- DB 列允许 `NULL` → Rust 字段用 `Option<T>`
- DB 列 `NOT NULL` → Rust 字段用 `T`(直接,不用 Option)

**如果不匹配会出错**:
- DB `NOT NULL` 但 Rust 用 `Option<T>` → sqlx warning(可能在 prepare 时拒)
- DB 允许 NULL 但 Rust 用 `T` → 运行时遇到 NULL 报 `ColumnDecode("unexpected null")`

我们 `MemoryRow`:
- `description` / `workspace` → schema 允许 NULL → `Option<String>` ✅
- 其他字段 → `NOT NULL` → 直接 `String` ✅

---

## 📚 4. bind 参数 vs SQL 注入

### 4.1 绝对铁律:**永远不要 `format!` SQL**

```rust
// ❌ 危险!SQL 注入漏洞
let sql = format!("SELECT * FROM memory WHERE name = '{}'", user_input);
sqlx::query(&sql).fetch_all(&pool).await?;

// 用户输入 user_input = "'; DROP TABLE memory; --"
// 拼出来: SELECT * FROM memory WHERE name = ''; DROP TABLE memory; --'
// 数据库被毁
```

### 4.2 正确做法:`?` 占位符 + `.bind()`

```rust
// ✅ 安全
sqlx::query("SELECT * FROM memory WHERE name = ?")
    .bind(&user_input)
    .fetch_all(&pool)
    .await?;
```

sqlx 把参数走 **prepared statement**:db 把 SQL 和参数分别送、参数永远不被解析为 SQL。注入根本不可能。

### 4.3 `?` 占位符的数量和顺序

```rust
sqlx::query("SELECT * FROM memory WHERE kind = ? AND workspace = ? ORDER BY created_at LIMIT ?")
    .bind(&kind)              // 第 1 个 ?
    .bind(&workspace)         // 第 2 个 ?
    .bind(limit)              // 第 3 个 ?
    .fetch_all(&pool).await?;
```

**顺序必须对齐**,sqlx 不会按变量名匹配(SQL 里就没变量名)。

### 4.4 sqlx 的"为什么我让你失败编译"

```rust
let kind = MemoryKind::User;        // enum
sqlx::query("INSERT INTO memory (kind) VALUES (?)")
    .bind(kind)                      // ❌ sqlx 不知道怎么 bind enum
```

sqlx 不自动序列化 enum。**要显式转字符串**:
```rust
.bind(kind.as_str())                 // ✅ &'static str
```

这就是为什么决策 (i) 加了 `as_str()` 方法 — sqlx 边界处的"出口"。

反方向(从 db 读):
```rust
#[derive(sqlx::FromRow)]
struct MemoryRow {
    kind: String,                    // ← 不是 MemoryKind!
    ...
}
```

为什么 read 端也用 `String`?**两个原因**:

1. sqlx 默认不知道怎么把 `String` 反向转 `MemoryKind`(需要实现 `Decode + Type`,工作量大)
2. **DB 层 trust 边界:** `CHECK` 约束确保 db 里只有 4 个合法值,读出来必然是合法 string
3. **DTO 层转换:** `MemoryRow.kind: String` → API 序列化时再转 `MemoryKind`(或不转,直接以 string 返回)

实际操作:**`MemoryRow.kind` 用 `String`**,接口返回给前端时 serde 自动序列化字符串。前端 TS 端定义 union type `'user' | 'feedback' | 'project' | 'reference'` 拿到合法值。

---

## 📚 5. 4 种 fetch 模式

`query!` / `query_as` / `query` 末尾要选一个 fetch 方法决定如何处理结果:

### 5.1 `.execute()` —— 不在乎返回

```rust
let result = sqlx::query!("DELETE FROM memory WHERE id = ?", id)
    .execute(&pool).await?;
println!("rows affected: {}", result.rows_affected());
```

- 返回 `SqliteQueryResult`(只有 `rows_affected` / `last_insert_rowid`)
- INSERT / UPDATE / DELETE 用这个

### 5.2 `.fetch_one()` —— 必须正好 1 行

```rust
let row = sqlx::query!("SELECT * FROM memory WHERE id = ?", id)
    .fetch_one(&pool).await?;
```

- 0 行 → `Err(sqlx::Error::RowNotFound)`
- 1 行 → `Ok(row)`
- 多行 → 静默只返回第一行(SQLite 实现差异,有时给警告)

**何时用**:确信结果有 1 行(主键查询、唯一约束查询)

### 5.3 `.fetch_optional()` —— 0 或 1 行

```rust
let row = sqlx::query!("SELECT * FROM memory WHERE id = ?", id)
    .fetch_optional(&pool).await?;
match row {
    Some(r) => { /* 找到 */ },
    None => { /* 没找到 */ },
}
```

- 0 行 → `Ok(None)`
- 1 行 → `Ok(Some(row))`
- 多行 → 同 fetch_one

**何时用**:查询可能没结果(C2 的 `load_workspace` 就是这种)

### 5.4 `.fetch_all()` —— 任意多行

```rust
let rows = sqlx::query!("SELECT * FROM memory WHERE kind = ?", "user")
    .fetch_all(&pool).await?;
for row in rows { ... }
```

- 返回 `Vec<Row>`(可能空)

**何时用**:list / search,任意多行结果

### 5.5 选用决策树

```
要返回多行?         → fetch_all
要返回 0 或 1 行?    → fetch_optional
要返回必然 1 行?    → fetch_one (主键查询场景)
不在乎返回(INSERT/UPDATE/DELETE)?  → execute
```

C3 的 5 个命令:
- `memory_save` —— INSERT → `execute()`,然后用 `fetch_one()` 查回完整行返回
- `memory_recall` —— `fetch_all()`
- `memory_list` —— `fetch_all()`
- `memory_delete` —— `execute()`
- `memory_update` —— `execute()`,然后 `fetch_one()` 查回完整行返回

---

## 📚 6. `.sqlx` 离线缓存(可选,推荐入 git)

### 问题

`query!` 编译期连 `DATABASE_URL` 校验。CI / 其他设备没有 dev.db / sqlx-cli 怎么办?

### 解决:离线模式

```bash
cd src-tauri
cargo sqlx prepare      # 把所有 query!() 元数据 dump 到 .sqlx/ 目录
git add .sqlx
git commit
```

`.sqlx/` 是 json 文件目录,每个 query 一个文件(以 SQL hash 命名),记录:
- SQL 字符串
- 参数类型
- 结果列名 + 类型

设置 `SQLX_OFFLINE=true`(或在 CI env vars 里设)或 `.cargo/config.toml`:
```toml
[env]
SQLX_OFFLINE = "true"
```

sqlx 会从 `.sqlx/` 读取元数据校验,不再连 db。

### 何时跑 `prepare`

**改了任何 `query!()` 或 schema 后都要重跑**。否则 CI 会用旧元数据校验新 SQL,报 mismatch。

### C3 阶段建议

**先不入 git**,加 `.sqlx/` 进 `.gitignore`。原因:

- Phase C 反复迭代 query,每次都跑 prepare 烦
- 你单人开发,没有 CI 校验需求
- C5 结束或者要 push 时跑一次 prepare 入 git 就够了

**`.sqlx/` 加到 `.gitignore` 的位置**:`src-tauri/.gitignore` 里加。

---

## 📚 7. DTO 设计三原则

复述决策 (ii) 的具体形状:

### 7.1 原则一:input ≠ row

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySaveInput {
    pub name: String,                           // 必填
    pub kind: MemoryKind,                       // 必填,enum 严格
    pub content: String,                        // 必填
    pub description: Option<String>,            // 可选
    pub metadata: Option<serde_json::Value>,    // 可选,默认 {}
    pub workspace: Option<String>,              // 可选,NULL = 全局
}
// 注意:没有 id / created_at / updated_at,db 自动填
```

```rust
#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct MemoryRow {
    pub id: String,
    pub name: String,
    pub kind: String,                           // ← String 不是 enum,见 §4.4
    pub description: Option<String>,
    pub content: String,
    pub metadata: String,                       // ← JSON 字符串,前端解析
    pub workspace: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
```

### 7.2 原则二:Update 全 Option(patch 语义)

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryUpdate {
    pub name: Option<String>,
    pub description: Option<String>,
    pub content: Option<String>,
    pub metadata: Option<serde_json::Value>,
}
```

前端可以只传想改的字段:
```json
{ "content": "new content only" }   // 只改 content
```

SQL 用 `COALESCE` 模式:
```sql
UPDATE memory SET
  name        = COALESCE(?1, name),
  description = COALESCE(?2, description),
  content     = COALESCE(?3, content),
  metadata    = COALESCE(?4, metadata),
  updated_at  = datetime('now')
WHERE id = ?5
```

`COALESCE(NULL, original)` 取 original(不改);`COALESCE('new', original)` 取 'new'(改)。

### 7.3 原则三:`#[serde(rename_all = "camelCase")]` 跨边界

- DB 列:`snake_case`(`created_at`)
- Rust 字段:`snake_case`(`created_at`)
- JSON / 前端:`camelCase`(`createdAt`)

`#[serde(rename_all = "camelCase")]` **加在 struct 上**,serde 自动转 `created_at` ↔ `createdAt`。

无须每个字段手写 `#[serde(rename = "createdAt")]`,**省一大堆 boilerplate**。

---

## 📚 8. `#[sqlx::test]` 集成测试

### 痛点

写 sqlx 业务逻辑要测试,常规思路:

- mock 整个 sqlx pool?**太复杂,sqlx 类型系统接近无法 mock**
- 用真 dev.db 测?**测试互相污染**(test A 写的数据 test B 看见)
- 每个 test 手动建临时 db?**boilerplate 巨大**

### sqlx 的解法:`#[sqlx::test]`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    #[sqlx::test(migrations = "./migrations")]
    async fn save_then_recall(pool: SqlitePool) -> sqlx::Result<()> {
        // pool 是这个 test 独有的临时 sqlite (内存或临时文件)
        // migrations 已经自动跑过
        
        let id = save_memory(&pool, MemorySaveInput {
            name: "hello".into(),
            kind: MemoryKind::User,
            content: "world".into(),
            description: None,
            metadata: None,
            workspace: None,
        }).await?;
        
        let rows = recall_memory(&pool, "world", 10, None).await?;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, id);
        Ok(())
    }
}
```

### `#[sqlx::test]` 做什么

1. 创建独立临时 SQLite db(内存或 `/tmp` 文件)
2. 自动应用 `migrations = "..."` 指定目录的所有迁移
3. 注入 pool 作为 test 函数参数
4. test 结束自动 cleanup

**每个 test 完全隔离**,测试间不污染、不竞争。

### 我们 C3 要写至少 3 个 test

最少:
1. `save_then_recall` —— 写入,FTS5 能查到
2. `delete_then_recall` —— 删除,FTS5 不再查到(验证 trigger 工作)
3. `update_then_recall` —— 更新 content,新 content 能查到,旧 content 查不到(验证 update trigger)

可选加:
4. `recall_filters_by_kind` —— recall 时 kind 过滤生效
5. `list_returns_all_kinds_when_none` —— list 不传 kind 返回全部

---

## 📚 9. chrono ↔ SQLite datetime 格式不兼容(隐藏陷阱)

### 问题

SQLite `datetime('now')` 默认输出:
```
2026-06-09 12:34:56
```

(无 T 分隔符、无时区)

Rust chrono 默认解析 RFC3339:
```
2026-06-09T12:34:56Z
```

(有 T、有时区)

格式不匹配 → sqlx 把 `created_at` 列尝试 decode 到 `chrono::DateTime<Utc>` 时 **panic**(或 `ColumnDecode` 错)。

### 三种解法

#### 解法 A:Rust 端就用 `String`(本课推荐)

```rust
pub struct MemoryRow {
    pub created_at: String,
    pub updated_at: String,
}
```

- ✅ 简单,sqlx 零负担
- ✅ 前端 JSON 字符串直接显示("2026-06-09 12:34:56")可读
- ❌ Rust 端要做日期运算时麻烦(要手动 parse)

**C3 选这个**,因为目前没有日期运算需求(只是存 + 取 + 显示)。Phase H 如果要做"按时间段过滤 memory"再升级。

#### 解法 B:DB 端改用 ISO 8601 with T

修改 schema(写到 0002 migration):
```sql
-- 不可能,SQLite 没有"修改 DEFAULT 表达式"语法
-- 必须重建表
```

太重。Forward-only 迁移做这个事很折腾。

#### 解法 C:用 `chrono` 但 custom deserializer

```rust
#[derive(sqlx::FromRow)]
pub struct MemoryRow {
    #[sqlx(try_from = "String")]
    pub created_at: NaiveDateTime,
    ...
}

impl TryFrom<String> for NaiveDateTime { ... }
```

工作量大,**不推荐**(除非真要做日期运算)。

### 决策

**用 String**(解法 A)。`MemoryRow.created_at: String`,前端拿到字符串直接显示。

---

## 📚 10. `?` 跨 4-5 种错误源

C3 写代码会撞到这些错误:

| 错误 | 来源 | `AppError` 现状 |
|------|------|----------------|
| `std::io::Error` | 文件系统(本节无) | ✅ 已有 |
| `sqlx::Error` | 所有 sqlx 查询 | ✅ 已有 |
| `serde_json::Error` | metadata 序列化 / 反序列化 | ❌ **要加变体** |
| `tauri::Error` | (本节无) | ✅ 已加 (C2) |
| 业务错误(memory not found 等) | 你自己 | 用 `AppError::Other(...)` 或新加变体 |

### 加 `serde_json::Error` 变体

回 `error.rs`:
```rust
#[error("json: {0}")]
Json(#[from] serde_json::Error),
```

`Serialize` `code` match 加:
```rust
AppError::Json(_) => "json",
```

**B2 已经讲过这个流程**,C3 是反复巩固。

---

## 🔧 11. 代码结构 + 模板

C3 涉及 3 个文件层次:

```
src-tauri/src/
├── commands/memory.rs     ← 已存在,改成薄壳 (DTO + 调 db 层)
├── db/                    ← 新建子模块
│   ├── mod.rs             ← 新建,re-export
│   └── memory.rs          ← 新建,纯 SQL 层
├── error.rs               ← 改:加 Json 变体
└── lib.rs                 ← 改:加 mod db;
```

### 11.1 `db/mod.rs`

```rust
pub mod memory;
```

### 11.2 `db/memory.rs`(纯 SQL,无 Tauri / 无 #[command],可测)

```rust
use crate::AppResult;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

// ============ Types ============

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    User,
    Feedback,
    Project,
    Reference,
}

impl MemoryKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            MemoryKind::User => "user",
            MemoryKind::Feedback => "feedback",
            MemoryKind::Project => "project",
            MemoryKind::Reference => "reference",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySaveInput {
    pub name: String,
    pub kind: MemoryKind,
    pub content: String,
    pub description: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub workspace: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryUpdate {
    pub name: Option<String>,
    pub description: Option<String>,
    pub content: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct MemoryRow {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub description: Option<String>,
    pub content: String,
    pub metadata: String,
    pub workspace: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// ============ Operations ============

pub async fn save(pool: &SqlitePool, input: MemorySaveInput) -> AppResult<MemoryRow> {
    let id = Uuid::new_v4().to_string();
    let metadata_str = match input.metadata {
        Some(v) => serde_json::to_string(&v)?,    // serde_json::Error → AppError::Json (要加)
        None => "{}".to_string(),
    };

    sqlx::query(
        "INSERT INTO memory (id, name, kind, description, content, metadata, workspace) \
         VALUES (?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&id)
    .bind(&input.name)
    .bind(input.kind.as_str())
    .bind(&input.description)
    .bind(&input.content)
    .bind(&metadata_str)
    .bind(&input.workspace)
    .execute(pool)
    .await?;

    fetch(pool, &id).await
}

pub async fn fetch(pool: &SqlitePool, id: &str) -> AppResult<MemoryRow> {
    sqlx::query_as::<_, MemoryRow>("SELECT * FROM memory WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(Into::into)
}

pub async fn recall(
    pool: &SqlitePool,
    query: &str,
    limit: i64,
    kind: Option<&str>,
) -> AppResult<Vec<MemoryRow>> {
    // TODO: sanitize FTS5 special chars (paren / OR / quote) for human input.
    //       Trusted agent caller is OK for v1 MVP.
    let sql = match kind {
        Some(_) => "SELECT m.* FROM memory m JOIN memory_fts f ON f.rowid = m.rowid \
                    WHERE memory_fts MATCH ? AND m.kind = ? \
                    ORDER BY rank LIMIT ?",
        None    => "SELECT m.* FROM memory m JOIN memory_fts f ON f.rowid = m.rowid \
                    WHERE memory_fts MATCH ? \
                    ORDER BY rank LIMIT ?",
    };
    let mut q = sqlx::query_as::<_, MemoryRow>(sql).bind(query);
    if let Some(k) = kind {
        q = q.bind(k);
    }
    q.bind(limit).fetch_all(pool).await.map_err(Into::into)
}

pub async fn list(
    pool: &SqlitePool,
    kind: Option<&str>,
) -> AppResult<Vec<MemoryRow>> {
    match kind {
        Some(k) => sqlx::query_as::<_, MemoryRow>(
            "SELECT * FROM memory WHERE kind = ? ORDER BY updated_at DESC LIMIT 100"
        )
        .bind(k)
        .fetch_all(pool).await.map_err(Into::into),
        None => sqlx::query_as::<_, MemoryRow>(
            "SELECT * FROM memory ORDER BY updated_at DESC LIMIT 100"
        )
        .fetch_all(pool).await.map_err(Into::into),
    }
}

pub async fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM memory WHERE id = ?")
        .bind(id)
        .execute(pool).await?;
    Ok(())
}

pub async fn update(
    pool: &SqlitePool,
    id: &str,
    patch: MemoryUpdate,
) -> AppResult<MemoryRow> {
    let metadata_str = match patch.metadata {
        Some(v) => Some(serde_json::to_string(&v)?),
        None => None,
    };
    sqlx::query(
        "UPDATE memory SET \
           name        = COALESCE(?, name), \
           description = COALESCE(?, description), \
           content     = COALESCE(?, content), \
           metadata    = COALESCE(?, metadata), \
           updated_at  = datetime('now') \
         WHERE id = ?"
    )
    .bind(&patch.name)
    .bind(&patch.description)
    .bind(&patch.content)
    .bind(&metadata_str)
    .bind(id)
    .execute(pool).await?;

    fetch(pool, id).await
}

// ============ Tests ============

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    #[sqlx::test(migrations = "./migrations")]
    async fn save_then_recall(pool: SqlitePool) -> sqlx::Result<()> {
        let row = save(&pool, MemorySaveInput {
            name: "hello".into(),
            kind: MemoryKind::User,
            content: "rust async world".into(),
            description: None,
            metadata: None,
            workspace: None,
        }).await.unwrap();

        let hits = recall(&pool, "world", 10, None).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, row.id);
        Ok(())
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn delete_then_recall_returns_empty(pool: SqlitePool) -> sqlx::Result<()> {
        let row = save(&pool, MemorySaveInput {
            name: "doomed".into(),
            kind: MemoryKind::User,
            content: "to be deleted".into(),
            description: None,
            metadata: None,
            workspace: None,
        }).await.unwrap();

        delete(&pool, &row.id).await.unwrap();

        let hits = recall(&pool, "doomed", 10, None).await.unwrap();
        assert_eq!(hits.len(), 0, "DELETE trigger should remove from FTS index");
        Ok(())
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn update_content_changes_recall_target(pool: SqlitePool) -> sqlx::Result<()> {
        let row = save(&pool, MemorySaveInput {
            name: "n".into(),
            kind: MemoryKind::User,
            content: "alpha".into(),
            description: None,
            metadata: None,
            workspace: None,
        }).await.unwrap();

        update(&pool, &row.id, MemoryUpdate {
            name: None,
            description: None,
            content: Some("beta".into()),
            metadata: None,
        }).await.unwrap();

        assert_eq!(recall(&pool, "alpha", 10, None).await.unwrap().len(), 0);
        assert_eq!(recall(&pool, "beta",  10, None).await.unwrap().len(), 1);
        Ok(())
    }
}
```

### 11.3 `commands/memory.rs`(薄壳)

```rust
use crate::db;
use crate::state::AppState;
use crate::AppResult;
use tauri::State;

#[tauri::command]
pub async fn memory_save(
    input: db::memory::MemorySaveInput,
    state: State<'_, AppState>,
) -> AppResult<db::memory::MemoryRow> {
    db::memory::save(&state.db, input).await
}

#[tauri::command]
pub async fn memory_recall(
    query: String,
    limit: Option<i64>,
    kind: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<Vec<db::memory::MemoryRow>> {
    db::memory::recall(&state.db, &query, limit.unwrap_or(8), kind.as_deref()).await
}

#[tauri::command]
pub async fn memory_list(
    kind: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<Vec<db::memory::MemoryRow>> {
    db::memory::list(&state.db, kind.as_deref()).await
}

#[tauri::command]
pub async fn memory_delete(
    id: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    db::memory::delete(&state.db, &id).await
}

#[tauri::command]
pub async fn memory_update(
    id: String,
    patch: db::memory::MemoryUpdate,
    state: State<'_, AppState>,
) -> AppResult<db::memory::MemoryRow> {
    db::memory::update(&state.db, &id, patch).await
}
```

### 11.4 `lib.rs` 改一处

```rust
mod agent;
mod commands;
mod db;            // ← 新加
mod error;
mod llm;
mod path_safety;
mod state;
mod tools;
```

**`generate_handler!`** 里:
- 把 `memory_save / memory_recall / memory_list / memory_delete` 这 4 行确认还在
- **加一行** `commands::memory::memory_update`(B5 时漏列了,C3 命令加了一个)

### 11.5 `error.rs` 加 `Json` 变体

```rust
#[error("json: {0}")]
Json(#[from] serde_json::Error),
```

`code` match:
```rust
AppError::Json(_) => "json",
```

---

## 🔧 12. 任务清单

1. **改 `error.rs`** —— 加 `Json` 变体,同步 `Serialize` `code` match
2. **新建 `src-tauri/src/db/mod.rs`** —— `pub mod memory;`
3. **新建 `src-tauri/src/db/memory.rs`** —— 4 个 DTO + 5 个操作函数 + 3 个 test(模板见 §11.2,可直接抄)
4. **改 `src-tauri/src/lib.rs`** —— 加 `mod db;`,`generate_handler!` 加 `commands::memory::memory_update`
5. **改 `src-tauri/src/commands/memory.rs`** —— 替换 stub 为薄壳调用(模板见 §11.3)
6. **`.gitignore`** —— 加 `.sqlx/`(`src-tauri/.gitignore` 已加 `.env` / `dev.db`,这里再加 `.sqlx`)
7. **跑 cargo check + cargo test 验证**

---

## 🔧 13. 验证

### 13.1 编译

```bash
cd src-tauri
cargo check
```

应该通过。期望 warning 数量比 C2 少(因为 `commands::memory::*` 不再是 unused 占位)。

### 13.2 单元测试

```bash
cd src-tauri
DATABASE_URL=sqlite:./dev.db cargo test --lib memory
```

期望:3 个 test 全过。

### 13.3 启动 app + devtools 联调

```bash
pnpm tauri dev
```

devtools console:

```js
const { invoke } = await import('@tauri-apps/api/core');

// 写入
const saved = await invoke('memory_save', {
  input: {
    name: "test entry",
    kind: "user",
    content: "Hello memory layer!",
  }
});
console.log('saved:', saved);

// recall
const hits = await invoke('memory_recall', { query: "Hello" });
console.log('recall:', hits);
// 期望 hits[0].id === saved.id

// list
const all = await invoke('memory_list', {});
console.log('list:', all);

// update
const updated = await invoke('memory_update', {
  id: saved.id,
  patch: { content: "Hello updated content" }
});
console.log('updated:', updated);

// recall 新内容
const hits2 = await invoke('memory_recall', { query: "updated" });
console.log('updated recall:', hits2);

// delete
await invoke('memory_delete', { id: saved.id });

// 验证删除
const hits3 = await invoke('memory_recall', { query: "Hello" });
console.log('after delete:', hits3);  // 期望空数组
```

### 13.4 DB Browser 看真数据

```bash
sqlite3 "$HOME/Library/Application Support/xyz.yukin.agent/yukin.db" \
  "SELECT id, name, kind, content FROM memory;"
```

应该看到 13.3 留下的数据(如果你最后没 delete)。

---

## 📌 14. 卡点 / 易错点

### 编译错

- **`serde_json::Error` 没 `From` impl** —— 检查 error.rs 加了 `Json(#[from] serde_json::Error)` 没
- **`Serialize` `code` match non-exhaustive** —— 加新变体后忘记加 `AppError::Json(_) => "json"`
- **`#[derive(sqlx::FromRow)]` 字段类型对不上** —— `description` schema 是 NULL,Rust 要 `Option<String>`;反之 NOT NULL 字段不要 `Option`
- **`MemoryKind` enum + serde:不是 enum 字符串而是 enum object** —— 你写 `kind: "user"` 时 serde 默认期望 `"user"` 字符串(因为 unit variant);如果 `#[serde(tag = "type")]` 那种 enum 表示法就完全不一样。**我们用最简形式 + `rename_all = "snake_case"`**

### 运行时错

- **FTS5 query 报语法错** —— 用户传了 `"foo (bar"` 之类,FTS5 解析失败。MVP 接受(决策 iii),Phase H 真要严肃做就加 sanitize
- **`fetch_one` 报 RowNotFound** —— 用户传了不存在的 id,`memory_update` / `memory_delete` 报错。**MVP 接受**,Phase H 可加 `fetch_optional` + 业务错误处理
- **测试 `sqlx::test` 报"migrations not found"** —— 检查 `migrations = "./migrations"` 路径;sqlx 也是相对 `CARGO_MANIFEST_DIR`(`src-tauri/`)

### 设计陷阱

- **`db/memory.rs` 写命令而不只是 SQL** —— 千万**别**在 `db/` 层写 `#[tauri::command]`,这层只做 sqlx 调用,无 Tauri 依赖,这样才好测
- **commands 层做 SQL** —— 反过来也别,commands 只做 DTO 转换 + state 解包
- **patch 字段全 None 不报错** —— `memory_update` 传 patch 全 None 时啥也不更新但 `updated_at` 仍然变化(因为 SQL 里 `updated_at = datetime('now')` 是固定的)。**这是 feature 不是 bug**:语义"标记看过了",可接受

---

## 🔧 15. 写完贴给我 review 时,我会重点看

### 结构
- `db/memory.rs` 是否纯 SQL(无 `#[tauri::command]`、无 `State<AppState>`)
- `commands/memory.rs` 是否薄壳(每个命令 ≤ 5 行)
- `lib.rs` 是否 `mod db;` + `generate_handler!` 加 `memory_update`
- `error.rs` 是否加 `Json(#[from] serde_json::Error)` + 同步 `Serialize` code match

### DTO
- `MemoryKind` 是 enum + `#[serde(rename_all = "snake_case")]` + `as_str()`
- 3 个 DTO 分开定义(不是一个共享 `Option` 化的)
- 所有 struct 加 `#[serde(rename_all = "camelCase")]`
- `MemoryRow.created_at: String` 不是 `chrono::DateTime<_>`

### SQL
- `recall` 用 `WHERE memory_fts MATCH ?`(而不是 `LIKE`)
- `update` 用 `COALESCE` pattern
- 所有 SQL 走 `.bind()`,没有 `format!` SQL

### 测试
- 至少 3 个 `#[sqlx::test(migrations = "./migrations")]`
- `delete_then_recall_returns_empty` 验 DELETE trigger 工作
- `update_content_changes_recall_target` 验 UPDATE trigger 工作

### 验证
- `cargo test` 全过
- devtools 5 个命令链路(save → recall → list → update → delete)全通
- DB Browser 看到真数据

---

## 📌 16. 决策记录(填进主进度文档"实际收获"小节)

```
- 决策 (i) MemoryKind: enum + as_str + serde snake_case (类型安全 + 学 serde idiom)
- 决策 (ii) DTO 分离: MemorySaveInput / MemoryUpdate / MemoryRow (类型即契约,patch 只能这么写)
- 决策 (iii) FTS5 sanitize: 原样传 + TODO 注释 (调用方是 agent,v1 不需要防御性)
```
