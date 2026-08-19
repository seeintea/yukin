use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::Duration,
};

use futures_util::StreamExt;
use reqwest::{Client, StatusCode};

use crate::agent::{
    Completion, CompletionRequest, Message, ModelError, ReasoningEffort, Role, StreamEvent,
    ThinkingMode, TokenUsage,
};

use super::{complete, map_response_error, stream_completion, CompletionState};

#[test]
fn sends_non_streaming_request_and_returns_thinking_completion() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("local test listener");
    let address = listener.local_addr().expect("listener address");
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().expect("test request");
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("read timeout");

        let request = read_http_request(&mut socket);
        let body = concat!(
            "{\"choices\":[{\"message\":{\"reasoning_content\":\"先思考\",",
            "\"content\":\"答案\"},\"finish_reason\":\"stop\"}],",
            "\"usage\":{\"prompt_tokens\":2,\"completion_tokens\":3,\"total_tokens\":5}}"
        );
        write!(
            socket,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
        .expect("test response");

        request
    });

    let client = Client::new();
    let mut completion_request = CompletionRequest::new(
        "deepseek-v4-pro".into(),
        vec![Message::text(Role::User, "问题".into())],
    );
    completion_request.thinking = Some(ThinkingMode::Enabled);
    completion_request.reasoning_effort = Some(ReasoningEffort::Max);
    let output = tauri::async_runtime::block_on(complete(
        &client,
        &format!("http://{address}/chat/completions"),
        "test-key",
        completion_request,
    ))
    .expect("OpenAI completion");
    let request = server.join().expect("test server");

    assert!(request.contains(r#""stream":false"#));
    assert!(request.contains(r#""thinking":{"type":"enabled"}"#));
    assert!(request.contains(r#""reasoning_effort":"max""#));
    assert_eq!(
        output,
        Completion {
            content: Some("答案".into()),
            reasoning_content: Some("先思考".into()),
            finish_reason: Some("stop".into()),
            usage: Some(TokenUsage {
                prompt_tokens: 2,
                completion_tokens: 3,
                total_tokens: 5,
            }),
        }
    );
}

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
            CompletionRequest::new(
                "deepseek-chat".into(),
                vec![Message::text(Role::User, "你好".into())],
            ),
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

    state
        .consume(
            r#"{"choices":[{"delta":{"reasoning_content":"先思考","content":"你好"},"finish_reason":null}],"usage":null}"#,
        )
        .expect("valid delta");
    assert_eq!(
        state.pending.pop_front(),
        Some(StreamEvent::ReasoningDelta {
            content: "先思考".into()
        })
    );
    assert_eq!(
        state.pending.pop_front(),
        Some(StreamEvent::TextDelta {
            content: "你好".into()
        })
    );

    state
        .consume(
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call-1","function":{"name":"current_time","arguments":"{}"}}]},"finish_reason":null}]}"#,
        )
        .expect("valid tool call delta");
    assert_eq!(
        state.pending.pop_front(),
        Some(StreamEvent::ToolCallDelta {
            index: 0,
            id: Some("call-1".into()),
            name: Some("current_time".into()),
            arguments: "{}".into(),
        })
    );

    state
        .consume(
            r#"{"choices":[{"delta":{"content":null},"finish_reason":"stop"}],"usage":{"prompt_tokens":3,"completion_tokens":2,"total_tokens":5}}"#,
        )
        .expect("valid finished chunk");
    assert_eq!(state.pending.pop_front(), None);

    state.consume("[DONE]").expect("valid done event");
    assert_eq!(
        state.pending.pop_front(),
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
