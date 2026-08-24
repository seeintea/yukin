pub mod agent;
mod commands;
mod diagnostics;
mod error;
mod files;
mod model_provider;
mod protocol;
mod security;
mod state;
mod storage;
mod workflows;

pub use error::{AppError, AppResult};
pub use state::AppState;
use std::sync::mpsc;
use tauri::Manager;

pub fn run_crash_monitor_if_requested() {
    diagnostics::crash::run_monitor_if_requested();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let (diagnostics_sender, diagnostics_receiver) = mpsc::sync_channel(1);
    let run_result = tauri::Builder::default()
        .setup(move |app| {
            let (log_dir, log_dir_error) = match app.path().app_log_dir() {
                Ok(log_dir) => (Some(log_dir), None),
                Err(error) => (None, Some(error)),
            };
            let diagnostics = diagnostics::init(log_dir.as_deref());
            let _ = diagnostics_sender.send(diagnostics);
            if let Some(error) = log_dir_error {
                tracing::error!(
                    %error,
                    "failed to resolve application log directory; using console logging only"
                );
            }

            let handle = app.handle().clone();
            let state = tauri::async_runtime::block_on(async move { AppState::new(&handle).await })
                .inspect_err(|error| {
                    tracing::error!(
                        error_code = error.code(),
                        error = %error,
                        "application state initialization failed"
                    );
                })?;

            app.manage(state);

            tracing::info!("yukin setup complete");
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::agent_run::agent_run_start,
            commands::agent_run::agent_run_snapshot,
            commands::agent_run::agent_run_cancel,
            commands::agent_run::tool_call_decide,
            commands::skill::agent_skill_list,
            commands::imported_skill::imported_skill_import_directory,
            commands::imported_skill::imported_skill_import_archive,
            commands::imported_skill::imported_skill_list,
            commands::imported_skill::imported_skill_set_enabled,
            commands::imported_skill::imported_skill_delete,
            commands::mcp_server::mcp_server_import,
            commands::mcp_server::mcp_server_list,
            commands::mcp_server::mcp_server_set_enabled,
            commands::mcp_server::mcp_server_delete,
            commands::conversation::conversation_current,
            commands::conversation::conversation_find,
            commands::conversation::conversation_list,
            commands::conversation::conversation_create,
            commands::conversation::conversation_message_list,
            commands::conversation::conversation_rename,
            commands::conversation::conversation_delete,
            commands::diagnostics::diagnostics_frontend_error_report,
            commands::file::file_reference_select,
            commands::file::file_reference_release,
            commands::file::directory_reference_select,
            commands::file::directory_reference_release,
            commands::file::directory_entry_open,
            commands::file::directory_entry_reveal,
            commands::model_provider::model_provider_preset_list,
            commands::model_provider::model_provider_create,
            commands::model_provider::model_provider_find,
            commands::model_provider::model_provider_list,
            commands::model_provider::model_provider_update,
            commands::model_provider::model_provider_credential_replace,
            commands::model_provider::model_provider_delete,
            commands::model_provider::model_provider_test_connection,
        ])
        .run(tauri::generate_context!());

    if let Err(error) = &run_result {
        tracing::error!(%error, "tauri runtime failed");
    }
    drop(diagnostics_receiver);
    if let Err(error) = run_result {
        eprintln!("error while running yukin: {error}");
    }
}
