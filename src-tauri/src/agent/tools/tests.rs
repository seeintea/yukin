use std::path::PathBuf;

use serde_json::json;

use super::{arguments_digest, ExecutionAuthorization, ToolRegistry};
use crate::{
    agent::RuntimeError,
    files::{SelectedDirectories, SelectedFiles},
};

#[test]
fn rejects_unknown_and_invalid_tool_arguments() {
    let runtime = tauri::async_runtime::block_on(ToolRegistry::built_in(PathBuf::new()).execute(
        "current_time",
        &json!({ "utcOffset": "Asia/Shanghai" }),
        ExecutionAuthorization::NotRequired,
    ));
    assert!(matches!(
        runtime,
        Err(RuntimeError::InvalidToolArguments { .. })
    ));

    let unknown = tauri::async_runtime::block_on(ToolRegistry::built_in(PathBuf::new()).execute(
        "missing",
        &json!({}),
        ExecutionAuthorization::NotRequired,
    ));
    assert_eq!(unknown, Err(RuntimeError::ToolNotFound("missing".into())));
}

#[test]
fn executes_current_time_for_valid_offset() {
    let output = tauri::async_runtime::block_on(ToolRegistry::built_in(PathBuf::new()).execute(
        "current_time",
        &json!({ "utcOffset": "+08:00" }),
        ExecutionAuthorization::NotRequired,
    ))
    .expect("current time output");

    assert_eq!(output["utcOffset"], "+08:00");
    assert!(output["dateTime"]
        .as_str()
        .is_some_and(|value| value.ends_with("+08:00")));
}

#[test]
fn file_read_result_summary_excludes_content() {
    let registry = ToolRegistry::built_in(PathBuf::new());
    let summary = registry.result_summary(
        "read_selected_text_file",
        &json!({
            "fileName": "notes.txt",
            "size": 12,
            "content": "private text",
            "read": true
        }),
    );

    assert_eq!(summary["fileName"], "notes.txt");
    assert_eq!(summary["size"], 12);
    assert!(summary.get("content").is_none());
}

#[test]
fn rejects_a_file_reference_that_was_not_attached_to_the_run() {
    let result = tauri::async_runtime::block_on(ToolRegistry::built_in(PathBuf::new()).execute(
        "read_selected_text_file",
        &json!({ "referenceId": "not-authorized" }),
        ExecutionAuthorization::NotRequired,
    ));

    assert_eq!(
        result,
        Err(RuntimeError::File(
            crate::files::FileError::ReferenceInvalid
        ))
    );
}

#[test]
fn rejects_a_directory_reference_that_was_not_attached_to_the_run() {
    let result = tauri::async_runtime::block_on(ToolRegistry::built_in(PathBuf::new()).execute(
        "list_selected_directory",
        &json!({ "referenceId": "not-authorized" }),
        ExecutionAuthorization::NotRequired,
    ));

    assert_eq!(
        result,
        Err(RuntimeError::File(
            crate::files::FileError::ReferenceInvalid
        ))
    );

    assert_eq!(
        ToolRegistry::built_in(PathBuf::new()).validate(
            "create_text_file_in_selected_directory",
            &json!({
                "referenceId": "not-authorized",
                "fileName": "notes.txt",
                "content": "text"
            }),
        ),
        Err(RuntimeError::File(
            crate::files::FileError::ReferenceInvalid
        ))
    );
    assert_eq!(
        ToolRegistry::built_in(PathBuf::new()).validate(
            "create_directory_in_selected_directory",
            &json!({
                "referenceId": "not-authorized",
                "directoryName": "reports"
            }),
        ),
        Err(RuntimeError::File(
            crate::files::FileError::ReferenceInvalid
        ))
    );
    assert_eq!(
        ToolRegistry::built_in(PathBuf::new()).validate(
            "copy_directory_entry",
            &json!({
                "referenceId": "not-authorized",
                "sourceTargetReferenceId": "source",
                "sourceRelativePath": "source.txt",
                "destinationName": "copy.txt"
            }),
        ),
        Err(RuntimeError::File(
            crate::files::FileError::ReferenceInvalid
        ))
    );
}

#[test]
fn rejects_invalid_directory_search_arguments() {
    let result = tauri::async_runtime::block_on(ToolRegistry::built_in(PathBuf::new()).execute(
        "search_selected_directory",
        &json!({ "referenceId": "not-authorized", "query": "  " }),
        ExecutionAuthorization::NotRequired,
    ));

    assert!(matches!(
        result,
        Err(RuntimeError::InvalidToolArguments { .. })
    ));
}

#[tokio::test]
async fn reads_an_authorized_file_without_exposing_content_in_the_summary() {
    let path =
        std::env::temp_dir().join(format!("yukin-read-tool-test-{}.txt", uuid::Uuid::now_v7()));
    tokio::fs::write(&path, "private text")
        .await
        .expect("write selected file");
    let files = SelectedFiles::default();
    let reference = files.register(path.clone()).await.expect("register file");
    let file = files.take(&reference).expect("take reference");
    let registry = ToolRegistry::with_authorizations(PathBuf::new(), vec![file], Vec::new());
    let result = registry
        .execute(
            "read_selected_text_file",
            &json!({ "referenceId": reference.reference_id }),
            ExecutionAuthorization::NotRequired,
        )
        .await
        .expect("read selected file");

    assert_eq!(result["content"], "private text");
    assert!(registry
        .result_summary("read_selected_text_file", &result)
        .get("content")
        .is_none());

    tokio::fs::remove_file(path)
        .await
        .expect("remove selected file");
}

#[tokio::test]
async fn searches_an_authorized_directory_without_exposing_absolute_paths() {
    let path =
        std::env::temp_dir().join(format!("yukin-search-tool-test-{}", uuid::Uuid::now_v7()));
    tokio::fs::create_dir_all(path.join("nested"))
        .await
        .expect("create selected directory");
    tokio::fs::write(path.join("nested/report.txt"), "private text")
        .await
        .expect("write matching file");
    let directories = SelectedDirectories::default();
    let reference = directories
        .register(path.clone())
        .await
        .expect("register directory");
    let directory = directories.take(&reference).expect("take reference");
    let registry = ToolRegistry::with_authorizations(PathBuf::new(), Vec::new(), vec![directory]);
    let result = registry
        .execute(
            "search_selected_directory",
            &json!({
                "referenceId": reference.reference_id,
                "query": "REPORT",
                "kind": "file"
            }),
            ExecutionAuthorization::NotRequired,
        )
        .await
        .expect("search selected directory");

    assert_eq!(result["query"], "REPORT");
    assert_eq!(result["kind"], "file");
    assert_eq!(result["entries"][0]["relativePath"], "nested/report.txt");
    assert!(result["entries"][0]["targetReferenceId"].is_string());
    assert!(!result.to_string().contains(path.to_string_lossy().as_ref()));

    let target_reference_id = result["entries"][0]["targetReferenceId"]
        .as_str()
        .expect("target reference");
    let target_arguments = json!({
        "targetReferenceId": target_reference_id,
        "relativePath": "nested/report.txt"
    });
    let metadata = registry
        .execute(
            "get_directory_entry_metadata",
            &target_arguments,
            ExecutionAuthorization::NotRequired,
        )
        .await
        .expect("read entry metadata");
    assert_eq!(metadata["kind"], "file");
    assert_eq!(metadata["size"], 12);
    assert_eq!(metadata["extension"], "txt");
    assert!(metadata["modifiedAt"].is_string());

    registry
        .validate("open_directory_entry", &target_arguments)
        .expect("valid open target");
    assert_eq!(
        registry.validate(
            "open_directory_entry",
            &json!({
                "targetReferenceId": target_reference_id,
                "relativePath": "misleading-name.txt"
            }),
        ),
        Err(RuntimeError::File(
            crate::files::FileError::EntryReferenceInvalid
        ))
    );
    assert!(matches!(
        registry
            .execute(
                "open_directory_entry",
                &target_arguments,
                ExecutionAuthorization::NotRequired,
            )
            .await,
        Err(RuntimeError::InvalidToolApproval(_))
    ));

    tokio::fs::remove_dir_all(path)
        .await
        .expect("remove selected directory");
}

#[tokio::test]
async fn creates_a_text_file_in_an_authorized_directory_only_after_approval() {
    let path = std::env::temp_dir().join(format!(
        "yukin-create-text-tool-test-{}",
        uuid::Uuid::now_v7()
    ));
    tokio::fs::create_dir(&path)
        .await
        .expect("create selected directory");
    let directories = SelectedDirectories::default();
    let reference = directories
        .register(path.clone())
        .await
        .expect("register directory");
    let directory = directories.take(&reference).expect("take reference");
    let registry = ToolRegistry::with_authorizations(PathBuf::new(), Vec::new(), vec![directory]);
    let arguments = json!({
        "referenceId": reference.reference_id,
        "fileName": "created.txt",
        "content": "private created content"
    });

    assert!(matches!(
        registry
            .execute(
                "create_text_file_in_selected_directory",
                &arguments,
                ExecutionAuthorization::NotRequired,
            )
            .await,
        Err(RuntimeError::InvalidToolApproval(_))
    ));
    assert!(!path.join("created.txt").exists());

    let result = registry
        .execute(
            "create_text_file_in_selected_directory",
            &arguments,
            ExecutionAuthorization::Approved {
                arguments_digest: arguments_digest(&arguments).expect("arguments digest").1,
            },
        )
        .await
        .expect("create approved text file");
    assert_eq!(result["fileName"], "created.txt");
    assert_eq!(result["created"], true);
    assert!(result["targetReferenceId"].is_string());
    assert!(!result.to_string().contains("private created content"));
    assert!(!result.to_string().contains(path.to_string_lossy().as_ref()));
    assert_eq!(
        tokio::fs::read_to_string(path.join("created.txt"))
            .await
            .expect("read created file"),
        "private created content"
    );

    assert_eq!(
        registry
            .execute(
                "create_text_file_in_selected_directory",
                &arguments,
                ExecutionAuthorization::Approved {
                    arguments_digest: arguments_digest(&arguments).expect("arguments digest").1,
                },
            )
            .await,
        Err(RuntimeError::File(crate::files::FileError::AlreadyExists))
    );

    tokio::fs::remove_dir_all(path)
        .await
        .expect("remove selected directory");
}

#[tokio::test]
async fn creates_a_child_directory_only_after_approval() {
    let path = std::env::temp_dir().join(format!(
        "yukin-create-directory-tool-test-{}",
        uuid::Uuid::now_v7()
    ));
    tokio::fs::create_dir(&path)
        .await
        .expect("create selected directory");
    let directories = SelectedDirectories::default();
    let reference = directories
        .register(path.clone())
        .await
        .expect("register directory");
    let directory = directories.take(&reference).expect("take reference");
    let registry = ToolRegistry::with_authorizations(PathBuf::new(), Vec::new(), vec![directory]);
    let arguments = json!({
        "referenceId": reference.reference_id,
        "directoryName": "reports"
    });

    assert!(matches!(
        registry
            .execute(
                "create_directory_in_selected_directory",
                &arguments,
                ExecutionAuthorization::NotRequired,
            )
            .await,
        Err(RuntimeError::InvalidToolApproval(_))
    ));
    assert!(!path.join("reports").exists());

    let result = registry
        .execute(
            "create_directory_in_selected_directory",
            &arguments,
            ExecutionAuthorization::Approved {
                arguments_digest: arguments_digest(&arguments).expect("arguments digest").1,
            },
        )
        .await
        .expect("create approved directory");
    assert_eq!(result["createdDirectoryName"], "reports");
    assert_eq!(result["created"], true);
    assert!(result["targetReferenceId"].is_string());
    assert!(!result.to_string().contains(path.to_string_lossy().as_ref()));
    assert!(path.join("reports").is_dir());

    assert_eq!(
        registry
            .execute(
                "create_directory_in_selected_directory",
                &arguments,
                ExecutionAuthorization::Approved {
                    arguments_digest: arguments_digest(&arguments).expect("arguments digest").1,
                },
            )
            .await,
        Err(RuntimeError::File(crate::files::FileError::AlreadyExists))
    );

    tokio::fs::remove_dir_all(path)
        .await
        .expect("remove selected directory");
}

#[tokio::test]
async fn copies_a_referenced_entry_only_after_approval() {
    let path = std::env::temp_dir().join(format!(
        "yukin-copy-entry-tool-test-{}",
        uuid::Uuid::now_v7()
    ));
    tokio::fs::create_dir_all(path.join("destination"))
        .await
        .expect("create selected directory");
    tokio::fs::write(path.join("source.txt"), "copy content")
        .await
        .expect("write source");
    let directories = SelectedDirectories::default();
    let reference = directories
        .register(path.clone())
        .await
        .expect("register directory");
    let directory = directories.take(&reference).expect("take reference");
    let listing = directory.list().await.expect("list directory");
    let source = listing
        .entries
        .iter()
        .find(|entry| entry.name == "source.txt")
        .expect("source entry");
    let destination = listing
        .entries
        .iter()
        .find(|entry| entry.name == "destination")
        .expect("destination entry");
    let arguments = json!({
        "referenceId": reference.reference_id,
        "sourceTargetReferenceId": source.target_reference_id,
        "sourceRelativePath": "source.txt",
        "destinationDirectoryTargetReferenceId": destination.target_reference_id,
        "destinationDirectoryRelativePath": "destination",
        "destinationName": "copy.txt"
    });
    let registry = ToolRegistry::with_authorizations(PathBuf::new(), Vec::new(), vec![directory]);

    assert!(matches!(
        registry
            .execute(
                "copy_directory_entry",
                &arguments,
                ExecutionAuthorization::NotRequired,
            )
            .await,
        Err(RuntimeError::InvalidToolApproval(_))
    ));
    assert!(!path.join("destination/copy.txt").exists());

    let result = registry
        .execute(
            "copy_directory_entry",
            &arguments,
            ExecutionAuthorization::Approved {
                arguments_digest: arguments_digest(&arguments).expect("arguments digest").1,
            },
        )
        .await
        .expect("copy approved entry");
    assert_eq!(result["relativePath"], "destination/copy.txt");
    assert_eq!(result["kind"], "file");
    assert_eq!(result["copiedEntries"], 1);
    assert!(result["targetReferenceId"].is_string());
    assert!(!result.to_string().contains(path.to_string_lossy().as_ref()));
    assert_eq!(
        tokio::fs::read_to_string(path.join("destination/copy.txt"))
            .await
            .expect("read copied file"),
        "copy content"
    );

    tokio::fs::remove_dir_all(path)
        .await
        .expect("remove selected directory");
}

#[test]
fn rejects_note_paths_outside_managed_directory() {
    let output = tauri::async_runtime::block_on(
        ToolRegistry::built_in(PathBuf::new()).execute(
            "save_text_note",
            &json!({ "fileName": "../note.txt", "content": "unsafe" }),
            ExecutionAuthorization::Approved {
                arguments_digest: arguments_digest(
                    &json!({ "fileName": "../note.txt", "content": "unsafe" }),
                )
                .expect("arguments digest")
                .1,
            },
        ),
    );

    assert!(matches!(
        output,
        Err(RuntimeError::InvalidToolArguments { .. })
    ));
}

#[tokio::test]
async fn creates_note_once_without_overwriting_existing_file() {
    let directory = std::env::temp_dir().join(format!("yukin-tool-test-{}", uuid::Uuid::now_v7()));
    let registry = ToolRegistry::built_in(directory.clone());
    let arguments = json!({ "fileName": "note.txt", "content": "approved content" });
    let digest = arguments_digest(&arguments).expect("arguments digest").1;

    assert!(registry
        .execute(
            "save_text_note",
            &arguments,
            ExecutionAuthorization::NotRequired,
        )
        .await
        .is_err());
    assert!(!directory.join("note.txt").exists());

    registry
        .execute(
            "save_text_note",
            &arguments,
            ExecutionAuthorization::Approved {
                arguments_digest: digest.clone(),
            },
        )
        .await
        .expect("new note");
    assert_eq!(
        std::fs::read_to_string(directory.join("note.txt")).expect("saved note"),
        "approved content"
    );
    assert!(registry
        .execute(
            "save_text_note",
            &arguments,
            ExecutionAuthorization::Approved {
                arguments_digest: digest,
            },
        )
        .await
        .is_err());

    std::fs::remove_dir_all(directory).expect("remove test directory");
}
