use serde::{Deserialize, Serialize};

use crate::agent::{CompletionRequest, Message, ReasoningEffort, ThinkingMode};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<StreamOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<Thinking>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<ReasoningEffortValue>,
}

impl ChatCompletionRequest {
    pub fn streaming(request: CompletionRequest) -> Self {
        Self {
            model: request.model,
            messages: request.messages,
            stream: true,
            stream_options: Some(StreamOptions {
                include_usage: true,
            }),
            thinking: request.thinking.map(Into::into),
            reasoning_effort: request.reasoning_effort.map(Into::into),
        }
    }

    pub fn non_streaming(request: CompletionRequest) -> Self {
        Self {
            model: request.model,
            messages: request.messages,
            stream: false,
            stream_options: None,
            thinking: request.thinking.map(Into::into),
            reasoning_effort: request.reasoning_effort.map(Into::into),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(super) struct StreamOptions {
    pub include_usage: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(super) struct Thinking {
    #[serde(rename = "type")]
    pub kind: ThinkingModeValue,
}

impl From<ThinkingMode> for Thinking {
    fn from(mode: ThinkingMode) -> Self {
        Self {
            kind: match mode {
                ThinkingMode::Enabled => ThinkingModeValue::Enabled,
                ThinkingMode::Disabled => ThinkingModeValue::Disabled,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ThinkingModeValue {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ReasoningEffortValue {
    None,
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
    Max,
}

impl From<ReasoningEffort> for ReasoningEffortValue {
    fn from(effort: ReasoningEffort) -> Self {
        match effort {
            ReasoningEffort::None => Self::None,
            ReasoningEffort::Minimal => Self::Minimal,
            ReasoningEffort::Low => Self::Low,
            ReasoningEffort::Medium => Self::Medium,
            ReasoningEffort::High => Self::High,
            ReasoningEffort::XHigh => Self::XHigh,
            ReasoningEffort::Max => Self::Max,
        }
    }
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
    #[serde(default)]
    pub reasoning_content: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(super) struct ChatCompletionResponse {
    pub choices: Vec<ResponseChoice>,
    #[serde(default)]
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(super) struct ResponseChoice {
    pub message: ResponseMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(super) struct ResponseMessage {
    pub content: Option<String>,
    #[serde(default)]
    pub reasoning_content: Option<String>,
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
    use crate::agent::{CompletionRequest, Message, ReasoningEffort, Role, ThinkingMode};

    use super::{ChatCompletionChunk, ChatCompletionRequest, ChatCompletionResponse, Delta, Usage};

    #[test]
    fn serializes_streaming_chat_completion_request() {
        let mut request = CompletionRequest::new(
            "deepseek-v4-pro".into(),
            vec![Message {
                role: Role::User,
                content: "Hello".into(),
            }],
        );
        request.thinking = Some(ThinkingMode::Enabled);
        request.reasoning_effort = Some(ReasoningEffort::Max);
        let request = ChatCompletionRequest::streaming(request);

        let value = serde_json::to_value(request).expect("serializable OpenAI request");

        assert_eq!(
            value,
            serde_json::json!({
                "model": "deepseek-v4-pro",
                "messages": [
                    { "role": "user", "content": "Hello" }
                ],
                "stream": true,
                "stream_options": { "include_usage": true },
                "thinking": { "type": "enabled" },
                "reasoning_effort": "max"
            })
        );
    }

    #[test]
    fn serializes_non_streaming_request_without_stream_options() {
        let request = ChatCompletionRequest::non_streaming(CompletionRequest::new(
            "deepseek-v4-flash".into(),
            vec![Message {
                role: Role::User,
                content: "Hello".into(),
            }],
        ));

        let value = serde_json::to_value(request).expect("serializable OpenAI request");

        assert_eq!(
            value,
            serde_json::json!({
                "model": "deepseek-v4-flash",
                "messages": [{ "role": "user", "content": "Hello" }],
                "stream": false
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
                content: Some("你好".into()),
                reasoning_content: None,
            }
        );
        assert_eq!(chunk.choices[0].finish_reason, None);
        assert_eq!(chunk.usage, None);
    }

    #[test]
    fn deserializes_non_streaming_thinking_response() {
        let response: ChatCompletionResponse = serde_json::from_value(serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "reasoning_content": "先比较整数部分。",
                    "content": "9.8 更大。"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 8,
                "completion_tokens": 12,
                "total_tokens": 20
            }
        }))
        .expect("valid non-streaming response");

        let choice = &response.choices[0];
        assert_eq!(
            choice.message.reasoning_content.as_deref(),
            Some("先比较整数部分。")
        );
        assert_eq!(choice.message.content.as_deref(), Some("9.8 更大。"));
        assert_eq!(choice.finish_reason.as_deref(), Some("stop"));
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
