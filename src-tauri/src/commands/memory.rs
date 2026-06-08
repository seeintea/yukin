use crate::{AppError, AppResult};

#[tauri::command]
pub async fn memory_save(
    name: String,
    kind: String,
    content: String,
    description: Option<String>,
) -> AppResult<String> {
    let _ = (name, kind, content, description);
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn memory_recall(query: String, limit: Option<i64>) -> AppResult<Vec<String>> {
    let _ = (query, limit);
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn memory_list(kind: Option<String>) -> AppResult<Vec<String>> {
    let _ = kind;
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn memory_delete(id: String) -> AppResult<()> {
    let _ = id;
    Err(AppError::Other("todo".into()))
}
