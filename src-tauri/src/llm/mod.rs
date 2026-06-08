// TODO(Phase F): expand ChatMessage to Vec<ContentBlock> and LlmEvent
// to streaming variants (TextDelta, ToolCallStart/InputDelta/End, MessageStop).
// Current types are skeleton placeholders.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::AppResult;

pub mod anthropic;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub enum LlmEvent {
    Text(String),
    ToolCall {
        name: String,
        args: serde_json::Value,
    },
    Done,
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn chat(&self, message: Vec<ChatMessage>) -> AppResult<()>;
}