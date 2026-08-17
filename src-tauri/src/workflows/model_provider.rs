use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{
    model_provider::catalog,
    protocol::model_provider::{CreateRequest, ModelProvider},
    security::keychain,
    storage::model_provider::{self, CreateParams},
    AppResult,
};

pub async fn create(pool: &SqlitePool, request: CreateRequest) -> AppResult<ModelProvider> {
    let preset = catalog::find_model_provider_preset(&request.provider_key).ok_or_else(|| {
        crate::AppError::Other(format!(
            "unsupported model provider: {}",
            request.provider_key
        ))
    })?;
    if !preset
        .connections
        .iter()
        .any(|connection| connection.api_format == request.api_format)
    {
        return Err(crate::AppError::Other(format!(
            "unsupported API format {} for model provider {}",
            request.api_format.as_str(),
            request.provider_key
        )));
    }

    let id = Uuid::now_v7().to_string();
    let api_key_alias = Uuid::now_v7().to_string();

    keychain::set(&api_key_alias, &request.api_key)?;

    let result = model_provider::create(
        pool,
        CreateParams {
            id,
            provider_key: preset.provider_key,
            api_format: request.api_format,
            base_url: request.base_url,
            provider_alias: request.provider_alias,
            api_key_alias: api_key_alias.clone(),
        },
    )
    .await;

    if result.is_err() {
        rollback_credential(&api_key_alias, None);
    }

    result
}

pub async fn replace_credential(pool: &SqlitePool, id: &str, api_key: &str) -> AppResult<()> {
    let api_key_alias = model_provider::find_api_key_alias(pool, id).await?;
    keychain::set(&api_key_alias, api_key)?;

    tracing::info!(provider_id = %id, "model provider credential replaced");
    Ok(())
}

pub async fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let api_key_alias = model_provider::find_api_key_alias(pool, id).await?;
    let previous = keychain::get(&api_key_alias)?;

    keychain::remove(&api_key_alias)?;

    let result = model_provider::delete(pool, id).await;
    if result.is_err() {
        rollback_credential(&api_key_alias, previous.as_deref());
    }

    result
}

fn rollback_credential(api_key_alias: &str, password: Option<&str>) {
    let result = match password {
        Some(password) => keychain::set(api_key_alias, password),
        None => keychain::remove(api_key_alias),
    };

    if let Err(error) = result {
        tracing::error!(%api_key_alias, %error, "failed to roll back model provider credential");
    }
}
