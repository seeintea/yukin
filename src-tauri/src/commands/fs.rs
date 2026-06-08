use crate::{AppError, AppResult};

#[tauri::command]
pub async fn fs_read_text_file(path: String) -> AppResult<String> {
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn fs_write_text_file(path: String, content: String) -> AppResult<()> {
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn fs_glob(pattern: String) -> AppResult<Vec<String>> {
    Err(AppError::Other("todo".into()))
}
