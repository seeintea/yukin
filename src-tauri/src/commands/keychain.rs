use crate::{AppError, AppResult};

#[tauri::command]
pub async fn keychain_get(service: String, account: String) -> AppResult<Option<String>> {
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn keychain_set(service: String, account: String, secret: String) -> AppResult<()> {
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn keychain_delete(service: String, account: String) -> AppResult<()> {
    Err(AppError::Other("todo".into()))
}
