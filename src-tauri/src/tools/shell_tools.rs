use super::Tool;
use crate::{AppError, AppResult};
use async_trait::async_trait;

pub struct ShellTools;

#[async_trait]
impl Tool for ShellTools {
    fn name(&self) -> &'static str {
        "shell"
    }
    async fn call(&self, _args: serde_json::Value) -> AppResult<serde_json::Value> {
        Err(AppError::Other("todo".into()))
    }
}
