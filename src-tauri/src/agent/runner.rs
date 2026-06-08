#![allow(dead_code)]

use crate::{AppError, AppResult};

pub async fn run_agent(_prompt: String) -> AppResult<String> {
    Err(AppError::Other("todo".into()))
}
