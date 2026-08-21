use tauri::{AppHandle, State};

use crate::{
    diagnostics::result::LogError,
    protocol::file::{Reference, ReleaseRequest},
    workflows::file,
    AppResult, AppState,
};

#[tauri::command]
pub async fn file_reference_select(
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<Option<Reference>> {
    file::select_text(app, state.selected_files().clone())
        .await
        .log_error("file_reference_select")
}

#[tauri::command]
pub fn file_reference_release(state: State<'_, AppState>, request: ReleaseRequest) {
    state.selected_files().release(&request.reference_id);
}
