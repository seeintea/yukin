use std::path::PathBuf;

use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::{AppError, AppResult, AppState};

#[tauri::command]
pub async fn get_workspace(state: State<'_, AppState>) -> AppResult<Option<String>> {
    Ok(state
        .workspace
        .read()
        .await
        .as_ref()
        .map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
pub async fn select_workspace(app: AppHandle, state: State<'_, AppState>) -> AppResult<String> {
    let folder = tokio::task::spawn_blocking(move || app.dialog().file().blocking_pick_folder())
        .await?
        .ok_or(AppError::DialogCancelled)?;

    let path = folder
        .into_path()
        .map_err(|e| AppError::Other(format!("dialog path: {e}")))?
        .canonicalize()?;

    persist_workspace(&state, &path).await?;

    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn set_workspace(path: String, state: State<'_, AppState>) -> AppResult<String> {
    let path = PathBuf::from(&path).canonicalize()?;
    persist_workspace(&state, &path).await?;
    Ok(path.to_string_lossy().to_string())
}

/// 持久化 workspace 到 settings 表 + 更新 in-memory state。
/// select_workspace 和 set_workspace 共用。
async fn persist_workspace(state: &AppState, path: &PathBuf) -> AppResult<()> {
    let path_str = path.to_string_lossy().to_string();
    sqlx::query!(
        "INSERT INTO settings (key, value) VALUES ('workspace_path', ?1) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        path_str,
    )
    .execute(&state.db)
    .await?;
    *state.workspace.write().await = Some(path.clone());
    Ok(())
}
