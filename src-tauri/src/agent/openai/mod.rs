mod protocol;

use std::collections::VecDeque;

use futures_util::{stream, StreamExt};
use reqwest::{header::ACCEPT, Client, StatusCode};

use crate::agent::{
    sse, Completion, CompletionRequest, CompletionStream, ModelError, StreamEvent, TokenUsage,
};

use protocol::{
    ChatCompletionChunk, ChatCompletionRequest, ChatCompletionResponse, ErrorResponse, Usage,
};

pub(crate) async fn complete(
    client: &Client,
    base_url: &str,
    api_key: &str,
    request: CompletionRequest,
) -> Result<Completion, ModelError> {
    let response = client
        .post(base_url)
        .bearer_auth(api_key)
        .json(&ChatCompletionRequest::non_streaming(request))
        .send()
        .await
        .map_err(map_transport_error)?;

    let status = response.status();
    let body = response.text().await.map_err(map_transport_error)?;
    if !status.is_success() {
        return Err(map_response_error(status, &body));
    }

    let response = serde_json::from_str::<ChatCompletionResponse>(&body)
        .map_err(|error| ModelError::Protocol(format!("invalid OpenAI response: {error}")))?;
    let choice = response
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| ModelError::Protocol("OpenAI response contained no choices".into()))?;

    Ok(Completion {
        content: choice.message.content,
        reasoning_content: choice.message.reasoning_content,
        finish_reason: choice.finish_reason,
        usage: response.usage.map(Into::into),
    })
}

pub(crate) async fn stream_completion(
    client: &Client,
    base_url: &str,
    api_key: &str,
    request: CompletionRequest,
) -> Result<CompletionStream, ModelError> {
    let request = ChatCompletionRequest::streaming(request);
    let response = client
        .post(base_url)
        .bearer_auth(api_key)
        .header(ACCEPT, "text/event-stream")
        .json(&request)
        .send()
        .await
        .map_err(map_transport_error)?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.map_err(map_transport_error)?;
        return Err(map_response_error(status, &body));
    }

    let events = Box::pin(sse::events(response.bytes_stream()));
    let state = CompletionState::new(events);

    Ok(stream::try_unfold(state, |mut state| async move {
        loop {
            if let Some(output) = state.pending.pop_front() {
                return Ok(Some((output, state)));
            }
            if state.completed {
                return Ok(None);
            }

            let event = state.events.next().await.ok_or_else(|| {
                ModelError::Protocol("stream ended before the [DONE] event".into())
            })?;
            let event = event.map_err(map_sse_error)?;
            state.consume(&event.data)?;
        }
    })
    .boxed())
}

struct CompletionState<S> {
    events: S,
    finish_reason: Option<String>,
    usage: Option<TokenUsage>,
    pending: VecDeque<StreamEvent>,
    completed: bool,
}

impl<S> CompletionState<S> {
    fn new(events: S) -> Self {
        Self {
            events,
            finish_reason: None,
            usage: None,
            pending: VecDeque::new(),
            completed: false,
        }
    }

    fn consume(&mut self, data: &str) -> Result<(), ModelError> {
        if data.trim() == "[DONE]" {
            self.completed = true;
            self.pending.push_back(StreamEvent::Completed {
                finish_reason: self.finish_reason.take(),
                usage: self.usage.take(),
            });
            return Ok(());
        }

        let chunk = serde_json::from_str::<ChatCompletionChunk>(data).map_err(|error| {
            if let Ok(response) = serde_json::from_str::<ErrorResponse>(data) {
                ModelError::Upstream {
                    status: None,
                    message: response.error.message,
                }
            } else {
                ModelError::Protocol(format!("invalid OpenAI chunk: {error}"))
            }
        })?;

        if let Some(usage) = chunk.usage {
            self.usage = Some(usage.into());
        }

        let Some(choice) = chunk.choices.into_iter().next() else {
            return Ok(());
        };

        if let Some(finish_reason) = choice.finish_reason {
            self.finish_reason = Some(finish_reason);
        }

        if let Some(content) = choice
            .delta
            .reasoning_content
            .filter(|content| !content.is_empty())
        {
            self.pending
                .push_back(StreamEvent::ReasoningDelta { content });
        }
        if let Some(content) = choice.delta.content.filter(|content| !content.is_empty()) {
            self.pending.push_back(StreamEvent::TextDelta { content });
        }
        for tool_call in choice.delta.tool_calls {
            let (name, arguments) = tool_call
                .function
                .map(|function| (function.name, function.arguments.unwrap_or_default()))
                .unwrap_or_default();
            self.pending.push_back(StreamEvent::ToolCallDelta {
                index: tool_call.index,
                id: tool_call.id,
                name,
                arguments,
            });
        }

        Ok(())
    }
}

impl From<Usage> for TokenUsage {
    fn from(usage: Usage) -> Self {
        Self {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            total_tokens: usage.total_tokens,
        }
    }
}

fn map_transport_error(error: reqwest::Error) -> ModelError {
    if error.is_timeout() {
        ModelError::Timeout
    } else {
        ModelError::Transport(error.to_string())
    }
}

fn map_sse_error(error: sse::Error<reqwest::Error>) -> ModelError {
    match error {
        sse::Error::InvalidData(message) => ModelError::Protocol(message),
        sse::Error::Transport(error) => map_transport_error(error),
    }
}

fn map_response_error(status: StatusCode, body: &str) -> ModelError {
    let message = serde_json::from_str::<ErrorResponse>(body)
        .map(|response| response.error.message)
        .unwrap_or_else(|_| status.canonical_reason().unwrap_or("request failed").into());

    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => ModelError::Authentication { message },
        StatusCode::TOO_MANY_REQUESTS => ModelError::RateLimited { message },
        StatusCode::REQUEST_TIMEOUT | StatusCode::GATEWAY_TIMEOUT => ModelError::Timeout,
        status if status.is_client_error() => ModelError::InvalidRequest {
            status: status.as_u16(),
            message,
        },
        status => ModelError::Upstream {
            status: Some(status.as_u16()),
            message,
        },
    }
}

#[cfg(test)]
mod tests;
