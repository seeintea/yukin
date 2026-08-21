use std::{
    collections::{HashMap, HashSet},
    path::{Component, Path, PathBuf},
};

use chrono::{FixedOffset, SecondsFormat, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

use crate::files::AuthorizedFile;
use crate::protocol::agent_run::{ToolApprovalPolicy, ToolRiskLevel};

use super::{RuntimeError, ToolDefinition};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RiskLevel {
    ReadOnly,
    Write,
}

impl RiskLevel {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read_only",
            Self::Write => "write",
        }
    }
}

impl From<RiskLevel> for ToolRiskLevel {
    fn from(value: RiskLevel) -> Self {
        match value {
            RiskLevel::ReadOnly => Self::ReadOnly,
            RiskLevel::Write => Self::Write,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ApprovalPolicy {
    Never,
    Always,
}

impl ApprovalPolicy {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Never => "never",
            Self::Always => "always",
        }
    }
}

impl From<ApprovalPolicy> for ToolApprovalPolicy {
    fn from(value: ApprovalPolicy) -> Self {
        match value {
            ApprovalPolicy::Never => Self::Never,
            ApprovalPolicy::Always => Self::Always,
        }
    }
}

pub(crate) struct ToolRegistry {
    data_dir: PathBuf,
    authorized_files: HashMap<String, AuthorizedFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExecutionAuthorization {
    NotRequired,
    Approved { arguments_digest: String },
}

impl ToolRegistry {
    pub(crate) fn built_in(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            authorized_files: HashMap::new(),
        }
    }

    pub(crate) fn with_authorized_files(
        data_dir: PathBuf,
        authorized_files: Vec<AuthorizedFile>,
    ) -> Self {
        Self {
            data_dir,
            authorized_files: authorized_files
                .into_iter()
                .map(|file| (file.reference().reference_id.clone(), file))
                .collect(),
        }
    }

    pub(crate) fn definitions(&self) -> Vec<ToolDefinition> {
        vec![
            ToolDefinition {
                name: "current_time".into(),
                description: "Get the current date and time for a UTC offset. Use this when the user asks for the current time or date.".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "utcOffset": {
                            "type": "string",
                            "description": "UTC offset in ±HH:MM format, for example +08:00. Defaults to +00:00."
                        }
                    },
                    "additionalProperties": false
                }),
            },
            ToolDefinition {
                name: "save_text_note".into(),
                description: "Create a new UTF-8 text note in the app's managed agent-files directory. Existing files are never overwritten. This changes the filesystem and always requires user approval.".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "fileName": {
                            "type": "string",
                            "description": "A plain .txt file name without directories, for example notes.txt."
                        },
                        "content": { "type": "string", "description": "Text to save." }
                    },
                    "required": ["fileName", "content"],
                    "additionalProperties": false
                }),
            },
            ToolDefinition {
                name: "read_selected_text_file".into(),
                description: "Read the UTF-8 contents of a text file explicitly attached to this message. Only the supplied opaque referenceId is accepted.".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "referenceId": {
                            "type": "string",
                            "description": "The opaque referenceId of the attached file."
                        }
                    },
                    "required": ["referenceId"],
                    "additionalProperties": false
                }),
            },
        ]
    }

    pub(crate) fn names(&self) -> HashSet<String> {
        self.definitions()
            .into_iter()
            .map(|definition| definition.name)
            .collect()
    }

    pub(crate) fn definitions_for(&self, allowed: &HashSet<String>) -> Vec<ToolDefinition> {
        self.definitions()
            .into_iter()
            .filter(|definition| allowed.contains(&definition.name))
            .collect()
    }

    pub(crate) fn metadata(&self, name: &str) -> Result<(RiskLevel, ApprovalPolicy), RuntimeError> {
        match name {
            "current_time" => Ok((RiskLevel::ReadOnly, ApprovalPolicy::Never)),
            "save_text_note" => Ok((RiskLevel::Write, ApprovalPolicy::Always)),
            "read_selected_text_file" => Ok((RiskLevel::ReadOnly, ApprovalPolicy::Never)),
            _ => Err(RuntimeError::ToolNotFound(name.into())),
        }
    }

    pub(crate) async fn execute(
        &self,
        name: &str,
        arguments: &Value,
        authorization: ExecutionAuthorization,
    ) -> Result<Value, RuntimeError> {
        let (_, approval_policy) = self.metadata(name)?;
        match (approval_policy, authorization) {
            (ApprovalPolicy::Never, ExecutionAuthorization::NotRequired) => {}
            (
                ApprovalPolicy::Always,
                ExecutionAuthorization::Approved {
                    arguments_digest: approved_digest,
                },
            ) if arguments_digest(arguments)?.1 == approved_digest => {}
            _ => return Err(RuntimeError::InvalidToolApproval(name.into())),
        }
        self.validate(name, arguments)?;
        match name {
            "current_time" => current_time(arguments),
            "save_text_note" => save_text_note(&self.data_dir, arguments).await,
            "read_selected_text_file" => self.read_selected_text_file(arguments).await,
            _ => Err(RuntimeError::ToolNotFound(name.into())),
        }
    }

    pub(crate) fn validate(&self, name: &str, arguments: &Value) -> Result<(), RuntimeError> {
        match name {
            "current_time" => {
                let arguments: CurrentTimeArguments = serde_json::from_value(arguments.clone())
                    .map_err(|error| RuntimeError::InvalidToolArguments {
                        name: name.into(),
                        message: error.to_string(),
                    })?;
                parse_utc_offset(&arguments.utc_offset)?;
                Ok(())
            }
            "save_text_note" => {
                let arguments: SaveTextNoteArguments = serde_json::from_value(arguments.clone())
                    .map_err(|error| RuntimeError::InvalidToolArguments {
                        name: name.into(),
                        message: error.to_string(),
                    })?;
                validate_note_arguments(&arguments)
            }
            "read_selected_text_file" => {
                let arguments: ReadSelectedTextFileArguments =
                    serde_json::from_value(arguments.clone()).map_err(|error| {
                        RuntimeError::InvalidToolArguments {
                            name: name.into(),
                            message: error.to_string(),
                        }
                    })?;
                if self.authorized_files.contains_key(&arguments.reference_id) {
                    Ok(())
                } else {
                    Err(crate::files::FileError::ReferenceInvalid.into())
                }
            }
            _ => Err(RuntimeError::ToolNotFound(name.into())),
        }
    }

    pub(crate) fn result_summary(&self, name: &str, result: &Value) -> Value {
        if name == "read_selected_text_file" {
            json!({
                "fileName": result["fileName"],
                "size": result["size"],
                "read": result["read"]
            })
        } else {
            result.clone()
        }
    }

    async fn read_selected_text_file(&self, arguments: &Value) -> Result<Value, RuntimeError> {
        let arguments: ReadSelectedTextFileArguments = serde_json::from_value(arguments.clone())
            .map_err(|error| RuntimeError::InvalidToolArguments {
                name: "read_selected_text_file".into(),
                message: error.to_string(),
            })?;
        let file = self
            .authorized_files
            .get(&arguments.reference_id)
            .ok_or(crate::files::FileError::ReferenceInvalid)?;
        let content = file.read().await?;
        Ok(json!({
            "fileName": file.reference().name,
            "size": file.reference().size,
            "content": content,
            "read": true
        }))
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReadSelectedTextFileArguments {
    reference_id: String,
}

pub(crate) fn arguments_digest(arguments: &Value) -> Result<(String, String), RuntimeError> {
    let canonical =
        serde_json::to_string(arguments).map_err(|error| RuntimeError::ToolExecution {
            name: "arguments".into(),
            message: error.to_string(),
        })?;
    let digest = hex::encode(Sha256::digest(canonical.as_bytes()));
    Ok((canonical, digest))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CurrentTimeArguments {
    #[serde(default = "default_utc_offset")]
    utc_offset: String,
}

fn default_utc_offset() -> String {
    "+00:00".into()
}

fn current_time(arguments: &Value) -> Result<Value, RuntimeError> {
    let arguments: CurrentTimeArguments =
        serde_json::from_value(arguments.clone()).map_err(|error| {
            RuntimeError::InvalidToolArguments {
                name: "current_time".into(),
                message: error.to_string(),
            }
        })?;
    let offset = parse_utc_offset(&arguments.utc_offset)?;
    let local = Utc::now().with_timezone(&offset);

    Ok(json!({
        "dateTime": local.to_rfc3339_opts(SecondsFormat::Secs, true),
        "utcOffset": arguments.utc_offset,
        "unixTimestamp": local.timestamp()
    }))
}

fn parse_utc_offset(value: &str) -> Result<FixedOffset, RuntimeError> {
    let bytes = value.as_bytes();
    let valid_shape = bytes.len() == 6
        && matches!(bytes[0], b'+' | b'-')
        && bytes[3] == b':'
        && bytes[1..3].iter().all(u8::is_ascii_digit)
        && bytes[4..6].iter().all(u8::is_ascii_digit);
    if !valid_shape {
        return Err(invalid_offset(value));
    }
    let hours = value[1..3]
        .parse::<i32>()
        .map_err(|_| invalid_offset(value))?;
    let minutes = value[4..6]
        .parse::<i32>()
        .map_err(|_| invalid_offset(value))?;
    if hours > 23 || minutes > 59 {
        return Err(invalid_offset(value));
    }
    let seconds = (hours * 60 + minutes) * 60 * if bytes[0] == b'-' { -1 } else { 1 };
    FixedOffset::east_opt(seconds).ok_or_else(|| invalid_offset(value))
}

fn invalid_offset(value: &str) -> RuntimeError {
    RuntimeError::InvalidToolArguments {
        name: "current_time".into(),
        message: format!("utcOffset must use ±HH:MM format, received {value}"),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SaveTextNoteArguments {
    file_name: String,
    content: String,
}

async fn save_text_note(data_dir: &Path, arguments: &Value) -> Result<Value, RuntimeError> {
    let arguments: SaveTextNoteArguments =
        serde_json::from_value(arguments.clone()).map_err(|error| {
            RuntimeError::InvalidToolArguments {
                name: "save_text_note".into(),
                message: error.to_string(),
            }
        })?;
    validate_note_arguments(&arguments)?;
    tokio::fs::create_dir_all(data_dir)
        .await
        .map_err(|error| tool_io_error(error.to_string()))?;
    let path = data_dir.join(&arguments.file_name);
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .await
        .map_err(|error| tool_io_error(error.to_string()))?;
    let write_result = async {
        file.write_all(arguments.content.as_bytes()).await?;
        file.flush().await
    }
    .await;
    if let Err(error) = write_result {
        drop(file);
        let _ = tokio::fs::remove_file(&path).await;
        return Err(tool_io_error(error.to_string()));
    }
    Ok(json!({
        "fileName": arguments.file_name,
        "path": path,
        "saved": true
    }))
}

fn validate_note_arguments(arguments: &SaveTextNoteArguments) -> Result<(), RuntimeError> {
    let path = Path::new(&arguments.file_name);
    let valid_name = arguments.file_name.len() <= 128
        && !arguments.file_name.starts_with('.')
        && path.extension().and_then(|value| value.to_str()) == Some("txt")
        && matches!(
            path.components().collect::<Vec<_>>().as_slice(),
            [Component::Normal(_)]
        );
    if !valid_name {
        return Err(RuntimeError::InvalidToolArguments {
            name: "save_text_note".into(),
            message: "fileName must be a plain .txt name without directories".into(),
        });
    }
    if arguments.content.len() > 32 * 1024 {
        return Err(RuntimeError::InvalidToolArguments {
            name: "save_text_note".into(),
            message: "content must not exceed 32 KiB".into(),
        });
    }
    Ok(())
}

fn tool_io_error(message: String) -> RuntimeError {
    RuntimeError::ToolExecution {
        name: "save_text_note".into(),
        message,
    }
}

#[cfg(test)]
mod tests;
