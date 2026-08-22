use tauri::{AppHandle, State};

use crate::{
    diagnostics::result::LogError,
    protocol::file::{DirectoryEntryActionRequest, DirectoryReference, Reference, ReleaseRequest},
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
pub async fn directory_reference_select(
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<Option<DirectoryReference>> {
    file::select_directory(app, state.selected_directories().clone())
        .await
        .log_error("directory_reference_select")
}

#[tauri::command]
pub fn directory_reference_release(state: State<'_, AppState>, request: ReleaseRequest) {
    state.selected_directories().release(&request.reference_id);
}

#[tauri::command]
pub fn file_reference_release(state: State<'_, AppState>, request: ReleaseRequest) {
    state.selected_files().release(&request.reference_id);
}

#[tauri::command]
pub async fn directory_entry_open(
    state: State<'_, AppState>,
    request: DirectoryEntryActionRequest,
) -> AppResult<()> {
    file::open_directory_entry(state.selected_directories().clone(), request)
        .await
        .log_error("directory_entry_open")
}

#[tauri::command]
pub async fn directory_entry_reveal(
    state: State<'_, AppState>,
    request: DirectoryEntryActionRequest,
) -> AppResult<()> {
    file::reveal_directory_entry(state.selected_directories().clone(), request)
        .await
        .log_error("directory_entry_reveal")
}
