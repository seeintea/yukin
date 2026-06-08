mod agent;
mod commands;
mod error;
mod llm;
mod path_safety;
mod state;
mod tools;

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_tracing();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_sql::Builder::default().build())
        .setup(|app| {
            let handle = app.handle().clone();
            let state =
                tauri::async_runtime::block_on(async move { AppState::new(&handle).await })?;
            app.manage(state);
            tracing::info!("yukin setup complete");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::workspace::get_workspace,
            commands::workspace::select_workspace,
            commands::workspace::set_workspace,
            commands::fs::fs_read,
            commands::fs::fs_write,
            commands::fs::fs_edit,
            commands::fs::fs_list_dir,
            commands::fs::fs_glob,
            commands::fs::fs_exists,
            commands::keychain::key_set,
            commands::keychain::key_exists,
            commands::keychain::key_delete,
            commands::keychain::key_list_providers,
            commands::memory::memory_save,
            commands::memory::memory_recall,
            commands::memory::memory_list,
            commands::memory::memory_delete,
            commands::session::session_create,
            commands::session::session_list,
            commands::session::session_update,
            commands::session::session_delete,
            commands::session::session_append_message,
            commands::session::session_load_messages,
            commands::agent::chat_send,
            commands::agent::chat_abort,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
