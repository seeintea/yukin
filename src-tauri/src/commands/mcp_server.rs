use tauri::{AppHandle, State};

use crate::{
    diagnostics::result::LogError,
    protocol::mcp_server::{DeleteRequest, McpServer, SetEnabledRequest},
    storage::mcp_server,
    workflows::mcp_server as mcp_server_workflow,
    AppResult, AppState,
};

#[tauri::command]
pub async fn mcp_server_import(
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<Option<McpServer>> {
    mcp_server_workflow::import(app, state.db())
        .await
        .log_error("mcp_server_import")
}

#[tauri::command]
pub async fn mcp_server_list(state: State<'_, AppState>) -> AppResult<Vec<McpServer>> {
    mcp_server::list(state.db())
        .await
        .log_error("mcp_server_list")
}

#[tauri::command]
pub async fn mcp_server_set_enabled(
    state: State<'_, AppState>,
    request: SetEnabledRequest,
) -> AppResult<McpServer> {
    mcp_server::set_enabled(state.db(), &request.id, request.enabled)
        .await
        .log_error("mcp_server_set_enabled")
}

#[tauri::command]
pub async fn mcp_server_delete(
    app: AppHandle,
    state: State<'_, AppState>,
    request: DeleteRequest,
) -> AppResult<()> {
    mcp_server_workflow::delete(app, state.db(), &request.id)
        .await
        .log_error("mcp_server_delete")
}
