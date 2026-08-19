use serde::{Deserialize, Serialize};

use crate::agent::{CompletionRequest, Message, ReasoningEffort, ThinkingMode, ToolDefinition};

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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<Tool>,
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
            tools: request.tools.into_iter().map(Into::into).collect(),
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
            tools: request.tools.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct Tool {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub function: FunctionDefinition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct FunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

impl From<ToolDefinition> for Tool {
    fn from(tool: ToolDefinition) -> Self {
        Self {
            kind: "function",
            function: FunctionDefinition {
                name: tool.name,
                description: tool.description,
                parameters: tool.input_schema,
            },
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
    #[serde(default)]
    pub tool_calls: Vec<ToolCallDelta>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(super) struct ToolCallDelta {
    pub index: usize,
    pub id: Option<String>,
    pub function: Option<FunctionCallDelta>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(super) struct FunctionCallDelta {
    pub name: Option<String>,
    pub arguments: Option<String>,
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
    use crate::agent::{
        CompletionRequest, Message, ReasoningEffort, Role, ThinkingMode, ToolCall,
        ToolCallFunction, ToolCallType, ToolDefinition,
    };

    use super::{ChatCompletionChunk, ChatCompletionRequest, ChatCompletionResponse, Delta, Usage};

    #[test]
    fn serializes_streaming_chat_completion_request() {
        let mut request = CompletionRequest::new(
            "deepseek-v4-pro".into(),
            vec![Message::text(Role::User, "Hello".into())],
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
            vec![Message::text(Role::User, "Hello".into())],
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
    fn serializes_multi_turn_messages_in_order() {
        let request = ChatCompletionRequest::streaming(CompletionRequest::new(
            "deepseek-v4-flash".into(),
            vec![
                Message::text(Role::System, "遵循选中的 Skill".into()),
                Message::text(Role::User, "我叫 Yukin".into()),
                Message::text(Role::Assistant, "记住了".into()),
                Message::text(Role::User, "我叫什么？".into()),
            ],
        ));

        let value = serde_json::to_value(request).expect("serializable OpenAI request");

        assert_eq!(
            value["messages"],
            serde_json::json!([
                { "role": "system", "content": "遵循选中的 Skill" },
                { "role": "user", "content": "我叫 Yukin" },
                { "role": "assistant", "content": "记住了" },
                { "role": "user", "content": "我叫什么？" }
            ])
        );
    }

    #[test]
    fn serializes_tool_definition_call_and_result() {
        let tool_call = ToolCall {
            id: "call-1".into(),
            kind: ToolCallType::Function,
            function: ToolCallFunction {
                name: "current_time".into(),
                arguments: r#"{"utcOffset":"+08:00"}"#.into(),
            },
        };
        let mut request = CompletionRequest::new(
            "deepseek-chat".into(),
            vec![
                Message::assistant_tool_calls("".into(), None, vec![tool_call]),
                Message::tool("call-1".into(), r#"{"dateTime":"now"}"#.into()),
            ],
        );
        request.tools.push(ToolDefinition {
            name: "current_time".into(),
            description: "Get the current time".into(),
            input_schema: serde_json::json!({ "type": "object" }),
        });

        let value = serde_json::to_value(ChatCompletionRequest::streaming(request))
            .expect("serializable tool request");

        assert_eq!(value["tools"][0]["type"], "function");
        assert_eq!(value["tools"][0]["function"]["name"], "current_time");
        assert_eq!(value["messages"][0]["tool_calls"][0]["id"], "call-1");
        assert_eq!(value["messages"][1]["role"], "tool");
        assert_eq!(value["messages"][1]["tool_call_id"], "call-1");
    }

    #[test]
    fn deserializes_streamed_tool_call_delta() {
        let chunk: ChatCompletionChunk = serde_json::from_value(serde_json::json!({
            "choices": [{
                "delta": {
                    "tool_calls": [{
                        "index": 0,
                        "id": "call-1",
                        "function": { "name": "current_time", "arguments": "{\\\"utcOffset\\\":" }
                    }]
                },
                "finish_reason": null
            }]
        }))
        .expect("valid tool call delta");

        let tool_call = &chunk.choices[0].delta.tool_calls[0];
        assert_eq!(tool_call.index, 0);
        assert_eq!(tool_call.id.as_deref(), Some("call-1"));
        assert_eq!(
            tool_call
                .function
                .as_ref()
                .and_then(|function| function.name.as_deref()),
            Some("current_time")
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
                tool_calls: Vec::new(),
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
