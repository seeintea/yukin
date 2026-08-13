mod commands;
mod error;
mod protocol;
mod state;
mod storage;

pub use error::{AppError, AppResult};
pub use state::AppState;
use tauri::Manager;

fn init_tracing() {
    use tracing_subscriber::EnvFilter;
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,yukin=debug")),
        )
        .init();
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_tracing();

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
            greet,
            commands::model_provider::model_provider_create,
            commands::model_provider::model_provider_get,
            commands::model_provider::model_provider_list,
            commands::model_provider::model_provider_update,
            commands::model_provider::model_provider_delete,
        ])
        .run(tauri::generate_context!())
        .expect("error while running yukin.");
}
