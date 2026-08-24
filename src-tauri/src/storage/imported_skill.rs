use sqlx::{FromRow, SqlitePool};

use crate::{
    protocol::{
        common::RecordMetadata,
        imported_skill::{ImportedSkill, SourceKind},
    },
    AppError, AppResult,
};

pub(crate) struct CreateParams {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source_kind: SourceKind,
    pub managed_path: String,
    pub content_digest: String,
}

#[derive(FromRow)]
struct ImportedSkillRecord {
    id: String,
    name: String,
    description: String,
    source_kind: String,
    content_digest: String,
    enabled: bool,
    created_at: String,
    updated_at: String,
}

impl TryFrom<ImportedSkillRecord> for ImportedSkill {
    type Error = AppError;

    fn try_from(record: ImportedSkillRecord) -> Result<Self, Self::Error> {
        Ok(Self {
            id: record.id,
            name: record.name,
            description: record.description,
            source_kind: SourceKind::try_from(record.source_kind).map_err(AppError::Other)?,
            content_digest: record.content_digest,
            enabled: record.enabled,
            metadata: RecordMetadata {
                created_at: record.created_at,
                updated_at: record.updated_at,
            },
        })
    }
}

pub async fn create(pool: &SqlitePool, params: CreateParams) -> AppResult<ImportedSkill> {
    let record = sqlx::query_as::<_, ImportedSkillRecord>(
        "INSERT INTO imported_skills (id, name, description, source_kind, managed_path, content_digest) VALUES (?, ?, ?, ?, ?, ?) RETURNING id, name, description, source_kind, content_digest, enabled, created_at, updated_at",
    )
        .bind(params.id)
        .bind(params.name)
        .bind(params.description)
        .bind(params.source_kind.as_str())
        .bind(params.managed_path)
        .bind(params.content_digest)
        .fetch_one(pool)
        .await?;
    record.try_into()
}

pub async fn list(pool: &SqlitePool) -> AppResult<Vec<ImportedSkill>> {
    sqlx::query_as::<_, ImportedSkillRecord>(
        "SELECT id, name, description, source_kind, content_digest, enabled, created_at, updated_at FROM imported_skills WHERE deleted_at IS NULL ORDER BY created_at DESC, id DESC",
    )
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(TryInto::try_into)
        .collect()
}

pub async fn set_enabled(pool: &SqlitePool, id: &str, enabled: bool) -> AppResult<ImportedSkill> {
    let record = sqlx::query_as::<_, ImportedSkillRecord>(
        "UPDATE imported_skills SET enabled = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ? AND deleted_at IS NULL RETURNING id, name, description, source_kind, content_digest, enabled, created_at, updated_at",
    )
        .bind(enabled)
        .bind(id)
        .fetch_one(pool)
        .await?;
    record.try_into()
}

pub async fn managed_path(pool: &SqlitePool, id: &str) -> AppResult<String> {
    Ok(sqlx::query_scalar::<_, String>(
        "SELECT managed_path FROM imported_skills WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_one(pool)
    .await?)
}

pub async fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE imported_skills SET deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}
