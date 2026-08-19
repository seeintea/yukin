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
            value => return Err(AppError::Other(format!("invalid message role: {value}"))),
        };
        let status = match record.status.as_str() {
            "streaming" => MessageStatus::Streaming,
            "completed" => MessageStatus::Completed,
            "failed" => MessageStatus::Failed,
            value => return Err(AppError::Other(format!("invalid message status: {value}"))),
        };

        Ok(Self {
            id: record.id,
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

async fn list_messages(pool: &SqlitePool, conversation_id: &str) -> AppResult<Vec<Message>> {
    let records = sqlx::query_as::<_, MessageRecord>(
        r#"
        SELECT id, role, content, status, sequence, created_at, updated_at
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
