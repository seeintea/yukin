use tauri::State;

use crate::{
    protocol::model_provider::{
        CreateRequest, DeleteRequest, FindRequest, ModelProvider, UpdateRequest,
    },
    storage::model_provider,
    AppResult, AppState,
};

#[tauri::command]
pub async fn model_provider_create(
    state: State<'_, AppState>,
    request: CreateRequest,
) -> AppResult<ModelProvider> {
    model_provider::create(state.db(), request).await
}

#[tauri::command]
pub async fn model_provider_find(
    state: State<'_, AppState>,
    request: FindRequest,
) -> AppResult<ModelProvider> {
    model_provider::find(state.db(), &request.id).await
}

#[tauri::command]
pub async fn model_provider_list(state: State<'_, AppState>) -> AppResult<Vec<ModelProvider>> {
    model_provider::list(state.db()).await
}

#[tauri::command]
pub async fn model_provider_update(
    state: State<'_, AppState>,
    request: UpdateRequest,
) -> AppResult<ModelProvider> {
    model_provider::update(state.db(), request).await
}

#[tauri::command]
pub async fn model_provider_delete(
    state: State<'_, AppState>,
    request: DeleteRequest,
) -> AppResult<()> {
    model_provider::delete(state.db(), &request.id).await
}
