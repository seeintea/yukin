use super::{
    is_sensitive_directory_scope, BatchMoveConflictStrategy, BatchMoveEntryRequest,
    BatchMoveItemStatus, DirectorySearchKind, FileError, MoveRollback, SelectedDirectories,
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
    assert_eq!(
        directory
            .move_entry(source_reference, Some(nested_reference), "moved")
            .await,
        Err(FileError::MoveIntoSource)
    );
    assert!(!path.join("source/nested/moved").exists());

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

#[tokio::test]
async fn moves_and_renames_entries_while_invalidating_old_references() {
    let path = test_path("move-entry");
    tokio::fs::create_dir_all(path.join("source/nested"))
        .await
        .expect("create source tree");
    tokio::fs::create_dir(path.join("destination"))
        .await
        .expect("create destination");
    tokio::fs::write(path.join("source/nested/report.txt"), "report")
        .await
        .expect("write nested file");
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
    let search = directory
        .search("report", DirectorySearchKind::File)
        .await
        .expect("search nested file");
    let nested_file_reference = &search.entries[0].target_reference_id;

    let result = directory
        .move_entry(
            source_reference,
            Some(destination_reference),
            "renamed-source",
        )
        .await
        .expect("move directory");
    assert_eq!(result.previous_relative_path, "source");
    assert_eq!(result.metadata.relative_path, "destination/renamed-source");
    assert_eq!(result.metadata.kind, DirectorySearchKind::Directory);
    assert!(!path.join("source").exists());
    assert_eq!(
        tokio::fs::read_to_string(path.join("destination/renamed-source/nested/report.txt"))
            .await
            .expect("read moved file"),
        "report"
    );
    assert_eq!(
        directory.entry_metadata(source_reference).await,
        Err(FileError::EntryReferenceInvalid)
    );
    assert_eq!(
        directory.entry_metadata(nested_file_reference).await,
        Err(FileError::EntryReferenceInvalid)
    );
    assert!(directory
        .entry_metadata(&result.metadata.target_reference_id)
        .await
        .is_ok());

    tokio::fs::remove_dir_all(path)
        .await
        .expect("remove directory");
}

#[tokio::test]
async fn refuses_to_overwrite_when_moving_or_renaming() {
    let path = test_path("move-conflict");
    tokio::fs::create_dir_all(&path)
        .await
        .expect("create directory");
    tokio::fs::write(path.join("source.txt"), "source")
        .await
        .expect("write source");
    tokio::fs::write(path.join("existing.txt"), "existing")
        .await
        .expect("write existing");
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
        .find(|entry| entry.name == "source.txt")
        .and_then(|entry| entry.target_reference_id.as_deref())
        .expect("source reference");

    assert_eq!(
        directory
            .move_entry(source_reference, None, "existing.txt")
            .await,
        Err(FileError::AlreadyExists)
    );
    assert_eq!(
        tokio::fs::read_to_string(path.join("source.txt"))
            .await
            .expect("read source"),
        "source"
    );
    assert_eq!(
        tokio::fs::read_to_string(path.join("existing.txt"))
            .await
            .expect("read existing"),
        "existing"
    );
    assert_eq!(
        directory
            .move_entry(source_reference, None, "../escape.txt")
            .await,
        Err(FileError::MoveDestinationInvalid)
    );

    tokio::fs::remove_dir_all(path)
        .await
        .expect("remove directory");
}

#[tokio::test]
async fn trashes_an_entry_and_invalidates_its_reference_tree() {
    let path = test_path("trash-entry");
    tokio::fs::create_dir_all(path.join("source/nested"))
        .await
        .expect("create source tree");
    tokio::fs::write(path.join("source/nested/report.txt"), "report")
        .await
        .expect("write nested file");
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
    let search = directory
        .search("report", DirectorySearchKind::File)
        .await
        .expect("search nested file");
    let nested_file_reference = &search.entries[0].target_reference_id;
    let fake_trash_path = path.join("fake-trash");

    let result = directory
        .trash_entry_with(source_reference, {
            let fake_trash_path = fake_trash_path.clone();
            move |source_path| {
                std::fs::rename(source_path, fake_trash_path).map_err(FileError::from)
            }
        })
        .await
        .expect("trash directory");
    assert_eq!(result.name, "source");
    assert_eq!(result.relative_path, "source");
    assert_eq!(result.kind, DirectorySearchKind::Directory);
    assert!(!path.join("source").exists());
    assert!(fake_trash_path.join("nested/report.txt").is_file());
    assert_eq!(
        directory.entry_metadata(source_reference).await,
        Err(FileError::EntryReferenceInvalid)
    );
    assert_eq!(
        directory.entry_metadata(nested_file_reference).await,
        Err(FileError::EntryReferenceInvalid)
    );

    tokio::fs::remove_dir_all(path)
        .await
        .expect("remove directory");
}

#[tokio::test]
async fn keeps_the_entry_authorized_when_trashing_fails() {
    let path = test_path("trash-failure");
    tokio::fs::create_dir_all(&path)
        .await
        .expect("create directory");
    tokio::fs::write(path.join("source.txt"), "source")
        .await
        .expect("write source");
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
            .trash_entry_with(source_reference, |_| { Err(FileError::Trash) })
            .await,
        Err(FileError::Trash)
    );
    assert!(path.join("source.txt").is_file());
    assert!(directory.entry_metadata(source_reference).await.is_ok());

    tokio::fs::remove_dir_all(path)
        .await
        .expect("remove directory");
}

#[tokio::test]
async fn batch_moves_independent_entries_and_skips_conflicts() {
    let path = test_path("batch-move");
    tokio::fs::create_dir_all(path.join("destination"))
        .await
        .expect("create destination");
    tokio::fs::write(path.join("alpha.txt"), "alpha")
        .await
        .expect("write alpha");
    tokio::fs::write(path.join("beta.txt"), "beta")
        .await
        .expect("write beta");
    tokio::fs::write(path.join("destination/beta.txt"), "existing")
        .await
        .expect("write conflict");
    let directories = SelectedDirectories::default();
    let reference = directories
        .register(path.clone())
        .await
        .expect("register directory");
    let directory = directories.take(&reference).expect("take directory");
    let listing = directory.list().await.expect("list root");
    let entry_reference = |name: &str| {
        listing
            .entries
            .iter()
            .find(|entry| entry.name == name)
            .and_then(|entry| entry.target_reference_id.clone())
            .expect("entry reference")
    };
    let destination_reference = entry_reference("destination");
    let alpha_reference = entry_reference("alpha.txt");
    let beta_reference = entry_reference("beta.txt");
    let requests = [
        BatchMoveEntryRequest {
            source_reference_id: alpha_reference.clone(),
            destination_directory_reference_id: Some(destination_reference.clone()),
            destination_name: "alpha.txt".into(),
        },
        BatchMoveEntryRequest {
            source_reference_id: beta_reference.clone(),
            destination_directory_reference_id: Some(destination_reference),
            destination_name: "beta.txt".into(),
        },
    ];

    let result = directory
        .move_entries(&requests, BatchMoveConflictStrategy::Skip)
        .await
        .expect("batch move");
    assert_eq!(result.moved, 1);
    assert_eq!(result.skipped, 1);
    assert_eq!(result.items[0].status, BatchMoveItemStatus::Moved);
    assert_eq!(result.items[1].status, BatchMoveItemStatus::Skipped);
    assert!(!path.join("alpha.txt").exists());
    assert_eq!(
        tokio::fs::read_to_string(path.join("destination/alpha.txt"))
            .await
            .expect("read moved alpha"),
        "alpha"
    );
    assert_eq!(
        tokio::fs::read_to_string(path.join("destination/beta.txt"))
            .await
            .expect("read existing beta"),
        "existing"
    );
    assert!(path.join("beta.txt").is_file());
    assert_eq!(
        directory.entry_metadata(&alpha_reference).await,
        Err(FileError::EntryReferenceInvalid)
    );
    assert!(directory.entry_metadata(&beta_reference).await.is_ok());

    tokio::fs::remove_dir_all(path)
        .await
        .expect("remove directory");
}

#[tokio::test]
async fn batch_move_fail_strategy_keeps_all_sources_unchanged() {
    let path = test_path("batch-move-fail");
    tokio::fs::create_dir_all(path.join("destination"))
        .await
        .expect("create destination");
    tokio::fs::write(path.join("alpha.txt"), "alpha")
        .await
        .expect("write alpha");
    tokio::fs::write(path.join("beta.txt"), "beta")
        .await
        .expect("write beta");
    tokio::fs::write(path.join("destination/beta.txt"), "existing")
        .await
        .expect("write conflict");
    let directories = SelectedDirectories::default();
    let reference = directories
        .register(path.clone())
        .await
        .expect("register directory");
    let directory = directories.take(&reference).expect("take directory");
    let listing = directory.list().await.expect("list root");
    let entry_reference = |name: &str| {
        listing
            .entries
            .iter()
            .find(|entry| entry.name == name)
            .and_then(|entry| entry.target_reference_id.clone())
            .expect("entry reference")
    };
    let destination_reference = entry_reference("destination");
    let requests = [
        BatchMoveEntryRequest {
            source_reference_id: entry_reference("alpha.txt"),
            destination_directory_reference_id: Some(destination_reference.clone()),
            destination_name: "alpha.txt".into(),
        },
        BatchMoveEntryRequest {
            source_reference_id: entry_reference("beta.txt"),
            destination_directory_reference_id: Some(destination_reference),
            destination_name: "beta.txt".into(),
        },
    ];

    assert_eq!(
        directory
            .move_entries(&requests, BatchMoveConflictStrategy::Fail)
            .await,
        Err(FileError::AlreadyExists)
    );
    assert!(path.join("alpha.txt").is_file());
    assert!(path.join("beta.txt").is_file());
    assert!(!path.join("destination/alpha.txt").exists());

    tokio::fs::remove_dir_all(path)
        .await
        .expect("remove directory");
}

#[test]
fn batch_move_rollback_restores_completed_moves_in_reverse_order() {
    let path = test_path("batch-move-rollback");
    std::fs::create_dir_all(&path).expect("create directory");
    let alpha = path.join("alpha.txt");
    let beta = path.join("beta.txt");
    let moved_alpha = path.join("moved-alpha.txt");
    let moved_beta = path.join("moved-beta.txt");
    std::fs::write(&alpha, "alpha").expect("write alpha");
    std::fs::write(&beta, "beta").expect("write beta");

    {
        let mut rollback = MoveRollback::default();
        std::fs::rename(&alpha, &moved_alpha).expect("move alpha");
        rollback.push(alpha.clone(), moved_alpha.clone());
        std::fs::rename(&beta, &moved_beta).expect("move beta");
        rollback.push(beta.clone(), moved_beta.clone());
    }

    assert!(alpha.is_file());
    assert!(beta.is_file());
    assert!(!moved_alpha.exists());
    assert!(!moved_beta.exists());
    std::fs::remove_dir_all(path).expect("remove directory");
}
