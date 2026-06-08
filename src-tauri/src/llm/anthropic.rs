#![allow(dead_code)]

use super::{ChatMessage, LlmProvider};
use crate::{AppError, AppResult};
use async_trait::async_trait;

pub struct AnthropicProvider {}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    async fn chat(&self, _message: Vec<ChatMessage>) -> AppResult<()> {
        Err(AppError::Other("todo".into()))
    }
}
