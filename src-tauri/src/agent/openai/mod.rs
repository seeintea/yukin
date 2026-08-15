mod protocol;

use futures_util::{stream, StreamExt};
use reqwest::{header::ACCEPT, Client, StatusCode};

use crate::agent::{sse, CompletionStream, Message, ModelError, StreamEvent, TokenUsage};

use protocol::{ChatCompletionChunk, ChatCompletionRequest, ErrorResponse, Usage};

pub(crate) async fn stream_completion(
    client: &Client,
    base_url: &str,
    api_key: &str,
    model: String,
    messages: Vec<Message>,
) -> Result<CompletionStream, ModelError> {
    let request = ChatCompletionRequest::streaming(model, messages);
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
        if state.completed {
            return Ok(None);
        }

        loop {
            let event = state.events.next().await.ok_or_else(|| {
                ModelError::Protocol("stream ended before the [DONE] event".into())
            })?;
            let event = event.map_err(map_sse_error)?;

            if let Some(output) = state.consume(&event.data)? {
                return Ok(Some((output, state)));
            }
        }
    })
    .boxed())
}

struct CompletionState<S> {
    events: S,
    finish_reason: Option<String>,
    usage: Option<TokenUsage>,
    completed: bool,
}

impl<S> CompletionState<S> {
    fn new(events: S) -> Self {
        Self {
            events,
            finish_reason: None,
            usage: None,
            completed: false,
        }
    }

    fn consume(&mut self, data: &str) -> Result<Option<StreamEvent>, ModelError> {
        if data.trim() == "[DONE]" {
            self.completed = true;
            return Ok(Some(StreamEvent::Completed {
                finish_reason: self.finish_reason.take(),
                usage: self.usage.take(),
            }));
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
            return Ok(None);
        };

        if let Some(finish_reason) = choice.finish_reason {
            self.finish_reason = Some(finish_reason);
        }

        Ok(choice
            .delta
            .content
            .filter(|content| !content.is_empty())
            .map(|content| StreamEvent::TextDelta { content }))
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
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
        time::Duration,
    };

    use futures_util::StreamExt;
    use reqwest::{Client, StatusCode};

    use crate::agent::{Message, ModelError, Role, StreamEvent, TokenUsage};

    use super::{map_response_error, stream_completion, CompletionState};

    #[test]
    fn sends_request_and_streams_openai_events() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("local test listener");
        let address = listener.local_addr().expect("listener address");
        let server = thread::spawn(move || {
            let (mut socket, _) = listener.accept().expect("test request");
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .expect("read timeout");

            let request = read_http_request(&mut socket);
            let body = concat!(
                "data: {\"choices\":[{\"delta\":{\"content\":\"你\"},\"finish_reason\":null}]}\n\n",
                "data: {\"choices\":[{\"delta\":{\"content\":null},\"finish_reason\":\"stop\"}],",
                "\"usage\":{\"prompt_tokens\":2,\"completion_tokens\":1,\"total_tokens\":3}}\n\n",
                "data: [DONE]\n\n"
            );
            write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .expect("test response");

            request
        });

        let client = Client::new();
        let output = tauri::async_runtime::block_on(async {
            let stream = stream_completion(
                &client,
                &format!("http://{address}/chat/completions"),
                "test-key",
                "deepseek-chat".into(),
                vec![Message {
                    role: Role::User,
                    content: "你好".into(),
                }],
            )
            .await
            .expect("OpenAI stream");

            stream.collect::<Vec<_>>().await
        });
        let request = server.join().expect("test server");

        assert!(request.starts_with("POST /chat/completions HTTP/1.1\r\n"));
        assert!(request
            .to_ascii_lowercase()
            .contains("authorization: bearer test-key\r\n"));
        assert!(request.contains(r#""model":"deepseek-chat""#));
        assert!(request.contains(r#""content":"你好""#));
        assert_eq!(
            output,
            vec![
                Ok(StreamEvent::TextDelta {
                    content: "你".into(),
                }),
                Ok(StreamEvent::Completed {
                    finish_reason: Some("stop".into()),
                    usage: Some(TokenUsage {
                        prompt_tokens: 2,
                        completion_tokens: 1,
                        total_tokens: 3,
                    }),
                }),
            ]
        );
    }

    #[test]
    fn maps_chunks_and_done_to_internal_events() {
        let mut state = CompletionState::new(());

        let delta = state
            .consume(
                r#"{"choices":[{"delta":{"content":"你好"},"finish_reason":null}],"usage":null}"#,
            )
            .expect("valid delta");
        assert_eq!(
            delta,
            Some(StreamEvent::TextDelta {
                content: "你好".into()
            })
        );

        let finished = state
            .consume(
                r#"{"choices":[{"delta":{"content":null},"finish_reason":"stop"}],"usage":{"prompt_tokens":3,"completion_tokens":2,"total_tokens":5}}"#,
            )
            .expect("valid finished chunk");
        assert_eq!(finished, None);

        let completed = state.consume("[DONE]").expect("valid done event");
        assert_eq!(
            completed,
            Some(StreamEvent::Completed {
                finish_reason: Some("stop".into()),
                usage: Some(TokenUsage {
                    prompt_tokens: 3,
                    completion_tokens: 2,
                    total_tokens: 5,
                })
            })
        );
    }

    #[test]
    fn maps_streamed_provider_error() {
        let mut state = CompletionState::new(());

        let error = state
            .consume(r#"{"error":{"message":"service unavailable","type":"server_error"}}"#)
            .expect_err("provider error");

        assert!(matches!(
            error,
            ModelError::Upstream {
                status: None,
                message
            } if message == "service unavailable"
        ));
    }

    #[test]
    fn maps_http_error_categories() {
        let authentication = map_response_error(
            StatusCode::UNAUTHORIZED,
            r#"{"error":{"message":"invalid key"}}"#,
        );
        assert!(matches!(
            authentication,
            ModelError::Authentication { message } if message == "invalid key"
        ));

        let rate_limited = map_response_error(
            StatusCode::TOO_MANY_REQUESTS,
            r#"{"error":{"message":"slow down"}}"#,
        );
        assert!(matches!(
            rate_limited,
            ModelError::RateLimited { message } if message == "slow down"
        ));

        let upstream = map_response_error(StatusCode::BAD_GATEWAY, "not json");
        assert!(matches!(
            upstream,
            ModelError::Upstream {
                status: Some(502),
                message
            } if message == "Bad Gateway"
        ));
    }

    fn read_http_request(socket: &mut impl Read) -> String {
        let mut request = Vec::new();
        let mut buffer = [0_u8; 1024];

        loop {
            let count = socket.read(&mut buffer).expect("request bytes");
            assert!(count > 0, "request ended before its body was complete");
            request.extend_from_slice(&buffer[..count]);

            let Some(header_end) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") else {
                continue;
            };
            let body_start = header_end + 4;
            let headers = String::from_utf8_lossy(&request[..header_end]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().expect("content length"))
                })
                .expect("content-length header");

            if request.len() >= body_start + content_length {
                return String::from_utf8(request).expect("UTF-8 test request");
            }
        }
    }
}
