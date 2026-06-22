//! 纯数据库层 - memory 表 CRUD + FTS5 搜索
//!
//! 设计原则(详见 docs/plan/learning-notes/C3-sqlx-and-dto-and-tests.md):
//! - 无 #[tauri::command],无 State<AppState>,可独立测试
//! - 操作函数收 `&SqlitePool`,由 commands 层注入
//! - DTO 三类型分离: Input(写) / Update(patch) / Row(读)

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::AppResult;

// ============ Types ============

/// memory.kind 字段的合法值。
///
/// - 序列化为 snake_case 字符串(`user` / `feedback` / `project` / `reference`),
///   与 schema CHECK 约束 + 前端 JSON 对齐
/// - sqlx 端通过 `as_str()` 显式转字符串后 `bind`(不实现 sqlx::Type 以避免反向 decode 复杂度)
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

/// 创建一条 memory 时的输入(用户 → 后端)。
///
/// 不含 id / created_at / updated_at —— 前者由 `Uuid::new_v4()` 在 save 内生成,
/// 后两者由 SQLite `DEFAULT (datetime('now'))` 自动填。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySaveInput {
    pub name: String,
    pub kind: MemoryKind,
    pub content: String,
    pub description: Option<String>,
    /// JSON 任意结构;db 层 `serde_json::to_string` 后存为 TEXT。`None` → `"{}"`
    pub metadata: Option<serde_json::Value>,
    /// `None` 表示全局 memory(跨 workspace 可见)
    pub workspace: Option<String>,
}

/// 更新一条 memory 时的 patch(用户 → 后端)。
///
/// 全字段 `Option`:`Some(v)` 表示要改成 `v`,`None` 表示不动。
/// SQL 用 `COALESCE(?, original)` 模式实现部分更新。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryUpdate {
    pub name: Option<String>,
    pub description: Option<String>,
    pub content: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// 从 db 读出的完整 memory 行(后端 → 用户)。
///
/// - `kind` 是 String(不是 `MemoryKind`):避免实现 sqlx Decode/Type;
///   db CHECK 已保证只有 4 个合法值,前端 TS 端用 union type 收
/// - `metadata` 是 JSON 字符串:前端自行 `JSON.parse`
/// - `created_at` / `updated_at` 是 String:格式 `YYYY-MM-DD HH:MM:SS`(SQLite `datetime('now')`),
///   字典序 = 时间序,排序/范围过滤直接走 SQL,Rust 端无需 chrono
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
        Some(v) => serde_json::to_string(&v)?,
        None => "{}".to_string(),
    };
    let kind_str = input.kind.as_str();

    sqlx::query!(
        "INSERT INTO memory (id, name, kind, description, content, metadata, workspace) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
        id,
        input.name,
        kind_str,
        input.description,
        input.content,
        metadata_str,
        input.workspace,
    )
    .execute(pool)
    .await?;

    fetch(pool, &id).await
}

pub async fn fetch(pool: &SqlitePool, id: &str) -> AppResult<MemoryRow> {
    // SQLite 不向 sqlx 暴露 NOT NULL 信息,需要用 AS "col!" 语法显式断言
    // 非空列 (id / name / kind / content / metadata / created_at / updated_at)。
    // description / workspace 是 NULL-able,无需断言。
    sqlx::query_as!(
        MemoryRow,
        r#"SELECT
             id          AS "id!",
             name        AS "name!",
             kind        AS "kind!",
             description,
             content     AS "content!",
             metadata    AS "metadata!",
             workspace,
             created_at  AS "created_at!",
             updated_at  AS "updated_at!"
           FROM memory WHERE id = ?"#,
        id
    )
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

/// FTS5 全文搜索 memory.{name, description, content} 三列。
///
/// `kind` 可选过滤;`limit` 由调用方决定(commands 层用 8 作默认)。
///
/// TODO: sanitize FTS5 special chars (paren / OR / quote / star) for human input.
///       Caller is the agent (LLM) for v1, trusted to produce sane queries.
pub async fn recall(
    pool: &SqlitePool,
    query: &str,
    limit: i64,
    kind: Option<&str>,
) -> AppResult<Vec<MemoryRow>> {
    // memory_fts 是 FTS5 虚拟表,sqlx 编译期宏不识别它的列;
    // 所以这里用 query_as runtime 校验。
    match kind {
        Some(k) => sqlx::query_as::<_, MemoryRow>(
            "SELECT m.* FROM memory m \
             JOIN memory_fts f ON f.rowid = m.rowid \
             WHERE memory_fts MATCH ?1 AND m.kind = ?2 \
             ORDER BY rank LIMIT ?3",
        )
        .bind(query)
        .bind(k)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(Into::into),
        None => sqlx::query_as::<_, MemoryRow>(
            "SELECT m.* FROM memory m \
             JOIN memory_fts f ON f.rowid = m.rowid \
             WHERE memory_fts MATCH ?1 \
             ORDER BY rank LIMIT ?2",
        )
        .bind(query)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(Into::into),
    }
}

/// 列出所有 memory,可按 kind 过滤。最多 100 条,按 updated_at 倒序。
pub async fn list(pool: &SqlitePool, kind: Option<&str>) -> AppResult<Vec<MemoryRow>> {
    // ORDER BY rowid DESC 作 tiebreaker:datetime('now') 秒级精度,
    // 同秒创建的多条 updated_at 相同,rowid 降序保证后插入的排前。
    match kind {
        Some(k) => sqlx::query_as!(
            MemoryRow,
            r#"SELECT
                 id          AS "id!",
                 name        AS "name!",
                 kind        AS "kind!",
                 description,
                 content     AS "content!",
                 metadata    AS "metadata!",
                 workspace,
                 created_at  AS "created_at!",
                 updated_at  AS "updated_at!"
               FROM memory WHERE kind = ? ORDER BY updated_at DESC, rowid DESC LIMIT 100"#,
            k
        )
        .fetch_all(pool)
        .await
        .map_err(Into::into),
        None => sqlx::query_as!(
            MemoryRow,
            r#"SELECT
                 id          AS "id!",
                 name        AS "name!",
                 kind        AS "kind!",
                 description,
                 content     AS "content!",
                 metadata    AS "metadata!",
                 workspace,
                 created_at  AS "created_at!",
                 updated_at  AS "updated_at!"
               FROM memory ORDER BY updated_at DESC, rowid DESC LIMIT 100"#
        )
        .fetch_all(pool)
        .await
        .map_err(Into::into),
    }
}

/// 删除一条 memory。FTS5 索引由 schema 的 `memory_ad` trigger 自动同步。
///
/// 不存在的 id 视作幂等(execute 静默成功,rows_affected=0)。
pub async fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    sqlx::query!("DELETE FROM memory WHERE id = ?", id)
        .execute(pool)
        .await?;
    Ok(())
}

/// 部分更新一条 memory。`MemoryUpdate` 全 Option,`None` 字段保持原值。
///
/// SQL 用 `COALESCE(?, original)` 实现 patch 语义。
/// `updated_at` 总是被刷新,即使所有 patch 字段都是 None
/// (语义"标记看过了",可接受的 feature)。
pub async fn update(
    pool: &SqlitePool,
    id: &str,
    patch: MemoryUpdate,
) -> AppResult<MemoryRow> {
    let metadata_str = match patch.metadata {
        Some(v) => Some(serde_json::to_string(&v)?),
        None => None,
    };

    sqlx::query!(
        "UPDATE memory SET \
           name        = COALESCE(?1, name), \
           description = COALESCE(?2, description), \
           content     = COALESCE(?3, content), \
           metadata    = COALESCE(?4, metadata), \
           updated_at  = datetime('now') \
         WHERE id = ?5",
        patch.name,
        patch.description,
        patch.content,
        metadata_str,
        id,
    )
    .execute(pool)
    .await?;

    fetch(pool, id).await
}

// ============ Tests ============

#[cfg(test)]
mod tests {
    use super::*;

    fn input(name: &str, content: &str) -> MemorySaveInput {
        MemorySaveInput {
            name: name.into(),
            kind: MemoryKind::User,
            content: content.into(),
            description: None,
            metadata: None,
            workspace: None,
        }
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn save_then_recall(pool: SqlitePool) {
        let row = save(&pool, input("hello", "rust async world")).await.unwrap();

        let hits = recall(&pool, "world", 10, None).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, row.id);
        assert_eq!(hits[0].content, "rust async world");
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn delete_then_recall_returns_empty(pool: SqlitePool) {
        // 验 schema 的 memory_ad trigger:删除后 FTS 索引同步清理
        let row = save(&pool, input("doomed", "to be deleted")).await.unwrap();
        delete(&pool, &row.id).await.unwrap();

        let hits = recall(&pool, "doomed", 10, None).await.unwrap();
        assert_eq!(
            hits.len(),
            0,
            "DELETE trigger should remove the row from memory_fts"
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn update_content_changes_recall_target(pool: SqlitePool) {
        // 验 schema 的 memory_au trigger:更新后旧内容不再命中,新内容命中
        let row = save(&pool, input("n", "alpha")).await.unwrap();

        update(
            &pool,
            &row.id,
            MemoryUpdate {
                name: None,
                description: None,
                content: Some("beta".into()),
                metadata: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(recall(&pool, "alpha", 10, None).await.unwrap().len(), 0);
        assert_eq!(recall(&pool, "beta", 10, None).await.unwrap().len(), 1);
    }
}
