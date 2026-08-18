use reqwest::Client;
use sqlx::SqlitePool;

use crate::{
    agent::{self, CompletionRequest, CompletionStream, Message, ModelError, Role, ThinkingMode},
    protocol::model_provider::ReasoningEffort,
    security::keychain,
    storage::model_provider,
    AppResult,
};

pub(crate) struct StreamParams {
    pub provider_id: String,
    pub model_id: String,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub content: String,
}

pub(crate) async fn stream(
    pool: &SqlitePool,
    client: &Client,
    params: StreamParams,
) -> AppResult<CompletionStream> {
    let config = model_provider::find_runtime_config(pool, &params.provider_id).await?;
    let api_key = keychain::get(&config.api_key_alias)?.ok_or(ModelError::MissingCredential)?;

    let mut request = CompletionRequest::new(
        params.model_id,
        vec![Message {
            role: Role::User,
            content: params.content,
        }],
    );
    if let Some(reasoning_effort) = params.reasoning_effort {
        request.thinking = Some(ThinkingMode::Enabled);
        request.reasoning_effort = Some(reasoning_effort.into());
    }

    agent::stream_completion(
        client,
        config.api_format,
        &config.base_url,
        &api_key,
        request,
    )
    .await
    .map_err(Into::into)
}
