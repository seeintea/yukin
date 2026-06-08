use crate::{AppError, AppResult};

#[tauri::command]
pub async fn memory_save(
    _name: String,
    _kind: String,
    _content: String,
    _description: Option<String>,
) -> AppResult<String> {
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn memory_recall(_query: String, _limit: Option<i64>) -> AppResult<Vec<String>> {
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn memory_list(_kind: Option<String>) -> AppResult<Vec<String>> {
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn memory_delete(_id: String) -> AppResult<()> {
    Err(AppError::Other("todo".into()))
}
