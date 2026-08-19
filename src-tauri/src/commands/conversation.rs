use tauri::State;

use crate::{protocol::conversation::Snapshot, storage::conversation, AppResult, AppState};

#[tauri::command]
pub async fn conversation_current(state: State<'_, AppState>) -> AppResult<Snapshot> {
    conversation::current(state.db()).await
}
