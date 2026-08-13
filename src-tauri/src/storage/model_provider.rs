use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{
    protocol::{
        common::RecordTimestamps,
        model_provider::{
            ApiFormat, CreateModelProviderInput, ModelProvider, UpdateModelProviderInput,
        },
    },
    AppError, AppResult,
};

struct ModelProviderRow {
    id: String,
    provider_name: String,
    api_format: String,
    base_url: String,
    provider_alias: String,
    api_key_alias: String,
    created_at: String,
    updated_at: String,
}

impl TryFrom<ModelProviderRow> for ModelProvider {
    type Error = AppError;

    fn try_from(row: ModelProviderRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            provider_name: row.provider_name,
            api_format: ApiFormat::try_from(row.api_format).map_err(AppError::Other)?,
            base_url: row.base_url,
            provider_alias: row.provider_alias,
            api_key_alias: row.api_key_alias,
            timestamps: RecordTimestamps {
                created_at: row.created_at,
                updated_at: row.updated_at,
            },
        })
    }
}

pub async fn create(
    pool: &SqlitePool,
    input: CreateModelProviderInput,
) -> AppResult<ModelProvider> {
    let id = Uuid::now_v7().to_string();
    let row = sqlx::query_as!(
        ModelProviderRow,
        r#"
        INSERT INTO model_providers (
            id, provider_name, api_format, base_url, provider_alias, api_key_alias
        ) VALUES (?, ?, ?, ?, ?, ?)
        RETURNING id, provider_name, api_format, base_url, provider_alias,
                  api_key_alias, created_at, updated_at
        "#,
        id,
        input.provider_name,
        input.api_format.as_str(),
        input.base_url,
        input.provider_alias,
        input.api_key_alias,
    )
    .fetch_one(pool)
    .await?;

    tracing::info!(provider_id = %row.id, "model provider created");
    row.try_into()
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<ModelProvider> {
    let row = sqlx::query_as!(
        ModelProviderRow,
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

    tracing::info!(provider_id = %row.id, "model provider loaded");
    row.try_into()
}

pub async fn list(pool: &SqlitePool, include_deleted: bool) -> AppResult<Vec<ModelProvider>> {
    let rows = if include_deleted {
        sqlx::query_as!(
            ModelProviderRow,
            r#"
            SELECT id, provider_name, api_format, base_url, provider_alias,
                   api_key_alias, created_at, updated_at
            FROM model_providers
            ORDER BY created_at DESC, id DESC
            "#,
        )
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as!(
            ModelProviderRow,
            r#"
            SELECT id, provider_name, api_format, base_url, provider_alias,
                   api_key_alias, created_at, updated_at
            FROM model_providers
            WHERE deleted_at IS NULL
            ORDER BY created_at DESC, id DESC
            "#,
        )
        .fetch_all(pool)
        .await?
    };

    tracing::info!(
        count = rows.len(),
        include_deleted,
        "model providers listed"
    );
    rows.into_iter().map(TryInto::try_into).collect()
}

pub async fn update(
    pool: &SqlitePool,
    input: UpdateModelProviderInput,
) -> AppResult<ModelProvider> {
    let api_format = input.api_format.map(ApiFormat::as_str);
    let row = sqlx::query_as!(
        ModelProviderRow,
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
        input.provider_name,
        api_format,
        input.base_url,
        input.provider_alias,
        input.api_key_alias,
        input.id,
    )
    .fetch_one(pool)
    .await?;

    tracing::info!(provider_id = %row.id, "model provider updated");
    row.try_into()
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
