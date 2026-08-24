use std::collections::BTreeMap;

use serde::Deserialize;
use sqlx::{FromRow, SqlitePool};

use crate::{
    protocol::{
        common::RecordMetadata,
        mcp_server::{ConfigField, DeclaredTool, McpServer, ServerType},
    },
    AppError, AppResult,
};

pub(crate) struct CreateParams {
    pub id: String,
    pub name: String,
    pub display_name: Option<String>,
    pub version: String,
    pub description: String,
    pub author_name: String,
    pub server_type: ServerType,
    pub managed_path: String,
    pub manifest_json: String,
}

#[derive(FromRow)]
struct McpServerRecord {
    id: String,
    name: String,
    display_name: Option<String>,
    version: String,
    description: String,
    author_name: String,
    server_type: String,
    manifest_json: String,
    enabled: bool,
    created_at: String,
    updated_at: String,
}

#[derive(Default, Deserialize)]
struct ManifestDetails {
    #[serde(default)]
    tools: Vec<DeclaredTool>,
    #[serde(default)]
    user_config: BTreeMap<String, ManifestConfigField>,
}

#[derive(Deserialize)]
struct ManifestConfigField {
    #[serde(rename = "type")]
    field_type: String,
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    required: bool,
    #[serde(default)]
    sensitive: bool,
}

impl TryFrom<McpServerRecord> for McpServer {
    type Error = AppError;

    fn try_from(record: McpServerRecord) -> Result<Self, Self::Error> {
        let details: ManifestDetails = serde_json::from_str(&record.manifest_json)
            .map_err(|error| AppError::Other(format!("stored MCP manifest is invalid: {error}")))?;
        let config_fields = details
            .user_config
            .into_iter()
            .map(|(name, field)| ConfigField {
                name,
                title: field.title,
                description: field.description,
                field_type: field.field_type,
                required: field.required,
                sensitive: field.sensitive,
            })
            .collect();
        Ok(Self {
            id: record.id,
            name: record.name,
            display_name: record.display_name,
            version: record.version,
            description: record.description,
            author_name: record.author_name,
            server_type: ServerType::try_from(record.server_type).map_err(AppError::Other)?,
            enabled: record.enabled,
            declared_tools: details.tools,
            config_fields,
            metadata: RecordMetadata {
                created_at: record.created_at,
                updated_at: record.updated_at,
            },
        })
    }
}

pub async fn create(pool: &SqlitePool, params: CreateParams) -> AppResult<McpServer> {
    let record = sqlx::query_as::<_, McpServerRecord>(
        "INSERT INTO mcp_servers (id, name, display_name, version, description, author_name, server_type, managed_path, manifest_json) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id, name, display_name, version, description, author_name, server_type, manifest_json, enabled, created_at, updated_at",
    )
        .bind(params.id)
        .bind(params.name)
        .bind(params.display_name)
        .bind(params.version)
        .bind(params.description)
        .bind(params.author_name)
        .bind(params.server_type.as_str())
        .bind(params.managed_path)
        .bind(params.manifest_json)
        .fetch_one(pool)
        .await?;
    record.try_into()
}

pub async fn list(pool: &SqlitePool) -> AppResult<Vec<McpServer>> {
    sqlx::query_as::<_, McpServerRecord>(
        "SELECT id, name, display_name, version, description, author_name, server_type, manifest_json, enabled, created_at, updated_at FROM mcp_servers WHERE deleted_at IS NULL ORDER BY created_at DESC, id DESC",
    )
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(TryInto::try_into)
        .collect()
}

pub async fn set_enabled(pool: &SqlitePool, id: &str, enabled: bool) -> AppResult<McpServer> {
    let record = sqlx::query_as::<_, McpServerRecord>(
        "UPDATE mcp_servers SET enabled = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ? AND deleted_at IS NULL RETURNING id, name, display_name, version, description, author_name, server_type, manifest_json, enabled, created_at, updated_at",
    )
        .bind(enabled)
        .bind(id)
        .fetch_one(pool)
        .await?;
    record.try_into()
}

pub async fn managed_path(pool: &SqlitePool, id: &str) -> AppResult<String> {
    Ok(sqlx::query_scalar::<_, String>(
        "SELECT managed_path FROM mcp_servers WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_one(pool)
    .await?)
}

pub async fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE mcp_servers SET deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}
