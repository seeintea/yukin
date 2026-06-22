// ============ Types ============

use crate::AppResult;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

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
    pub role: String,    // 'system' | 'user' | 'assistant' | 'tool'
    pub content: String, // JSON 字符串
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
    // ORDER BY rowid DESC 作为 tiebreaker:
    // datetime('now') 是秒级精度,同一秒内创建的多个 session updated_at 相同,
    // 此时按 rowid(隐式自增插入顺序)降序 → 后创建的排前,符合"最近优先"语义。
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

pub async fn update(pool: &SqlitePool, id: &str, patch: SessionUpdate) -> AppResult<SessionRow> {
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

pub async fn append_message(pool: &SqlitePool, input: MessageAppendInput) -> AppResult<MessageRow> {
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

pub async fn load_messages(pool: &SqlitePool, session_id: &str) -> AppResult<Vec<MessageRow>> {
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
        let s1 = create(
            &pool,
            SessionCreateInput {
                title: "first".into(),
                workspace_path: None,
                provider: None,
                model: None,
            },
        )
        .await
        .unwrap();
        let _s2 = create(
            &pool,
            SessionCreateInput {
                title: "second".into(),
                workspace_path: None,
                provider: None,
                model: None,
            },
        )
        .await
        .unwrap();

        let all = list(&pool).await.unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(
            all[0].title, "second",
            "list should be ordered by updated_at DESC"
        );

        let fetched = fetch(&pool, &s1.id).await.unwrap();
        assert_eq!(fetched.title, "first");
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn delete_cascades_messages(pool: SqlitePool) {
        // 重要:#[sqlx::test] 临时 db 默认 foreign_keys = OFF
        // 必须手动开,模拟 production AppState::new 的 PRAGMA
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .unwrap();

        let s = create(
            &pool,
            SessionCreateInput {
                title: "doomed".into(),
                workspace_path: None,
                provider: None,
                model: None,
            },
        )
        .await
        .unwrap();

        append_message(
            &pool,
            MessageAppendInput {
                session_id: s.id.clone(),
                role: "user".into(),
                content: r#"[{"type":"text","text":"hi"}]"#.into(),
                tool_calls: None,
                tool_results: None,
                step_index: None,
            },
        )
        .await
        .unwrap();
        append_message(
            &pool,
            MessageAppendInput {
                session_id: s.id.clone(),
                role: "assistant".into(),
                content: r#"[{"type":"text","text":"yo"}]"#.into(),
                tool_calls: None,
                tool_results: None,
                step_index: None,
            },
        )
        .await
        .unwrap();

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
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(
            remaining.len(),
            0,
            "ON DELETE CASCADE should remove messages"
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn update_patch_only_title(pool: SqlitePool) {
        let s = create(
            &pool,
            SessionCreateInput {
                title: "old".into(),
                workspace_path: Some("/tmp".into()),
                provider: Some("anthropic".into()),
                model: None,
            },
        )
        .await
        .unwrap();

        let updated = update(
            &pool,
            &s.id,
            SessionUpdate {
                title: Some("new".into()),
                workspace_path: None,
                provider: None,
                model: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(updated.title, "new");
        assert_eq!(
            updated.workspace_path.as_deref(),
            Some("/tmp"),
            "patch shouldn't touch unspecified fields"
        );
        assert_eq!(updated.provider.as_deref(), Some("anthropic"));
    }
}
