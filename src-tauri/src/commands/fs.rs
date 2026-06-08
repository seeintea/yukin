use crate::{AppError, AppResult};

#[tauri::command]
pub async fn fs_read(_path: String) -> AppResult<String> {
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn fs_write(_path: String, _content: String, _create_dirs: Option<bool>) -> AppResult<()> {
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn fs_edit(
    _path: String,
    _search: String,
    _replace: String,
    _all: Option<bool>,
) -> AppResult<()> {
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn fs_list_dir(_path: String) -> AppResult<Vec<String>> {
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn fs_glob(_pattern: String) -> AppResult<Vec<String>> {
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn fs_exists(_path: String) -> AppResult<bool> {
    Err(AppError::Other("todo".into()))
}
