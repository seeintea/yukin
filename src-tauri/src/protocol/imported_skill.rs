use serde::{Deserialize, Serialize};

use super::common::RecordMetadata;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedSkill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source_kind: SourceKind,
    pub content_digest: String,
    pub enabled: bool,
    #[serde(flatten)]
    pub metadata: RecordMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Directory,
    Archive,
}

impl SourceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Directory => "directory",
            Self::Archive => "archive",
        }
    }
}

impl TryFrom<String> for SourceKind {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "directory" => Ok(Self::Directory),
            "archive" => Ok(Self::Archive),
            _ => Err(format!("unsupported skill source kind: {value}")),
        }
    }
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
