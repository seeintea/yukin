use futures_util::StreamExt;
use tauri::{ipc::Channel, State};

use crate::{
    agent,
    protocol::model_response::{
        StreamEvent as ResponseStreamEvent, StreamRequest, TokenUsage as ResponseTokenUsage,
    },
    storage::model_response as response_storage,
    workflows::model_response::{self, StreamParams},
    AppResult, AppState,
};

#[tauri::command]
pub async fn model_response_stream(
    state: State<'_, AppState>,
    request: StreamRequest,
    events: Channel<ResponseStreamEvent>,
) -> AppResult<()> {
    let started = model_response::stream(
        state.db(),
        state.http(),
        StreamParams {
            conversation_id: request.conversation_id,
            provider_id: request.provider_id,
            model_id: request.model_id,
            reasoning_effort: request.reasoning_effort,
            content: request.content,
        },
    )
    .await?;
    let mut stream = started.stream;
    let mut content = String::new();
    let mut usage = None;

    while let Some(event) = stream.next().await {
        match event {
            Ok(agent::StreamEvent::ReasoningDelta { .. }) => {}
            Ok(agent::StreamEvent::TextDelta { content: delta }) => {
                content.push_str(&delta);
                events.send(ResponseStreamEvent::OutputDelta { content: delta })?;
            }
            Ok(agent::StreamEvent::Completed {
                finish_reason,
                usage: completed_usage,
            }) => {
                usage = completed_usage;
                events.send(ResponseStreamEvent::Completed {
                    finish_reason,
                    usage: completed_usage.map(|usage| ResponseTokenUsage {
                        prompt_tokens: usage.prompt_tokens,
                        completion_tokens: usage.completion_tokens,
                        total_tokens: usage.total_tokens,
                    }),
                })?;
            }
            Err(error) => {
                response_storage::fail(
                    state.db(),
                    &started.run_id,
                    &started.assistant_message_id,
                    &content,
                    &error.to_string(),
                )
                .await?;
                return Err(error.into());
            }
        }
    }

    response_storage::complete(
        state.db(),
        &started.run_id,
        &started.assistant_message_id,
        &content,
        usage,
    )
    .await?;

    Ok(())
}
