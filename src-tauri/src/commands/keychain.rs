use crate::{AppError, AppResult};

#[tauri::command]
pub async fn key_set(_provider: String, _key: String) -> AppResult<()> {
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn key_exists(_provider: String) -> AppResult<bool> {
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn key_delete(_provider: String) -> AppResult<()> {
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn key_list_providers() -> AppResult<Vec<String>> {
    Err(AppError::Other("todo".into()))
}
