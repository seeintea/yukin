use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use serde::Deserialize;
use sqlx::SqlitePool;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;

use crate::{
    protocol::mcp_server::{McpServer, ServerType},
    storage::mcp_server::{self, CreateParams},
    AppError, AppResult,
};

use super::package_import;

#[derive(Deserialize)]
struct Manifest {
    manifest_version: String,
    name: String,
    display_name: Option<String>,
    version: String,
    description: String,
    author: Author,
    server: Server,
}

#[derive(Deserialize)]
struct Author {
    name: String,
}

#[derive(Deserialize)]
struct Server {
    #[serde(rename = "type")]
    server_type: ServerType,
    entry_point: String,
}

pub async fn import(app: AppHandle, pool: &SqlitePool) -> AppResult<Option<McpServer>> {
    let Some(source) = pick_bundle(app.clone()).await? else {
        return Ok(None);
    };
    let id = Uuid::now_v7().to_string();
    let base = app.path().app_data_dir()?.join("mcp-servers");
    let staging = base.join(format!(".import-{id}"));
    let destination = base.join(&id);
    let prepared = tauri::async_runtime::spawn_blocking(move || {
        let result = (|| {
            fs::create_dir_all(&base)?;
            package_import::extract_zip(&source, &staging)?;
            let manifest_path = staging.join("manifest.json");
            if !manifest_path.is_file() {
                return Err(AppError::Validation(
                    "MCPB must contain manifest.json at its root".into(),
                ));
            }
            let manifest_json = package_import::read_metadata(&manifest_path)?;
            let manifest: Manifest = serde_json::from_str(&manifest_json)
                .map_err(|error| AppError::Validation(format!("invalid MCPB manifest: {error}")))?;
            validate_manifest(&manifest, &staging)?;
            fs::rename(&staging, &destination)?;
            Ok::<_, AppError>((manifest, manifest_json, destination))
        })();
        if result.is_err() {
            let _ = fs::remove_dir_all(&staging);
        }
        result
    })
    .await
    .map_err(|error| AppError::Other(format!("MCPB import task failed: {error}")))??;

    let (manifest, manifest_json, destination) = prepared;
    let result = mcp_server::create(
        pool,
        CreateParams {
            id,
            name: manifest.name,
            display_name: manifest.display_name,
            version: manifest.version,
            description: manifest.description,
            author_name: manifest.author.name,
            server_type: manifest.server.server_type,
            managed_path: destination.to_string_lossy().into_owned(),
            manifest_json,
        },
    )
    .await;
    if result.is_err() {
        let _ = fs::remove_dir_all(destination);
    }
    result.map(Some)
}

pub async fn delete(app: AppHandle, pool: &SqlitePool, id: &str) -> AppResult<()> {
    let managed_path = PathBuf::from(mcp_server::managed_path(pool, id).await?);
    mcp_server::delete(pool, id).await?;
    let base = app.path().app_data_dir()?.join("mcp-servers");
    if managed_path.parent() == Some(base.as_path()) {
        if let Err(error) = fs::remove_dir_all(&managed_path) {
            tracing::warn!(?managed_path, %error, "failed to remove managed MCPB directory");
        }
    }
    Ok(())
}

fn validate_manifest(manifest: &Manifest, root: &Path) -> AppResult<()> {
    for (label, value) in [
        ("manifest_version", manifest.manifest_version.as_str()),
        ("name", manifest.name.as_str()),
        ("version", manifest.version.as_str()),
        ("description", manifest.description.as_str()),
        ("author.name", manifest.author.name.as_str()),
        ("server.entry_point", manifest.server.entry_point.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(AppError::Validation(format!(
                "MCPB manifest field {label} must not be empty"
            )));
        }
    }
    if manifest.name.len() > 256 {
        return Err(AppError::Validation("MCP server name is too long".into()));
    }
    let relative_entry_point = Path::new(&manifest.server.entry_point);
    if !is_safe_relative_path(relative_entry_point) {
        return Err(AppError::Validation(
            "MCPB server entry point must be a relative package path".into(),
        ));
    }
    let entry_point = root.join(relative_entry_point);
    if !entry_point.is_file() {
        return Err(AppError::Validation(
            "MCPB server entry point is missing or unsafe".into(),
        ));
    }
    Ok(())
}

fn is_safe_relative_path(path: &Path) -> bool {
    path.components()
        .all(|component| matches!(component, Component::Normal(_)))
}

async fn pick_bundle(app: AppHandle) -> AppResult<Option<PathBuf>> {
    tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .add_filter("MCP Bundle", &["mcpb"])
            .blocking_pick_file()
            .map(|path| path.into_path())
    })
    .await
    .map_err(|error| AppError::Other(format!("MCPB file dialog failed: {error}")))?
    .transpose()
    .map_err(|_| AppError::Validation("selected MCPB path is unsupported".into()))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::is_safe_relative_path;

    #[test]
    fn accepts_relative_package_entry_point() {
        assert!(is_safe_relative_path(Path::new("server/index.js")));
    }

    #[test]
    fn rejects_entry_point_that_escapes_package() {
        assert!(!is_safe_relative_path(Path::new("../server.js")));
        assert!(!is_safe_relative_path(Path::new("/tmp/server.js")));
    }
}
