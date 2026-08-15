use futures_util::StreamExt;
use tauri::{ipc::Channel, State};

use crate::{
    agent,
    protocol::model_response::{
        StreamEvent as ResponseStreamEvent, StreamRequest, TokenUsage as ResponseTokenUsage,
    },
    workflows::model_response::{self, StreamParams},
    AppResult, AppState,
};

#[tauri::command]
pub async fn model_response_stream(
    state: State<'_, AppState>,
    request: StreamRequest,
    events: Channel<ResponseStreamEvent>,
) -> AppResult<()> {
    let mut stream = model_response::stream(
        state.db(),
        state.http(),
        StreamParams {
            provider_id: request.provider_id,
            model_id: request.model_id,
            content: request.content,
        },
    )
    .await?;

    while let Some(event) = stream.next().await {
        events.send(map_event(event?))?;
    }

    Ok(())
}

fn map_event(event: agent::StreamEvent) -> ResponseStreamEvent {
    match event {
        agent::StreamEvent::TextDelta { content } => ResponseStreamEvent::OutputDelta { content },
        agent::StreamEvent::Completed {
            finish_reason,
            usage,
        } => ResponseStreamEvent::Completed {
            finish_reason,
            usage: usage.map(|usage| ResponseTokenUsage {
                prompt_tokens: usage.prompt_tokens,
                completion_tokens: usage.completion_tokens,
                total_tokens: usage.total_tokens,
            }),
        },
    }
}
