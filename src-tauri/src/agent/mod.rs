mod openai;
mod sse;

use futures_util::stream::BoxStream;
use reqwest::Client;
use serde::Serialize;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
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
}

impl CompletionRequest {
    pub fn new(model: String, messages: Vec<Message>) -> Self {
        Self {
            model,
            messages,
            thinking: None,
            reasoning_effort: None,
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
