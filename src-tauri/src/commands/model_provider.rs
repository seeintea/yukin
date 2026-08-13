use tauri::State;

use crate::{
    protocol::model_provider::{
        CreateModelProviderInput, DeleteModelProviderInput, GetModelProviderInput,
        ListModelProvidersInput, ModelProvider, UpdateModelProviderInput,
    },
    storage::model_provider,
    AppResult, AppState,
};

#[tauri::command]
pub async fn model_provider_create(
    state: State<'_, AppState>,
    input: CreateModelProviderInput,
) -> AppResult<ModelProvider> {
    model_provider::create(state.db(), input).await
}

#[tauri::command]
pub async fn model_provider_get(
    state: State<'_, AppState>,
    input: GetModelProviderInput,
) -> AppResult<ModelProvider> {
    model_provider::get(state.db(), &input.id).await
}

#[tauri::command]
pub async fn model_provider_list(
    state: State<'_, AppState>,
    input: ListModelProvidersInput,
) -> AppResult<Vec<ModelProvider>> {
    model_provider::list(state.db(), input.include_deleted).await
}

#[tauri::command]
pub async fn model_provider_update(
    state: State<'_, AppState>,
    input: UpdateModelProviderInput,
) -> AppResult<ModelProvider> {
    model_provider::update(state.db(), input).await
}

#[tauri::command]
pub async fn model_provider_delete(
    state: State<'_, AppState>,
    input: DeleteModelProviderInput,
) -> AppResult<()> {
    model_provider::delete(state.db(), &input.id).await
}
