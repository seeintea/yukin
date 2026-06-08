use crate::{AppError, AppResult};

#[tauri::command]
pub async fn fs_read(path: String) -> AppResult<String> {
    let _ = path;
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn fs_write(path: String, content: String, create_dirs: Option<bool>) -> AppResult<()> {
    let _ = (path, content, create_dirs);
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn fs_edit(
    path: String,
    search: String,
    replace: String,
    all: Option<bool>,
) -> AppResult<()> {
    let _ = (path, search, replace, all);
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn fs_list_dir(path: String) -> AppResult<Vec<String>> {
    let _ = path;
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn fs_glob(pattern: String) -> AppResult<Vec<String>> {
    let _ = pattern;
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn fs_exists(path: String) -> AppResult<bool> {
    let _ = path;
    Err(AppError::Other("todo".into()))
}
