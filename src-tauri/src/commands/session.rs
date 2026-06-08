use crate::{AppError, AppResult};

#[tauri::command]
pub async fn session_create(_title: String) -> AppResult<String> {
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn session_list() -> AppResult<Vec<String>> {
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn session_update(_id: String, _title: Option<String>) -> AppResult<()> {
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn session_delete(_id: String) -> AppResult<()> {
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn session_append_message(
    _session_id: String,
    _role: String,
    _content: String,
) -> AppResult<String> {
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn session_load_messages(_session_id: String) -> AppResult<Vec<String>> {
    Err(AppError::Other("todo".into()))
}
