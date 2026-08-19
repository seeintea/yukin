use std::path::PathBuf;

use serde_json::json;

use super::{arguments_digest, ExecutionAuthorization, ToolRegistry};
use crate::agent::RuntimeError;

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
