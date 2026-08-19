use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

use crate::{
    protocol::{
        common::RecordMetadata,
        conversation::{Conversation, Message, MessageRole, MessageStatus, Snapshot},
    },
    AppError, AppResult,
};

#[derive(FromRow)]
struct ConversationRecord {
    id: String,
    title: String,
    created_at: String,
    updated_at: String,
}

#[derive(FromRow)]
struct MessageRecord {
    id: String,
    run_id: Option<String>,
    role: String,
    content: String,
    status: String,
    sequence: i64,
    created_at: String,
    updated_at: String,
}

impl From<ConversationRecord> for Conversation {
    fn from(record: ConversationRecord) -> Self {
        Self {
            id: record.id,
            title: record.title,
            metadata: RecordMetadata {
                created_at: record.created_at,
                updated_at: record.updated_at,
            },
        }
    }
}

impl TryFrom<MessageRecord> for Message {
    type Error = AppError;

    fn try_from(record: MessageRecord) -> Result<Self, Self::Error> {
        let role = match record.role.as_str() {
            "user" => MessageRole::User,
            "assistant" => MessageRole::Assistant,
            "tool" => MessageRole::Tool,
            value => return Err(AppError::Other(format!("invalid message role: {value}"))),
        };
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
            role,
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

pub async fn current(pool: &SqlitePool) -> AppResult<Conversation> {
    let record = sqlx::query_as::<_, ConversationRecord>(
        r#"
        SELECT id, title, created_at, updated_at
        FROM conversations
        ORDER BY updated_at DESC, id DESC
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await?;

    Ok(match record {
        Some(record) => record.into(),
        None => create(pool).await?,
    })
}

pub async fn find(pool: &SqlitePool, id: &str) -> AppResult<Snapshot> {
    let record = sqlx::query_as::<_, ConversationRecord>(
        r#"
        SELECT id, title, created_at, updated_at
        FROM conversations
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await?;
    let conversation = record.into();
    let messages = list_messages(pool, id).await?;

    Ok(Snapshot {
        conversation,
        messages,
    })
}

pub async fn list(pool: &SqlitePool) -> AppResult<Vec<Conversation>> {
    let records = sqlx::query_as::<_, ConversationRecord>(
        r#"
        SELECT id, title, created_at, updated_at
        FROM conversations
        ORDER BY updated_at DESC, id DESC
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(records.into_iter().map(Into::into).collect())
}

pub async fn create(pool: &SqlitePool) -> AppResult<Conversation> {
    let id = Uuid::now_v7().to_string();
    let record = sqlx::query_as::<_, ConversationRecord>(
        r#"
        INSERT INTO conversations (id, title)
        VALUES (?, '新对话')
        RETURNING id, title, created_at, updated_at
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await?;

    Ok(record.into())
}

pub async fn rename(pool: &SqlitePool, id: &str, title: &str) -> AppResult<Conversation> {
    let record = sqlx::query_as::<_, ConversationRecord>(
        r#"
        UPDATE conversations
        SET title = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?
        RETURNING id, title, created_at, updated_at
        "#,
    )
    .bind(title)
    .bind(id)
    .fetch_one(pool)
    .await?;

    Ok(record.into())
}

pub async fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let result = sqlx::query(
        r#"
        DELETE FROM conversations
        WHERE id = ?
          AND NOT EXISTS (
              SELECT 1
              FROM runs
              WHERE conversation_id = ?
                AND status IN ('pending', 'running', 'waiting_approval')
          )
        "#,
    )
    .bind(id)
    .bind(id)
    .execute(pool)
    .await?;
    if result.rows_affected() == 1 {
        return Ok(());
    }

    let has_active_run: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM runs
            WHERE conversation_id = ?
              AND status IN ('pending', 'running', 'waiting_approval')
        )
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await?;
    if has_active_run {
        Err(AppError::RunState(
            "conversation has an active agent run".into(),
        ))
    } else {
        Err(sqlx::Error::RowNotFound.into())
    }
}

pub async fn list_messages(pool: &SqlitePool, conversation_id: &str) -> AppResult<Vec<Message>> {
    let records = sqlx::query_as::<_, MessageRecord>(
        r#"
        SELECT id, run_id, role, content, status, sequence, created_at, updated_at
        FROM messages
        WHERE conversation_id = ?
        ORDER BY sequence
        "#,
    )
    .bind(conversation_id)
    .fetch_all(pool)
    .await?;

    records.into_iter().map(TryInto::try_into).collect()
}

#[cfg(test)]
mod tests {
    use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

    use crate::AppError;

    use super::{create, delete, find, rename};

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
        pool
    }

    #[tokio::test]
    async fn renames_and_deletes_conversation_with_related_records() {
        let pool = setup().await;
        let conversation = create(&pool).await.expect("conversation");
        let renamed = rename(&pool, &conversation.id, "项目讨论")
            .await
            .expect("rename");
        assert_eq!(renamed.title, "项目讨论");

        sqlx::query(
            r#"
            INSERT INTO runs (id, conversation_id, provider_id, model_id, status)
            VALUES ('run-1', ?, 'provider-1', 'model-1', 'completed')
            "#,
        )
        .bind(&conversation.id)
        .execute(&pool)
        .await
        .expect("run");
        sqlx::query(
            r#"
            INSERT INTO messages (id, conversation_id, run_id, role, content, status, sequence)
            VALUES ('message-1', ?, 'run-1', 'user', 'question', 'completed', 1)
            "#,
        )
        .bind(&conversation.id)
        .execute(&pool)
        .await
        .expect("message");

        delete(&pool, &conversation.id).await.expect("delete");

        assert!(matches!(
            find(&pool, &conversation.id).await,
            Err(AppError::Db(sqlx::Error::RowNotFound))
        ));
        let runs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM runs WHERE conversation_id = ?")
            .bind(&conversation.id)
            .fetch_one(&pool)
            .await
            .expect("run count");
        assert_eq!(runs, 0);
    }

    #[tokio::test]
    async fn refuses_to_delete_conversation_with_active_run() {
        let pool = setup().await;
        let conversation = create(&pool).await.expect("conversation");
        sqlx::query(
            r#"
            INSERT INTO runs (id, conversation_id, provider_id, model_id, status)
            VALUES ('run-active', ?, 'provider-1', 'model-1', 'running')
            "#,
        )
        .bind(&conversation.id)
        .execute(&pool)
        .await
        .expect("active run");

        assert!(matches!(
            delete(&pool, &conversation.id).await,
            Err(AppError::RunState(_))
        ));
    }
}
