use crate::{AppError, AppResult};

#[tauri::command]
pub async fn session_create(title: String) -> AppResult<String> {
    let _ = title;
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn session_list() -> AppResult<Vec<String>> {
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn session_update(id: String, title: Option<String>) -> AppResult<()> {
    let _ = (id, title);
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn session_delete(id: String) -> AppResult<()> {
    let _ = id;
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn session_append_message(
    session_id: String,
    role: String,
    content: String,
) -> AppResult<String> {
    let _ = (session_id, role, content);
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn session_load_messages(session_id: String) -> AppResult<Vec<String>> {
    let _ = session_id;
    Err(AppError::Other("todo".into()))
}
