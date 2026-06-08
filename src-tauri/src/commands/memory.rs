use crate::{AppError, AppResult};

#[tauri::command]
pub async fn memory_read() -> AppResult<String> {
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn memory_append(text: String) -> AppResult<()> {
    Err(AppError::Other("todo".into()))
}
