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
mod tests;
