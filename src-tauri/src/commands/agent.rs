use crate::{AppError, AppResult};

#[tauri::command]
pub async fn chat_send(_session_id: String, _content: String) -> AppResult<String> {
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn chat_abort(_run_id: String) -> AppResult<()> {
    Err(AppError::Other("todo".into()))
}
