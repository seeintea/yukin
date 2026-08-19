use reqwest::Client;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{
    agent::{self, CompletionRequest, CompletionStream, Message, ModelError, Role, ThinkingMode},
    protocol::model_provider::ReasoningEffort,
    security::keychain,
    storage::{model_provider, model_response},
    AppResult,
};

pub(crate) struct StreamParams {
    pub conversation_id: String,
    pub provider_id: String,
    pub model_id: String,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub content: String,
}

pub(crate) struct StartedStream {
    pub stream: CompletionStream,
    pub run_id: String,
    pub assistant_message_id: String,
}

pub(crate) async fn stream(
    pool: &SqlitePool,
    client: &Client,
    params: StreamParams,
) -> AppResult<StartedStream> {
    let config = model_provider::find_runtime_config(pool, &params.provider_id).await?;
    let api_key = keychain::get(&config.api_key_alias)?.ok_or(ModelError::MissingCredential)?;
    let run_id = Uuid::now_v7().to_string();
    let user_message_id = Uuid::now_v7().to_string();
    let assistant_message_id = Uuid::now_v7().to_string();
    let history = model_response::start(
        pool,
        model_response::StartParams {
            conversation_id: params.conversation_id,
            run_id: run_id.clone(),
            user_message_id,
            assistant_message_id: assistant_message_id.clone(),
            provider_id: params.provider_id,
            model_id: params.model_id.clone(),
            reasoning_effort: params.reasoning_effort.map(|effort| effort.as_str().into()),
            content: params.content.clone(),
        },
    )
    .await?;

    let mut messages = history
        .into_iter()
        .map(|message| {
            let role = match message.role.as_str() {
                "user" => Role::User,
                "assistant" => Role::Assistant,
                value => {
                    return Err(crate::AppError::Other(format!(
                        "invalid message role: {value}"
                    )))
                }
            };
            Ok(Message {
                role,
                content: message.content,
            })
        })
        .collect::<AppResult<Vec<_>>>()?;
    messages.push(Message {
        role: Role::User,
        content: params.content,
    });

    let mut request = CompletionRequest::new(params.model_id, messages);
    if let Some(reasoning_effort) = params.reasoning_effort {
        request.thinking = Some(ThinkingMode::Enabled);
        request.reasoning_effort = Some(reasoning_effort.into());
    }

    let stream = agent::stream_completion(
        client,
        config.api_format,
        &config.base_url,
        &api_key,
        request,
    )
    .await;

    match stream {
        Ok(stream) => Ok(StartedStream {
            stream,
            run_id,
            assistant_message_id,
        }),
        Err(error) => {
            if let Err(storage_error) =
                model_response::fail(pool, &run_id, &assistant_message_id, "", &error.to_string())
                    .await
            {
                tracing::error!(%storage_error, %run_id, "failed to persist model error");
            }
            Err(error.into())
        }
    }
}
