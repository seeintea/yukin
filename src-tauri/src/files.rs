use std::{
    collections::{HashMap, VecDeque},
    fs::Metadata,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::protocol::file::{DirectoryReference, Reference};

mod copy;
mod move_entry;

#[cfg(test)]
use move_entry::MoveRollback;

pub(crate) const MAX_SELECTED_FILE_BYTES: u64 = 16 * 1024;
pub(crate) const MAX_DIRECTORY_ENTRIES: usize = 100;
pub(crate) const MAX_DIRECTORY_SEARCH_DEPTH: usize = 4;
pub(crate) const MAX_DIRECTORY_SEARCH_RESULTS: usize = 50;
pub(crate) const MAX_CREATED_TEXT_FILE_BYTES: usize = 32 * 1024;
pub(crate) const MAX_COPY_ENTRIES: usize = 100;
pub(crate) const MAX_COPY_DEPTH: usize = 8;
pub(crate) const MAX_COPY_BYTES: u64 = 16 * 1024 * 1024;
pub(crate) const MAX_BATCH_MOVE_ENTRIES: usize = 20;

#[derive(Debug, Clone)]
pub(crate) struct AuthorizedFile {
    reference: Reference,
    selected_path: PathBuf,
    canonical_path: PathBuf,
    content_digest: String,
}

#[derive(Debug, Clone)]
pub(crate) struct AuthorizedDirectory {
    reference: DirectoryReference,
    selected_path: PathBuf,
    canonical_path: PathBuf,
    entries: Arc<Mutex<HashMap<String, AuthorizedDirectoryEntry>>>,
}

#[derive(Debug, Clone)]
struct AuthorizedDirectoryEntry {
    reference_id: String,
    directory_reference_id: String,
    selected_root: PathBuf,
    canonical_root: PathBuf,
    selected_path: PathBuf,
    name: String,
    relative_path: String,
}

impl AuthorizedDirectory {
    pub(crate) fn reference(&self) -> &DirectoryReference {
        &self.reference
    }

    pub(crate) async fn list(&self) -> Result<DirectoryListing, FileError> {
        let canonical_path = self.validate_root().await?;
        let mut reader = tokio::fs::read_dir(&canonical_path).await?;
        let mut entries = Vec::new();
        let mut truncated = false;
        while let Some(entry) = reader.next_entry().await? {
            if entries.len() == MAX_DIRECTORY_ENTRIES {
                truncated = true;
                break;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let metadata = tokio::fs::symlink_metadata(entry.path()).await?;
            let (kind, size, target_reference_id) = if metadata.file_type().is_symlink() {
                ("symlink", None, None)
            } else if metadata.is_dir() {
                (
                    "directory",
                    None,
                    Some(self.register_entry(entry.path(), PathBuf::from(&name), name.clone())),
                )
            } else if metadata.is_file() {
                (
                    "file",
                    Some(metadata.len()),
                    Some(self.register_entry(entry.path(), PathBuf::from(&name), name.clone())),
                )
            } else {
                ("other", None, None)
            };
            entries.push(DirectoryEntry {
                name,
                kind,
                size,
                target_reference_id,
            });
        }
        entries.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(DirectoryListing { entries, truncated })
    }

    pub(crate) async fn search(
        &self,
        query: &str,
        kind: DirectorySearchKind,
    ) -> Result<DirectorySearch, FileError> {
        let canonical_path = self.validate_root().await?;
        let normalized_query = query.to_lowercase();
        let mut pending = VecDeque::from([(canonical_path.clone(), PathBuf::new(), 0)]);
        let mut entries = Vec::new();
        let mut truncated = false;

        while let Some((directory_path, relative_directory, depth)) = pending.pop_front() {
            let metadata = tokio::fs::symlink_metadata(&directory_path).await?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                continue;
            }
            let resolved_directory = tokio::fs::canonicalize(&directory_path).await?;
            if !resolved_directory.starts_with(&canonical_path) {
                continue;
            }
            let mut reader = tokio::fs::read_dir(resolved_directory).await?;
            let mut children = Vec::new();
            while let Some(entry) = reader.next_entry().await? {
                children.push(entry);
            }
            children.sort_by_key(tokio::fs::DirEntry::file_name);

            for child in children {
                let metadata = tokio::fs::symlink_metadata(child.path()).await?;
                if metadata.file_type().is_symlink() {
                    continue;
                }
                let name = child.file_name().to_string_lossy().into_owned();
                let relative_path = relative_directory.join(&name);
                let child_depth = depth + 1;
                if child_depth > MAX_DIRECTORY_SEARCH_DEPTH {
                    continue;
                }
                let (entry_kind, size) = if metadata.is_dir() {
                    if child_depth < MAX_DIRECTORY_SEARCH_DEPTH {
                        pending.push_back((child.path(), relative_path.clone(), child_depth));
                    }
                    (DirectorySearchKind::Directory, None)
                } else if metadata.is_file() {
                    (DirectorySearchKind::File, Some(metadata.len()))
                } else {
                    continue;
                };

                if name.to_lowercase().contains(&normalized_query) && kind.matches(entry_kind) {
                    if entries.len() == MAX_DIRECTORY_SEARCH_RESULTS {
                        truncated = true;
                        break;
                    }
                    entries.push(DirectorySearchEntry {
                        target_reference_id: self.register_entry(
                            child.path(),
                            relative_path.clone(),
                            name.clone(),
                        ),
                        name,
                        relative_path: display_relative_path(&relative_path),
                        kind: entry_kind,
                        size,
                    });
                }
            }
            if truncated {
                break;
            }
        }

        entries.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        Ok(DirectorySearch { entries, truncated })
    }

    async fn validate_root(&self) -> Result<PathBuf, FileError> {
        validate_directory_path(&self.selected_path).await?;
        let canonical_path = tokio::fs::canonicalize(&self.selected_path).await?;
        if canonical_path != self.canonical_path {
            return Err(FileError::Changed);
        }
        Ok(canonical_path)
    }

    fn register_entry(
        &self,
        selected_path: PathBuf,
        relative_path: PathBuf,
        name: String,
    ) -> String {
        let relative_path = display_relative_path(&relative_path);
        let mut entries = self.entries.lock().expect("directory entry registry lock");
        if let Some(entry) = entries.values().find(|entry| {
            entry.directory_reference_id == self.reference.reference_id
                && entry.relative_path == relative_path
        }) {
            return entry.reference_id.clone();
        }
        let reference_id = Uuid::now_v7().to_string();
        entries.insert(
            reference_id.clone(),
            AuthorizedDirectoryEntry {
                reference_id: reference_id.clone(),
                directory_reference_id: self.reference.reference_id.clone(),
                selected_root: self.selected_path.clone(),
                canonical_root: self.canonical_path.clone(),
                selected_path,
                name,
                relative_path,
            },
        );
        reference_id
    }

    pub(crate) fn validates_entry_reference(
        &self,
        reference_id: &str,
        relative_path: &str,
    ) -> bool {
        self.entries
            .lock()
            .expect("directory entry registry lock")
            .get(reference_id)
            .is_some_and(|entry| {
                entry.directory_reference_id == self.reference.reference_id
                    && entry.relative_path == relative_path
            })
    }

    pub(crate) async fn entry_metadata(
        &self,
        reference_id: &str,
    ) -> Result<DirectoryEntryMetadata, FileError> {
        let entry = self.entry(reference_id)?;
        entry.metadata().await
    }

    pub(crate) async fn resolve_entry(&self, reference_id: &str) -> Result<PathBuf, FileError> {
        let entry = self.entry(reference_id)?;
        entry.resolve().await.map(|(path, _)| path)
    }

    pub(crate) async fn create_text_file(
        &self,
        file_name: &str,
        content: &str,
    ) -> Result<DirectoryEntryMetadata, FileError> {
        validate_created_text_file(file_name, content)?;
        let canonical_root = self.validate_root().await?;
        let path = canonical_root.join(file_name);
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .await
            .map_err(map_create_error)?;
        let write_result = async {
            file.write_all(content.as_bytes()).await?;
            file.flush().await
        }
        .await;
        if let Err(error) = write_result {
            drop(file);
            let _ = tokio::fs::remove_file(&path).await;
            return Err(FileError::Io(error.to_string()));
        }
        let modified_at = file.metadata().await.ok().and_then(|metadata| {
            metadata.modified().ok().map(|modified| {
                chrono::DateTime::<chrono::Utc>::from(modified)
                    .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
            })
        });
        drop(file);
        let target_reference_id =
            self.register_entry(path, PathBuf::from(file_name), file_name.to_owned());
        Ok(DirectoryEntryMetadata {
            target_reference_id,
            name: file_name.to_owned(),
            relative_path: file_name.to_owned(),
            kind: DirectorySearchKind::File,
            size: Some(content.len() as u64),
            modified_at,
            extension: Some("txt".into()),
        })
    }

    pub(crate) async fn create_directory(
        &self,
        directory_name: &str,
    ) -> Result<DirectoryEntryMetadata, FileError> {
        validate_created_directory(directory_name)?;
        let canonical_root = self.validate_root().await?;
        let path = canonical_root.join(directory_name);
        tokio::fs::create_dir(&path)
            .await
            .map_err(map_create_error)?;
        let modified_at = tokio::fs::metadata(&path).await.ok().and_then(|metadata| {
            metadata.modified().ok().map(|modified| {
                chrono::DateTime::<chrono::Utc>::from(modified)
                    .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
            })
        });
        let target_reference_id = self.register_entry(
            path,
            PathBuf::from(directory_name),
            directory_name.to_owned(),
        );
        Ok(DirectoryEntryMetadata {
            target_reference_id,
            name: directory_name.to_owned(),
            relative_path: directory_name.to_owned(),
            kind: DirectorySearchKind::Directory,
            size: None,
            modified_at,
            extension: None,
        })
    }

    pub(crate) async fn trash_entry(
        &self,
        source_reference_id: &str,
    ) -> Result<DirectoryTrashResult, FileError> {
        self.trash_entry_with(source_reference_id, |path| {
            trash::delete(path).map_err(|_| FileError::Trash)
        })
        .await
    }

    async fn trash_entry_with<F>(
        &self,
        source_reference_id: &str,
        trash_action: F,
    ) -> Result<DirectoryTrashResult, FileError>
    where
        F: FnOnce(PathBuf) -> Result<(), FileError> + Send + 'static,
    {
        self.validate_root().await?;
        let source = self.entry(source_reference_id)?;
        let (source_path, source_metadata) = source.resolve().await?;
        let kind = if source_metadata.is_dir() {
            DirectorySearchKind::Directory
        } else {
            DirectorySearchKind::File
        };
        tauri::async_runtime::spawn_blocking(move || trash_action(source_path))
            .await
            .map_err(|_| FileError::Trash)??;
        self.invalidate_entry_tree(&source.relative_path);
        Ok(DirectoryTrashResult {
            name: source.name,
            relative_path: source.relative_path,
            kind,
        })
    }

    fn invalidate_entry_tree(&self, relative_path: &str) {
        let relative_path = Path::new(relative_path);
        self.entries
            .lock()
            .expect("directory entry registry lock")
            .retain(|_, entry| {
                entry.directory_reference_id != self.reference.reference_id
                    || !Path::new(&entry.relative_path).starts_with(relative_path)
            });
    }

    fn entry(&self, reference_id: &str) -> Result<AuthorizedDirectoryEntry, FileError> {
        self.entries
            .lock()
            .expect("directory entry registry lock")
            .get(reference_id)
            .filter(|entry| entry.directory_reference_id == self.reference.reference_id)
            .cloned()
            .ok_or(FileError::EntryReferenceInvalid)
    }
}

async fn validate_scoped_source(
    path: &Path,
    source_scope: &Path,
    expected_directory: bool,
    expected_size: u64,
) -> Result<(), FileError> {
    let metadata = tokio::fs::symlink_metadata(path).await?;
    let kind_matches = if expected_directory {
        metadata.is_dir()
    } else {
        metadata.is_file() && metadata.len() == expected_size
    };
    if metadata.file_type().is_symlink() || !kind_matches {
        return Err(FileError::Changed);
    }
    let canonical_path = tokio::fs::canonicalize(path).await?;
    if !canonical_path.starts_with(source_scope) {
        return Err(FileError::EntryOutsideScope);
    }
    Ok(())
}

async fn validate_resolved_directory(path: &Path) -> Result<(), FileError> {
    let metadata = tokio::fs::symlink_metadata(path).await?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(FileError::Changed);
    }
    if tokio::fs::canonicalize(path).await? != path {
        return Err(FileError::Changed);
    }
    Ok(())
}

impl AuthorizedDirectoryEntry {
    async fn resolve(&self) -> Result<(PathBuf, Metadata), FileError> {
        validate_directory_path(&self.selected_root).await?;
        let canonical_root = tokio::fs::canonicalize(&self.selected_root).await?;
        if canonical_root != self.canonical_root {
            return Err(FileError::Changed);
        }
        let metadata = tokio::fs::symlink_metadata(&self.selected_path).await?;
        if metadata.file_type().is_symlink() {
            return Err(FileError::Symlink);
        }
        if !metadata.is_file() && !metadata.is_dir() {
            return Err(FileError::EntryUnsupported);
        }
        let canonical_path = tokio::fs::canonicalize(&self.selected_path).await?;
        if !canonical_path.starts_with(&canonical_root) {
            return Err(FileError::EntryOutsideScope);
        }
        Ok((canonical_path, metadata))
    }

    async fn metadata(&self) -> Result<DirectoryEntryMetadata, FileError> {
        let (_, metadata) = self.resolve().await?;
        let kind = if metadata.is_dir() {
            DirectorySearchKind::Directory
        } else {
            DirectorySearchKind::File
        };
        let modified_at = metadata.modified().ok().map(|modified| {
            chrono::DateTime::<chrono::Utc>::from(modified)
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
        });
        let extension = (kind == DirectorySearchKind::File)
            .then(|| {
                Path::new(&self.name)
                    .extension()
                    .and_then(|value| value.to_str())
                    .map(str::to_lowercase)
            })
            .flatten();
        Ok(DirectoryEntryMetadata {
            target_reference_id: self.reference_id.clone(),
            name: self.name.clone(),
            relative_path: self.relative_path.clone(),
            kind,
            size: (kind == DirectorySearchKind::File).then_some(metadata.len()),
            modified_at,
            extension,
        })
    }
}

fn display_relative_path(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => Some(value.to_string_lossy()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

pub(crate) struct DirectoryListing {
    pub(crate) entries: Vec<DirectoryEntry>,
    pub(crate) truncated: bool,
}

pub(crate) struct DirectoryEntry {
    pub(crate) name: String,
    pub(crate) kind: &'static str,
    pub(crate) size: Option<u64>,
    pub(crate) target_reference_id: Option<String>,
}

pub(crate) struct DirectorySearch {
    pub(crate) entries: Vec<DirectorySearchEntry>,
    pub(crate) truncated: bool,
}

pub(crate) struct DirectorySearchEntry {
    pub(crate) target_reference_id: String,
    pub(crate) name: String,
    pub(crate) relative_path: String,
    pub(crate) kind: DirectorySearchKind,
    pub(crate) size: Option<u64>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct DirectoryEntryMetadata {
    pub(crate) target_reference_id: String,
    pub(crate) name: String,
    pub(crate) relative_path: String,
    pub(crate) kind: DirectorySearchKind,
    pub(crate) size: Option<u64>,
    pub(crate) modified_at: Option<String>,
    pub(crate) extension: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct DirectoryCopyResult {
    pub(crate) metadata: DirectoryEntryMetadata,
    pub(crate) copied_entries: usize,
    pub(crate) copied_bytes: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct DirectoryMoveResult {
    pub(crate) previous_relative_path: String,
    pub(crate) metadata: DirectoryEntryMetadata,
}

#[derive(Debug)]
pub(crate) struct BatchMoveEntryRequest {
    pub(crate) source_reference_id: String,
    pub(crate) destination_directory_reference_id: Option<String>,
    pub(crate) destination_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BatchMoveConflictStrategy {
    Fail,
    Skip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BatchMoveItemStatus {
    Moved,
    Skipped,
}

impl BatchMoveItemStatus {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Moved => "moved",
            Self::Skipped => "skipped",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct BatchMoveItemResult {
    pub(crate) previous_relative_path: String,
    pub(crate) relative_path: String,
    pub(crate) kind: DirectorySearchKind,
    pub(crate) status: BatchMoveItemStatus,
    pub(crate) target_reference_id: Option<String>,
    pub(crate) error_code: Option<&'static str>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct BatchMoveResult {
    pub(crate) items: Vec<BatchMoveItemResult>,
    pub(crate) moved: usize,
    pub(crate) skipped: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct DirectoryTrashResult {
    pub(crate) name: String,
    pub(crate) relative_path: String,
    pub(crate) kind: DirectorySearchKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DirectorySearchKind {
    Any,
    File,
    Directory,
}

impl DirectorySearchKind {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Any => "any",
            Self::File => "file",
            Self::Directory => "directory",
        }
    }

    fn matches(self, entry_kind: Self) -> bool {
        self == Self::Any || self == entry_kind
    }
}

impl AuthorizedFile {
    pub(crate) fn reference(&self) -> &Reference {
        &self.reference
    }

    pub(crate) async fn read(&self) -> Result<String, FileError> {
        validate_selected_path(&self.selected_path).await?;
        let canonical_path = tokio::fs::canonicalize(&self.selected_path).await?;
        if canonical_path != self.canonical_path {
            return Err(FileError::Changed);
        }

        let bytes = read_limited(&canonical_path).await?;
        if digest(&bytes) != self.content_digest {
            return Err(FileError::Changed);
        }
        decode_text(bytes)
    }
}

#[derive(Clone, Default)]
pub(crate) struct SelectedDirectories {
    directories: Arc<Mutex<HashMap<String, AuthorizedDirectory>>>,
    entries: Arc<Mutex<HashMap<String, AuthorizedDirectoryEntry>>>,
}

impl SelectedDirectories {
    pub(crate) async fn register(&self, path: PathBuf) -> Result<DirectoryReference, FileError> {
        validate_directory_path(&path).await?;
        let canonical_path = tokio::fs::canonicalize(&path).await?;
        if canonical_path.parent().is_none() {
            return Err(FileError::DirectoryScopeTooBroad);
        }
        if is_sensitive_directory_scope(&canonical_path).await {
            return Err(FileError::DirectoryScopeSensitive);
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.trim().is_empty())
            .ok_or(FileError::InvalidName)?
            .to_owned();
        let reference = DirectoryReference {
            reference_id: Uuid::now_v7().to_string(),
            name,
        };
        let directory = AuthorizedDirectory {
            reference: reference.clone(),
            selected_path: path,
            canonical_path,
            entries: self.entries.clone(),
        };
        let mut directories = self
            .directories
            .lock()
            .expect("selected directory registry lock");
        directories.clear();
        self.entries
            .lock()
            .expect("directory entry registry lock")
            .clear();
        directories.insert(reference.reference_id.clone(), directory);
        Ok(reference)
    }

    pub(crate) fn take(
        &self,
        reference: &DirectoryReference,
    ) -> Result<AuthorizedDirectory, FileError> {
        let directory = self
            .directories
            .lock()
            .expect("selected directory registry lock")
            .remove(&reference.reference_id)
            .ok_or(FileError::ReferenceInvalid)?;
        if directory.reference != *reference {
            return Err(FileError::ReferenceInvalid);
        }
        Ok(directory)
    }

    pub(crate) fn release(&self, reference_id: &str) {
        self.directories
            .lock()
            .expect("selected directory registry lock")
            .remove(reference_id);
        self.entries
            .lock()
            .expect("directory entry registry lock")
            .retain(|_, entry| entry.directory_reference_id != reference_id);
    }

    pub(crate) async fn resolve_entry(&self, reference_id: &str) -> Result<PathBuf, FileError> {
        let entry = self
            .entries
            .lock()
            .expect("directory entry registry lock")
            .get(reference_id)
            .cloned()
            .ok_or(FileError::EntryReferenceInvalid)?;
        entry.resolve().await.map(|(path, _)| path)
    }
}

#[derive(Clone, Default)]
pub(crate) struct SelectedFiles {
    files: Arc<Mutex<HashMap<String, AuthorizedFile>>>,
}

impl SelectedFiles {
    pub(crate) async fn register(&self, path: PathBuf) -> Result<Reference, FileError> {
        validate_selected_path(&path).await?;
        let canonical_path = tokio::fs::canonicalize(&path).await?;
        let bytes = read_limited(&canonical_path).await?;
        decode_text(bytes.clone())?;

        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.trim().is_empty())
            .ok_or(FileError::InvalidName)?
            .to_owned();
        let reference = Reference {
            reference_id: Uuid::now_v7().to_string(),
            name,
            size: bytes.len() as u64,
        };
        let file = AuthorizedFile {
            reference: reference.clone(),
            selected_path: path,
            canonical_path,
            content_digest: digest(&bytes),
        };
        let mut files = self.files.lock().expect("selected file registry lock");
        files.clear();
        files.insert(reference.reference_id.clone(), file);
        Ok(reference)
    }

    pub(crate) fn take(&self, reference: &Reference) -> Result<AuthorizedFile, FileError> {
        let mut files = self.files.lock().expect("selected file registry lock");
        let file = files
            .remove(&reference.reference_id)
            .ok_or(FileError::ReferenceInvalid)?;
        if file.reference != *reference {
            return Err(FileError::ReferenceInvalid);
        }
        Ok(file)
    }

    pub(crate) fn release(&self, reference_id: &str) {
        self.files
            .lock()
            .expect("selected file registry lock")
            .remove(reference_id);
    }
}

async fn validate_selected_path(path: &Path) -> Result<(), FileError> {
    let metadata = tokio::fs::symlink_metadata(path).await?;
    if metadata.file_type().is_symlink() {
        return Err(FileError::Symlink);
    }
    if !metadata.is_file() {
        return Err(FileError::NotRegularFile);
    }
    if metadata.len() > MAX_SELECTED_FILE_BYTES {
        return Err(FileError::TooLarge);
    }
    Ok(())
}

async fn validate_directory_path(path: &Path) -> Result<(), FileError> {
    let metadata = tokio::fs::symlink_metadata(path).await?;
    if metadata.file_type().is_symlink() {
        return Err(FileError::Symlink);
    }
    if !metadata.is_dir() {
        return Err(FileError::NotDirectory);
    }
    Ok(())
}

pub(crate) fn validate_created_text_file(file_name: &str, content: &str) -> Result<(), FileError> {
    let path = Path::new(file_name);
    let valid_name = validate_created_entry_name(file_name)
        && path.extension().and_then(|value| value.to_str()) == Some("txt")
        && path.file_stem().is_some_and(|value| !value.is_empty());
    if !valid_name {
        return Err(FileError::InvalidName);
    }
    if content.len() > MAX_CREATED_TEXT_FILE_BYTES {
        return Err(FileError::CreatedTextTooLarge);
    }
    Ok(())
}

pub(crate) fn validate_created_directory(directory_name: &str) -> Result<(), FileError> {
    if validate_created_entry_name(directory_name) {
        Ok(())
    } else {
        Err(FileError::DirectoryNameInvalid)
    }
}

pub(crate) fn validate_copy_destination_name(name: &str) -> Result<(), FileError> {
    if validate_created_entry_name(name) {
        Ok(())
    } else {
        Err(FileError::CopyDestinationInvalid)
    }
}

pub(crate) fn validate_move_destination_name(name: &str) -> Result<(), FileError> {
    if validate_created_entry_name(name) {
        Ok(())
    } else {
        Err(FileError::MoveDestinationInvalid)
    }
}

fn validate_created_entry_name(name: &str) -> bool {
    let path = Path::new(name);
    !name.is_empty()
        && name.len() <= 128
        && name.trim() == name
        && !name.starts_with('.')
        && !name
            .chars()
            .any(|character| character.is_control() || matches!(character, '/' | '\\'))
        && matches!(
            path.components().collect::<Vec<_>>().as_slice(),
            [std::path::Component::Normal(_)]
        )
}

fn map_create_error(error: std::io::Error) -> FileError {
    if error.kind() == std::io::ErrorKind::AlreadyExists {
        FileError::AlreadyExists
    } else {
        FileError::Io(error.to_string())
    }
}

async fn is_sensitive_directory_scope(path: &Path) -> bool {
    if let Some(home) = dirs::home_dir() {
        if tokio::fs::canonicalize(home)
            .await
            .is_ok_and(|home| home == path)
        {
            return true;
        }
    }

    #[cfg(unix)]
    {
        for sensitive in ["/dev", "/etc", "/proc", "/sys"] {
            if tokio::fs::canonicalize(sensitive)
                .await
                .is_ok_and(|sensitive| sensitive == path)
            {
                return true;
            }
        }
    }
    false
}

async fn read_limited(path: &Path) -> Result<Vec<u8>, FileError> {
    let bytes = tokio::fs::read(path).await?;
    if bytes.len() as u64 > MAX_SELECTED_FILE_BYTES {
        return Err(FileError::TooLarge);
    }
    Ok(bytes)
}

fn decode_text(bytes: Vec<u8>) -> Result<String, FileError> {
    let content = String::from_utf8(bytes).map_err(|_| FileError::InvalidEncoding)?;
    if content
        .chars()
        .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    {
        return Err(FileError::NotText);
    }
    Ok(content)
}

fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum FileError {
    #[error("selected file reference is invalid or has expired")]
    ReferenceInvalid,
    #[error("directory entry reference is invalid or has expired")]
    EntryReferenceInvalid,
    #[error("selected file name is invalid")]
    InvalidName,
    #[error("directory name is invalid")]
    DirectoryNameInvalid,
    #[error("a file or directory with the requested name already exists")]
    AlreadyExists,
    #[error("copy destination name is invalid")]
    CopyDestinationInvalid,
    #[error("copy exceeds the limit of 100 entries, 8 levels, or 16 MiB")]
    CopyLimitExceeded,
    #[error("a directory cannot be copied into itself or one of its descendants")]
    CopyIntoSource,
    #[error("move destination name is invalid")]
    MoveDestinationInvalid,
    #[error("a directory cannot be moved into itself or one of its descendants")]
    MoveIntoSource,
    #[error("moving the entry to the system trash failed")]
    Trash,
    #[error("batch move must contain 1 to 20 independent entries")]
    BatchMoveInvalid,
    #[error("selected path must be a regular file")]
    NotRegularFile,
    #[error("selected path must be a directory")]
    NotDirectory,
    #[error("filesystem roots cannot be authorized as directory scopes")]
    DirectoryScopeTooBroad,
    #[error("home and system configuration roots cannot be authorized as directory scopes")]
    DirectoryScopeSensitive,
    #[error("directory entry is outside the authorized scope")]
    EntryOutsideScope,
    #[error("directory entry type is not supported")]
    EntryUnsupported,
    #[error("symbolic links are not supported")]
    Symlink,
    #[error("selected file exceeds the 16 KiB size limit")]
    TooLarge,
    #[error("text file content exceeds the 32 KiB size limit")]
    CreatedTextTooLarge,
    #[error("selected file is not valid UTF-8")]
    InvalidEncoding,
    #[error("selected file contains non-text control characters")]
    NotText,
    #[error("selected file changed after it was authorized")]
    Changed,
    #[error("system file action failed: {0}")]
    SystemAction(String),
    #[error("file system access failed: {0}")]
    Io(String),
}

impl FileError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::ReferenceInvalid => "file_reference_invalid",
            Self::EntryReferenceInvalid => "directory_entry_reference_invalid",
            Self::InvalidName => "file_name_invalid",
            Self::DirectoryNameInvalid => "directory_name_invalid",
            Self::AlreadyExists => "file_already_exists",
            Self::CopyDestinationInvalid => "file_copy_destination_invalid",
            Self::CopyLimitExceeded => "file_copy_limit_exceeded",
            Self::CopyIntoSource => "file_copy_into_source",
            Self::MoveDestinationInvalid => "file_move_destination_invalid",
            Self::MoveIntoSource => "file_move_into_source",
            Self::Trash => "file_trash",
            Self::BatchMoveInvalid => "file_batch_move_invalid",
            Self::NotRegularFile => "file_not_regular",
            Self::NotDirectory => "directory_not_found",
            Self::DirectoryScopeTooBroad => "directory_scope_too_broad",
            Self::DirectoryScopeSensitive => "directory_scope_sensitive",
            Self::EntryOutsideScope => "directory_entry_outside_scope",
            Self::EntryUnsupported => "directory_entry_unsupported",
            Self::Symlink => "file_symlink_unsupported",
            Self::TooLarge => "file_too_large",
            Self::CreatedTextTooLarge => "file_content_too_large",
            Self::InvalidEncoding => "file_encoding_invalid",
            Self::NotText => "file_not_text",
            Self::Changed => "file_changed",
            Self::SystemAction(_) => "file_system_action",
            Self::Io(_) => "file_io",
        }
    }
}

impl From<std::io::Error> for FileError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

#[cfg(test)]
mod tests;
