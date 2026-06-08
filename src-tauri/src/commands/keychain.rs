use crate::{AppError, AppResult};

#[tauri::command]
pub async fn key_set(provider: String, key: String) -> AppResult<()> {
    let _ = (provider, key);
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn key_exists(provider: String) -> AppResult<bool> {
    let _ = provider;
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn key_delete(provider: String) -> AppResult<()> {
    let _ = provider;
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn key_list_providers() -> AppResult<Vec<String>> {
    Err(AppError::Other("todo".into()))
}
