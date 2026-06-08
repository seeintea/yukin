use crate::{AppError, AppResult};

#[tauri::command]
pub async fn session_list() -> AppResult<Vec<String>> {
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn session_load(id: String) -> AppResult<String> {
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn session_create() -> AppResult<String> {
    Err(AppError::Other("todo".into()))
}
