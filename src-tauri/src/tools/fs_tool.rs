#![allow(dead_code)]

use super::Tool;
use crate::{AppError, AppResult};
use async_trait::async_trait;

pub struct FsTools;

#[async_trait]
impl Tool for FsTools {
    fn name(&self) -> &'static str {
        "fs"
    }
    async fn call(&self, _args: serde_json::Value) -> AppResult<serde_json::Value> {
        Err(AppError::Other("todo".into()))
    }
}
