use crate::{AppError, AppResult};

#[tauri::command]
pub async fn get_workspace() -> AppResult<Option<String>> {
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn select_workspace() -> AppResult<String> {
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn set_workspace(path: String) -> AppResult<String> {
    Err(AppError::Other("todo".into()))
}
