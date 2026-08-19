use std::time::Duration;

use reqwest::Client;
use sqlx::SqlitePool;
use tokio::time::{timeout, Instant};
use uuid::Uuid;

use crate::{
    agent::{self, CompletionRequest, Message, ModelError, Role},
    model_provider::catalog,
    protocol::model_provider::{
        CreateRequest, ModelProvider, TestConnectionRequest, TestConnectionResponse,
    },
    security::keychain,
    storage::model_provider::{self, CreateParams},
    AppResult,
};

const CONNECTION_TEST_TIMEOUT: Duration = Duration::from_secs(15);

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

pub async fn test_connection(
    pool: &SqlitePool,
    client: &Client,
    request: TestConnectionRequest,
) -> AppResult<TestConnectionResponse> {
    let provider = model_provider::find(pool, &request.provider_id).await?;
    let preset = catalog::find_model_provider_preset(&provider.provider_key).ok_or_else(|| {
        crate::AppError::Other(format!(
            "unsupported model provider: {}",
            provider.provider_key
        ))
    })?;
    let model_supported = preset
        .connections
        .iter()
        .find(|connection| connection.api_format == provider.api_format)
        .is_some_and(|connection| {
            connection
                .models
                .iter()
                .any(|model| model.model_id == request.model_id)
        });
    if !model_supported {
        return Err(ModelError::InvalidRequest {
            status: 400,
            message: format!(
                "model {} is not supported by provider {}",
                request.model_id, provider.provider_key
            ),
        }
        .into());
    }

    let config = model_provider::find_runtime_config(pool, &request.provider_id).await?;
    let api_key = keychain::get(&config.api_key_alias)?
        .ok_or_else(|| crate::AppError::from(ModelError::MissingCredential))?;
    let completion_request = CompletionRequest::new(
        request.model_id.clone(),
        vec![Message::text(
            Role::User,
            "Reply with only OK to confirm the connection.".into(),
        )],
    );
    let started_at = Instant::now();
    let completion = timeout(
        CONNECTION_TEST_TIMEOUT,
        agent::complete(
            client,
            config.api_format,
            &config.base_url,
            &api_key,
            completion_request,
        ),
    )
    .await
    .map_err(|_| ModelError::Timeout)??;
    if !completion
        .content
        .as_deref()
        .is_some_and(|content| !content.trim().is_empty())
    {
        return Err(ModelError::Protocol("connection test returned no text".into()).into());
    }

    Ok(TestConnectionResponse {
        model_id: request.model_id,
        latency_ms: started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
    })
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

#[cfg(test)]
mod tests {
    use reqwest::Client;
    use sqlx::sqlite::SqlitePoolOptions;

    use crate::{agent::ModelError, protocol::model_provider::TestConnectionRequest, AppError};

    use super::test_connection;

    #[tokio::test]
    async fn rejects_model_outside_provider_preset_before_request() {
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

        let error = test_connection(
            &pool,
            &Client::new(),
            TestConnectionRequest {
                provider_id: "provider-1".into(),
                model_id: "unknown-model".into(),
            },
        )
        .await
        .expect_err("unsupported model rejected");

        assert!(matches!(
            error,
            AppError::Model(ModelError::InvalidRequest { status: 400, .. })
        ));
    }
}
