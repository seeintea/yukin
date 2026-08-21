use std::collections::HashMap;

use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

use crate::{
    protocol::{
        common::RecordMetadata,
        conversation::{
            Attachment, Conversation, DirectoryScope, Message, MessageRole, MessageStatus, Snapshot,
        },
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

#[derive(FromRow)]
struct AttachmentRecord {
    message_id: String,
    name: String,
    size: i64,
}

#[derive(FromRow)]
struct DirectoryScopeRecord {
    message_id: String,
    name: String,
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
        let role = MessageRole::try_from(record.role.as_str()).map_err(AppError::Other)?;
        let status = MessageStatus::try_from(record.status.as_str()).map_err(AppError::Other)?;

        Ok(Self {
            id: record.id,
            run_id: record.run_id,
            role,
            content: record.content,
            attachments: Vec::new(),
            directory_scopes: Vec::new(),
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

    let attachment_records = sqlx::query_as::<_, AttachmentRecord>(
        r#"
        SELECT attachment.message_id, attachment.name, attachment.size
        FROM message_attachments attachment
        INNER JOIN messages message ON message.id = attachment.message_id
        WHERE message.conversation_id = ?
        ORDER BY attachment.name
        "#,
    )
    .bind(conversation_id)
    .fetch_all(pool)
    .await?;
    let mut attachments = HashMap::<String, Vec<Attachment>>::new();
    for record in attachment_records {
        attachments
            .entry(record.message_id)
            .or_default()
            .push(Attachment {
                name: record.name,
                size: record.size as u64,
            });
    }
    let scope_records = sqlx::query_as::<_, DirectoryScopeRecord>(
        r#"
        SELECT scope.message_id, scope.name
        FROM message_directory_scopes scope
        INNER JOIN messages message ON message.id = scope.message_id
        WHERE message.conversation_id = ?
        ORDER BY scope.name
        "#,
    )
    .bind(conversation_id)
    .fetch_all(pool)
    .await?;
    let mut directory_scopes = HashMap::<String, Vec<DirectoryScope>>::new();
    for record in scope_records {
        directory_scopes
            .entry(record.message_id)
            .or_default()
            .push(DirectoryScope { name: record.name });
    }

    records
        .into_iter()
        .map(|record| {
            let mut message: Message = record.try_into()?;
            message.attachments = attachments.remove(&message.id).unwrap_or_default();
            message.directory_scopes = directory_scopes.remove(&message.id).unwrap_or_default();
            Ok(message)
        })
        .collect()
}

#[cfg(test)]
mod tests;
