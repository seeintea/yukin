mod openai;
mod sse;

use futures_util::stream::BoxStream;
use reqwest::Client;
use serde::Serialize;

use crate::protocol::model_provider::ApiFormat;

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
pub(crate) struct Message {
    pub role: Role,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Role {
    User,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StreamEvent {
    TextDelta {
        content: String,
    },
    Completed {
        finish_reason: Option<String>,
        usage: Option<TokenUsage>,
    },
}

pub(crate) type CompletionStream = BoxStream<'static, Result<StreamEvent, ModelError>>;

pub(crate) async fn stream_completion(
    client: &Client,
    api_format: ApiFormat,
    base_url: &str,
    api_key: &str,
    model: String,
    messages: Vec<Message>,
) -> Result<CompletionStream, ModelError> {
    match api_format {
        ApiFormat::OpenAi => {
            openai::stream_completion(client, base_url, api_key, model, messages).await
        }
        ApiFormat::Anthropic => Err(ModelError::UnsupportedFormat(api_format.as_str())),
    }
}
