use sqlx::SqlitePool;

use crate::{
    protocol::{
        common::RecordMetadata,
        model_provider::{ApiFormat, ModelProvider},
    },
    AppError, AppResult,
};

pub(crate) struct CreateParams {
    pub id: String,
    pub provider_key: String,
    pub api_format: ApiFormat,
    pub base_url: String,
    pub provider_alias: String,
    pub api_key_alias: String,
}

pub(crate) struct UpdateParams {
    pub id: String,
    pub api_format: Option<ApiFormat>,
    pub base_url: Option<String>,
    pub provider_alias: Option<String>,
}

pub(crate) struct RuntimeConfig {
    pub api_format: ApiFormat,
    pub base_url: String,
    pub api_key_alias: String,
}

struct ModelProviderRecord {
    id: String,
    provider_key: String,
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
            provider_key: record.provider_key,
            api_format: ApiFormat::try_from(record.api_format).map_err(AppError::Other)?,
            base_url: record.base_url,
            provider_alias: record.provider_alias,
            metadata: RecordMetadata {
                created_at: record.created_at,
                updated_at: record.updated_at,
            },
        })
    }
}

pub async fn create(pool: &SqlitePool, params: CreateParams) -> AppResult<ModelProvider> {
    let record = sqlx::query_as!(
        ModelProviderRecord,
        r#"
        INSERT INTO model_providers (
            id, provider_key, api_format, base_url, provider_alias, api_key_alias
        ) VALUES (?, ?, ?, ?, ?, ?)
        RETURNING id, provider_key, api_format, base_url, provider_alias,
                  api_key_alias, created_at, updated_at
        "#,
        params.id,
        params.provider_key,
        params.api_format.as_str(),
        params.base_url,
        params.provider_alias,
        params.api_key_alias
    )
    .fetch_one(pool)
    .await?;

    tracing::debug!(provider_id = %&record.id, "model provider created");
    record.try_into()
}

pub async fn find(pool: &SqlitePool, id: &str) -> AppResult<ModelProvider> {
    find_record(pool, id).await?.try_into()
}

pub(crate) async fn find_api_key_alias(pool: &SqlitePool, id: &str) -> AppResult<String> {
    Ok(find_record(pool, id).await?.api_key_alias)
}

pub(crate) async fn find_runtime_config(pool: &SqlitePool, id: &str) -> AppResult<RuntimeConfig> {
    let record = find_record(pool, id).await?;

    Ok(RuntimeConfig {
        api_format: ApiFormat::try_from(record.api_format).map_err(AppError::Other)?,
        base_url: record.base_url,
        api_key_alias: record.api_key_alias,
    })
}

async fn find_record(pool: &SqlitePool, id: &str) -> AppResult<ModelProviderRecord> {
    let record = sqlx::query_as!(
        ModelProviderRecord,
        r#"
        SELECT id, provider_key, api_format, base_url, provider_alias,
               api_key_alias, created_at, updated_at
        FROM model_providers
        WHERE id = ? AND deleted_at IS NULL
        "#,
        id,
    )
    .fetch_one(pool)
    .await?;

    tracing::debug!(provider_id = %record.id, "model provider loaded");
    Ok(record)
}

pub async fn list(pool: &SqlitePool) -> AppResult<Vec<ModelProvider>> {
    let records = sqlx::query_as!(
        ModelProviderRecord,
        r#"
            SELECT id, provider_key, api_format, base_url, provider_alias,
                   api_key_alias, created_at, updated_at
            FROM model_providers
            WHERE deleted_at IS NULL
            ORDER BY created_at DESC, id DESC
            "#,
    )
    .fetch_all(pool)
    .await?;

    tracing::debug!(count = records.len(), "model providers listed");
    records.into_iter().map(TryInto::try_into).collect()
}

pub async fn update(pool: &SqlitePool, params: UpdateParams) -> AppResult<ModelProvider> {
    let api_format = params.api_format.map(ApiFormat::as_str);
    let record = sqlx::query_as!(
        ModelProviderRecord,
        r#"
        UPDATE model_providers
        SET api_format = COALESCE(?, api_format),
            base_url = COALESCE(?, base_url),
            provider_alias = COALESCE(?, provider_alias),
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ? AND deleted_at IS NULL
        RETURNING id, provider_key, api_format, base_url, provider_alias,
                  api_key_alias, created_at, updated_at
        "#,
        api_format,
        params.base_url,
        params.provider_alias,
        params.id,
    )
    .fetch_one(pool)
    .await?;

    tracing::debug!(provider_id = %record.id, "model provider updated");
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

    tracing::debug!(
        provider_id = %id,
        rows_affected = result.rows_affected(),
        "model provider soft deleted"
    );
    Ok(())
}
