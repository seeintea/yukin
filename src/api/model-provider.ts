import { invoke } from "@tauri-apps/api/core";

import type {
  CreateRequest,
  DeleteRequest,
  FindRequest,
  ModelProvider,
  ModelProviderPreset,
  ReplaceCredentialRequest,
  TestConnectionRequest,
  TestConnectionResponse,
  UpdateRequest,
} from "#/protocol/model-provider";

export function modelProviderPresetList(): Promise<ModelProviderPreset[]> {
  return invoke<ModelProviderPreset[]>("model_provider_preset_list");
}

export function modelProviderCreate(request: CreateRequest): Promise<ModelProvider> {
  return invoke<ModelProvider>("model_provider_create", { request });
}

export function modelProviderFind(request: FindRequest): Promise<ModelProvider> {
  return invoke<ModelProvider>("model_provider_find", { request });
}

export function modelProviderList(): Promise<ModelProvider[]> {
  return invoke<ModelProvider[]>("model_provider_list");
}

export function modelProviderUpdate(request: UpdateRequest): Promise<ModelProvider> {
  return invoke<ModelProvider>("model_provider_update", { request });
}

export async function modelProviderCredentialReplace(
  request: ReplaceCredentialRequest,
): Promise<void> {
  await invoke("model_provider_credential_replace", { request });
}

export async function modelProviderDelete(request: DeleteRequest): Promise<void> {
  await invoke("model_provider_delete", { request });
}

export function modelProviderTestConnection(
  request: TestConnectionRequest,
): Promise<TestConnectionResponse> {
  return invoke("model_provider_test_connection", { request });
}
