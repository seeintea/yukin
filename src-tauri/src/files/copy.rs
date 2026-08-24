use super::*;

impl AuthorizedDirectory {
    pub(crate) async fn copy_entry(
        &self,
        source_reference_id: &str,
        destination_directory_reference_id: Option<&str>,
        destination_name: &str,
    ) -> Result<DirectoryCopyResult, FileError> {
        validate_copy_destination_name(destination_name)?;
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
            return Err(FileError::CopyIntoSource);
        }

        let destination_path = destination_directory.join(destination_name);
        let destination_relative_path = destination_relative_directory.join(destination_name);
        let plan = build_copy_plan(&source_path, &source_metadata).await?;
        validate_resolved_directory(&destination_directory).await?;
        validate_scoped_source(
            &source_path,
            &source_path,
            source_metadata.is_dir(),
            source_metadata.len(),
        )
        .await?;
        let cleanup_kind = if source_metadata.is_dir() {
            tokio::fs::create_dir(&destination_path)
                .await
                .map_err(map_create_error)?;
            CopyCleanupKind::Directory
        } else {
            tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&destination_path)
                .await
                .map_err(map_create_error)?;
            CopyCleanupKind::File
        };
        let mut cleanup = CopyCleanup::new(destination_path.clone(), cleanup_kind);

        if source_metadata.is_file() {
            copy_file_checked(
                &source_path,
                &destination_path,
                source_metadata.len(),
                false,
                &source_path,
            )
            .await?;
        } else {
            for entry in &plan.entries {
                let source_child = source_path.join(&entry.relative_path);
                let destination_child = destination_path.join(&entry.relative_path);
                match entry.kind {
                    CopyPlanKind::Directory => {
                        validate_scoped_source(&source_child, &source_path, true, 0).await?;
                        tokio::fs::create_dir(&destination_child)
                            .await
                            .map_err(map_create_error)?;
                    }
                    CopyPlanKind::File => {
                        copy_file_checked(
                            &source_child,
                            &destination_child,
                            entry.size,
                            true,
                            &source_path,
                        )
                        .await?;
                    }
                }
            }
        }

        let destination_metadata = tokio::fs::metadata(&destination_path).await?;
        let kind = if destination_metadata.is_dir() {
            DirectorySearchKind::Directory
        } else {
            DirectorySearchKind::File
        };
        let modified_at = destination_metadata.modified().ok().map(|modified| {
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
        let target_reference_id = self.register_entry(
            destination_path.clone(),
            destination_relative_path.clone(),
            destination_name.to_owned(),
        );
        let metadata = DirectoryEntryMetadata {
            target_reference_id,
            name: destination_name.to_owned(),
            relative_path: display_relative_path(&destination_relative_path),
            kind,
            size: (kind == DirectorySearchKind::File).then_some(destination_metadata.len()),
            modified_at,
            extension,
        };
        cleanup.disarm();
        Ok(DirectoryCopyResult {
            metadata,
            copied_entries: plan.copied_entries,
            copied_bytes: plan.copied_bytes,
        })
    }
}

#[derive(Debug, Clone, Copy)]
enum CopyCleanupKind {
    File,
    Directory,
}

struct CopyCleanup {
    path: PathBuf,
    kind: CopyCleanupKind,
    armed: bool,
}

impl CopyCleanup {
    fn new(path: PathBuf, kind: CopyCleanupKind) -> Self {
        Self {
            path,
            kind,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for CopyCleanup {
    fn drop(&mut self) {
        if self.armed {
            match self.kind {
                CopyCleanupKind::File => {
                    let _ = std::fs::remove_file(&self.path);
                }
                CopyCleanupKind::Directory => {
                    let _ = std::fs::remove_dir_all(&self.path);
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum CopyPlanKind {
    File,
    Directory,
}

#[derive(Debug)]
struct CopyPlanEntry {
    relative_path: PathBuf,
    kind: CopyPlanKind,
    size: u64,
}

#[derive(Debug)]
struct CopyPlan {
    entries: Vec<CopyPlanEntry>,
    copied_entries: usize,
    copied_bytes: u64,
}

async fn build_copy_plan(source: &Path, metadata: &Metadata) -> Result<CopyPlan, FileError> {
    if metadata.is_file() {
        if metadata.len() > MAX_COPY_BYTES {
            return Err(FileError::CopyLimitExceeded);
        }
        return Ok(CopyPlan {
            entries: Vec::new(),
            copied_entries: 1,
            copied_bytes: metadata.len(),
        });
    }

    let mut pending = VecDeque::from([(source.to_path_buf(), PathBuf::new(), 0)]);
    let mut entries = Vec::new();
    let mut copied_entries = 1;
    let mut copied_bytes = 0_u64;
    while let Some((directory, relative_directory, depth)) = pending.pop_front() {
        let mut reader = tokio::fs::read_dir(directory).await?;
        let mut children = Vec::new();
        while let Some(child) = reader.next_entry().await? {
            children.push(child);
        }
        children.sort_by_key(tokio::fs::DirEntry::file_name);
        for child in children {
            let child_depth = depth + 1;
            if child_depth > MAX_COPY_DEPTH {
                return Err(FileError::CopyLimitExceeded);
            }
            let metadata = tokio::fs::symlink_metadata(child.path()).await?;
            if metadata.file_type().is_symlink() {
                return Err(FileError::Symlink);
            }
            let relative_path = relative_directory.join(child.file_name());
            let (kind, size) = if metadata.is_dir() {
                pending.push_back((child.path(), relative_path.clone(), child_depth));
                (CopyPlanKind::Directory, 0)
            } else if metadata.is_file() {
                copied_bytes = copied_bytes
                    .checked_add(metadata.len())
                    .ok_or(FileError::CopyLimitExceeded)?;
                (CopyPlanKind::File, metadata.len())
            } else {
                return Err(FileError::EntryUnsupported);
            };
            copied_entries += 1;
            if copied_entries > MAX_COPY_ENTRIES || copied_bytes > MAX_COPY_BYTES {
                return Err(FileError::CopyLimitExceeded);
            }
            entries.push(CopyPlanEntry {
                relative_path,
                kind,
                size,
            });
        }
    }
    Ok(CopyPlan {
        entries,
        copied_entries,
        copied_bytes,
    })
}

async fn copy_file_checked(
    source: &Path,
    destination: &Path,
    expected_size: u64,
    create_destination: bool,
    source_scope: &Path,
) -> Result<(), FileError> {
    validate_scoped_source(source, source_scope, false, expected_size).await?;
    let mut source_file = tokio::fs::File::open(source).await?;
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true);
    if create_destination {
        options.create_new(true);
    } else {
        options.truncate(true);
    }
    let mut destination_file = options.open(destination).await.map_err(map_create_error)?;
    let copied = tokio::io::copy(&mut source_file, &mut destination_file).await?;
    destination_file.flush().await?;
    if copied != expected_size {
        return Err(FileError::Changed);
    }
    Ok(())
}
