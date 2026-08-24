use super::*;

impl ToolRegistry {
    pub(super) async fn read_selected_text_file(
        &self,
        arguments: &Value,
    ) -> Result<Value, RuntimeError> {
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

    pub(super) async fn list_selected_directory(
        &self,
        arguments: &Value,
    ) -> Result<Value, RuntimeError> {
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

    pub(super) async fn search_selected_directory(
        &self,
        arguments: &Value,
    ) -> Result<Value, RuntimeError> {
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

    pub(super) async fn directory_entry_metadata(
        &self,
        arguments: &Value,
    ) -> Result<Value, RuntimeError> {
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

    pub(super) async fn create_text_file_in_selected_directory(
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

    pub(super) async fn create_directory_in_selected_directory(
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

    pub(super) async fn copy_directory_entry(
        &self,
        arguments: &Value,
    ) -> Result<Value, RuntimeError> {
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

    pub(super) async fn move_directory_entry(
        &self,
        arguments: &Value,
    ) -> Result<Value, RuntimeError> {
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

    pub(super) async fn trash_directory_entry(
        &self,
        arguments: &Value,
    ) -> Result<Value, RuntimeError> {
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

    pub(super) async fn batch_move_directory_entries(
        &self,
        arguments: &Value,
    ) -> Result<Value, RuntimeError> {
        let arguments = parse_batch_move_arguments("batch_move_directory_entries", arguments)?;
        let directory = self
            .authorized_directories
            .get(&arguments.reference_id)
            .ok_or(crate::files::FileError::ReferenceInvalid)?;
        let requests = arguments
            .items
            .iter()
            .map(|item| BatchMoveEntryRequest {
                source_reference_id: item.source_target_reference_id.clone(),
                destination_directory_reference_id: item
                    .destination_directory_target_reference_id
                    .clone(),
                destination_name: item.destination_name.clone(),
            })
            .collect::<Vec<_>>();
        let result = directory
            .move_entries(&requests, arguments.conflict_strategy.into())
            .await?;
        Ok(json!({
            "directoryName": directory.reference().name,
            "items": result.items.into_iter().map(|item| json!({
                "previousRelativePath": item.previous_relative_path,
                "relativePath": item.relative_path,
                "kind": item.kind.as_str(),
                "status": item.status.as_str(),
                "targetReferenceId": item.target_reference_id,
                "errorCode": item.error_code
            })).collect::<Vec<_>>(),
            "moved": result.moved,
            "skipped": result.skipped,
            "completed": true
        }))
    }

    pub(super) async fn directory_entry_action(
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

    pub(super) fn directory_for_entry(
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

pub(super) fn directory_entry_input_schema() -> Value {
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
pub(super) struct ReadSelectedTextFileArguments {
    pub(super) reference_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct DirectoryReferenceArguments {
    pub(super) reference_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct DirectorySearchArguments {
    pub(super) reference_id: String,
    pub(super) query: String,
    #[serde(default)]
    pub(super) kind: DirectorySearchKindArgument,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct DirectoryEntryArguments {
    pub(super) target_reference_id: String,
    pub(super) relative_path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CreateTextFileArguments {
    pub(super) reference_id: String,
    pub(super) file_name: String,
    pub(super) content: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CreateDirectoryArguments {
    pub(super) reference_id: String,
    pub(super) directory_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CopyDirectoryEntryArguments {
    pub(super) reference_id: String,
    pub(super) source_target_reference_id: String,
    pub(super) source_relative_path: String,
    pub(super) destination_directory_target_reference_id: Option<String>,
    pub(super) destination_directory_relative_path: Option<String>,
    pub(super) destination_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct MoveDirectoryEntryArguments {
    pub(super) reference_id: String,
    pub(super) source_target_reference_id: String,
    pub(super) source_relative_path: String,
    pub(super) destination_directory_target_reference_id: Option<String>,
    pub(super) destination_directory_relative_path: Option<String>,
    pub(super) destination_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct BatchMoveArguments {
    pub(super) reference_id: String,
    pub(super) items: Vec<BatchMoveItemArguments>,
    #[serde(default)]
    pub(super) conflict_strategy: BatchMoveConflictStrategyArgument,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct BatchMoveItemArguments {
    pub(super) source_target_reference_id: String,
    pub(super) source_relative_path: String,
    pub(super) destination_directory_target_reference_id: Option<String>,
    pub(super) destination_directory_relative_path: Option<String>,
    pub(super) destination_name: String,
}

#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum BatchMoveConflictStrategyArgument {
    #[default]
    Fail,
    Skip,
}

impl From<BatchMoveConflictStrategyArgument> for BatchMoveConflictStrategy {
    fn from(value: BatchMoveConflictStrategyArgument) -> Self {
        match value {
            BatchMoveConflictStrategyArgument::Fail => Self::Fail,
            BatchMoveConflictStrategyArgument::Skip => Self::Skip,
        }
    }
}

pub(super) fn parse_batch_move_arguments(
    name: &str,
    arguments: &Value,
) -> Result<BatchMoveArguments, RuntimeError> {
    serde_json::from_value(arguments.clone()).map_err(|error| RuntimeError::InvalidToolArguments {
        name: name.into(),
        message: error.to_string(),
    })
}

pub(super) fn parse_move_directory_entry_arguments(
    name: &str,
    arguments: &Value,
) -> Result<MoveDirectoryEntryArguments, RuntimeError> {
    serde_json::from_value(arguments.clone()).map_err(|error| RuntimeError::InvalidToolArguments {
        name: name.into(),
        message: error.to_string(),
    })
}

pub(super) fn parse_copy_directory_entry_arguments(
    name: &str,
    arguments: &Value,
) -> Result<CopyDirectoryEntryArguments, RuntimeError> {
    serde_json::from_value(arguments.clone()).map_err(|error| RuntimeError::InvalidToolArguments {
        name: name.into(),
        message: error.to_string(),
    })
}

pub(super) fn parse_create_directory_arguments(
    name: &str,
    arguments: &Value,
) -> Result<CreateDirectoryArguments, RuntimeError> {
    serde_json::from_value(arguments.clone()).map_err(|error| RuntimeError::InvalidToolArguments {
        name: name.into(),
        message: error.to_string(),
    })
}

pub(super) fn parse_create_text_file_arguments(
    name: &str,
    arguments: &Value,
) -> Result<CreateTextFileArguments, RuntimeError> {
    serde_json::from_value(arguments.clone()).map_err(|error| RuntimeError::InvalidToolArguments {
        name: name.into(),
        message: error.to_string(),
    })
}

pub(super) fn parse_directory_entry_arguments(
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
pub(super) enum DirectorySearchKindArgument {
    #[default]
    Any,
    File,
    Directory,
}

impl DirectorySearchKindArgument {
    pub(super) const fn as_str(self) -> &'static str {
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

pub(super) fn parse_directory_search_arguments(
    name: &str,
    arguments: &Value,
) -> Result<DirectorySearchArguments, RuntimeError> {
    serde_json::from_value(arguments.clone()).map_err(|error| RuntimeError::InvalidToolArguments {
        name: name.into(),
        message: error.to_string(),
    })
}

pub(super) fn validate_directory_search_arguments(
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
