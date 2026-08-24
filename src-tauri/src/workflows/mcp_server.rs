use std::{fs, path::PathBuf};

use sqlx::SqlitePool;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;

use crate::{
    protocol::mcp_server::McpServer,
    storage::mcp_server::{self, CreateCommandParams, CreateParams},
    AppError, AppResult,
};

use super::package_import;

mod package;

#[cfg(test)]
use package::is_safe_relative_path;
use package::{prepare_package, PreparedPackage};

#[derive(Clone, Copy)]
enum ImportKind {
    Archive,
    Directory,
}

impl ImportKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Archive => "archive",
            Self::Directory => "directory",
        }
    }
}

pub async fn import_archive(app: AppHandle, pool: &SqlitePool) -> AppResult<Option<McpServer>> {
    let Some(source) = pick_archive(app.clone()).await? else {
        return Ok(None);
    };
    import(app, pool, source, ImportKind::Archive)
        .await
        .map(Some)
}

pub async fn import_directory(app: AppHandle, pool: &SqlitePool) -> AppResult<Option<McpServer>> {
    let Some(source) = pick_directory(app.clone()).await? else {
        return Ok(None);
    };
    import(app, pool, source, ImportKind::Directory)
        .await
        .map(Some)
}

async fn import(
    app: AppHandle,
    pool: &SqlitePool,
    source: PathBuf,
    import_kind: ImportKind,
) -> AppResult<McpServer> {
    let import_kind_label = import_kind.as_str();
    let id = Uuid::now_v7().to_string();
    let base = app.path().app_data_dir()?.join("mcp-servers");
    let staging = base.join(format!(".import-{id}"));
    let destination = base.join(&id);
    let prepared = tauri::async_runtime::spawn_blocking(move || {
        let result = (|| {
            fs::create_dir_all(&base)?;
            match import_kind {
                ImportKind::Archive => package_import::extract_zip(&source, &staging)?,
                ImportKind::Directory => package_import::copy_directory(&source, &staging)?,
            }
            let package = prepare_package(&staging)?;
            fs::rename(&staging, &destination)?;
            Ok::<_, AppError>((package, destination))
        })();
        if result.is_err() {
            let _ = fs::remove_dir_all(&staging);
        }
        result
    })
    .await
    .map_err(|error| AppError::Other(format!("MCP package import task failed: {error}")))??;

    let (package, destination) = prepared;
    let managed_path = destination.to_string_lossy().into_owned();
    let result = match package {
        PreparedPackage::Bundle {
            manifest,
            manifest_json,
        } => {
            mcp_server::create(
                pool,
                CreateParams {
                    id,
                    name: manifest.name,
                    display_name: manifest.display_name,
                    version: manifest.version,
                    description: manifest.description,
                    author_name: manifest.author.name,
                    server_type: manifest.server.server_type,
                    managed_path,
                    manifest_json,
                },
            )
            .await
        }
        PreparedPackage::Command {
            name,
            version,
            description,
            author_name,
            server_type,
            command,
            args,
            manifest_json,
        } => {
            mcp_server::create_command(
                pool,
                CreateCommandParams {
                    id,
                    display_name: Some(name.clone()),
                    name,
                    version,
                    description,
                    author_name,
                    server_type,
                    managed_path,
                    manifest_json,
                    command,
                    args,
                },
            )
            .await
        }
    };
    if result.is_err() {
        let _ = fs::remove_dir_all(destination);
    }
    let server = result?;
    tracing::info!(
        mcp_server_id = %server.id,
        mcp_server_name = %server.name,
        import_kind = import_kind_label,
        source_kind = server.source_kind.as_str(),
        server_type = server.server_type.as_str(),
        "MCP server package stored"
    );
    Ok(server)
}

pub async fn delete(app: AppHandle, pool: &SqlitePool, id: &str) -> AppResult<()> {
    let managed_path = mcp_server::managed_path(pool, id).await?.map(PathBuf::from);
    mcp_server::delete(pool, id).await?;
    let base = app.path().app_data_dir()?.join("mcp-servers");
    if let Some(managed_path) = managed_path {
        if managed_path.parent() == Some(base.as_path()) {
            if let Err(error) = fs::remove_dir_all(&managed_path) {
                tracing::warn!(?managed_path, %error, "failed to remove managed MCP directory");
            }
        }
    }
    tracing::info!(mcp_server_id = %id, "MCP server deleted");
    Ok(())
}

async fn pick_archive(app: AppHandle) -> AppResult<Option<PathBuf>> {
    tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .add_filter("MCP package", &["zip", "mcpb"])
            .blocking_pick_file()
            .map(|path| path.into_path())
    })
    .await
    .map_err(|error| AppError::Other(format!("MCP package dialog failed: {error}")))?
    .transpose()
    .map_err(|_| AppError::Validation("selected MCP package path is unsupported".into()))
}

async fn pick_directory(app: AppHandle) -> AppResult<Option<PathBuf>> {
    tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .blocking_pick_folder()
            .map(|path| path.into_path())
    })
    .await
    .map_err(|error| AppError::Other(format!("MCP directory dialog failed: {error}")))?
    .transpose()
    .map_err(|_| AppError::Validation("selected MCP directory path is unsupported".into()))
}

#[cfg(test)]
mod tests {
    use std::{
        fs::{self, File},
        io::Write,
        path::Path,
    };

    use sqlx::sqlite::SqlitePoolOptions;

    use crate::{
        protocol::mcp_server::{ServerType, SourceKind},
        storage::mcp_server::{self, CreateCommandParams},
    };

    use super::{is_safe_relative_path, package_import, prepare_package, PreparedPackage};

    const CHROME_PLUGIN_JSON: &str = r#"{
        "name": "chrome-devtools-mcp",
        "version": "1.7.0",
        "description": "Chrome DevTools for coding agents",
        "mcpServers": {
            "chrome-devtools": {
                "command": "npx",
                "args": ["chrome-devtools-mcp@1.7.0"]
            }
        }
    }"#;

    #[test]
    fn accepts_relative_package_entry_point() {
        assert!(is_safe_relative_path(Path::new("server/index.js")));
    }

    #[test]
    fn rejects_entry_point_that_escapes_package() {
        assert!(!is_safe_relative_path(Path::new("../server.js")));
        assert!(!is_safe_relative_path(Path::new("/tmp/server.js")));
    }

    #[test]
    fn parses_chrome_plugin_from_extracted_repository() {
        let root = std::env::temp_dir().join(format!("yukin-mcp-test-{}", uuid::Uuid::now_v7()));
        let repository = root.join("chrome-devtools-mcp-main");
        fs::create_dir_all(repository.join(".github/plugin")).unwrap();
        fs::write(
            repository.join(".github/plugin/plugin.json"),
            CHROME_PLUGIN_JSON,
        )
        .unwrap();
        fs::write(
            repository.join("package.json"),
            r#"{"author":"Google LLC"}"#,
        )
        .unwrap();

        let package = prepare_package(&root).unwrap();
        let PreparedPackage::Command {
            name,
            author_name,
            server_type,
            command,
            args,
            ..
        } = package
        else {
            panic!("expected command package");
        };

        assert_eq!(name, "chrome-devtools-mcp");
        assert_eq!(author_name, "Google LLC");
        assert_eq!(server_type, ServerType::Node);
        assert_eq!(command, "npx");
        assert_eq!(args, ["chrome-devtools-mcp@1.7.0"]);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn extracts_and_parses_chrome_repository_zip() {
        let root =
            std::env::temp_dir().join(format!("yukin-mcp-zip-test-{}", uuid::Uuid::now_v7()));
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("chrome-devtools-mcp.zip");
        let mut archive = zip::ZipWriter::new(File::create(&archive_path).unwrap());
        archive
            .start_file(
                "chrome-devtools-mcp-main/.github/plugin/plugin.json",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        archive.write_all(CHROME_PLUGIN_JSON.as_bytes()).unwrap();
        archive.finish().unwrap();

        let extracted = root.join("extracted");
        package_import::extract_zip(&archive_path, &extracted).unwrap();
        let package = prepare_package(&extracted).unwrap();

        let PreparedPackage::Command { command, args, .. } = package else {
            panic!("expected command package");
        };
        assert_eq!(command, "npx");
        assert_eq!(args, ["chrome-devtools-mcp@1.7.0"]);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn stores_imported_plugin_command() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

        let server = mcp_server::create_command(
            &pool,
            CreateCommandParams {
                id: uuid::Uuid::now_v7().to_string(),
                name: "chrome-devtools-mcp".into(),
                display_name: Some("chrome-devtools-mcp".into()),
                version: "1.7.0".into(),
                description: "Chrome DevTools for coding agents".into(),
                author_name: "Google LLC".into(),
                server_type: ServerType::Node,
                managed_path: "/managed/chrome-devtools-mcp".into(),
                manifest_json: CHROME_PLUGIN_JSON.into(),
                command: "npx".into(),
                args: vec!["chrome-devtools-mcp@1.7.0".into()],
            },
        )
        .await
        .unwrap();

        assert_eq!(server.source_kind, SourceKind::Command);
        assert_eq!(server.command.as_deref(), Some("npx"));
        assert_eq!(server.args, ["chrome-devtools-mcp@1.7.0"]);
    }
}
