mod commands;
mod diagnostics;
mod error;
mod protocol;
mod security;
mod state;
mod storage;
mod workflows;

pub use error::{AppError, AppResult};
pub use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    diagnostics::tracing::init();

    tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle().clone();
            let state =
                tauri::async_runtime::block_on(async move { AppState::new(&handle).await })?;

            app.manage(state);

            tracing::info!("yukin setup complete");
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::model_provider::model_provider_create,
            commands::model_provider::model_provider_find,
            commands::model_provider::model_provider_list,
            commands::model_provider::model_provider_update,
            commands::model_provider::model_provider_credential_replace,
            commands::model_provider::model_provider_delete,
        ])
        .run(tauri::generate_context!())
        .expect("error while running yukin.");
}
