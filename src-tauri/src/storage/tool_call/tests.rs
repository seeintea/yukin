use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

use crate::{
    protocol::agent_run::{ToolCallDecision, ToolCallStatus},
    storage::model_response::{self, StartParams},
};

use super::{
    complete, create, decide, expire, list, mark_running, wait_for_approval, CreateParams,
};

async fn setup(run_id: &str) -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("in-memory database");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrations");
    sqlx::query(
        r#"
        INSERT INTO model_providers (
            id, provider_key, api_format, base_url, provider_alias, api_key_alias
        ) VALUES (
            'provider-1', 'deepseek', 'openai', 'https://example.com', 'default', 'key-1'
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("provider");
    sqlx::query("INSERT INTO conversations (id, title) VALUES ('conversation-1', '新对话')")
        .execute(&pool)
        .await
        .expect("conversation");
    model_response::start(
        &pool,
        StartParams {
            conversation_id: "conversation-1".into(),
            run_id: run_id.into(),
            user_message_id: format!("{run_id}-user"),
            assistant_message_id: format!("{run_id}-assistant"),
            provider_id: "provider-1".into(),
            model_id: "model-1".into(),
            reasoning_effort: None,
            content: "save a note".into(),
            skills: Vec::new(),
            attachments: Vec::new(),
        },
    )
    .await
    .expect("start run");
    model_response::mark_started(&pool, run_id)
        .await
        .expect("running run");
    pool
}

async fn create_write_call(pool: &SqlitePool, run_id: &str, tool_call_id: &str) {
    create(
        pool,
        CreateParams {
            id: tool_call_id,
            run_id,
            name: "save_text_note",
            arguments_json: r#"{"content":"hello","fileName":"note.txt"}"#,
            arguments_digest: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            risk_level: "write",
            approval_policy: "always",
        },
    )
    .await
    .expect("tool call");
}

#[tokio::test]
async fn binds_approval_to_run_tool_and_arguments_before_execution() {
    let pool = setup("run-approval").await;
    create_write_call(&pool, "run-approval", "tool-1").await;
    assert!(mark_running(&pool, "run-approval", "tool-1").await.is_err());
    wait_for_approval(&pool, "run-approval", "tool-1", "2999-01-01T00:00:00.000Z")
        .await
        .expect("waiting approval");

    let wrong_digest = decide(
        &pool,
        "run-approval",
        "tool-1",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        ToolCallDecision::Allow,
    )
    .await;
    assert!(wrong_digest.is_err());
    assert_eq!(
        list(&pool, "run-approval").await.expect("tool calls")[0].status,
        ToolCallStatus::WaitingApproval
    );

    decide(
        &pool,
        "run-approval",
        "tool-1",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ToolCallDecision::Allow,
    )
    .await
    .expect("allow tool");
    mark_running(&pool, "run-approval", "tool-1")
        .await
        .expect("run tool");
    complete(&pool, "tool-1", r#"{"saved":true}"#)
        .await
        .expect("complete tool");

    let tool_call = &list(&pool, "run-approval").await.expect("tool calls")[0];
    assert_eq!(tool_call.status, ToolCallStatus::Completed);
    assert_eq!(tool_call.result.as_ref().expect("result")["saved"], true);
}

#[tokio::test]
async fn persists_rejected_approval_without_starting_tool() {
    let pool = setup("run-rejected").await;
    create_write_call(&pool, "run-rejected", "tool-2").await;
    wait_for_approval(&pool, "run-rejected", "tool-2", "2999-01-01T00:00:00.000Z")
        .await
        .expect("waiting approval");
    decide(
        &pool,
        "run-rejected",
        "tool-2",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ToolCallDecision::Reject,
    )
    .await
    .expect("reject tool");

    let tool_call = &list(&pool, "run-rejected").await.expect("tool calls")[0];
    assert_eq!(tool_call.status, ToolCallStatus::Rejected);
    assert!(mark_running(&pool, "run-rejected", "tool-2").await.is_err());
}

#[tokio::test]
async fn expires_approval_without_starting_tool() {
    let pool = setup("run-expired").await;
    create_write_call(&pool, "run-expired", "tool-3").await;
    wait_for_approval(&pool, "run-expired", "tool-3", "2000-01-01T00:00:00.000Z")
        .await
        .expect("waiting approval");

    assert!(decide(
        &pool,
        "run-expired",
        "tool-3",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ToolCallDecision::Allow,
    )
    .await
    .is_err());
    expire(&pool, "run-expired", "tool-3")
        .await
        .expect("expire approval");

    let tool_call = &list(&pool, "run-expired").await.expect("tool calls")[0];
    assert_eq!(tool_call.status, ToolCallStatus::Failed);
    assert_eq!(
        tool_call.error_code.as_deref(),
        Some("tool_approval_expired")
    );
}

#[tokio::test]
async fn cancellation_marks_waiting_tool_call_cancelled() {
    let pool = setup("run-cancelled").await;
    create_write_call(&pool, "run-cancelled", "tool-4").await;
    wait_for_approval(&pool, "run-cancelled", "tool-4", "2999-01-01T00:00:00.000Z")
        .await
        .expect("waiting approval");

    model_response::cancel(&pool, "run-cancelled", "run-cancelled-assistant", "")
        .await
        .expect("cancel run");

    assert_eq!(
        list(&pool, "run-cancelled").await.expect("tool calls")[0].status,
        ToolCallStatus::Cancelled
    );
}
