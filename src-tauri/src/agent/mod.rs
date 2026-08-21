mod openai;
pub(crate) mod skills;
mod sse;
pub(crate) mod tools;

use futures_util::stream::BoxStream;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use crate::protocol::model_provider::ApiFormat;

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum ModelError {
    #[error("model provider credential is missing")]
    MissingCredential,
    #[error("unsupported model API format: {0}")]
    UnsupportedFormat(&'static str),
    #[error("model authentication failed: {message}")]
    Authentication { message: String },
    #[error("model request was rate limited: {message}")]
    RateLimited { message: String },
    #[error("invalid model request ({status}): {message}")]
    InvalidRequest { status: u16, message: String },
    #[error("model provider failed ({status:?}): {message}")]
    Upstream {
        status: Option<u16>,
        message: String,
    },
    #[error("model request timed out")]
    Timeout,
    #[error("model transport failed: {0}")]
    Transport(String),
    #[error("invalid model stream: {0}")]
    Protocol(String),
}

impl ModelError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::MissingCredential => "model_credential_missing",
            Self::UnsupportedFormat(_) => "model_format_unsupported",
            Self::Authentication { .. } => "model_authentication",
            Self::RateLimited { .. } => "model_rate_limited",
            Self::InvalidRequest { .. } => "model_invalid_request",
            Self::Upstream { .. } => "model_upstream",
            Self::Timeout => "model_timeout",
            Self::Transport(_) => "model_transport",
            Self::Protocol(_) => "model_protocol",
        }
    }
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum RuntimeError {
    #[error("agent reached the maximum model steps")]
    StepLimit,
    #[error("agent reached the maximum tool calls")]
    ToolCallLimit,
    #[error("skill is not registered: {0}")]
    SkillNotFound(String),
    #[error("skill {skill} requires unavailable tool: {tool}")]
    SkillToolUnavailable { skill: String, tool: String },
    #[error("tool is not allowed for this run: {0}")]
    ToolNotAllowed(String),
    #[error("tool is not registered: {0}")]
    ToolNotFound(String),
    #[error("invalid arguments for tool {name}: {message}")]
    InvalidToolArguments { name: String, message: String },
    #[error("tool call timed out: {0}")]
    ToolTimeout(String),
    #[error("tool approval expired: {0}")]
    ApprovalExpired(String),
    #[error("tool approval is missing or does not match arguments: {0}")]
    InvalidToolApproval(String),
    #[error("tool output exceeded the size limit: {0}")]
    ToolOutputLimit(String),
    #[error("repeated tool call detected: {0}")]
    RepeatedToolCall(String),
    #[error("tool execution failed for {name}: {message}")]
    ToolExecution { name: String, message: String },
    #[error("file tool failed: {0}")]
    File(#[from] crate::files::FileError),
}

impl RuntimeError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::StepLimit => "agent_step_limit",
            Self::ToolCallLimit => "agent_tool_call_limit",
            Self::SkillNotFound(_) => "skill_not_found",
            Self::SkillToolUnavailable { .. } => "skill_tool_unavailable",
            Self::ToolNotAllowed(_) => "tool_not_allowed",
            Self::ToolNotFound(_) => "tool_not_found",
            Self::InvalidToolArguments { .. } => "tool_invalid_arguments",
            Self::ToolTimeout(_) => "tool_timeout",
            Self::ApprovalExpired(_) => "tool_approval_expired",
            Self::InvalidToolApproval(_) => "tool_approval_invalid",
            Self::ToolOutputLimit(_) => "tool_output_limit",
            Self::RepeatedToolCall(_) => "tool_repeated_call",
            Self::ToolExecution { .. } => "tool_execution",
            Self::File(error) => error.code(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
}

impl Message {
    pub fn text(role: Role, content: String) -> Self {
        Self {
            role,
            content,
            tool_calls: Vec::new(),
            tool_call_id: None,
            reasoning_content: None,
        }
    }

    pub fn assistant_tool_calls(
        content: String,
        reasoning_content: Option<String>,
        tool_calls: Vec<ToolCall>,
    ) -> Self {
        Self {
            role: Role::Assistant,
            content,
            tool_calls,
            tool_call_id: None,
            reasoning_content,
        }
    }

    pub fn tool(tool_call_id: String, content: String) -> Self {
        Self {
            role: Role::Tool,
            content,
            tool_calls: Vec::new(),
            tool_call_id: Some(tool_call_id),
            reasoning_content: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: ToolCallType,
    pub function: ToolCallFunction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallType {
    Function,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThinkingMode {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasoningEffort {
    None,
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
    Max,
}

impl From<crate::protocol::model_provider::ReasoningEffort> for ReasoningEffort {
    fn from(effort: crate::protocol::model_provider::ReasoningEffort) -> Self {
        match effort {
            crate::protocol::model_provider::ReasoningEffort::None => Self::None,
            crate::protocol::model_provider::ReasoningEffort::Minimal => Self::Minimal,
            crate::protocol::model_provider::ReasoningEffort::Low => Self::Low,
            crate::protocol::model_provider::ReasoningEffort::Medium => Self::Medium,
            crate::protocol::model_provider::ReasoningEffort::High => Self::High,
            crate::protocol::model_provider::ReasoningEffort::XHigh => Self::XHigh,
            crate::protocol::model_provider::ReasoningEffort::Max => Self::Max,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub thinking: Option<ThinkingMode>,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub tools: Vec<ToolDefinition>,
}

impl CompletionRequest {
    pub fn new(model: String, messages: Vec<Message>) -> Self {
        Self {
            model,
            messages,
            thinking: None,
            reasoning_effort: None,
            tools: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    pub content: Option<String>,
    pub reasoning_content: Option<String>,
    pub finish_reason: Option<String>,
    pub usage: Option<TokenUsage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamEvent {
    ReasoningDelta {
        content: String,
    },
    TextDelta {
        content: String,
    },
    ToolCallDelta {
        index: usize,
        id: Option<String>,
        name: Option<String>,
        arguments: String,
    },
    Completed {
        finish_reason: Option<String>,
        usage: Option<TokenUsage>,
    },
}

pub type CompletionStream = BoxStream<'static, Result<StreamEvent, ModelError>>;

pub async fn stream_completion(
    client: &Client,
    api_format: ApiFormat,
    base_url: &str,
    api_key: &str,
    request: CompletionRequest,
) -> Result<CompletionStream, ModelError> {
    match api_format {
        ApiFormat::OpenAi => openai::stream_completion(client, base_url, api_key, request).await,
        ApiFormat::Anthropic => Err(ModelError::UnsupportedFormat(api_format.as_str())),
    }
}

pub async fn complete(
    client: &Client,
    api_format: ApiFormat,
    base_url: &str,
    api_key: &str,
    request: CompletionRequest,
) -> Result<Completion, ModelError> {
    match api_format {
        ApiFormat::OpenAi => openai::complete(client, base_url, api_key, request).await,
        ApiFormat::Anthropic => Err(ModelError::UnsupportedFormat(api_format.as_str())),
    }
}
