use tauri::State;

use crate::{
    diagnostics::result::LogError,
    protocol::conversation::{
        Conversation, DeleteRequest, FindRequest, Message, RenameRequest, Snapshot,
    },
    storage::conversation,
    workflows::conversation as conversation_workflow,
    AppResult, AppState,
};

#[tauri::command]
pub async fn conversation_current(state: State<'_, AppState>) -> AppResult<Conversation> {
    conversation::current(state.db())
        .await
        .log_error("conversation_current")
}

#[tauri::command]
pub async fn conversation_find(
    state: State<'_, AppState>,
    request: FindRequest,
) -> AppResult<Snapshot> {
    conversation::find(state.db(), &request.id)
        .await
        .log_error("conversation_find")
}

#[tauri::command]
pub async fn conversation_list(state: State<'_, AppState>) -> AppResult<Vec<Conversation>> {
    conversation::list(state.db())
        .await
        .log_error("conversation_list")
}

#[tauri::command]
pub async fn conversation_create(state: State<'_, AppState>) -> AppResult<Conversation> {
    conversation::create(state.db())
        .await
        .log_error("conversation_create")
}

#[tauri::command]
pub async fn conversation_message_list(
    state: State<'_, AppState>,
    request: FindRequest,
) -> AppResult<Vec<Message>> {
    conversation::list_messages(state.db(), &request.id)
        .await
        .log_error("conversation_message_list")
}

#[tauri::command]
pub async fn conversation_rename(
    state: State<'_, AppState>,
    request: RenameRequest,
) -> AppResult<Conversation> {
    conversation_workflow::rename(state.db(), request)
        .await
        .log_error("conversation_rename")
}

#[tauri::command]
pub async fn conversation_delete(
    state: State<'_, AppState>,
    request: DeleteRequest,
) -> AppResult<()> {
    conversation_workflow::delete(state.db(), &request.id)
        .await
        .log_error("conversation_delete")
}
