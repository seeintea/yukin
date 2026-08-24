use std::{
    collections::{HashMap, HashSet},
    path::{Component, Path, PathBuf},
};

use chrono::{FixedOffset, SecondsFormat, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

use crate::files::{
    validate_copy_destination_name, validate_created_directory, validate_created_text_file,
    validate_move_destination_name, AuthorizedDirectory, AuthorizedFile, DirectorySearchKind,
};
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
    authorized_directories: HashMap<String, AuthorizedDirectory>,
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
            authorized_directories: HashMap::new(),
        }
    }

    pub(crate) fn with_authorizations(
        data_dir: PathBuf,
        authorized_files: Vec<AuthorizedFile>,
        authorized_directories: Vec<AuthorizedDirectory>,
    ) -> Self {
        Self {
            data_dir,
            authorized_files: authorized_files
                .into_iter()
                .map(|file| (file.reference().reference_id.clone(), file))
                .collect(),
            authorized_directories: authorized_directories
                .into_iter()
                .map(|directory| (directory.reference().reference_id.clone(), directory))
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
                name: "list_selected_directory".into(),
                description: "List up to 100 direct children of a directory explicitly selected by the user. This does not recurse or expose local paths.".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "referenceId": {
                            "type": "string",
                            "description": "The opaque referenceId of the selected directory."
                        }
                    },
                    "required": ["referenceId"],
                    "additionalProperties": false
                }),
            },
            ToolDefinition {
                name: "search_selected_directory".into(),
                description: "Search file and directory names within a directory explicitly selected by the user. Returns up to 50 relative paths, searches at most 4 levels deep, skips symbolic links, and never exposes local absolute paths.".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "referenceId": {
                            "type": "string",
                            "description": "The opaque referenceId of the selected directory."
                        },
                        "query": {
                            "type": "string",
                            "minLength": 1,
                            "maxLength": 128,
                            "description": "Case-insensitive text to match within each file or directory name."
                        },
                        "kind": {
                            "type": "string",
                            "enum": ["any", "file", "directory"],
                            "description": "Optional result type filter. Defaults to any."
                        }
                    },
                    "required": ["referenceId", "query"],
                    "additionalProperties": false
                }),
            },
            ToolDefinition {
                name: "create_text_file_in_selected_directory".into(),
                description: "Create a new UTF-8 .txt file at the root of a directory explicitly selected by the user. Existing files are never overwritten. This always requires user approval.".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "referenceId": {
                            "type": "string",
                            "description": "The opaque referenceId of the selected directory."
                        },
                        "fileName": {
                            "type": "string",
                            "description": "A plain .txt file name without directories, for example notes.txt."
                        },
                        "content": {
                            "type": "string",
                            "description": "UTF-8 text content, limited to 32 KiB."
                        }
                    },
                    "required": ["referenceId", "fileName", "content"],
                    "additionalProperties": false
                }),
            },
            ToolDefinition {
                name: "create_directory_in_selected_directory".into(),
                description: "Create one new child directory at the root of a directory explicitly selected by the user. Existing entries are never replaced. This always requires user approval.".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "referenceId": {
                            "type": "string",
                            "description": "The opaque referenceId of the selected directory."
                        },
                        "directoryName": {
                            "type": "string",
                            "description": "A plain directory name without path separators."
                        }
                    },
                    "required": ["referenceId", "directoryName"],
                    "additionalProperties": false
                }),
            },
            ToolDefinition {
                name: "copy_directory_entry".into(),
                description: "Copy one previously listed or searched file or directory within the same user-authorized directory. The destination may be the selected root or a previously listed or searched directory. Existing entries are never overwritten. Copies are limited to 100 entries, 8 levels, and 16 MiB, and always require user approval.".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "referenceId": {
                            "type": "string",
                            "description": "The opaque referenceId of the selected directory."
                        },
                        "sourceTargetReferenceId": {
                            "type": "string",
                            "description": "The opaque targetReferenceId of the source entry."
                        },
                        "sourceRelativePath": {
                            "type": "string",
                            "description": "The matching source relativePath returned with the source reference."
                        },
                        "destinationDirectoryTargetReferenceId": {
                            "type": "string",
                            "description": "Optional opaque targetReferenceId of a destination directory. Omit to use the selected root."
                        },
                        "destinationDirectoryRelativePath": {
                            "type": "string",
                            "description": "Required matching relativePath when a destination directory reference is supplied."
                        },
                        "destinationName": {
                            "type": "string",
                            "description": "A plain file or directory name for the copy, without path separators."
                        }
                    },
                    "required": ["referenceId", "sourceTargetReferenceId", "sourceRelativePath", "destinationName"],
                    "additionalProperties": false
                }),
            },
            ToolDefinition {
                name: "move_directory_entry".into(),
                description: "Move or rename one previously listed or searched file or directory within the same user-authorized directory. The destination may be the selected root or a previously listed or searched directory. Existing entries are never overwritten, directories cannot be moved into themselves, and this always requires user approval.".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "referenceId": {
                            "type": "string",
                            "description": "The opaque referenceId of the selected directory."
                        },
                        "sourceTargetReferenceId": {
                            "type": "string",
                            "description": "The opaque targetReferenceId of the source entry."
                        },
                        "sourceRelativePath": {
                            "type": "string",
                            "description": "The matching source relativePath returned with the source reference."
                        },
                        "destinationDirectoryTargetReferenceId": {
                            "type": "string",
                            "description": "Optional opaque targetReferenceId of a destination directory. Omit to use the selected root."
                        },
                        "destinationDirectoryRelativePath": {
                            "type": "string",
                            "description": "Required matching relativePath when a destination directory reference is supplied."
                        },
                        "destinationName": {
                            "type": "string",
                            "description": "A plain name for the moved entry, without path separators."
                        }
                    },
                    "required": ["referenceId", "sourceTargetReferenceId", "sourceRelativePath", "destinationName"],
                    "additionalProperties": false
                }),
            },
            ToolDefinition {
                name: "trash_directory_entry".into(),
                description: "Move one previously listed or searched file or directory to the operating system trash or recycle bin. This never permanently deletes the entry and always requires user approval.".into(),
                input_schema: directory_entry_input_schema(),
            },
            ToolDefinition {
                name: "get_directory_entry_metadata".into(),
                description: "Get size, modification time, entry type, and file extension for an entry returned by list_selected_directory or search_selected_directory.".into(),
                input_schema: directory_entry_input_schema(),
            },
            ToolDefinition {
                name: "open_directory_entry".into(),
                description: "Open a previously listed or searched directory entry with the system default application. This always requires user approval.".into(),
                input_schema: directory_entry_input_schema(),
            },
            ToolDefinition {
                name: "reveal_directory_entry".into(),
                description: "Reveal a previously listed or searched directory entry in the system file manager. This always requires user approval.".into(),
                input_schema: directory_entry_input_schema(),
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
            "list_selected_directory" => Ok((RiskLevel::ReadOnly, ApprovalPolicy::Never)),
            "search_selected_directory" => Ok((RiskLevel::ReadOnly, ApprovalPolicy::Never)),
            "get_directory_entry_metadata" => Ok((RiskLevel::ReadOnly, ApprovalPolicy::Never)),
            "create_text_file_in_selected_directory" => {
                Ok((RiskLevel::Write, ApprovalPolicy::Always))
            }
            "create_directory_in_selected_directory" => {
                Ok((RiskLevel::Write, ApprovalPolicy::Always))
            }
            "copy_directory_entry" => Ok((RiskLevel::Write, ApprovalPolicy::Always)),
            "move_directory_entry" => Ok((RiskLevel::Write, ApprovalPolicy::Always)),
            "trash_directory_entry" => Ok((RiskLevel::Write, ApprovalPolicy::Always)),
            "open_directory_entry" | "reveal_directory_entry" => {
                Ok((RiskLevel::Write, ApprovalPolicy::Always))
            }
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
            "list_selected_directory" => self.list_selected_directory(arguments).await,
            "search_selected_directory" => self.search_selected_directory(arguments).await,
            "get_directory_entry_metadata" => self.directory_entry_metadata(arguments).await,
            "create_text_file_in_selected_directory" => {
                self.create_text_file_in_selected_directory(arguments).await
            }
            "create_directory_in_selected_directory" => {
                self.create_directory_in_selected_directory(arguments).await
            }
            "copy_directory_entry" => self.copy_directory_entry(arguments).await,
            "move_directory_entry" => self.move_directory_entry(arguments).await,
            "trash_directory_entry" => self.trash_directory_entry(arguments).await,
            "open_directory_entry" => self.directory_entry_action(name, arguments, false).await,
            "reveal_directory_entry" => self.directory_entry_action(name, arguments, true).await,
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
            "list_selected_directory" => {
                let arguments: DirectoryReferenceArguments =
                    serde_json::from_value(arguments.clone()).map_err(|error| {
                        RuntimeError::InvalidToolArguments {
                            name: name.into(),
                            message: error.to_string(),
                        }
                    })?;
                if self
                    .authorized_directories
                    .contains_key(&arguments.reference_id)
                {
                    Ok(())
                } else {
                    Err(crate::files::FileError::ReferenceInvalid.into())
                }
            }
            "search_selected_directory" => {
                let arguments = parse_directory_search_arguments(name, arguments)?;
                validate_directory_search_arguments(name, &arguments)?;
                if self
                    .authorized_directories
                    .contains_key(&arguments.reference_id)
                {
                    Ok(())
                } else {
                    Err(crate::files::FileError::ReferenceInvalid.into())
                }
            }
            "get_directory_entry_metadata"
            | "open_directory_entry"
            | "reveal_directory_entry"
            | "trash_directory_entry" => {
                let arguments = parse_directory_entry_arguments(name, arguments)?;
                self.directory_for_entry(&arguments).map(|_| ())
            }
            "create_text_file_in_selected_directory" => {
                let arguments = parse_create_text_file_arguments(name, arguments)?;
                validate_created_text_file(&arguments.file_name, &arguments.content)?;
                if self
                    .authorized_directories
                    .contains_key(&arguments.reference_id)
                {
                    Ok(())
                } else {
                    Err(crate::files::FileError::ReferenceInvalid.into())
                }
            }
            "create_directory_in_selected_directory" => {
                let arguments = parse_create_directory_arguments(name, arguments)?;
                validate_created_directory(&arguments.directory_name)?;
                if self
                    .authorized_directories
                    .contains_key(&arguments.reference_id)
                {
                    Ok(())
                } else {
                    Err(crate::files::FileError::ReferenceInvalid.into())
                }
            }
            "copy_directory_entry" => {
                let arguments = parse_copy_directory_entry_arguments(name, arguments)?;
                validate_copy_destination_name(&arguments.destination_name)?;
                let directory = self
                    .authorized_directories
                    .get(&arguments.reference_id)
                    .ok_or(crate::files::FileError::ReferenceInvalid)?;
                if !directory.validates_entry_reference(
                    &arguments.source_target_reference_id,
                    &arguments.source_relative_path,
                ) {
                    return Err(crate::files::FileError::EntryReferenceInvalid.into());
                }
                match (
                    &arguments.destination_directory_target_reference_id,
                    &arguments.destination_directory_relative_path,
                ) {
                    (None, None) => Ok(()),
                    (Some(reference_id), Some(relative_path))
                        if directory.validates_entry_reference(reference_id, relative_path) =>
                    {
                        Ok(())
                    }
                    _ => Err(crate::files::FileError::EntryReferenceInvalid.into()),
                }
            }
            "move_directory_entry" => {
                let arguments = parse_move_directory_entry_arguments(name, arguments)?;
                validate_move_destination_name(&arguments.destination_name)?;
                let directory = self
                    .authorized_directories
                    .get(&arguments.reference_id)
                    .ok_or(crate::files::FileError::ReferenceInvalid)?;
                if !directory.validates_entry_reference(
                    &arguments.source_target_reference_id,
                    &arguments.source_relative_path,
                ) {
                    return Err(crate::files::FileError::EntryReferenceInvalid.into());
                }
                match (
                    &arguments.destination_directory_target_reference_id,
                    &arguments.destination_directory_relative_path,
                ) {
                    (None, None) => Ok(()),
                    (Some(reference_id), Some(relative_path))
                        if directory.validates_entry_reference(reference_id, relative_path) =>
                    {
                        Ok(())
                    }
                    _ => Err(crate::files::FileError::EntryReferenceInvalid.into()),
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

    async fn list_selected_directory(&self, arguments: &Value) -> Result<Value, RuntimeError> {
        let arguments: DirectoryReferenceArguments = serde_json::from_value(arguments.clone())
            .map_err(|error| RuntimeError::InvalidToolArguments {
                name: "list_selected_directory".into(),
                message: error.to_string(),
            })?;
        let directory = self
            .authorized_directories
            .get(&arguments.reference_id)
            .ok_or(crate::files::FileError::ReferenceInvalid)?;
        let listing = directory.list().await?;
        let entries = listing
            .entries
            .into_iter()
            .map(|entry| {
                json!({
                    "name": entry.name,
                    "kind": entry.kind,
                    "size": entry.size,
                    "targetReferenceId": entry.target_reference_id
                })
            })
            .collect::<Vec<_>>();
        Ok(json!({
            "directoryName": directory.reference().name,
            "entries": entries,
            "truncated": listing.truncated
        }))
    }

    async fn search_selected_directory(&self, arguments: &Value) -> Result<Value, RuntimeError> {
        let arguments = parse_directory_search_arguments("search_selected_directory", arguments)?;
        validate_directory_search_arguments("search_selected_directory", &arguments)?;
        let directory = self
            .authorized_directories
            .get(&arguments.reference_id)
            .ok_or(crate::files::FileError::ReferenceInvalid)?;
        let query = arguments.query.trim();
        let search = directory.search(query, arguments.kind.into()).await?;
        let entries = search
            .entries
            .into_iter()
            .map(|entry| {
                json!({
                    "targetReferenceId": entry.target_reference_id,
                    "name": entry.name,
                    "relativePath": entry.relative_path,
                    "kind": entry.kind.as_str(),
                    "size": entry.size
                })
            })
            .collect::<Vec<_>>();
        Ok(json!({
            "directoryName": directory.reference().name,
            "query": query,
            "kind": arguments.kind.as_str(),
            "entries": entries,
            "truncated": search.truncated
        }))
    }

    async fn directory_entry_metadata(&self, arguments: &Value) -> Result<Value, RuntimeError> {
        let arguments = parse_directory_entry_arguments("get_directory_entry_metadata", arguments)?;
        let directory = self.directory_for_entry(&arguments)?;
        let metadata = directory
            .entry_metadata(&arguments.target_reference_id)
            .await?;
        Ok(json!({
            "targetReferenceId": metadata.target_reference_id,
            "name": metadata.name,
            "relativePath": metadata.relative_path,
            "kind": metadata.kind.as_str(),
            "size": metadata.size,
            "modifiedAt": metadata.modified_at,
            "extension": metadata.extension
        }))
    }

    async fn create_text_file_in_selected_directory(
        &self,
        arguments: &Value,
    ) -> Result<Value, RuntimeError> {
        let arguments =
            parse_create_text_file_arguments("create_text_file_in_selected_directory", arguments)?;
        validate_created_text_file(&arguments.file_name, &arguments.content)?;
        let directory = self
            .authorized_directories
            .get(&arguments.reference_id)
            .ok_or(crate::files::FileError::ReferenceInvalid)?;
        let metadata = directory
            .create_text_file(&arguments.file_name, &arguments.content)
            .await?;
        Ok(json!({
            "directoryName": directory.reference().name,
            "targetReferenceId": metadata.target_reference_id,
            "fileName": metadata.name,
            "relativePath": metadata.relative_path,
            "size": metadata.size,
            "created": true
        }))
    }

    async fn create_directory_in_selected_directory(
        &self,
        arguments: &Value,
    ) -> Result<Value, RuntimeError> {
        let arguments =
            parse_create_directory_arguments("create_directory_in_selected_directory", arguments)?;
        validate_created_directory(&arguments.directory_name)?;
        let directory = self
            .authorized_directories
            .get(&arguments.reference_id)
            .ok_or(crate::files::FileError::ReferenceInvalid)?;
        let metadata = directory
            .create_directory(&arguments.directory_name)
            .await?;
        Ok(json!({
            "directoryName": directory.reference().name,
            "targetReferenceId": metadata.target_reference_id,
            "createdDirectoryName": metadata.name,
            "relativePath": metadata.relative_path,
            "created": true
        }))
    }

    async fn copy_directory_entry(&self, arguments: &Value) -> Result<Value, RuntimeError> {
        let arguments = parse_copy_directory_entry_arguments("copy_directory_entry", arguments)?;
        validate_copy_destination_name(&arguments.destination_name)?;
        let directory = self
            .authorized_directories
            .get(&arguments.reference_id)
            .ok_or(crate::files::FileError::ReferenceInvalid)?;
        let result = directory
            .copy_entry(
                &arguments.source_target_reference_id,
                arguments
                    .destination_directory_target_reference_id
                    .as_deref(),
                &arguments.destination_name,
            )
            .await?;
        Ok(json!({
            "directoryName": directory.reference().name,
            "targetReferenceId": result.metadata.target_reference_id,
            "name": result.metadata.name,
            "relativePath": result.metadata.relative_path,
            "kind": result.metadata.kind.as_str(),
            "size": result.metadata.size,
            "copiedEntries": result.copied_entries,
            "copiedBytes": result.copied_bytes,
            "copied": true
        }))
    }

    async fn move_directory_entry(&self, arguments: &Value) -> Result<Value, RuntimeError> {
        let arguments = parse_move_directory_entry_arguments("move_directory_entry", arguments)?;
        validate_move_destination_name(&arguments.destination_name)?;
        let directory = self
            .authorized_directories
            .get(&arguments.reference_id)
            .ok_or(crate::files::FileError::ReferenceInvalid)?;
        let result = directory
            .move_entry(
                &arguments.source_target_reference_id,
                arguments
                    .destination_directory_target_reference_id
                    .as_deref(),
                &arguments.destination_name,
            )
            .await?;
        Ok(json!({
            "directoryName": directory.reference().name,
            "previousRelativePath": result.previous_relative_path,
            "targetReferenceId": result.metadata.target_reference_id,
            "name": result.metadata.name,
            "relativePath": result.metadata.relative_path,
            "kind": result.metadata.kind.as_str(),
            "size": result.metadata.size,
            "moved": true
        }))
    }

    async fn trash_directory_entry(&self, arguments: &Value) -> Result<Value, RuntimeError> {
        let arguments = parse_directory_entry_arguments("trash_directory_entry", arguments)?;
        let directory = self.directory_for_entry(&arguments)?;
        let result = directory
            .trash_entry(&arguments.target_reference_id)
            .await?;
        Ok(json!({
            "directoryName": directory.reference().name,
            "name": result.name,
            "relativePath": result.relative_path,
            "kind": result.kind.as_str(),
            "trashed": true
        }))
    }

    async fn directory_entry_action(
        &self,
        name: &str,
        arguments: &Value,
        reveal: bool,
    ) -> Result<Value, RuntimeError> {
        let arguments = parse_directory_entry_arguments(name, arguments)?;
        let directory = self.directory_for_entry(&arguments)?;
        let path = directory
            .resolve_entry(&arguments.target_reference_id)
            .await?;
        let action = tauri::async_runtime::spawn_blocking(move || {
            if reveal {
                tauri_plugin_opener::reveal_item_in_dir(path)
            } else {
                tauri_plugin_opener::open_path(path, None::<&str>)
            }
        })
        .await
        .map_err(|_| RuntimeError::ToolExecution {
            name: name.into(),
            message: "system file action task failed".into(),
        })?;
        action.map_err(|_| RuntimeError::ToolExecution {
            name: name.into(),
            message: "system file action failed".into(),
        })?;
        Ok(json!({
            "relativePath": arguments.relative_path,
            "action": if reveal { "revealed" } else { "opened" },
            "completed": true
        }))
    }

    fn directory_for_entry(
        &self,
        arguments: &DirectoryEntryArguments,
    ) -> Result<&AuthorizedDirectory, RuntimeError> {
        self.authorized_directories
            .values()
            .find(|directory| {
                directory.validates_entry_reference(
                    &arguments.target_reference_id,
                    &arguments.relative_path,
                )
            })
            .ok_or(crate::files::FileError::EntryReferenceInvalid.into())
    }
}

fn directory_entry_input_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "targetReferenceId": {
                "type": "string",
                "description": "The opaque targetReferenceId returned for the entry."
            },
            "relativePath": {
                "type": "string",
                "description": "The matching relativePath returned with the target reference, used for a clear approval summary."
            }
        },
        "required": ["targetReferenceId", "relativePath"],
        "additionalProperties": false
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReadSelectedTextFileArguments {
    reference_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DirectoryReferenceArguments {
    reference_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DirectorySearchArguments {
    reference_id: String,
    query: String,
    #[serde(default)]
    kind: DirectorySearchKindArgument,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DirectoryEntryArguments {
    target_reference_id: String,
    relative_path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateTextFileArguments {
    reference_id: String,
    file_name: String,
    content: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateDirectoryArguments {
    reference_id: String,
    directory_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CopyDirectoryEntryArguments {
    reference_id: String,
    source_target_reference_id: String,
    source_relative_path: String,
    destination_directory_target_reference_id: Option<String>,
    destination_directory_relative_path: Option<String>,
    destination_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MoveDirectoryEntryArguments {
    reference_id: String,
    source_target_reference_id: String,
    source_relative_path: String,
    destination_directory_target_reference_id: Option<String>,
    destination_directory_relative_path: Option<String>,
    destination_name: String,
}

fn parse_move_directory_entry_arguments(
    name: &str,
    arguments: &Value,
) -> Result<MoveDirectoryEntryArguments, RuntimeError> {
    serde_json::from_value(arguments.clone()).map_err(|error| RuntimeError::InvalidToolArguments {
        name: name.into(),
        message: error.to_string(),
    })
}

fn parse_copy_directory_entry_arguments(
    name: &str,
    arguments: &Value,
) -> Result<CopyDirectoryEntryArguments, RuntimeError> {
    serde_json::from_value(arguments.clone()).map_err(|error| RuntimeError::InvalidToolArguments {
        name: name.into(),
        message: error.to_string(),
    })
}

fn parse_create_directory_arguments(
    name: &str,
    arguments: &Value,
) -> Result<CreateDirectoryArguments, RuntimeError> {
    serde_json::from_value(arguments.clone()).map_err(|error| RuntimeError::InvalidToolArguments {
        name: name.into(),
        message: error.to_string(),
    })
}

fn parse_create_text_file_arguments(
    name: &str,
    arguments: &Value,
) -> Result<CreateTextFileArguments, RuntimeError> {
    serde_json::from_value(arguments.clone()).map_err(|error| RuntimeError::InvalidToolArguments {
        name: name.into(),
        message: error.to_string(),
    })
}

fn parse_directory_entry_arguments(
    name: &str,
    arguments: &Value,
) -> Result<DirectoryEntryArguments, RuntimeError> {
    serde_json::from_value(arguments.clone()).map_err(|error| RuntimeError::InvalidToolArguments {
        name: name.into(),
        message: error.to_string(),
    })
}

#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum DirectorySearchKindArgument {
    #[default]
    Any,
    File,
    Directory,
}

impl DirectorySearchKindArgument {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Any => "any",
            Self::File => "file",
            Self::Directory => "directory",
        }
    }
}

impl From<DirectorySearchKindArgument> for DirectorySearchKind {
    fn from(value: DirectorySearchKindArgument) -> Self {
        match value {
            DirectorySearchKindArgument::Any => Self::Any,
            DirectorySearchKindArgument::File => Self::File,
            DirectorySearchKindArgument::Directory => Self::Directory,
        }
    }
}

fn parse_directory_search_arguments(
    name: &str,
    arguments: &Value,
) -> Result<DirectorySearchArguments, RuntimeError> {
    serde_json::from_value(arguments.clone()).map_err(|error| RuntimeError::InvalidToolArguments {
        name: name.into(),
        message: error.to_string(),
    })
}

fn validate_directory_search_arguments(
    name: &str,
    arguments: &DirectorySearchArguments,
) -> Result<(), RuntimeError> {
    let query = arguments.query.trim();
    if query.is_empty() || query.chars().count() > 128 || query.chars().any(char::is_control) {
        return Err(RuntimeError::InvalidToolArguments {
            name: name.into(),
            message: "query must contain 1 to 128 non-control characters".into(),
        });
    }
    Ok(())
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
