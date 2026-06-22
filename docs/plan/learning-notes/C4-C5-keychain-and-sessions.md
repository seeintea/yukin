# C4 + C5 — Keychain (spawn_blocking) + Sessions/Messages(概念课合并)

> 创建日期: 2026-06-15
> 配套: [phase C 学习总入口](../2026-06-08-phase-c-learning.md) / [phase C 架构定义](../2026-06-06-phase-c-sqlite-keychain-session.md)
> 用途: C4 + C5 合并教学。两步都是 C3 套路的应用(sqlx 三件套 + DTO 三件套 + commands 薄壳),各自只多 1-2 个新知识点。

---

## 节标记图例(沿用 C3)

- 📚 **知识点** —— 概念、原理、对比
- 🔧 **操作** —— 你要动手写
- 📌 **背景** —— 决策记录、参考

---

## 📌 0. 为什么合并

C3 已经把这些概念打透了:

- sqlx 三件套(`query!` / `query_as!` / `query`)+ FromRow
- DTO 三件套(Input / Update / Row)
- commands 薄壳模式(`db/` 纯 SQL,`commands/` 5 行包装)
- `?` 跨多错误源穿透
- `#[sqlx::test]` 集成测试
- SQLite `AS "col!"` 显式 NOT NULL 断言

C4/C5 **重复练习这些** + **各加 1 个新东西**:

| 阶段 | 新知识                                                              |
| ---- | ------------------------------------------------------------------- |
| C4   | `tokio::task::spawn_blocking` —— 同步 keyring crate 在 async 里 |
| C5   | JSON 字段 + 1:N cascade 实测                                        |

合并文档密度高、避免重复、你内存里 C3 还热乎、心智成本最低。

---

## 📌 1. 决策点(已定 2026-06-15)

| #       | 决策                                       | 选择                                                                          |
| ------- | ------------------------------------------ | ----------------------------------------------------------------------------- |
| C4 (i)  | `spawn_blocking` JoinError 处理          | **加 `AppError::JoinError(#[from] tokio::task::JoinError)`**,纪律一致 |
| C4 (ii) | `keyring::Error::NoEntry` 视为?          | **`Ok(None)`**,跟 C2 `load_workspace` 同思路                        |
| C5 (i)  | `messages.content` 用 String 还是 Value? | **String + 前端 JSON.parse**,跟 `memory.metadata` 一致                |
| C5 (ii) | `session_update` 全 None 时?             | **`Ok(())`**,跟 `memory_update` 一致(后端宽松)                      |

---

# C4 — Keychain (`spawn_blocking`)

## 📚 2. 为什么 keychain 要 `spawn_blocking`

### 2.1 keyring crate 是同步 API

```rust
// keyring crate 文档示例
let entry = keyring::Entry::new("xyz.yukin.agent", "anthropic")?;
entry.set_password("sk-ant-...")?;     // ← sync, 阻塞当前线程直到完成
let pwd = entry.get_password()?;        // ← sync, 同上
entry.delete_credential()?;             // ← sync
```

**所有方法都是同步阻塞**。底层调 macOS Keychain Services / Windows Credential Manager / Linux Secret Service 的系统 API,这些 API 自身就是同步的(GUI 弹"是否允许访问"框时阻塞)。

### 2.2 同步阻塞为什么不能直接进 async fn

回看 B4 概念课:tokio runtime 默认 N 个 worker 线程(N = CPU 核数),所有 async task 在这些线程间协作调度。**关键约束**:任一线程被阻塞 = 这些线程上的所有 task 都卡住。

```rust
#[tauri::command]
async fn key_get(provider: String) -> AppResult<Option<String>> {
    let entry = keyring::Entry::new("xyz.yukin.agent", &provider)?;
    let pwd = entry.get_password()?;     // ❌ 阻塞 worker thread
    Ok(Some(pwd))
}
```

这条命令在 worker thread A 上跑。`get_password()` 同步阻塞 A,期间 A 不能跑别的 task。如果 keychain 弹出系统授权框等用户点击(几秒),**整个 app 的 IPC 都僵死**。

### 2.3 解法:`tokio::task::spawn_blocking`

tokio 还有第二类线程池:**blocking pool**(默认 512 个线程)。专门跑这种"同步阻塞操作"。

```rust
#[tauri::command]
async fn key_get(provider: String) -> AppResult<Option<String>> {
    let result = tokio::task::spawn_blocking(move || {
        let entry = keyring::Entry::new("xyz.yukin.agent", &provider)?;
        match entry.get_password() {
            Ok(pwd) => Ok(Some(pwd)),
            Err(keyring::Error::NoEntry) => Ok(None),     // 决策 (ii)
            Err(e) => Err(AppError::from(e)),
        }
    }).await??;                                            // ← 双层 ?,见下
    Ok(result)
}
```

`spawn_blocking(closure)` 把 closure 调度到 blocking pool,**返回 `JoinHandle<R>`**。`.await` 它得到 `R`(closure 真正完成时)。

worker thread 这边只做"等 blocking pool 完成"——**真等待是非阻塞的**(返回 future,await 让出 runtime)。

### 2.4 关键约束:closure 必须 `Send + 'static`

```rust
spawn_blocking(move || { ... })
//             ^^^^ 必须 move 进所有借用变量
```

- **`Send`**:closure 要被发到别的线程(blocking pool 线程)
- **`'static`**:不能持有任何外部借用 —— 数据要么 `move` 进来 owned,要么是 `'static` 引用

**实际影响**:

```rust
// ❌ 不能直接借用外部 &str
let provider = "anthropic";
spawn_blocking(|| {
    keyring::Entry::new("...", provider)?    // provider: &'_ str, 不是 'static
});

// ✅ 必须 clone 成 owned String,move 进去
let provider = String::from("anthropic");
spawn_blocking(move || {                      // move 把 provider 转移进 closure
    keyring::Entry::new("...", &provider)?    // 这里借用是 closure 内部的,合法
});
```

C4 我们的命令参数就是 `String`(由 `#[tauri::command]` 反序列化得来),天然 owned,`move` 进去刚好。

### 2.5 双层 `Result` 三种拆法

```rust
spawn_blocking(...).await
// 类型: Result<R, JoinError>
//                R = closure 返回值(我们写的是 AppResult<T> 即 Result<T, AppError>)
//      所以是: Result<Result<T, AppError>, JoinError>
```

**外层 JoinError** —— blocking task 自己 panic 或被 cancel
**内层 AppError** —— 业务错(keyring fail / 我们 raise 的)

3 种处理方式:

```rust
// 方式 A: 双 ?(决策 (i) 推荐,加 JoinError variant 后)
let pwd = spawn_blocking(...).await??;       // 第一个 ? 解 JoinError, 第二个解 AppError

// 方式 B: 显式 match
let outer = spawn_blocking(...).await;
match outer {
    Ok(Ok(v)) => v,
    Ok(Err(app_err)) => return Err(app_err),
    Err(join_err) => return Err(AppError::JoinError(join_err)),
}

// 方式 C: 双 .map_err 吊链(不推荐,啰嗦)
let v = spawn_blocking(...).await
    .map_err(|e| AppError::Other(e.to_string()))?
    .map_err(|e| e)?;
```

C4 用方式 A,前提是先加 `AppError::JoinError`。

## 📚 3. `providers` 表的真实作用

C1 schema 里有这张表:

```sql
CREATE TABLE providers (
  name          TEXT PRIMARY KEY,        -- 'anthropic' / 'openai' / ...
  has_key       INT NOT NULL DEFAULT 0,
  default_model TEXT,
  ...
);
```

**疑问**:keychain 都管 key 了,为什么还要这张表?

### 答案:keychain 没有"列举 API"

keyring crate 提供:

- `Entry::new(service, account).set_password(...)`
- `Entry::new(service, account).get_password()`
- `Entry::new(service, account).delete_credential()`

**不提供** "列出 service=xxx 下所有 account"的 API。理由:

- macOS Keychain 有 ACL 机制,跨进程列举有权限问题
- Windows / Linux 实现也不统一
- keyring crate 选择**只暴露最小公共子集**

所以你想给前端显示"已配置的 provider 列表",必须**自己存一份索引**。`providers.has_key` 就是这个索引。

### 操作时序

```rust
// key_set("anthropic", "sk-ant-xxx"):
1. spawn_blocking → keyring.set_password("sk-ant-xxx")
2. UPDATE providers SET has_key=1 WHERE name='anthropic'  (没有就 INSERT)

// key_get("anthropic"):
1. spawn_blocking → keyring.get_password()  # 直接问 keychain
   (不查 providers 表,因为答案就在 keychain)

// key_delete("anthropic"):
1. spawn_blocking → keyring.delete_credential()
2. UPDATE providers SET has_key=0 WHERE name='anthropic'

// key_list_providers():
1. SELECT name FROM providers WHERE has_key=1
   (只查 db,从不查 keychain)
```

**`providers.has_key` 是真相镜像,需要你 keychain 操作完后同步**。如果两边不一致(比如代码崩溃在中间),`key_get` 拿不到 key 但 `list_providers` 还显示有。容忍度可接受 — `key_get` 返回 `None` 时前端就重新走配置流程。

## 📚 4. SQLite UPSERT 语法

`key_set` 要做"existing → UPDATE,not exists → INSERT",sqlite 的 upsert:

```sql
INSERT INTO providers (name, has_key) VALUES (?, 1)
ON CONFLICT(name) DO UPDATE SET
  has_key = 1,
  updated_at = datetime('now');
```

要点:

- **`ON CONFLICT(<col>)`** 必须是 PRIMARY KEY 或 UNIQUE constraint
- **`DO UPDATE SET ...`** 是冲突时的更新逻辑
- 跟 PostgreSQL `ON CONFLICT (...) DO UPDATE SET ...` 语法一致;跟 MySQL `ON DUPLICATE KEY UPDATE` 不一样

## 🔧 5. C4 任务清单

### 5.1 改 `error.rs`

加变体:

```rust
#[error("join: {0}")]
JoinError(#[from] tokio::task::JoinError),
```

`Serialize` `code` match 加:

```rust
AppError::JoinError(_) => "join_error",
```

### 5.2 新建 `db/keychain.rs` —— 仅 providers 表的 db 层

```rust
//! providers 表的 db 操作。Keychain 真值在 OS 层,这里只维护索引。

use crate::AppResult;
use sqlx::SqlitePool;

pub async fn upsert_has_key(pool: &SqlitePool, provider: &str, has: bool) -> AppResult<()> {
    let v: i64 = if has { 1 } else { 0 };
    sqlx::query!(
        "INSERT INTO providers (name, has_key) VALUES (?, ?)
         ON CONFLICT(name) DO UPDATE SET
           has_key = excluded.has_key,
           updated_at = datetime('now')",
        provider,
        v,
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_providers_with_key(pool: &SqlitePool) -> AppResult<Vec<String>> {
    let rows = sqlx::query!(r#"SELECT name AS "name!" FROM providers WHERE has_key = 1"#)
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(|r| r.name).collect())
}
```

注意 `excluded.has_key` 是 SQLite 特殊语法 —— 引用本次 INSERT 试图插入的值。等价于直接写 `has_key = ?2`,但更可读。

### 5.3 在 `db/mod.rs` 加 `pub mod keychain;`

### 5.4 改 `commands/keychain.rs` —— 实现 4 个命令

```rust
use crate::db;
use crate::state::AppState;
use crate::{AppError, AppResult};
use tauri::State;
use tokio::task::spawn_blocking;

const KEYRING_SERVICE: &str = "xyz.yukin.agent";

#[tauri::command]
pub async fn key_set(
    provider: String,
    key: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let provider_clone = provider.clone();
    spawn_blocking(move || -> AppResult<()> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, &provider_clone)?;
        entry.set_password(&key)?;
        Ok(())
    })
    .await??;

    db::keychain::upsert_has_key(&state.db, &provider, true).await
}

#[tauri::command]
pub async fn key_exists(provider: String) -> AppResult<bool> {
    let result = spawn_blocking(move || -> AppResult<bool> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, &provider)?;
        match entry.get_password() {
            Ok(_) => Ok(true),
            Err(keyring::Error::NoEntry) => Ok(false),    // 决策 C4 (ii)
            Err(e) => Err(AppError::from(e)),
        }
    })
    .await??;
    Ok(result)
}

#[tauri::command]
pub async fn key_delete(
    provider: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let provider_clone = provider.clone();
    spawn_blocking(move || -> AppResult<()> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, &provider_clone)?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),       // 已经不存在 = 幂等
            Err(e) => Err(AppError::from(e)),
        }
    })
    .await??;

    db::keychain::upsert_has_key(&state.db, &provider, false).await
}

#[tauri::command]
pub async fn key_list_providers(state: State<'_, AppState>) -> AppResult<Vec<String>> {
    db::keychain::list_providers_with_key(&state.db).await
}
```

**注意**:`key_get` 这个内部函数 **(返回 String 给 Rust 内部用,不暴露给前端)** 后续 Phase F 才需要。C4 我们只暴露 `key_exists`(返回 bool),前端永远不直接拿 key。

> **Phase E 文档已经讲过**:`tauri.ts` wrapper 不要包装 `key_get`,只暴露 `set/exists/delete/list`。但 C4 阶段我们 stub 里既有 `key_set/exists/delete/list`,**没有 `key_get`**(B5 时就没注册到 generate_handler!),所以 C4 不用动 `lib.rs`。

### 5.5 不需要改 `lib.rs`

`generate_handler!` 里 `key_set / key_exists / key_delete / key_list_providers` B5 时已经齐全。

## 🔧 6. C4 验证

### 6.1 单元测试(可选但建议)

`db/keychain.rs` 末尾加:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test(migrations = "./migrations")]
    async fn upsert_then_list(pool: SqlitePool) {
        upsert_has_key(&pool, "anthropic", true).await.unwrap();
        upsert_has_key(&pool, "openai", false).await.unwrap();
        upsert_has_key(&pool, "google", true).await.unwrap();

        let mut list = list_providers_with_key(&pool).await.unwrap();
        list.sort();
        assert_eq!(list, vec!["anthropic", "google"]);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn upsert_overwrites(pool: SqlitePool) {
        upsert_has_key(&pool, "anthropic", true).await.unwrap();
        upsert_has_key(&pool, "anthropic", false).await.unwrap();
        let list = list_providers_with_key(&pool).await.unwrap();
        assert_eq!(list, Vec::<String>::new());
    }
}
```

注意:**这些测试不碰真 keychain**,只测 db 层。keychain 操作要真用户机器才能验证(下面 6.2)。

### 6.2 端到端(devtools)

```js
const { invoke } = await import('@tauri-apps/api/core');

// 设
await invoke('key_set', { provider: 'anthropic', key: 'sk-ant-test-DELETE-ME' });

// 列
await invoke('key_list_providers');                    // ['anthropic']

// 验
await invoke('key_exists', { provider: 'anthropic' }); // true
await invoke('key_exists', { provider: 'openai' });    // false

// macOS:打开 Keychain Access, 搜 'xyz.yukin.agent', 应该看到一条 anthropic

// 删
await invoke('key_delete', { provider: 'anthropic' });

await invoke('key_list_providers');                    // []
await invoke('key_exists', { provider: 'anthropic' }); // false
```

### 6.3 cargo check + cargo test

```
cargo check              # 应通过
cargo test --lib keychain   # 2 个 db 测试过
```

## 📌 7. C4 卡点

- **`spawn_blocking` 不能借用 state** —— `&AppState` 不是 `'static`,closure 进不去。所以"db 操作 + keychain 操作"必须在 spawn_blocking **外面** 拆开
- **macOS 第一次跑会弹授权框** —— 询问"是否允许 yukin 访问 Keychain",点允许;之后 Always allow
- **`Entry::new` 在 Linux 可能失败** —— 没装 secret service 时(罕见,GNOME/KDE 默认有)。我们 v1 不处理,失败让用户看错误
- **clone 噪音** —— `provider.clone()` 让 spawn_blocking 和后面的 db 操作各持一份。代价小,可读

---

# C5 — Sessions + Messages (1:N + JSON content)

## 📚 8. Sessions / Messages 是什么

```
sessions
─────────
id            TEXT PK  (uuid)
title         TEXT     (用户可改)
workspace_path TEXT     (会话锁定的 workspace)
provider      TEXT
model         TEXT
timestamps

      │ 1:N (ON DELETE CASCADE)
      ▼

messages
─────────
id           TEXT PK (uuid)
session_id   TEXT FK → sessions(id)
role         TEXT CHECK ∈ {system,user,assistant,tool}
content      TEXT     (JSON 字符串, Anthropic Messages 格式)
tool_calls   TEXT?    (Phase G 填)
tool_results TEXT?    (Phase G 填)
step_index   INTEGER?
created_at   TEXT
```

**业务上**:每个会话(Chat) 有一串消息;删会话 → 消息也删(cascade)。

**关键的 6 个命令**:

| 命令                                  | 操作                                                |
| ------------------------------------- | --------------------------------------------------- |
| `session_create(title)`             | 新会话,返回 Session row                             |
| `session_list()`                    | 列出所有会话(按 updated_at 降序)                    |
| `session_update(id, patch)`         | patch 语义更新(title/workspace_path/provider/model) |
| `session_delete(id)`                | 删 → cascade 删 messages                           |
| `session_append_message(...)`       | 加一条 message                                      |
| `session_load_messages(session_id)` | 列出该会话的所有消息                                |

## 📚 9. JSON content 字段(决策 C5 (i))

`messages.content` 存什么?Anthropic Messages API 的 content 是**数组**(可包含多个 block):

```json
[
  { "type": "text", "text": "Hello" },
  { "type": "tool_use", "id": "...", "name": "fs_read", "input": { ... } },
  { "type": "tool_result", "tool_use_id": "...", "content": "file content" }
]
```

**v1 我们怎么存**:**把整个数组 `JSON.stringify` 后存为 TEXT**(跟 `memory.metadata` 一致)。

```rust
// Rust 端 DTO
pub struct MessageRow {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,           // ← 仍是 String, 前端 JSON.parse
    pub tool_calls: Option<String>,
    pub tool_results: Option<String>,
    pub step_index: Option<i64>,
    pub created_at: String,
}
```

**为什么不用 sqlx::types::Json`<Value>`?**

- 跟 `memory.metadata` 风格一致(C3 已经决定 String + 前端 parse)
- v1 后端不读 content 内部结构,**没必要付强类型成本**
- Phase G 真要按 content 内部 query(如统计 tool_use 次数)再升级

## 📚 10. 1:N cascade 实测(C5 重头戏)

C1 schema 写了 `ON DELETE CASCADE`,C2 在 `open_db` 加了 `foreign_keys(true)`。**但这两件事都没真验证过**。C5 的 `delete_cascades_messages` 测试就是验。

```rust
#[sqlx::test(migrations = "./migrations")]
async fn delete_session_cascades_messages(pool: SqlitePool) {
    // 注意:#[sqlx::test] 创建的临时 db 默认不开 foreign_keys!
    // 必须手动开,验我们 production AppState::new 的 PRAGMA 是否真生效
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool).await.unwrap();

    // arrange
    let session = create(&pool, "test session".into()).await.unwrap();
    append_message(&pool, &session.id, "user", "{\"text\":\"hi\"}").await.unwrap();
    append_message(&pool, &session.id, "assistant", "{\"text\":\"yo\"}").await.unwrap();
    assert_eq!(load_messages(&pool, &session.id).await.unwrap().len(), 2);

    // act
    delete(&pool, &session.id).await.unwrap();

    // assert
    let msgs: Vec<MessageRow> = sqlx::query_as!(
        MessageRow,
        // ...
    ).fetch_all(&pool).await.unwrap();
    assert_eq!(msgs.len(), 0, "messages should be cascaded");
}
```

**关键警示**:`#[sqlx::test]` 临时 db 默认 `foreign_keys=OFF`!**必须手动 PRAGMA**。这是 SQLite 原生行为(C2 概念课讲过 PRAGMA 是 per-connection)。production 路径 `AppState::new` 的 `foreign_keys(true)` 是对每个新连接都设,但 `#[sqlx::test]` 池子用了不同初始化路径。

> **生产 vs 测试的差异是常见陷阱** —— 测试通过不代表 production 一样。这个测试的真正价值是"模拟 production 配置后,cascade 还工作吗",所以 PRAGMA 必须模仿 production。

## 🔧 11. C5 任务清单

### 11.1 新建 `db/session.rs`

```rust
use crate::AppResult;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

// ============ Types ============

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCreateInput {
    pub title: String,
    pub workspace_path: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionUpdate {
    pub title: Option<String>,
    pub workspace_path: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct SessionRow {
    pub id: String,
    pub title: String,
    pub workspace_path: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageAppendInput {
    pub session_id: String,
    pub role: String,             // 'system' | 'user' | 'assistant' | 'tool'
    pub content: String,          // JSON 字符串
    pub tool_calls: Option<String>,
    pub tool_results: Option<String>,
    pub step_index: Option<i64>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct MessageRow {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub tool_calls: Option<String>,
    pub tool_results: Option<String>,
    pub step_index: Option<i64>,
    pub created_at: String,
}

// ============ Sessions ============

pub async fn create(pool: &SqlitePool, input: SessionCreateInput) -> AppResult<SessionRow> {
    let id = Uuid::new_v4().to_string();
    sqlx::query!(
        "INSERT INTO sessions (id, title, workspace_path, provider, model)
         VALUES (?, ?, ?, ?, ?)",
        id,
        input.title,
        input.workspace_path,
        input.provider,
        input.model,
    )
    .execute(pool)
    .await?;
    fetch(pool, &id).await
}

pub async fn fetch(pool: &SqlitePool, id: &str) -> AppResult<SessionRow> {
    sqlx::query_as!(
        SessionRow,
        r#"SELECT
             id             AS "id!",
             title          AS "title!",
             workspace_path,
             provider,
             model,
             created_at     AS "created_at!",
             updated_at     AS "updated_at!"
           FROM sessions WHERE id = ?"#,
        id,
    )
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

pub async fn list(pool: &SqlitePool) -> AppResult<Vec<SessionRow>> {
    // ORDER BY rowid DESC 作 tiebreaker:datetime('now') 秒级精度,
    // 同秒创建的多条 updated_at 相同,rowid 降序保证后插入的排前。
    sqlx::query_as!(
        SessionRow,
        r#"SELECT
             id             AS "id!",
             title          AS "title!",
             workspace_path,
             provider,
             model,
             created_at     AS "created_at!",
             updated_at     AS "updated_at!"
           FROM sessions ORDER BY updated_at DESC, rowid DESC LIMIT 100"#,
    )
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

pub async fn update(
    pool: &SqlitePool,
    id: &str,
    patch: SessionUpdate,
) -> AppResult<SessionRow> {
    sqlx::query!(
        "UPDATE sessions SET
           title          = COALESCE(?1, title),
           workspace_path = COALESCE(?2, workspace_path),
           provider       = COALESCE(?3, provider),
           model          = COALESCE(?4, model),
           updated_at     = datetime('now')
         WHERE id = ?5",
        patch.title,
        patch.workspace_path,
        patch.provider,
        patch.model,
        id,
    )
    .execute(pool)
    .await?;
    fetch(pool, id).await
}

pub async fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    sqlx::query!("DELETE FROM sessions WHERE id = ?", id)
        .execute(pool)
        .await?;
    Ok(())
}

// ============ Messages ============

pub async fn append_message(
    pool: &SqlitePool,
    input: MessageAppendInput,
) -> AppResult<MessageRow> {
    let id = Uuid::new_v4().to_string();
    sqlx::query!(
        "INSERT INTO messages
           (id, session_id, role, content, tool_calls, tool_results, step_index)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
        id,
        input.session_id,
        input.role,
        input.content,
        input.tool_calls,
        input.tool_results,
        input.step_index,
    )
    .execute(pool)
    .await?;

    sqlx::query_as!(
        MessageRow,
        r#"SELECT
             id            AS "id!",
             session_id    AS "session_id!",
             role          AS "role!",
             content       AS "content!",
             tool_calls,
             tool_results,
             step_index,
             created_at    AS "created_at!"
           FROM messages WHERE id = ?"#,
        id,
    )
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

pub async fn load_messages(
    pool: &SqlitePool,
    session_id: &str,
) -> AppResult<Vec<MessageRow>> {
    sqlx::query_as!(
        MessageRow,
        r#"SELECT
             id            AS "id!",
             session_id    AS "session_id!",
             role          AS "role!",
             content       AS "content!",
             tool_calls,
             tool_results,
             step_index,
             created_at    AS "created_at!"
           FROM messages WHERE session_id = ? ORDER BY created_at ASC"#,
        session_id,
    )
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

// ============ Tests ============

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test(migrations = "./migrations")]
    async fn create_then_list(pool: SqlitePool) {
        let s1 = create(&pool, SessionCreateInput {
            title: "first".into(),
            workspace_path: None, provider: None, model: None,
        }).await.unwrap();
        let _s2 = create(&pool, SessionCreateInput {
            title: "second".into(),
            workspace_path: None, provider: None, model: None,
        }).await.unwrap();

        let all = list(&pool).await.unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].title, "second", "list should be ordered by updated_at DESC");

        let fetched = fetch(&pool, &s1.id).await.unwrap();
        assert_eq!(fetched.title, "first");
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn delete_cascades_messages(pool: SqlitePool) {
        // 重要:#[sqlx::test] 临时 db 默认 foreign_keys = OFF
        // 必须手动开,模拟 production AppState::new 的 PRAGMA
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool).await.unwrap();

        let s = create(&pool, SessionCreateInput {
            title: "doomed".into(),
            workspace_path: None, provider: None, model: None,
        }).await.unwrap();

        append_message(&pool, MessageAppendInput {
            session_id: s.id.clone(), role: "user".into(),
            content: r#"[{"type":"text","text":"hi"}]"#.into(),
            tool_calls: None, tool_results: None, step_index: None,
        }).await.unwrap();
        append_message(&pool, MessageAppendInput {
            session_id: s.id.clone(), role: "assistant".into(),
            content: r#"[{"type":"text","text":"yo"}]"#.into(),
            tool_calls: None, tool_results: None, step_index: None,
        }).await.unwrap();

        assert_eq!(load_messages(&pool, &s.id).await.unwrap().len(), 2);

        delete(&pool, &s.id).await.unwrap();

        // 验证 cascade
        let remaining: Vec<MessageRow> = sqlx::query_as!(
            MessageRow,
            r#"SELECT
                 id            AS "id!",
                 session_id    AS "session_id!",
                 role          AS "role!",
                 content       AS "content!",
                 tool_calls,
                 tool_results,
                 step_index,
                 created_at    AS "created_at!"
               FROM messages WHERE session_id = ?"#,
            s.id,
        ).fetch_all(&pool).await.unwrap();
        assert_eq!(remaining.len(), 0, "ON DELETE CASCADE should remove messages");
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn update_patch_only_title(pool: SqlitePool) {
        let s = create(&pool, SessionCreateInput {
            title: "old".into(),
            workspace_path: Some("/tmp".into()),
            provider: Some("anthropic".into()),
            model: None,
        }).await.unwrap();

        let updated = update(&pool, &s.id, SessionUpdate {
            title: Some("new".into()),
            workspace_path: None,
            provider: None,
            model: None,
        }).await.unwrap();

        assert_eq!(updated.title, "new");
        assert_eq!(updated.workspace_path.as_deref(), Some("/tmp"), "patch shouldn't touch unspecified fields");
        assert_eq!(updated.provider.as_deref(), Some("anthropic"));
    }
}
```

### 11.2 在 `db/mod.rs` 加 `pub mod session;`

### 11.3 改 `commands/session.rs` —— 6 个命令的薄壳

```rust
use crate::db::session::{
    self, MessageAppendInput, MessageRow, SessionCreateInput, SessionRow, SessionUpdate,
};
use crate::state::AppState;
use crate::AppResult;
use tauri::State;

#[tauri::command]
pub async fn session_create(
    input: SessionCreateInput,
    state: State<'_, AppState>,
) -> AppResult<SessionRow> {
    session::create(&state.db, input).await
}

#[tauri::command]
pub async fn session_list(state: State<'_, AppState>) -> AppResult<Vec<SessionRow>> {
    session::list(&state.db).await
}

#[tauri::command]
pub async fn session_update(
    id: String,
    patch: SessionUpdate,
    state: State<'_, AppState>,
) -> AppResult<SessionRow> {
    session::update(&state.db, &id, patch).await
}

#[tauri::command]
pub async fn session_delete(id: String, state: State<'_, AppState>) -> AppResult<()> {
    session::delete(&state.db, &id).await
}

#[tauri::command]
pub async fn session_append_message(
    input: MessageAppendInput,
    state: State<'_, AppState>,
) -> AppResult<MessageRow> {
    session::append_message(&state.db, input).await
}

#[tauri::command]
pub async fn session_load_messages(
    session_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<MessageRow>> {
    session::load_messages(&state.db, &session_id).await
}
```

**注意**:旧 stub 的命令签名跟新签名不一样(stub 是 `session_create(_title: String)`,新是 `session_create(input: SessionCreateInput)`)。**直接覆盖整个文件**。

### 11.4 不需要改 `lib.rs`

`generate_handler!` 里 6 个 session 命令 B5 时已注册全。

## 🔧 12. C5 验证

### 12.1 cargo test

```
cargo test --lib session    # 应 3 个测试全过
```

特别留意 `delete_cascades_messages` —— 这个测试通过证明:

1. C1 schema 的 `ON DELETE CASCADE` 写对
2. `PRAGMA foreign_keys=ON` 真激活 cascade
3. `db::session::delete` 不需要手动删 messages,db 自己处理

### 12.2 端到端(devtools)

```js
const { invoke } = await import('@tauri-apps/api/core');

// 1. 创建会话
const s = await invoke('session_create', {
  input: {
    title: 'My first chat',
    workspacePath: '/tmp/yukin-test',
    provider: 'anthropic',
    model: 'claude-sonnet-4-6',
  }
});
console.log('session:', s);

// 2. append 几条消息
await invoke('session_append_message', {
  input: {
    sessionId: s.id,
    role: 'user',
    content: JSON.stringify([{ type: 'text', text: 'Hi' }]),
  }
});
await invoke('session_append_message', {
  input: {
    sessionId: s.id,
    role: 'assistant',
    content: JSON.stringify([{ type: 'text', text: 'Hello' }]),
  }
});

// 3. load messages
const msgs = await invoke('session_load_messages', { sessionId: s.id });
console.log('messages:', msgs);    // 应 2 条

// 4. update title
const updated = await invoke('session_update', {
  id: s.id,
  patch: { title: 'Renamed chat' }
});
console.log('updated:', updated);

// 5. list
const all = await invoke('session_list');
console.log('list:', all);

// 6. delete (cascade)
await invoke('session_delete', { id: s.id });

const after = await invoke('session_load_messages', { sessionId: s.id });
console.log('after cascade:', after);    // 应 []
```

(可以照 C3 在 settings.tsx 加个按钮跑这串,或者 devtools 手敲。)

## 📌 13. C5 卡点

- **`#[sqlx::test]` 默认 foreign_keys OFF** —— `delete_cascades_messages` 必须手动 `PRAGMA foreign_keys=ON`
- **append_message 的 INSERT-then-fetch** —— 跟 memory 一致,SQLite 没有 RETURNING for INSERT
- **content 是 String** —— 前端发的时候要 `JSON.stringify`,接到再 `JSON.parse`。这是项目级约定
- **stub 命令签名不一样** —— B3 stub 写的 `session_create(title: String)` 是 placeholder,C5 改成 `(input: SessionCreateInput)`,**整个文件覆盖**别 patch
- **`session_update` 全 None 时刷 updated_at** —— 跟 `memory_update` 一致,符合"标记看过"语义
- **`datetime('now')` 秒级精度 + ORDER BY 顺序未定义** ⚠️ 实测踩坑:
  - 测试里两次 `create` 同秒完成 → `updated_at` 完全相同
  - `ORDER BY updated_at DESC` 遇相等值时 SQLite 顺序未定义,实践中倾向按 `rowid` 升序,DESC 后反而"先创建的排前",`all[0].title` 是 `"first"` 而非 `"second"`
  - **production 也会撞**:用户快速连点两次"新建会话"即触发
  - **修法**:`ORDER BY updated_at DESC, rowid DESC`。`rowid` 是 SQLite 普通表的隐式自增列(= 插入顺序),作 tiebreaker 让后插入的排前,语义上"最近优先"更准。`memory::list` 同样要改

---

# 📌 14. 完成定义(C4 + C5 共同)

- [ ] `error.rs` 加 `JoinError` 变体 + serialize code
- [ ] `db/keychain.rs` 实装 + 2 个 db 测试通过
- [ ] `commands/keychain.rs` 4 个命令实装(`spawn_blocking` + double `?` + clone provider)
- [ ] keychain 端到端验证(devtools + Keychain Access 看到 `xyz.yukin.agent` 条目)
- [ ] `db/session.rs` 实装 + 3 个测试通过(含 cascade)
- [ ] `commands/session.rs` 6 个命令薄壳
- [ ] sessions 端到端验证(devtools 跑完 6 命令链路)
- [ ] `cargo check` 通过(剩 ≤5 warnings,都是 Phase F/H 占位)
- [ ] 主进度文档 C4 + C5 标 `[x]`,实际收获记录

---

# 📌 15. 决策记录(填进主进度文档)

```
- C4 (i): AppError 加 JoinError(#[from] JoinError),纪律一致
- C4 (ii): NoEntry 视作 Ok(None) / Ok(()),业务"未配置"非错误
- C5 (i): messages.content 用 String,跟 memory.metadata 一致
- C5 (ii): session_update 全 None 时 Ok(()),后端宽松
```

预估总时长 4–6h(C3 已经训练好肌肉记忆,C4/C5 是变奏不是新曲)。
