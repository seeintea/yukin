use crate::{AppError, AppResult};
use std::path::{Path, PathBuf};

pub fn resolve_within(workspace: &Path, rel: &str) -> AppResult<PathBuf> {
    Err(AppError::Other("todo".into()))
}
