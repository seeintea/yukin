use tauri::{AppHandle, State};

use crate::{
    diagnostics::result::LogError,
    protocol::imported_skill::{DeleteRequest, ImportedSkill, SetEnabledRequest},
    storage::imported_skill,
    workflows::imported_skill as imported_skill_workflow,
    AppResult, AppState,
};

#[tauri::command]
pub async fn imported_skill_import_directory(
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<Option<ImportedSkill>> {
    imported_skill_workflow::import_directory(app, state.db())
        .await
        .log_error("imported_skill_import_directory")
}

#[tauri::command]
pub async fn imported_skill_import_archive(
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<Option<ImportedSkill>> {
    imported_skill_workflow::import_archive(app, state.db())
        .await
        .log_error("imported_skill_import_archive")
}

#[tauri::command]
pub async fn imported_skill_list(state: State<'_, AppState>) -> AppResult<Vec<ImportedSkill>> {
    imported_skill::list(state.db())
        .await
        .log_error("imported_skill_list")
}

#[tauri::command]
pub async fn imported_skill_set_enabled(
    state: State<'_, AppState>,
    request: SetEnabledRequest,
) -> AppResult<ImportedSkill> {
    imported_skill::set_enabled(state.db(), &request.id, request.enabled)
        .await
        .log_error("imported_skill_set_enabled")
}

#[tauri::command]
pub async fn imported_skill_delete(
    app: AppHandle,
    state: State<'_, AppState>,
    request: DeleteRequest,
) -> AppResult<()> {
    imported_skill_workflow::delete(app, state.db(), &request.id)
        .await
        .log_error("imported_skill_delete")
}
