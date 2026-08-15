use serde::{Deserialize, Serialize};

use crate::agent::Message;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub stream: bool,
    pub stream_options: StreamOptions,
}

impl ChatCompletionRequest {
    pub fn streaming(model: String, messages: Vec<Message>) -> Self {
        Self {
            model,
            messages,
            stream: true,
            stream_options: StreamOptions {
                include_usage: true,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(super) struct StreamOptions {
    pub include_usage: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(super) struct ChatCompletionChunk {
    pub choices: Vec<ChunkChoice>,
    #[serde(default)]
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(super) struct ChunkChoice {
    pub delta: Delta,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(super) struct Delta {
    pub content: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub(super) struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Debug, Deserialize)]
pub(super) struct ErrorResponse {
    pub error: ErrorBody,
}

#[derive(Debug, Deserialize)]
pub(super) struct ErrorBody {
    pub message: String,
}

#[cfg(test)]
mod tests {
    use crate::agent::{Message, Role};

    use super::{ChatCompletionChunk, ChatCompletionRequest, Delta, Usage};

    #[test]
    fn serializes_streaming_chat_completion_request() {
        let request = ChatCompletionRequest::streaming(
            "deepseek-chat".into(),
            vec![Message {
                role: Role::User,
                content: "Hello".into(),
            }],
        );

        let value = serde_json::to_value(request).expect("serializable OpenAI request");

        assert_eq!(
            value,
            serde_json::json!({
                "model": "deepseek-chat",
                "messages": [
                    { "role": "user", "content": "Hello" }
                ],
                "stream": true,
                "stream_options": { "include_usage": true }
            })
        );
    }

    #[test]
    fn deserializes_text_delta_chunk_and_ignores_unused_fields() {
        let chunk: ChatCompletionChunk = serde_json::from_value(serde_json::json!({
            "id": "chatcmpl-123",
            "object": "chat.completion.chunk",
            "created": 1_723_000_000,
            "model": "deepseek-chat",
            "choices": [{
                "index": 0,
                "delta": {
                    "role": "assistant",
                    "content": "你好"
                },
                "finish_reason": null,
                "logprobs": null
            }],
            "system_fingerprint": "fp_123"
        }))
        .expect("valid OpenAI text delta chunk");

        assert_eq!(chunk.choices.len(), 1);
        assert_eq!(
            chunk.choices[0].delta,
            Delta {
                content: Some("你好".into())
            }
        );
        assert_eq!(chunk.choices[0].finish_reason, None);
        assert_eq!(chunk.usage, None);
    }

    #[test]
    fn deserializes_completion_chunk_with_usage() {
        let chunk: ChatCompletionChunk = serde_json::from_value(serde_json::json!({
            "id": "chatcmpl-123",
            "object": "chat.completion.chunk",
            "created": 1_723_000_000,
            "model": "deepseek-chat",
            "choices": [],
            "usage": {
                "prompt_tokens": 12,
                "completion_tokens": 7,
                "total_tokens": 19,
                "prompt_cache_hit_tokens": 4
            }
        }))
        .expect("valid OpenAI usage chunk");

        assert!(chunk.choices.is_empty());
        assert_eq!(
            chunk.usage,
            Some(Usage {
                prompt_tokens: 12,
                completion_tokens: 7,
                total_tokens: 19,
            })
        );
    }
}
