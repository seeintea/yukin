use crate::{AppError, AppResult};

#[tauri::command]
pub async fn agent_run(prompt: String) -> AppResult<String> {
    Err(AppError::Other("todo".into()))
}

#[tauri::command]
pub async fn agent_cancel(run_id: String) -> AppResult<()> {
    Err(AppError::Other("todo".into()))
}
