#![allow(dead_code)]

use crate::{AppError, AppResult};
use std::path::{Path, PathBuf};

pub fn resolve_within(_workspace: &Path, _rel: &str) -> AppResult<PathBuf> {
    Err(AppError::Other("todo".into()))
}
