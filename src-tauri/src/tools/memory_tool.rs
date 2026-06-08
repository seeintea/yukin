use super::Tool;
use crate::{AppError, AppResult};
use async_trait::async_trait;

pub struct MemoryTools;

#[async_trait]
impl Tool for MemoryTools {
    fn name(&self) -> &'static str {
        "memory"
    }
    async fn call(&self, _args: serde_json::Value) -> AppResult<serde_json::Value> {
        Err(AppError::Other("todo".into()))
    }
}
