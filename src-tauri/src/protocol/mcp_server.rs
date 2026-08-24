use serde::{Deserialize, Serialize};

use super::common::RecordMetadata;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServer {
    pub id: String,
    pub name: String,
    pub display_name: Option<String>,
    pub version: String,
    pub description: String,
    pub author_name: String,
    pub server_type: ServerType,
    pub source_kind: SourceKind,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub enabled: bool,
    pub declared_tools: Vec<DeclaredTool>,
    pub config_fields: Vec<ConfigField>,
    #[serde(flatten)]
    pub metadata: RecordMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Bundle,
    Command,
}

impl SourceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bundle => "bundle",
            Self::Command => "command",
        }
    }
}

impl TryFrom<String> for SourceKind {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "bundle" => Ok(Self::Bundle),
            "command" => Ok(Self::Command),
            _ => Err(format!("unsupported MCP source kind: {value}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServerType {
    Node,
    Python,
    Binary,
    Uv,
}

impl ServerType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Node => "node",
            Self::Python => "python",
            Self::Binary => "binary",
            Self::Uv => "uv",
        }
    }
}

impl TryFrom<String> for ServerType {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "node" => Ok(Self::Node),
            "python" => Ok(Self::Python),
            "binary" => Ok(Self::Binary),
            "uv" => Ok(Self::Uv),
            _ => Err(format!("unsupported MCP server type: {value}")),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeclaredTool {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigField {
    pub name: String,
    pub title: String,
    pub description: String,
    pub field_type: String,
    pub required: bool,
    pub sensitive: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetEnabledRequest {
    pub id: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteRequest {
    pub id: String,
}
