use std::{
    collections::BTreeMap,
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
    storage::mcp_server::{self, CreateCommandParams, CreateParams},
    AppError, AppResult,
};

use super::package_import;

#[derive(Clone, Copy)]
enum ImportKind {
    Archive,
    Directory,
}

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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluginManifest {
    name: String,
    version: String,
    #[serde(default)]
    description: String,
    author: Option<PackageAuthor>,
    #[serde(default)]
    mcp_servers: BTreeMap<String, PluginServer>,
}

#[derive(Deserialize)]
struct PluginServer {
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: BTreeMap<String, serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum PackageAuthor {
    Name(String),
    Details { name: String },
}

impl PackageAuthor {
    fn into_name(self) -> String {
        match self {
            Self::Name(name) | Self::Details { name } => name,
        }
    }
}

#[derive(Deserialize)]
struct PackageJson {
    author: Option<PackageAuthor>,
}

enum PreparedPackage {
    Bundle {
        manifest: Manifest,
        manifest_json: String,
    },
    Command {
        name: String,
        version: String,
        description: String,
        author_name: String,
        server_type: ServerType,
        command: String,
        args: Vec<String>,
        manifest_json: String,
    },
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
    result
}

fn prepare_package(staging: &Path) -> AppResult<PreparedPackage> {
    let roots = package_roots(staging)?;
    for root in &roots {
        let manifest_path = root.join("manifest.json");
        if manifest_path.is_file() {
            let manifest_json = package_import::read_metadata(&manifest_path)?;
            let manifest: Manifest = serde_json::from_str(&manifest_json)
                .map_err(|error| AppError::Validation(format!("invalid MCPB manifest: {error}")))?;
            validate_manifest(&manifest, root)?;
            return Ok(PreparedPackage::Bundle {
                manifest,
                manifest_json,
            });
        }
    }
    for root in roots {
        for relative_path in [
            ".github/plugin/plugin.json",
            ".codex-plugin/plugin.json",
            "plugin.json",
        ] {
            let manifest_path = root.join(relative_path);
            if manifest_path.is_file() {
                return prepare_plugin(&root, &manifest_path);
            }
        }
    }
    Err(AppError::Validation(
        "package must contain an MCPB manifest.json or a plugin.json with mcpServers".into(),
    ))
}

fn package_roots(staging: &Path) -> AppResult<Vec<PathBuf>> {
    let mut roots = vec![staging.to_path_buf()];
    let entries = fs::read_dir(staging)?.collect::<Result<Vec<_>, _>>()?;
    if entries.len() == 1 && entries[0].file_type()?.is_dir() {
        roots.push(entries[0].path());
    }
    Ok(roots)
}

fn prepare_plugin(root: &Path, manifest_path: &Path) -> AppResult<PreparedPackage> {
    let manifest_json = package_import::read_metadata(manifest_path)?;
    let plugin: PluginManifest = serde_json::from_str(&manifest_json)
        .map_err(|error| AppError::Validation(format!("invalid plugin manifest: {error}")))?;
    validate_plugin(&plugin)?;
    let mut servers = plugin.mcp_servers.into_iter();
    let (_, server) = servers.next().expect("validated one MCP server");
    let package_author = package_author(root)?;
    let author_name = plugin
        .author
        .map(PackageAuthor::into_name)
        .or(package_author)
        .unwrap_or_default();
    Ok(PreparedPackage::Command {
        name: plugin.name,
        version: plugin.version,
        description: plugin.description,
        author_name,
        server_type: command_server_type(&server.command),
        command: server.command,
        args: server.args,
        manifest_json,
    })
}

fn validate_plugin(plugin: &PluginManifest) -> AppResult<()> {
    for (label, value) in [
        ("name", plugin.name.as_str()),
        ("version", plugin.version.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(AppError::Validation(format!(
                "plugin manifest field {label} must not be empty"
            )));
        }
    }
    if plugin.mcp_servers.len() != 1 {
        return Err(AppError::Validation(
            "plugin package must declare exactly one MCP server".into(),
        ));
    }
    let server = plugin.mcp_servers.values().next().expect("one MCP server");
    if server.command.trim().is_empty() {
        return Err(AppError::Validation(
            "plugin MCP server command must not be empty".into(),
        ));
    }
    if !server.env.is_empty() {
        return Err(AppError::Validation(
            "plugin MCP environment variables are not supported yet".into(),
        ));
    }
    Ok(())
}

fn package_author(root: &Path) -> AppResult<Option<String>> {
    let path = root.join("package.json");
    if !path.is_file() {
        return Ok(None);
    }
    let content = package_import::read_metadata(&path)?;
    let package: PackageJson = serde_json::from_str(&content)
        .map_err(|error| AppError::Validation(format!("invalid package.json: {error}")))?;
    Ok(package.author.map(PackageAuthor::into_name))
}

fn command_server_type(command: &str) -> ServerType {
    let executable = Path::new(command)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(command);
    match executable {
        "node" | "npm" | "npx" => ServerType::Node,
        "python" | "python3" => ServerType::Python,
        "uv" | "uvx" => ServerType::Uv,
        _ => ServerType::Binary,
    }
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
