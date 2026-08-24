use super::*;

impl AuthorizedDirectory {
    pub(crate) async fn move_entry(
        &self,
        source_reference_id: &str,
        destination_directory_reference_id: Option<&str>,
        destination_name: &str,
    ) -> Result<DirectoryMoveResult, FileError> {
        validate_move_destination_name(destination_name)?;
        let canonical_root = self.validate_root().await?;
        let source = self.entry(source_reference_id)?;
        let (source_path, source_metadata) = source.resolve().await?;
        let (destination_directory, destination_relative_directory) =
            if let Some(reference_id) = destination_directory_reference_id {
                let destination = self.entry(reference_id)?;
                let (path, metadata) = destination.resolve().await?;
                if !metadata.is_dir() {
                    return Err(FileError::NotDirectory);
                }
                (path, PathBuf::from(destination.relative_path))
            } else {
                (canonical_root, PathBuf::new())
            };

        if source_metadata.is_dir() && destination_directory.starts_with(&source_path) {
            return Err(FileError::MoveIntoSource);
        }
        let destination_path = destination_directory.join(destination_name);
        if destination_path == source_path {
            return Err(FileError::AlreadyExists);
        }
        match tokio::fs::symlink_metadata(&destination_path).await {
            Ok(_) => return Err(FileError::AlreadyExists),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(FileError::Io(error.to_string())),
        }

        validate_resolved_directory(&destination_directory).await?;
        validate_scoped_source(
            &source_path,
            &source_path,
            source_metadata.is_dir(),
            source_metadata.len(),
        )
        .await?;
        tokio::fs::rename(&source_path, &destination_path)
            .await
            .map_err(map_create_error)?;

        let destination_relative_path = destination_relative_directory.join(destination_name);
        self.invalidate_entry_tree(&source.relative_path);
        let target_reference_id = self.register_entry(
            destination_path,
            destination_relative_path.clone(),
            destination_name.to_owned(),
        );
        let kind = if source_metadata.is_dir() {
            DirectorySearchKind::Directory
        } else {
            DirectorySearchKind::File
        };
        let modified_at = source_metadata.modified().ok().map(|modified| {
            chrono::DateTime::<chrono::Utc>::from(modified)
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
        });
        let extension = (kind == DirectorySearchKind::File)
            .then(|| {
                Path::new(destination_name)
                    .extension()
                    .and_then(|value| value.to_str())
                    .map(str::to_lowercase)
            })
            .flatten();
        Ok(DirectoryMoveResult {
            previous_relative_path: source.relative_path,
            metadata: DirectoryEntryMetadata {
                target_reference_id,
                name: destination_name.to_owned(),
                relative_path: display_relative_path(&destination_relative_path),
                kind,
                size: (kind == DirectorySearchKind::File).then_some(source_metadata.len()),
                modified_at,
                extension,
            },
        })
    }

    pub(crate) async fn move_entries(
        &self,
        requests: &[BatchMoveEntryRequest],
        conflict_strategy: BatchMoveConflictStrategy,
    ) -> Result<BatchMoveResult, FileError> {
        if requests.is_empty() || requests.len() > MAX_BATCH_MOVE_ENTRIES {
            return Err(FileError::BatchMoveInvalid);
        }
        let canonical_root = self.validate_root().await?;
        let mut plans = Vec::with_capacity(requests.len());
        for request in requests {
            validate_move_destination_name(&request.destination_name)?;
            if plans
                .iter()
                .any(|plan: &BatchMovePlan| plan.source.reference_id == request.source_reference_id)
            {
                return Err(FileError::BatchMoveInvalid);
            }
            let source = self.entry(&request.source_reference_id)?;
            let (source_path, source_metadata) = source.resolve().await?;
            let (destination_directory, destination_relative_directory) =
                if let Some(reference_id) = request.destination_directory_reference_id.as_deref() {
                    let destination = self.entry(reference_id)?;
                    let (path, metadata) = destination.resolve().await?;
                    if !metadata.is_dir() {
                        return Err(FileError::NotDirectory);
                    }
                    (path, PathBuf::from(destination.relative_path))
                } else {
                    (canonical_root.clone(), PathBuf::new())
                };
            if source_metadata.is_dir() && destination_directory.starts_with(&source_path) {
                return Err(FileError::MoveIntoSource);
            }
            let destination_path = destination_directory.join(&request.destination_name);
            if destination_path == source_path {
                return Err(FileError::AlreadyExists);
            }
            let destination_relative_path =
                destination_relative_directory.join(&request.destination_name);
            plans.push(BatchMovePlan {
                source,
                source_path,
                source_metadata,
                destination_directory,
                destination_path,
                destination_relative_path,
                destination_name: request.destination_name.clone(),
                conflict: false,
            });
        }

        for left_index in 0..plans.len() {
            for right_index in (left_index + 1)..plans.len() {
                let left = &plans[left_index];
                let right = &plans[right_index];
                if left.source_path.starts_with(&right.source_path)
                    || right.source_path.starts_with(&left.source_path)
                    || left.destination_directory.starts_with(&right.source_path)
                    || right.destination_directory.starts_with(&left.source_path)
                {
                    return Err(FileError::BatchMoveInvalid);
                }
            }
        }

        for index in 0..plans.len() {
            let conflicts_with_planned_target = plans[..index]
                .iter()
                .any(|plan| plan.destination_path == plans[index].destination_path);
            let target_exists =
                match tokio::fs::symlink_metadata(&plans[index].destination_path).await {
                    Ok(_) => true,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
                    Err(error) => return Err(FileError::Io(error.to_string())),
                };
            if conflicts_with_planned_target || target_exists {
                if conflict_strategy == BatchMoveConflictStrategy::Fail {
                    return Err(FileError::AlreadyExists);
                }
                plans[index].conflict = true;
            }
        }

        let mut rollback = MoveRollback::default();
        for plan in plans.iter_mut().filter(|plan| !plan.conflict) {
            validate_resolved_directory(&plan.destination_directory).await?;
            validate_scoped_source(
                &plan.source_path,
                &plan.source_path,
                plan.source_metadata.is_dir(),
                plan.source_metadata.len(),
            )
            .await?;
            match tokio::fs::symlink_metadata(&plan.destination_path).await {
                Ok(_) if conflict_strategy == BatchMoveConflictStrategy::Skip => {
                    plan.conflict = true;
                    continue;
                }
                Ok(_) => return Err(FileError::AlreadyExists),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(FileError::Io(error.to_string())),
            }
            tokio::fs::rename(&plan.source_path, &plan.destination_path)
                .await
                .map_err(map_create_error)?;
            rollback.push(plan.source_path.clone(), plan.destination_path.clone());
        }

        for plan in plans.iter().filter(|plan| !plan.conflict) {
            self.invalidate_entry_tree(&plan.source.relative_path);
        }
        let mut items = Vec::with_capacity(plans.len());
        for plan in plans {
            if plan.conflict {
                items.push(BatchMoveItemResult {
                    previous_relative_path: plan.source.relative_path.clone(),
                    relative_path: display_relative_path(&plan.destination_relative_path),
                    kind: if plan.source_metadata.is_dir() {
                        DirectorySearchKind::Directory
                    } else {
                        DirectorySearchKind::File
                    },
                    status: BatchMoveItemStatus::Skipped,
                    target_reference_id: None,
                    error_code: Some(FileError::AlreadyExists.code()),
                });
                continue;
            }
            let target_reference_id = self.register_entry(
                plan.destination_path,
                plan.destination_relative_path.clone(),
                plan.destination_name,
            );
            items.push(BatchMoveItemResult {
                previous_relative_path: plan.source.relative_path,
                relative_path: display_relative_path(&plan.destination_relative_path),
                kind: if plan.source_metadata.is_dir() {
                    DirectorySearchKind::Directory
                } else {
                    DirectorySearchKind::File
                },
                status: BatchMoveItemStatus::Moved,
                target_reference_id: Some(target_reference_id),
                error_code: None,
            });
        }
        rollback.disarm();
        let moved = items
            .iter()
            .filter(|item| item.status == BatchMoveItemStatus::Moved)
            .count();
        Ok(BatchMoveResult {
            skipped: items.len() - moved,
            moved,
            items,
        })
    }
}

struct BatchMovePlan {
    source: AuthorizedDirectoryEntry,
    source_path: PathBuf,
    source_metadata: Metadata,
    destination_directory: PathBuf,
    destination_path: PathBuf,
    destination_relative_path: PathBuf,
    destination_name: String,
    conflict: bool,
}

#[derive(Default)]
pub(super) struct MoveRollback {
    moves: Vec<(PathBuf, PathBuf)>,
    armed: bool,
}

impl MoveRollback {
    pub(super) fn push(&mut self, source: PathBuf, destination: PathBuf) {
        self.armed = true;
        self.moves.push((source, destination));
    }

    pub(super) fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for MoveRollback {
    fn drop(&mut self) {
        if self.armed {
            for (source, destination) in self.moves.iter().rev() {
                let _ = std::fs::rename(destination, source);
            }
        }
    }
}
