use tauri::State;

use crate::{
    model_provider::catalog,
    protocol::model_provider::{
        CreateRequest, DeleteRequest, FindRequest, ModelProvider, ModelProviderPreset,
        ReplaceCredentialRequest, TestConnectionRequest, TestConnectionResponse, UpdateRequest,
    },
    storage::model_provider::{self, UpdateParams},
    workflows::model_provider as model_provider_workflow,
    AppResult, AppState,
};

#[tauri::command]
pub fn model_provider_preset_list() -> Vec<ModelProviderPreset> {
    catalog::model_provider_presets()
}

#[tauri::command]
pub async fn model_provider_create(
    state: State<'_, AppState>,
    request: CreateRequest,
) -> AppResult<ModelProvider> {
    model_provider_workflow::create(state.db(), request).await
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
    model_provider::update(
        state.db(),
        UpdateParams {
            id: request.id,
            api_format: request.api_format,
            base_url: request.base_url,
            provider_alias: request.provider_alias,
        },
    )
    .await
}

#[tauri::command]
pub async fn model_provider_credential_replace(
    state: State<'_, AppState>,
    request: ReplaceCredentialRequest,
) -> AppResult<()> {
    model_provider_workflow::replace_credential(state.db(), &request.id, &request.api_key).await
}

#[tauri::command]
pub async fn model_provider_delete(
    state: State<'_, AppState>,
    request: DeleteRequest,
) -> AppResult<()> {
    model_provider_workflow::delete(state.db(), &request.id).await
}

#[tauri::command]
pub async fn model_provider_test_connection(
    state: State<'_, AppState>,
    request: TestConnectionRequest,
) -> AppResult<TestConnectionResponse> {
    model_provider_workflow::test_connection(state.db(), state.http(), request).await
}
