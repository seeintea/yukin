use crate::{AppError, AppResult};

#[tauri::command]
pub async fn chat_send(session_id: String, content: String) -> AppResult<String> {
    let _ = (session_id, content);
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn chat_abort(run_id: String) -> AppResult<()> {
    let _ = run_id;
    Err(AppError::Other("todo".into()))
}
