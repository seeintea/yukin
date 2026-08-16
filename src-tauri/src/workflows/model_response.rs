use reqwest::Client;
use sqlx::SqlitePool;

use crate::{
    agent::{self, CompletionRequest, CompletionStream, Message, ModelError, Role},
    security::keychain,
    storage::model_provider,
    AppResult,
};

pub(crate) struct StreamParams {
    pub provider_id: String,
    pub model_id: String,
    pub content: String,
}

pub(crate) async fn stream(
    pool: &SqlitePool,
    client: &Client,
    params: StreamParams,
) -> AppResult<CompletionStream> {
    let config = model_provider::find_runtime_config(pool, &params.provider_id).await?;
    let api_key = keychain::get(&config.api_key_alias)?.ok_or(ModelError::MissingCredential)?;

    agent::stream_completion(
        client,
        config.api_format,
        &config.base_url,
        &api_key,
        CompletionRequest::new(
            params.model_id,
            vec![Message {
                role: Role::User,
                content: params.content,
            }],
        ),
    )
    .await
    .map_err(Into::into)
}
