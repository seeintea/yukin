use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};

use crate::{AppError, AppResult};

const MAX_PACKAGE_ENTRIES: usize = 1_024;
const MAX_PACKAGE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_METADATA_BYTES: u64 = 256 * 1024;

pub(crate) fn copy_directory(source: &Path, destination: &Path) -> AppResult<()> {
    if !source.is_dir() {
        return Err(AppError::Validation(
            "selected path is not a directory".into(),
        ));
    }
    fs::create_dir_all(destination)?;
    let mut totals = PackageTotals::default();
    copy_children(source, destination, &mut totals)
}

fn copy_children(source: &Path, destination: &Path, totals: &mut PackageTotals) -> AppResult<()> {
    let mut entries = fs::read_dir(source)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        totals.add_entry()?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.file_type().is_symlink() {
            return Err(AppError::Validation(
                "packages containing symbolic links are not supported".into(),
            ));
        }
        let destination_path = destination.join(entry.file_name());
        if metadata.is_dir() {
            fs::create_dir(&destination_path)?;
            copy_children(&entry.path(), &destination_path, totals)?;
        } else if metadata.is_file() {
            totals.add_bytes(metadata.len())?;
            fs::copy(entry.path(), destination_path)?;
        } else {
            return Err(AppError::Validation(
                "packages may contain only files and directories".into(),
            ));
        }
    }
    Ok(())
}

pub(crate) fn extract_zip(source: &Path, destination: &Path) -> AppResult<()> {
    let file = File::open(source)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|error| AppError::Validation(format!("invalid ZIP archive: {error}")))?;
    if archive.len() > MAX_PACKAGE_ENTRIES {
        return Err(AppError::Validation(
            "package contains too many entries".into(),
        ));
    }
    fs::create_dir_all(destination)?;
    let mut total_bytes = 0_u64;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| AppError::Validation(format!("invalid ZIP entry: {error}")))?;
        let relative_path = entry
            .enclosed_name()
            .ok_or_else(|| AppError::Validation("package contains an unsafe path".into()))?
            .to_path_buf();
        if relative_path.as_os_str().is_empty() {
            continue;
        }
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170_000 == 0o120_000)
        {
            return Err(AppError::Validation(
                "packages containing symbolic links are not supported".into(),
            ));
        }
        total_bytes = total_bytes
            .checked_add(entry.size())
            .ok_or_else(|| AppError::Validation("package is too large".into()))?;
        if total_bytes > MAX_PACKAGE_BYTES {
            return Err(AppError::Validation("package is too large".into()));
        }
        let output_path = destination.join(relative_path);
        if entry.is_dir() {
            fs::create_dir_all(output_path)?;
            continue;
        }
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output_path)?;
        std::io::copy(&mut entry, &mut output)?;
        output.flush()?;
    }
    Ok(())
}

pub(crate) fn find_unique_file(root: &Path, file_name: &str) -> AppResult<PathBuf> {
    let mut matches = Vec::new();
    find_named_files(root, file_name, &mut matches)?;
    match matches.as_slice() {
        [path] => Ok(path.clone()),
        [] => Err(AppError::Validation(format!(
            "package does not contain {file_name}"
        ))),
        _ => Err(AppError::Validation(format!(
            "package contains multiple {file_name} files"
        ))),
    }
}

fn find_named_files(root: &Path, file_name: &str, matches: &mut Vec<PathBuf>) -> AppResult<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.file_type().is_symlink() {
            return Err(AppError::Validation(
                "packages containing symbolic links are not supported".into(),
            ));
        }
        if metadata.is_dir() {
            find_named_files(&entry.path(), file_name, matches)?;
        } else if metadata.is_file() && entry.file_name() == file_name {
            matches.push(entry.path());
        }
    }
    Ok(())
}

pub(crate) fn read_metadata(path: &Path) -> AppResult<String> {
    let metadata = fs::metadata(path)?;
    if metadata.len() > MAX_METADATA_BYTES {
        return Err(AppError::Validation(
            "package metadata file is too large".into(),
        ));
    }
    fs::read_to_string(path).map_err(Into::into)
}

pub(crate) fn digest_directory(root: &Path) -> AppResult<String> {
    let mut files = Vec::new();
    collect_files(root, root, &mut files)?;
    files.sort_by(|left, right| left.0.cmp(&right.0));
    let mut hasher = Sha256::new();
    for (relative_path, path) in files {
        hasher.update(relative_path.to_string_lossy().as_bytes());
        let mut file = File::open(path)?;
        let mut buffer = [0_u8; 16 * 1024];
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
    }
    Ok(hex::encode(hasher.finalize()))
}

fn collect_files(
    root: &Path,
    current: &Path,
    files: &mut Vec<(PathBuf, PathBuf)>,
) -> AppResult<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.is_dir() {
            collect_files(root, &entry.path(), files)?;
        } else if metadata.is_file() {
            let relative_path = entry
                .path()
                .strip_prefix(root)
                .map_err(|_| AppError::Validation("package path escaped its root".into()))?
                .to_path_buf();
            files.push((relative_path, entry.path()));
        }
    }
    Ok(())
}

#[derive(Default)]
struct PackageTotals {
    entries: usize,
    bytes: u64,
}

impl PackageTotals {
    fn add_entry(&mut self) -> AppResult<()> {
        self.entries += 1;
        if self.entries > MAX_PACKAGE_ENTRIES {
            return Err(AppError::Validation(
                "package contains too many entries".into(),
            ));
        }
        Ok(())
    }

    fn add_bytes(&mut self, bytes: u64) -> AppResult<()> {
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or_else(|| AppError::Validation("package is too large".into()))?;
        if self.bytes > MAX_PACKAGE_BYTES {
            return Err(AppError::Validation("package is too large".into()));
        }
        Ok(())
    }
}
