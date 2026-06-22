use crate::db::session::{
    self, MessageAppendInput, MessageRow, SessionCreateInput, SessionRow, SessionUpdate,
};
use crate::state::AppState;
use crate::AppResult;
use tauri::State;

#[tauri::command]
pub async fn session_create(
    input: SessionCreateInput,
    state: State<'_, AppState>,
) -> AppResult<SessionRow> {
    session::create(&state.db, input).await
}

#[tauri::command]
pub async fn session_list(state: State<'_, AppState>) -> AppResult<Vec<SessionRow>> {
    session::list(&state.db).await
}

#[tauri::command]
pub async fn session_update(
    id: String,
    patch: SessionUpdate,
    state: State<'_, AppState>,
) -> AppResult<SessionRow> {
    session::update(&state.db, &id, patch).await
}

#[tauri::command]
pub async fn session_delete(id: String, state: State<'_, AppState>) -> AppResult<()> {
    session::delete(&state.db, &id).await
}

#[tauri::command]
pub async fn session_append_message(
    input: MessageAppendInput,
    state: State<'_, AppState>,
) -> AppResult<MessageRow> {
    session::append_message(&state.db, input).await
}

#[tauri::command]
pub async fn session_load_messages(
    session_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<MessageRow>> {
    session::load_messages(&state.db, &session_id).await
}
