use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

use crate::{agent::TokenUsage, protocol::agent_run::RunStatus, AppError};

use super::{
    cancel, complete, fail_panicked, mark_started, recover_interrupted, snapshot, start,
    update_partial, RunSkill, StartParams,
};

async fn setup() -> SqlitePool {
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
    pool
}

fn start_params(run_id: &str) -> StartParams {
    StartParams {
        conversation_id: "conversation-1".into(),
        run_id: run_id.into(),
        user_message_id: format!("{run_id}-user"),
        assistant_message_id: format!("{run_id}-assistant"),
        provider_id: "provider-1".into(),
        model_id: "model-1".into(),
        reasoning_effort: None,
        content: "第一个问题".into(),
        skills: Vec::new(),
    }
}

#[tokio::test]
async fn persists_run_messages_partial_output_and_completion() {
    let pool = setup().await;
    start(&pool, start_params("run-1"))
        .await
        .expect("start run");

    let pending = snapshot(&pool, "run-1").await.expect("pending snapshot");
    assert_eq!(pending.run.status, RunStatus::Pending);
    assert_eq!(pending.assistant_message.run_id.as_deref(), Some("run-1"));

    mark_started(&pool, "run-1").await.expect("mark started");
    update_partial(&pool, "run-1-assistant", "部分回复")
        .await
        .expect("partial output");
    complete(
        &pool,
        "run-1",
        "run-1-assistant",
        "完整回复",
        Some(TokenUsage {
            prompt_tokens: 2,
            completion_tokens: 3,
            total_tokens: 5,
        }),
    )
    .await
    .expect("complete run");

    let completed = snapshot(&pool, "run-1").await.expect("completed snapshot");
    assert_eq!(completed.run.status, RunStatus::Completed);
    assert_eq!(completed.run.total_tokens, Some(5));
    assert_eq!(completed.assistant_message.content, "完整回复");
    assert!(matches!(
        mark_started(&pool, "run-1").await,
        Err(AppError::RunState(_))
    ));
}

#[tokio::test]
async fn persists_selected_skill_version() {
    let pool = setup().await;
    let mut params = start_params("run-skill");
    params.skills = vec![RunSkill {
        id: "time_assistant".into(),
        version: "1".into(),
    }];

    start(&pool, params).await.expect("start run with skill");

    let snapshot = snapshot(&pool, "run-skill").await.expect("snapshot");
    assert_eq!(snapshot.run.skills.len(), 1);
    assert_eq!(snapshot.run.skills[0].id, "time_assistant");
    assert_eq!(snapshot.run.skills[0].version, "1");
}

#[tokio::test]
async fn recovers_interrupted_run_and_preserves_partial_output() {
    let pool = setup().await;
    start(&pool, start_params("run-2"))
        .await
        .expect("start run");
    mark_started(&pool, "run-2").await.expect("mark started");
    update_partial(&pool, "run-2-assistant", "保留内容")
        .await
        .expect("partial output");

    assert_eq!(recover_interrupted(&pool).await.expect("recover runs"), 1);

    let recovered = snapshot(&pool, "run-2").await.expect("recovered snapshot");
    assert_eq!(recovered.run.status, RunStatus::Failed);
    assert_eq!(recovered.run.error_code.as_deref(), Some("run_interrupted"));
    assert_eq!(recovered.assistant_message.content, "保留内容");
    assert_eq!(
        serde_json::to_value(recovered.assistant_message.status)
            .expect("message status")
            .as_str(),
        Some("failed")
    );
}

#[tokio::test]
async fn fails_panicked_run_and_preserves_partial_output() {
    let pool = setup().await;
    start(&pool, start_params("run-panicked"))
        .await
        .expect("start run");
    mark_started(&pool, "run-panicked")
        .await
        .expect("mark started");
    update_partial(&pool, "run-panicked-assistant", "panic 前内容")
        .await
        .expect("partial output");

    fail_panicked(&pool, "run-panicked")
        .await
        .expect("fail panicked run");

    let failed = snapshot(&pool, "run-panicked")
        .await
        .expect("failed snapshot");
    assert_eq!(failed.run.status, RunStatus::Failed);
    assert_eq!(failed.run.error_code.as_deref(), Some("run_panicked"));
    assert_eq!(failed.assistant_message.content, "panic 前内容");
}

#[tokio::test]
async fn persists_cancelled_run_with_partial_output() {
    let pool = setup().await;
    start(&pool, start_params("run-3"))
        .await
        .expect("start run");
    mark_started(&pool, "run-3").await.expect("mark started");

    cancel(&pool, "run-3", "run-3-assistant", "停止前内容")
        .await
        .expect("cancel run");

    let cancelled = snapshot(&pool, "run-3").await.expect("cancelled snapshot");
    assert_eq!(cancelled.run.status, RunStatus::Cancelled);
    assert_eq!(cancelled.assistant_message.content, "停止前内容");
    assert_eq!(
        serde_json::to_value(cancelled.assistant_message.status)
            .expect("message status")
            .as_str(),
        Some("cancelled")
    );
}
