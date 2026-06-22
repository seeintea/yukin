use crate::db;
use crate::state::AppState;
use crate::{AppError, AppResult};
use tauri::State;
use tokio::task::spawn_blocking;

const KEYRING_SERVICE: &str = "xyz.yukin.agent";

#[tauri::command]
pub async fn key_set(provider: String, key: String, state: State<'_, AppState>) -> AppResult<()> {
    let provider_clone = provider.clone();
    spawn_blocking(move || -> AppResult<()> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, &provider_clone)?;
        entry.set_password(&key)?;
        Ok(())
    })
    .await??;

    db::keychain::upsert_has_key(&state.db, &provider, true).await
}

#[tauri::command]
pub async fn key_exists(provider: String) -> AppResult<bool> {
    let result = spawn_blocking(move || -> AppResult<bool> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, &provider)?;
        match entry.get_password() {
            Ok(_) => Ok(true),
            Err(keyring::Error::NoEntry) => Ok(false), // 决策 C4 (ii)
            Err(e) => Err(AppError::from(e)),
        }
    })
    .await??;
    Ok(result)
}

#[tauri::command]
pub async fn key_delete(provider: String, state: State<'_, AppState>) -> AppResult<()> {
    let provider_clone = provider.clone();
    spawn_blocking(move || -> AppResult<()> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, &provider_clone)?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()), // 已经不存在 = 幂等
            Err(e) => Err(AppError::from(e)),
        }
    })
    .await??;

    db::keychain::upsert_has_key(&state.db, &provider, false).await
}

#[tauri::command]
pub async fn key_list_providers(state: State<'_, AppState>) -> AppResult<Vec<String>> {
    db::keychain::list_providers_with_key(&state.db).await
}