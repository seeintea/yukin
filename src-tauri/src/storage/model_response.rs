use sqlx::{FromRow, SqlitePool};

use crate::{agent::TokenUsage, AppResult};

pub(crate) struct StartParams {
    pub conversation_id: String,
    pub run_id: String,
    pub user_message_id: String,
    pub assistant_message_id: String,
    pub provider_id: String,
    pub model_id: String,
    pub reasoning_effort: Option<String>,
    pub content: String,
}

pub(crate) struct HistoryMessage {
    pub role: String,
    pub content: String,
}

#[derive(FromRow)]
struct HistoryMessageRecord {
    role: String,
    content: String,
}

pub(crate) async fn start(
    pool: &SqlitePool,
    params: StartParams,
) -> AppResult<Vec<HistoryMessage>> {
    let mut transaction = pool.begin().await?;
    let records = sqlx::query_as::<_, HistoryMessageRecord>(
        r#"
        SELECT role, content
        FROM messages
        WHERE conversation_id = ? AND status = 'completed'
        ORDER BY sequence
        "#,
    )
    .bind(&params.conversation_id)
    .fetch_all(&mut *transaction)
    .await?;
    let next_sequence: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(sequence), 0) + 1 FROM messages WHERE conversation_id = ?",
    )
    .bind(&params.conversation_id)
    .fetch_one(&mut *transaction)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO runs (
            id, conversation_id, provider_id, model_id, reasoning_effort, status
        ) VALUES (?, ?, ?, ?, ?, 'running')
        "#,
    )
    .bind(&params.run_id)
    .bind(&params.conversation_id)
    .bind(&params.provider_id)
    .bind(&params.model_id)
    .bind(&params.reasoning_effort)
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO messages (
            id, conversation_id, run_id, role, content, status, sequence
        ) VALUES (?, ?, ?, 'user', ?, 'completed', ?)
        "#,
    )
    .bind(&params.user_message_id)
    .bind(&params.conversation_id)
    .bind(&params.run_id)
    .bind(&params.content)
    .bind(next_sequence)
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO messages (
            id, conversation_id, run_id, role, content, status, sequence
        ) VALUES (?, ?, ?, 'assistant', '', 'streaming', ?)
        "#,
    )
    .bind(&params.assistant_message_id)
    .bind(&params.conversation_id)
    .bind(&params.run_id)
    .bind(next_sequence + 1)
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        r#"
        UPDATE conversations
        SET title = CASE
                WHEN title = '新对话' THEN trim(?)
                ELSE title
            END,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?
        "#,
    )
    .bind(&params.content)
    .bind(&params.conversation_id)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;

    Ok(records
        .into_iter()
        .map(|record| HistoryMessage {
            role: record.role,
            content: record.content,
        })
        .collect())
}

pub(crate) async fn complete(
    pool: &SqlitePool,
    run_id: &str,
    assistant_message_id: &str,
    content: &str,
    usage: Option<TokenUsage>,
) -> AppResult<()> {
    let mut transaction = pool.begin().await?;
    sqlx::query(
        r#"
        UPDATE messages
        SET content = ?, status = 'completed',
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?
        "#,
    )
    .bind(content)
    .bind(assistant_message_id)
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        r#"
        UPDATE runs
        SET status = 'completed', prompt_tokens = ?, completion_tokens = ?, total_tokens = ?,
            completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?
        "#,
    )
    .bind(usage.map(|value| value.prompt_tokens as i64))
    .bind(usage.map(|value| value.completion_tokens as i64))
    .bind(usage.map(|value| value.total_tokens as i64))
    .bind(run_id)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(())
}

pub(crate) async fn fail(
    pool: &SqlitePool,
    run_id: &str,
    assistant_message_id: &str,
    content: &str,
    error_message: &str,
) -> AppResult<()> {
    let mut transaction = pool.begin().await?;
    sqlx::query(
        r#"
        UPDATE messages
        SET content = ?, status = 'failed',
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?
        "#,
    )
    .bind(content)
    .bind(assistant_message_id)
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        r#"
        UPDATE runs
        SET status = 'failed', error_message = ?,
            completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?
        "#,
    )
    .bind(error_message)
    .bind(run_id)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(())
}
