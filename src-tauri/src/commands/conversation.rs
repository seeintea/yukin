use tauri::State;

use crate::{
    protocol::conversation::{Conversation, FindRequest, Snapshot},
    storage::conversation,
    AppResult, AppState,
};

#[tauri::command]
pub async fn conversation_current(state: State<'_, AppState>) -> AppResult<Conversation> {
    conversation::current(state.db()).await
}

#[tauri::command]
pub async fn conversation_find(
    state: State<'_, AppState>,
    request: FindRequest,
) -> AppResult<Snapshot> {
    conversation::find(state.db(), &request.id).await
}

#[tauri::command]
pub async fn conversation_list(state: State<'_, AppState>) -> AppResult<Vec<Conversation>> {
    conversation::list(state.db()).await
}

#[tauri::command]
pub async fn conversation_create(state: State<'_, AppState>) -> AppResult<Conversation> {
    conversation::create(state.db()).await
}
