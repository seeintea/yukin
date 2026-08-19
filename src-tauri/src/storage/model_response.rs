use sqlx::{FromRow, SqlitePool};

use crate::{
    agent::TokenUsage,
    protocol::{
        agent_run::{Run, RunStatus, Snapshot},
        common::RecordMetadata,
        conversation::{Message, MessageRole, MessageStatus},
    },
    AppError, AppResult,
};

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

#[derive(FromRow)]
struct RunRecord {
    id: String,
    conversation_id: String,
    provider_id: String,
    model_id: String,
    reasoning_effort: Option<String>,
    status: String,
    error_code: Option<String>,
    error_message: Option<String>,
    prompt_tokens: Option<i64>,
    completion_tokens: Option<i64>,
    total_tokens: Option<i64>,
    created_at: String,
    started_at: Option<String>,
    completed_at: Option<String>,
}

#[derive(FromRow)]
struct AssistantMessageRecord {
    id: String,
    run_id: Option<String>,
    content: String,
    status: String,
    sequence: i64,
    created_at: String,
    updated_at: String,
}

impl TryFrom<RunRecord> for Run {
    type Error = AppError;

    fn try_from(record: RunRecord) -> Result<Self, Self::Error> {
        let status = match record.status.as_str() {
            "pending" => RunStatus::Pending,
            "running" => RunStatus::Running,
            "waiting_approval" => RunStatus::WaitingApproval,
            "completed" => RunStatus::Completed,
            "failed" => RunStatus::Failed,
            "cancelled" => RunStatus::Cancelled,
            value => return Err(AppError::Other(format!("invalid run status: {value}"))),
        };

        Ok(Self {
            id: record.id,
            conversation_id: record.conversation_id,
            provider_id: record.provider_id,
            model_id: record.model_id,
            reasoning_effort: record
                .reasoning_effort
                .map(TryInto::try_into)
                .transpose()
                .map_err(AppError::Other)?,
            status,
            error_code: record.error_code,
            error_message: record.error_message,
            prompt_tokens: record.prompt_tokens,
            completion_tokens: record.completion_tokens,
            total_tokens: record.total_tokens,
            created_at: record.created_at,
            started_at: record.started_at,
            completed_at: record.completed_at,
        })
    }
}

impl TryFrom<AssistantMessageRecord> for Message {
    type Error = AppError;

    fn try_from(record: AssistantMessageRecord) -> Result<Self, Self::Error> {
        let status = match record.status.as_str() {
            "streaming" => MessageStatus::Streaming,
            "completed" => MessageStatus::Completed,
            "failed" => MessageStatus::Failed,
            "cancelled" => MessageStatus::Cancelled,
            value => return Err(AppError::Other(format!("invalid message status: {value}"))),
        };

        Ok(Self {
            id: record.id,
            run_id: record.run_id,
            role: MessageRole::Assistant,
            content: record.content,
            status,
            sequence: record.sequence,
            metadata: RecordMetadata {
                created_at: record.created_at,
                updated_at: record.updated_at,
            },
        })
    }
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
        ) VALUES (?, ?, ?, ?, ?, 'pending')
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

pub(crate) async fn mark_started(pool: &SqlitePool, run_id: &str) -> AppResult<()> {
    let result = sqlx::query(
        r#"
        UPDATE runs
        SET status = 'running', started_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ? AND status = 'pending'
        "#,
    )
    .bind(run_id)
    .execute(pool)
    .await?;
    ensure_transition(result.rows_affected(), run_id, "pending", "running")?;
    Ok(())
}

pub(crate) async fn update_partial(
    pool: &SqlitePool,
    assistant_message_id: &str,
    content: &str,
) -> AppResult<()> {
    let result = sqlx::query(
        r#"
        UPDATE messages
        SET content = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ? AND status = 'streaming'
        "#,
    )
    .bind(content)
    .bind(assistant_message_id)
    .execute(pool)
    .await?;
    ensure_transition(
        result.rows_affected(),
        assistant_message_id,
        "streaming",
        "streaming",
    )?;
    Ok(())
}

pub(crate) async fn complete(
    pool: &SqlitePool,
    run_id: &str,
    assistant_message_id: &str,
    content: &str,
    usage: Option<TokenUsage>,
) -> AppResult<()> {
    let mut transaction = pool.begin().await?;
    let message_result = sqlx::query(
        r#"
        UPDATE messages
        SET content = ?, status = 'completed',
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ? AND status = 'streaming'
        "#,
    )
    .bind(content)
    .bind(assistant_message_id)
    .execute(&mut *transaction)
    .await?;
    let run_result = sqlx::query(
        r#"
        UPDATE runs
        SET status = 'completed', prompt_tokens = ?, completion_tokens = ?, total_tokens = ?,
            completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ? AND status = 'running'
        "#,
    )
    .bind(usage.map(|value| value.prompt_tokens as i64))
    .bind(usage.map(|value| value.completion_tokens as i64))
    .bind(usage.map(|value| value.total_tokens as i64))
    .bind(run_id)
    .execute(&mut *transaction)
    .await?;
    ensure_transition(
        message_result.rows_affected(),
        assistant_message_id,
        "streaming",
        "completed",
    )?;
    ensure_transition(run_result.rows_affected(), run_id, "running", "completed")?;
    transaction.commit().await?;
    Ok(())
}

pub(crate) async fn fail(
    pool: &SqlitePool,
    run_id: &str,
    assistant_message_id: &str,
    content: &str,
    error_code: &str,
    error_message: &str,
) -> AppResult<()> {
    let mut transaction = pool.begin().await?;
    let message_result = sqlx::query(
        r#"
        UPDATE messages
        SET content = ?, status = 'failed',
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ? AND status = 'streaming'
        "#,
    )
    .bind(content)
    .bind(assistant_message_id)
    .execute(&mut *transaction)
    .await?;
    let run_result = sqlx::query(
        r#"
        UPDATE runs
        SET status = 'failed', error_code = ?, error_message = ?,
            completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ? AND status IN ('pending', 'running', 'waiting_approval')
        "#,
    )
    .bind(error_code)
    .bind(error_message)
    .bind(run_id)
    .execute(&mut *transaction)
    .await?;
    ensure_transition(
        message_result.rows_affected(),
        assistant_message_id,
        "streaming",
        "failed",
    )?;
    ensure_transition(run_result.rows_affected(), run_id, "active", "failed")?;
    transaction.commit().await?;
    Ok(())
}

pub(crate) async fn cancel(
    pool: &SqlitePool,
    run_id: &str,
    assistant_message_id: &str,
    content: &str,
) -> AppResult<()> {
    let mut transaction = pool.begin().await?;
    let message_result = sqlx::query(
        r#"
        UPDATE messages
        SET content = ?, status = 'cancelled',
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ? AND status = 'streaming'
        "#,
    )
    .bind(content)
    .bind(assistant_message_id)
    .execute(&mut *transaction)
    .await?;
    let run_result = sqlx::query(
        r#"
        UPDATE runs
        SET status = 'cancelled', completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ? AND status IN ('pending', 'running', 'waiting_approval')
        "#,
    )
    .bind(run_id)
    .execute(&mut *transaction)
    .await?;
    ensure_transition(
        message_result.rows_affected(),
        assistant_message_id,
        "streaming",
        "cancelled",
    )?;
    ensure_transition(run_result.rows_affected(), run_id, "active", "cancelled")?;
    transaction.commit().await?;
    Ok(())
}

pub(crate) async fn snapshot(pool: &SqlitePool, run_id: &str) -> AppResult<Snapshot> {
    let run = sqlx::query_as::<_, RunRecord>(
        r#"
        SELECT id, conversation_id, provider_id, model_id, reasoning_effort, status,
               error_code, error_message, prompt_tokens, completion_tokens, total_tokens,
               created_at, started_at, completed_at
        FROM runs
        WHERE id = ?
        "#,
    )
    .bind(run_id)
    .fetch_one(pool)
    .await?
    .try_into()?;
    let assistant_message = sqlx::query_as::<_, AssistantMessageRecord>(
        r#"
        SELECT id, run_id, content, status, sequence, created_at, updated_at
        FROM messages
        WHERE run_id = ? AND role = 'assistant'
        ORDER BY sequence DESC
        LIMIT 1
        "#,
    )
    .bind(run_id)
    .fetch_one(pool)
    .await?
    .try_into()?;

    Ok(Snapshot {
        run,
        assistant_message,
    })
}

pub(crate) async fn recover_interrupted(pool: &SqlitePool) -> AppResult<u64> {
    let mut transaction = pool.begin().await?;
    sqlx::query(
        r#"
        UPDATE messages
        SET status = 'failed', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE status = 'streaming'
          AND run_id IN (
              SELECT id FROM runs
              WHERE status IN ('pending', 'running', 'waiting_approval')
          )
        "#,
    )
    .execute(&mut *transaction)
    .await?;
    let result = sqlx::query(
        r#"
        UPDATE runs
        SET status = 'failed', error_code = 'run_interrupted',
            error_message = 'run was interrupted by application shutdown',
            completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE status IN ('pending', 'running', 'waiting_approval')
        "#,
    )
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(result.rows_affected())
}

fn ensure_transition(rows_affected: u64, id: &str, expected: &str, target: &str) -> AppResult<()> {
    if rows_affected == 1 {
        Ok(())
    } else {
        Err(AppError::RunState(format!(
            "cannot transition {id} from {expected} to {target}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

    use crate::{agent::TokenUsage, protocol::agent_run::RunStatus, AppError};

    use super::{
        cancel, complete, mark_started, recover_interrupted, snapshot, start, update_partial,
        StartParams,
    };

    async fn setup() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory database");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrations");
        sqlx::query(
            r#"
            INSERT INTO model_providers (
                id, provider_key, api_format, base_url, provider_alias, api_key_alias
            ) VALUES (
                'provider-1', 'deepseek', 'openai', 'https://example.com', 'default', 'key-1'
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("provider");
        sqlx::query("INSERT INTO conversations (id, title) VALUES ('conversation-1', '新对话')")
            .execute(&pool)
            .await
            .expect("conversation");
        pool
    }

    fn start_params(run_id: &str) -> StartParams {
        StartParams {
            conversation_id: "conversation-1".into(),
            run_id: run_id.into(),
            user_message_id: format!("{run_id}-user"),
            assistant_message_id: format!("{run_id}-assistant"),
            provider_id: "provider-1".into(),
            model_id: "model-1".into(),
            reasoning_effort: None,
            content: "第一个问题".into(),
        }
    }

    #[tokio::test]
    async fn persists_run_messages_partial_output_and_completion() {
        let pool = setup().await;
        start(&pool, start_params("run-1"))
            .await
            .expect("start run");

        let pending = snapshot(&pool, "run-1").await.expect("pending snapshot");
        assert_eq!(pending.run.status, RunStatus::Pending);
        assert_eq!(pending.assistant_message.run_id.as_deref(), Some("run-1"));

        mark_started(&pool, "run-1").await.expect("mark started");
        update_partial(&pool, "run-1-assistant", "部分回复")
            .await
            .expect("partial output");
        complete(
            &pool,
            "run-1",
            "run-1-assistant",
            "完整回复",
            Some(TokenUsage {
                prompt_tokens: 2,
                completion_tokens: 3,
                total_tokens: 5,
            }),
        )
        .await
        .expect("complete run");

        let completed = snapshot(&pool, "run-1").await.expect("completed snapshot");
        assert_eq!(completed.run.status, RunStatus::Completed);
        assert_eq!(completed.run.total_tokens, Some(5));
        assert_eq!(completed.assistant_message.content, "完整回复");
        assert!(matches!(
            mark_started(&pool, "run-1").await,
            Err(AppError::RunState(_))
        ));
    }

    #[tokio::test]
    async fn recovers_interrupted_run_and_preserves_partial_output() {
        let pool = setup().await;
        start(&pool, start_params("run-2"))
            .await
            .expect("start run");
        mark_started(&pool, "run-2").await.expect("mark started");
        update_partial(&pool, "run-2-assistant", "保留内容")
            .await
            .expect("partial output");

        assert_eq!(recover_interrupted(&pool).await.expect("recover runs"), 1);

        let recovered = snapshot(&pool, "run-2").await.expect("recovered snapshot");
        assert_eq!(recovered.run.status, RunStatus::Failed);
        assert_eq!(recovered.run.error_code.as_deref(), Some("run_interrupted"));
        assert_eq!(recovered.assistant_message.content, "保留内容");
        assert_eq!(
            serde_json::to_value(recovered.assistant_message.status)
                .expect("message status")
                .as_str(),
            Some("failed")
        );
    }

    #[tokio::test]
    async fn persists_cancelled_run_with_partial_output() {
        let pool = setup().await;
        start(&pool, start_params("run-3"))
            .await
            .expect("start run");
        mark_started(&pool, "run-3").await.expect("mark started");

        cancel(&pool, "run-3", "run-3-assistant", "停止前内容")
            .await
            .expect("cancel run");

        let cancelled = snapshot(&pool, "run-3").await.expect("cancelled snapshot");
        assert_eq!(cancelled.run.status, RunStatus::Cancelled);
        assert_eq!(cancelled.assistant_message.content, "停止前内容");
        assert_eq!(
            serde_json::to_value(cancelled.assistant_message.status)
                .expect("message status")
                .as_str(),
            Some("cancelled")
        );
    }
}
