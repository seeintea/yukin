use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{
    protocol::{
        common::RecordMetadata,
        model_provider::{ApiFormat, CreateRequest, ModelProvider, UpdateRequest},
    },
    AppError, AppResult,
};

struct ModelProviderRecord {
    id: String,
    provider_name: String,
    api_format: String,
    base_url: String,
    provider_alias: String,
    api_key_alias: String,
    created_at: String,
    updated_at: String,
}

impl TryFrom<ModelProviderRecord> for ModelProvider {
    type Error = AppError;

    fn try_from(record: ModelProviderRecord) -> Result<Self, Self::Error> {
        Ok(Self {
            id: record.id,
            provider_name: record.provider_name,
            api_format: ApiFormat::try_from(record.api_format).map_err(AppError::Other)?,
            base_url: record.base_url,
            provider_alias: record.provider_alias,
            api_key_alias: record.api_key_alias,
            metadata: RecordMetadata {
                created_at: record.created_at,
                updated_at: record.updated_at,
            },
        })
    }
}

pub async fn create(pool: &SqlitePool, request: CreateRequest) -> AppResult<ModelProvider> {
    let id = Uuid::now_v7().to_string();
    // TODO save API KEY safety
    let api_key_alias = request.api_key;
    let record = sqlx::query_as!(
        ModelProviderRecord,
        r#"
        INSERT INTO model_providers (
            id, provider_name, api_format, base_url, provider_alias, api_key_alias
        ) VALUES (?, ?, ?, ?, ?, ?)
        RETURNING id, provider_name, api_format, base_url, provider_alias,
                  api_key_alias, created_at, updated_at
        "#,
        id,
        request.provider_name,
        request.api_format.as_str(),
        request.base_url,
        request.provider_alias,
        api_key_alias
    )
    .fetch_one(pool)
    .await?;

    tracing::info!(provider_id = %&record.id, "model provider created");
    record.try_into()
}

pub async fn find(pool: &SqlitePool, id: &str) -> AppResult<ModelProvider> {
    let record = sqlx::query_as!(
        ModelProviderRecord,
        r#"
        SELECT id, provider_name, api_format, base_url, provider_alias,
               api_key_alias, created_at, updated_at
        FROM model_providers
        WHERE id = ? AND deleted_at IS NULL
        "#,
        id,
    )
    .fetch_one(pool)
    .await?;

    tracing::info!(provider_id = %record.id, "model provider loaded");
    record.try_into()
}

pub async fn list(pool: &SqlitePool) -> AppResult<Vec<ModelProvider>> {
    let records = sqlx::query_as!(
        ModelProviderRecord,
        r#"
            SELECT id, provider_name, api_format, base_url, provider_alias,
                   api_key_alias, created_at, updated_at
            FROM model_providers
            WHERE deleted_at IS NULL
            ORDER BY created_at DESC, id DESC
            "#,
    )
    .fetch_all(pool)
    .await?;

    tracing::info!(count = records.len(), "model providers listed");
    records.into_iter().map(TryInto::try_into).collect()
}

pub async fn update(pool: &SqlitePool, request: UpdateRequest) -> AppResult<ModelProvider> {
    let api_format = request.api_format.map(ApiFormat::as_str);
    // TODO save API KEY safety
    let api_key_alias = match request.api_key {
        Some(api_key) => Some(api_key),
        None => None,
    };
    let record = sqlx::query_as!(
        ModelProviderRecord,
        r#"
        UPDATE model_providers
        SET provider_name = COALESCE(?, provider_name),
            api_format = COALESCE(?, api_format),
            base_url = COALESCE(?, base_url),
            provider_alias = COALESCE(?, provider_alias),
            api_key_alias = COALESCE(?, api_key_alias),
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ? AND deleted_at IS NULL
        RETURNING id, provider_name, api_format, base_url, provider_alias,
                  api_key_alias, created_at, updated_at
        "#,
        request.provider_name,
        api_format,
        request.base_url,
        request.provider_alias,
        api_key_alias,
        request.id,
    )
    .fetch_one(pool)
    .await?;

    tracing::info!(provider_id = %record.id, "model provider updated");
    record.try_into()
}

pub async fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let result = sqlx::query!(
        r#"
        UPDATE model_providers
        SET deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ? AND deleted_at IS NULL
        "#,
        id,
    )
    .execute(pool)
    .await?;

    tracing::info!(
        provider_id = %id,
        rows_affected = result.rows_affected(),
        "model provider soft deleted"
    );
    Ok(())
}
