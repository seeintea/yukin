use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::protocol::file::{DirectoryReference, Reference};

pub(crate) const MAX_SELECTED_FILE_BYTES: u64 = 16 * 1024;
pub(crate) const MAX_DIRECTORY_ENTRIES: usize = 100;

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
}

impl AuthorizedDirectory {
    pub(crate) fn reference(&self) -> &DirectoryReference {
        &self.reference
    }

    pub(crate) async fn list(&self) -> Result<DirectoryListing, FileError> {
        validate_directory_path(&self.selected_path).await?;
        let canonical_path = tokio::fs::canonicalize(&self.selected_path).await?;
        if canonical_path != self.canonical_path {
            return Err(FileError::Changed);
        }
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
            let (kind, size) = if metadata.file_type().is_symlink() {
                ("symlink", None)
            } else if metadata.is_dir() {
                ("directory", None)
            } else if metadata.is_file() {
                ("file", Some(metadata.len()))
            } else {
                ("other", None)
            };
            entries.push(DirectoryEntry { name, kind, size });
        }
        entries.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(DirectoryListing { entries, truncated })
    }
}

pub(crate) struct DirectoryListing {
    pub(crate) entries: Vec<DirectoryEntry>,
    pub(crate) truncated: bool,
}

pub(crate) struct DirectoryEntry {
    pub(crate) name: String,
    pub(crate) kind: &'static str,
    pub(crate) size: Option<u64>,
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
}

impl SelectedDirectories {
    pub(crate) async fn register(&self, path: PathBuf) -> Result<DirectoryReference, FileError> {
        validate_directory_path(&path).await?;
        let canonical_path = tokio::fs::canonicalize(&path).await?;
        if canonical_path.parent().is_none() {
            return Err(FileError::DirectoryScopeTooBroad);
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
        };
        let mut directories = self
            .directories
            .lock()
            .expect("selected directory registry lock");
        directories.clear();
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
    #[error("selected file name is invalid")]
    InvalidName,
    #[error("selected path must be a regular file")]
    NotRegularFile,
    #[error("selected path must be a directory")]
    NotDirectory,
    #[error("filesystem roots cannot be authorized as directory scopes")]
    DirectoryScopeTooBroad,
    #[error("symbolic links are not supported")]
    Symlink,
    #[error("selected file exceeds the 16 KiB size limit")]
    TooLarge,
    #[error("selected file is not valid UTF-8")]
    InvalidEncoding,
    #[error("selected file contains non-text control characters")]
    NotText,
    #[error("selected file changed after it was authorized")]
    Changed,
    #[error("file system access failed: {0}")]
    Io(String),
}

impl FileError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::ReferenceInvalid => "file_reference_invalid",
            Self::InvalidName => "file_name_invalid",
            Self::NotRegularFile => "file_not_regular",
            Self::NotDirectory => "directory_not_found",
            Self::DirectoryScopeTooBroad => "directory_scope_too_broad",
            Self::Symlink => "file_symlink_unsupported",
            Self::TooLarge => "file_too_large",
            Self::InvalidEncoding => "file_encoding_invalid",
            Self::NotText => "file_not_text",
            Self::Changed => "file_changed",
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
        FileError, SelectedDirectories, SelectedFiles, MAX_DIRECTORY_ENTRIES,
        MAX_SELECTED_FILE_BYTES,
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
}
