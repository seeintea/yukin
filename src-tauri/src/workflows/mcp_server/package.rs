use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
};

use serde::Deserialize;

use crate::{protocol::mcp_server::ServerType, AppError, AppResult};

use super::super::package_import;

#[derive(Deserialize)]
pub(super) struct Manifest {
    pub(super) manifest_version: String,
    pub(super) name: String,
    pub(super) display_name: Option<String>,
    pub(super) version: String,
    pub(super) description: String,
    pub(super) author: Author,
    pub(super) server: Server,
}

#[derive(Deserialize)]
pub(super) struct Author {
    pub(super) name: String,
}

#[derive(Deserialize)]
pub(super) struct Server {
    #[serde(rename = "type")]
    pub(super) server_type: ServerType,
    pub(super) entry_point: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PluginManifest {
    pub(super) name: String,
    pub(super) version: String,
    #[serde(default)]
    pub(super) description: String,
    pub(super) author: Option<PackageAuthor>,
    #[serde(default)]
    pub(super) mcp_servers: BTreeMap<String, PluginServer>,
}

#[derive(Deserialize)]
pub(super) struct PluginServer {
    pub(super) command: String,
    #[serde(default)]
    pub(super) args: Vec<String>,
    #[serde(default)]
    pub(super) env: BTreeMap<String, serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum PackageAuthor {
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
pub(super) struct PackageJson {
    pub(super) author: Option<PackageAuthor>,
}

pub(super) enum PreparedPackage {
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

pub(super) fn prepare_package(staging: &Path) -> AppResult<PreparedPackage> {
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

pub(super) fn package_roots(staging: &Path) -> AppResult<Vec<PathBuf>> {
    let mut roots = vec![staging.to_path_buf()];
    let entries = fs::read_dir(staging)?.collect::<Result<Vec<_>, _>>()?;
    if entries.len() == 1 && entries[0].file_type()?.is_dir() {
        roots.push(entries[0].path());
    }
    Ok(roots)
}

pub(super) fn prepare_plugin(root: &Path, manifest_path: &Path) -> AppResult<PreparedPackage> {
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

pub(super) fn validate_plugin(plugin: &PluginManifest) -> AppResult<()> {
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

pub(super) fn package_author(root: &Path) -> AppResult<Option<String>> {
    let path = root.join("package.json");
    if !path.is_file() {
        return Ok(None);
    }
    let content = package_import::read_metadata(&path)?;
    let package: PackageJson = serde_json::from_str(&content)
        .map_err(|error| AppError::Validation(format!("invalid package.json: {error}")))?;
    Ok(package.author.map(PackageAuthor::into_name))
}

pub(super) fn command_server_type(command: &str) -> ServerType {
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

pub(super) fn validate_manifest(manifest: &Manifest, root: &Path) -> AppResult<()> {
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

pub(super) fn is_safe_relative_path(path: &Path) -> bool {
    path.components()
        .all(|component| matches!(component, Component::Normal(_)))
}
