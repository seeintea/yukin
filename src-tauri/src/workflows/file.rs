use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

use crate::{
    files::{SelectedDirectories, SelectedFiles},
    protocol::file::{DirectoryReference, Reference},
    AppError, AppResult,
};

pub(crate) async fn select_text(
    app: AppHandle,
    selected_files: SelectedFiles,
) -> AppResult<Option<Reference>> {
    let path = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .blocking_pick_file()
            .map(|path| path.into_path())
    })
    .await
    .map_err(|error| AppError::Other(format!("file dialog task failed: {error}")))?
    .transpose()
    .map_err(|_| AppError::Validation("selected file path is unsupported".into()))?;

    match path {
        Some(path) => selected_files
            .register(path)
            .await
            .map(Some)
            .map_err(Into::into),
        None => Ok(None),
    }
}

pub(crate) async fn select_directory(
    app: AppHandle,
    selected_directories: SelectedDirectories,
) -> AppResult<Option<DirectoryReference>> {
    let path = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .blocking_pick_folder()
            .map(|path| path.into_path())
    })
    .await
    .map_err(|error| AppError::Other(format!("directory dialog task failed: {error}")))?
    .transpose()
    .map_err(|_| AppError::Validation("selected directory path is unsupported".into()))?;

    match path {
        Some(path) => selected_directories
            .register(path)
            .await
            .map(Some)
            .map_err(Into::into),
        None => Ok(None),
    }
}
