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

pub(crate) const MAX_SELECTED_FILE_BYTES: u64 = 16 * 1024;
pub(crate) const MAX_DIRECTORY_ENTRIES: usize = 100;
pub(crate) const MAX_DIRECTORY_SEARCH_DEPTH: usize = 4;
pub(crate) const MAX_DIRECTORY_SEARCH_RESULTS: usize = 50;
pub(crate) const MAX_CREATED_TEXT_FILE_BYTES: usize = 32 * 1024;
pub(crate) const MAX_COPY_ENTRIES: usize = 100;
pub(crate) const MAX_COPY_DEPTH: usize = 8;
pub(crate) const MAX_COPY_BYTES: u64 = 16 * 1024 * 1024;

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
        validate_copy_source(
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
                        validate_copy_source(&source_child, &source_path, true, 0).await?;
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
    validate_copy_source(source, source_scope, false, expected_size).await?;
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

async fn validate_copy_source(
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
mod tests {
    use super::{
        is_sensitive_directory_scope, DirectorySearchKind, FileError, SelectedDirectories,
        SelectedFiles, MAX_COPY_ENTRIES, MAX_CREATED_TEXT_FILE_BYTES, MAX_DIRECTORY_ENTRIES,
        MAX_DIRECTORY_SEARCH_RESULTS, MAX_SELECTED_FILE_BYTES,
    };

    fn test_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("yukin-file-test-{}-{name}", uuid::Uuid::now_v7()))
    }

    #[tokio::test]
    async fn reads_only_unchanged_authorized_utf8_file() {
        let path = test_path("note.txt");
        tokio::fs::write(&path, "安全内容")
            .await
            .expect("write file");
        let files = SelectedFiles::default();
        let reference = files.register(path.clone()).await.expect("register file");
        let file = files.take(&reference).expect("take reference");

        assert_eq!(file.read().await.expect("read file"), "安全内容");
        tokio::fs::write(&path, "已被替换")
            .await
            .expect("replace file");
        assert_eq!(file.read().await, Err(FileError::Changed));

        tokio::fs::remove_file(path).await.expect("remove file");
    }

    #[tokio::test]
    async fn rejects_oversized_and_invalid_utf8_files() {
        let oversized = test_path("large.txt");
        tokio::fs::write(&oversized, vec![b'a'; MAX_SELECTED_FILE_BYTES as usize + 1])
            .await
            .expect("write oversized file");
        let invalid = test_path("invalid.txt");
        tokio::fs::write(&invalid, [0xff, 0xfe])
            .await
            .expect("write invalid file");
        let files = SelectedFiles::default();

        assert_eq!(
            files.register(oversized.clone()).await,
            Err(FileError::TooLarge)
        );
        assert_eq!(
            files.register(invalid.clone()).await,
            Err(FileError::InvalidEncoding)
        );

        tokio::fs::remove_file(oversized)
            .await
            .expect("remove oversized file");
        tokio::fs::remove_file(invalid)
            .await
            .expect("remove invalid file");
    }

    #[tokio::test]
    async fn treats_the_home_root_as_a_sensitive_directory_scope() {
        let Some(home) = dirs::home_dir() else {
            return;
        };
        let home = tokio::fs::canonicalize(home).await.expect("canonical home");
        assert!(is_sensitive_directory_scope(&home).await);
    }

    #[tokio::test]
    async fn lists_only_direct_children_with_a_result_limit() {
        let path = test_path("directory");
        tokio::fs::create_dir(&path)
            .await
            .expect("create directory");
        tokio::fs::create_dir(path.join("nested"))
            .await
            .expect("create nested directory");
        tokio::fs::write(path.join("nested/hidden.txt"), "hidden")
            .await
            .expect("write nested file");
        for index in 0..MAX_DIRECTORY_ENTRIES {
            tokio::fs::write(path.join(format!("file-{index:03}.txt")), "text")
                .await
                .expect("write direct file");
        }
        let directories = SelectedDirectories::default();
        let reference = directories
            .register(path.clone())
            .await
            .expect("register directory");
        let directory = directories.take(&reference).expect("take directory");

        let listing = directory.list().await.expect("list directory");
        assert_eq!(listing.entries.len(), MAX_DIRECTORY_ENTRIES);
        assert!(listing.truncated);
        assert!(!listing
            .entries
            .iter()
            .any(|entry| entry.name == "hidden.txt"));

        tokio::fs::remove_dir_all(path)
            .await
            .expect("remove directory");
    }

    #[tokio::test]
    async fn searches_names_recursively_with_kind_and_relative_paths() {
        let path = test_path("search-directory");
        tokio::fs::create_dir_all(path.join("reports/archive"))
            .await
            .expect("create nested directories");
        tokio::fs::write(path.join("report-summary.txt"), "summary")
            .await
            .expect("write root match");
        tokio::fs::write(path.join("reports/archive/REPORT-2025.md"), "archive")
            .await
            .expect("write nested match");
        tokio::fs::write(path.join("reports/archive/notes.md"), "notes")
            .await
            .expect("write non-match");
        tokio::fs::create_dir_all(path.join("one/two/three/four"))
            .await
            .expect("create depth-limited directories");
        tokio::fs::write(path.join("one/two/three/four/too-deep-report.txt"), "deep")
            .await
            .expect("write depth-limited file");
        let directories = SelectedDirectories::default();
        let reference = directories
            .register(path.clone())
            .await
            .expect("register directory");
        let directory = directories.take(&reference).expect("take directory");

        let search = directory
            .search("report", DirectorySearchKind::File)
            .await
            .expect("search directory");
        let paths = search
            .entries
            .iter()
            .map(|entry| entry.relative_path.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            ["report-summary.txt", "reports/archive/REPORT-2025.md"]
        );
        assert!(!search.truncated);
        assert!(!paths.contains(&"one/two/three/four/too-deep-report.txt"));

        let directory_search = directory
            .search("reports", DirectorySearchKind::Directory)
            .await
            .expect("search directories");
        assert_eq!(directory_search.entries.len(), 1);
        assert_eq!(directory_search.entries[0].relative_path, "reports");

        tokio::fs::remove_dir_all(path)
            .await
            .expect("remove directory");
    }

    #[tokio::test]
    async fn limits_directory_search_results() {
        let path = test_path("limited-search-directory");
        tokio::fs::create_dir(&path)
            .await
            .expect("create directory");
        for index in 0..=MAX_DIRECTORY_SEARCH_RESULTS {
            tokio::fs::write(path.join(format!("match-{index:03}.txt")), "text")
                .await
                .expect("write matching file");
        }
        let directories = SelectedDirectories::default();
        let reference = directories
            .register(path.clone())
            .await
            .expect("register directory");
        let directory = directories.take(&reference).expect("take directory");

        let search = directory
            .search("match", DirectorySearchKind::Any)
            .await
            .expect("search directory");
        assert_eq!(search.entries.len(), MAX_DIRECTORY_SEARCH_RESULTS);
        assert!(search.truncated);

        tokio::fs::remove_dir_all(path)
            .await
            .expect("remove directory");
    }

    #[tokio::test]
    async fn resolves_listed_entry_metadata_through_an_opaque_reference() {
        let path = test_path("metadata-directory");
        tokio::fs::create_dir(&path)
            .await
            .expect("create directory");
        tokio::fs::write(path.join("Report.TXT"), "metadata")
            .await
            .expect("write file");
        let directories = SelectedDirectories::default();
        let directory_reference = directories
            .register(path.clone())
            .await
            .expect("register directory");
        let directory = directories
            .take(&directory_reference)
            .expect("take directory");
        let listing = directory.list().await.expect("list directory");
        let target_reference_id = listing.entries[0]
            .target_reference_id
            .as_deref()
            .expect("entry target reference");

        let metadata = directory
            .entry_metadata(target_reference_id)
            .await
            .expect("entry metadata");
        assert_eq!(metadata.name, "Report.TXT");
        assert_eq!(metadata.relative_path, "Report.TXT");
        assert_eq!(metadata.kind, DirectorySearchKind::File);
        assert_eq!(metadata.size, Some(8));
        assert_eq!(metadata.extension.as_deref(), Some("txt"));
        assert!(metadata.modified_at.is_some());
        assert_eq!(
            directories
                .resolve_entry(target_reference_id)
                .await
                .expect("resolve entry"),
            tokio::fs::canonicalize(path.join("Report.TXT"))
                .await
                .expect("canonical file")
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;

            let outside = test_path("outside.txt");
            tokio::fs::write(&outside, "outside")
                .await
                .expect("write outside file");
            tokio::fs::remove_file(path.join("Report.TXT"))
                .await
                .expect("remove authorized file");
            symlink(&outside, path.join("Report.TXT")).expect("replace entry with symlink");
            assert_eq!(
                directories.resolve_entry(target_reference_id).await,
                Err(FileError::Symlink)
            );
            tokio::fs::remove_file(outside)
                .await
                .expect("remove outside file");
        }

        tokio::fs::remove_dir_all(path)
            .await
            .expect("remove directory");
    }

    #[tokio::test]
    async fn creates_a_new_text_file_without_overwriting_or_escaping_scope() {
        let path = test_path("create-text-directory");
        tokio::fs::create_dir(&path)
            .await
            .expect("create directory");
        let directories = SelectedDirectories::default();
        let directory_reference = directories
            .register(path.clone())
            .await
            .expect("register directory");
        let directory = directories
            .take(&directory_reference)
            .expect("take directory");

        let metadata = directory
            .create_text_file("notes.txt", "安全内容")
            .await
            .expect("create text file");
        assert_eq!(metadata.relative_path, "notes.txt");
        assert_eq!(metadata.size, Some("安全内容".len() as u64));
        assert_eq!(
            tokio::fs::read_to_string(path.join("notes.txt"))
                .await
                .expect("read created file"),
            "安全内容"
        );
        assert_eq!(
            directory.create_text_file("notes.txt", "覆盖内容").await,
            Err(FileError::AlreadyExists)
        );
        assert_eq!(
            tokio::fs::read_to_string(path.join("notes.txt"))
                .await
                .expect("read unchanged file"),
            "安全内容"
        );

        for invalid_name in [
            "../escape.txt",
            "nested/file.txt",
            "nested\\file.txt",
            ".hidden.txt",
            "notes.md",
        ] {
            assert_eq!(
                directory.create_text_file(invalid_name, "text").await,
                Err(FileError::InvalidName)
            );
        }
        assert_eq!(
            directory
                .create_text_file("large.txt", &"a".repeat(MAX_CREATED_TEXT_FILE_BYTES + 1))
                .await,
            Err(FileError::CreatedTextTooLarge)
        );
        tokio::fs::remove_dir_all(path)
            .await
            .expect("remove directory");
    }

    #[tokio::test]
    async fn creates_a_new_directory_without_reusing_or_escaping_scope() {
        let path = test_path("create-directory");
        tokio::fs::create_dir(&path)
            .await
            .expect("create authorized directory");
        let directories = SelectedDirectories::default();
        let directory_reference = directories
            .register(path.clone())
            .await
            .expect("register directory");
        let directory = directories
            .take(&directory_reference)
            .expect("take directory");

        let metadata = directory
            .create_directory("项目资料")
            .await
            .expect("create child directory");
        assert_eq!(metadata.relative_path, "项目资料");
        assert_eq!(metadata.kind, DirectorySearchKind::Directory);
        assert!(path.join("项目资料").is_dir());
        assert_eq!(
            directory.create_directory("项目资料").await,
            Err(FileError::AlreadyExists)
        );

        for invalid_name in ["../escape", "nested/child", "nested\\child", ".hidden", " "] {
            assert_eq!(
                directory.create_directory(invalid_name).await,
                Err(FileError::DirectoryNameInvalid)
            );
        }

        tokio::fs::remove_dir_all(path)
            .await
            .expect("remove directory");
    }

    #[tokio::test]
    async fn copies_files_and_nested_directories_without_overwriting() {
        let path = test_path("copy-entry");
        tokio::fs::create_dir_all(path.join("source/nested"))
            .await
            .expect("create source tree");
        tokio::fs::create_dir(path.join("destination"))
            .await
            .expect("create destination");
        tokio::fs::write(path.join("source/nested/report.txt"), "report")
            .await
            .expect("write nested file");
        tokio::fs::write(path.join("note.txt"), "note")
            .await
            .expect("write source file");
        let directories = SelectedDirectories::default();
        let reference = directories
            .register(path.clone())
            .await
            .expect("register directory");
        let directory = directories.take(&reference).expect("take directory");
        let listing = directory.list().await.expect("list root");
        let source_reference = listing
            .entries
            .iter()
            .find(|entry| entry.name == "source")
            .and_then(|entry| entry.target_reference_id.as_deref())
            .expect("source reference");
        let destination_reference = listing
            .entries
            .iter()
            .find(|entry| entry.name == "destination")
            .and_then(|entry| entry.target_reference_id.as_deref())
            .expect("destination reference");
        let note_reference = listing
            .entries
            .iter()
            .find(|entry| entry.name == "note.txt")
            .and_then(|entry| entry.target_reference_id.as_deref())
            .expect("note reference");

        let directory_result = directory
            .copy_entry(source_reference, Some(destination_reference), "source-copy")
            .await
            .expect("copy directory");
        assert_eq!(directory_result.copied_entries, 3);
        assert_eq!(directory_result.copied_bytes, 6);
        assert_eq!(
            tokio::fs::read_to_string(path.join("destination/source-copy/nested/report.txt"))
                .await
                .expect("read copied file"),
            "report"
        );

        let file_result = directory
            .copy_entry(note_reference, None, "note-copy.txt")
            .await
            .expect("copy file");
        assert_eq!(file_result.copied_entries, 1);
        assert_eq!(file_result.copied_bytes, 4);
        assert_eq!(
            directory
                .copy_entry(note_reference, None, "note-copy.txt")
                .await,
            Err(FileError::AlreadyExists)
        );
        assert_eq!(
            tokio::fs::read_to_string(path.join("note-copy.txt"))
                .await
                .expect("read copied note"),
            "note"
        );

        tokio::fs::remove_dir_all(path)
            .await
            .expect("remove directory");
    }

    #[tokio::test]
    async fn rejects_copying_a_directory_into_its_descendant() {
        let path = test_path("copy-into-source");
        tokio::fs::create_dir_all(path.join("source/nested"))
            .await
            .expect("create source tree");
        let directories = SelectedDirectories::default();
        let reference = directories
            .register(path.clone())
            .await
            .expect("register directory");
        let directory = directories.take(&reference).expect("take directory");
        let search = directory
            .search("", DirectorySearchKind::Directory)
            .await
            .expect("search directories");
        let source_reference = search
            .entries
            .iter()
            .find(|entry| entry.relative_path == "source")
            .map(|entry| entry.target_reference_id.as_str())
            .expect("source reference");
        let nested_reference = search
            .entries
            .iter()
            .find(|entry| entry.relative_path == "source/nested")
            .map(|entry| entry.target_reference_id.as_str())
            .expect("nested reference");

        assert_eq!(
            directory
                .copy_entry(source_reference, Some(nested_reference), "copy")
                .await,
            Err(FileError::CopyIntoSource)
        );
        assert!(!path.join("source/nested/copy").exists());

        tokio::fs::remove_dir_all(path)
            .await
            .expect("remove directory");
    }

    #[tokio::test]
    async fn rejects_copy_plans_over_the_entry_limit_without_creating_a_target() {
        let path = test_path("copy-limit");
        tokio::fs::create_dir_all(path.join("source"))
            .await
            .expect("create source");
        for index in 0..MAX_COPY_ENTRIES {
            tokio::fs::write(path.join(format!("source/{index}.txt")), "")
                .await
                .expect("write source file");
        }
        let directories = SelectedDirectories::default();
        let reference = directories
            .register(path.clone())
            .await
            .expect("register directory");
        let directory = directories.take(&reference).expect("take directory");
        let listing = directory.list().await.expect("list root");
        let source_reference = listing.entries[0]
            .target_reference_id
            .as_deref()
            .expect("source reference");

        assert_eq!(
            directory
                .copy_entry(source_reference, None, "source-copy")
                .await,
            Err(FileError::CopyLimitExceeded)
        );
        assert!(!path.join("source-copy").exists());

        tokio::fs::remove_dir_all(path)
            .await
            .expect("remove directory");
    }
}
