use sqlx::{FromRow, SqlitePool};

use crate::{
    protocol::agent_run::{
        ToolApprovalPolicy, ToolCallDecision, ToolCallSnapshot, ToolCallStatus, ToolRiskLevel,
    },
    AppError, AppResult,
};

pub(crate) struct CreateParams<'a> {
    pub id: &'a str,
    pub run_id: &'a str,
    pub name: &'a str,
    pub arguments_json: &'a str,
    pub arguments_digest: &'a str,
    pub risk_level: &'a str,
    pub approval_policy: &'a str,
}

#[derive(FromRow)]
struct ToolCallRecord {
    id: String,
    run_id: String,
    name: String,
    arguments_json: String,
    arguments_digest: String,
    result_json: Option<String>,
    status: String,
    risk_level: String,
    approval_policy: String,
    error_code: Option<String>,
    error_message: Option<String>,
    approval_expires_at: Option<String>,
    created_at: String,
    completed_at: Option<String>,
}

impl TryFrom<ToolCallRecord> for ToolCallSnapshot {
    type Error = AppError;

    fn try_from(record: ToolCallRecord) -> Result<Self, Self::Error> {
        Ok(Self {
            id: record.id,
            run_id: record.run_id,
            name: record.name,
            arguments: serde_json::from_str(&record.arguments_json).map_err(|error| {
                AppError::Other(format!("invalid stored tool arguments: {error}"))
            })?,
            arguments_digest: record.arguments_digest,
            result: record
                .result_json
                .map(|value| serde_json::from_str(&value))
                .transpose()
                .map_err(|error| AppError::Other(format!("invalid stored tool result: {error}")))?,
            status: parse_status(&record.status)?,
            risk_level: match record.risk_level.as_str() {
                "read_only" => ToolRiskLevel::ReadOnly,
                "write" => ToolRiskLevel::Write,
                value => return Err(AppError::Other(format!("invalid tool risk level: {value}"))),
            },
            approval_policy: match record.approval_policy.as_str() {
                "never" => ToolApprovalPolicy::Never,
                "always" => ToolApprovalPolicy::Always,
                value => {
                    return Err(AppError::Other(format!(
                        "invalid tool approval policy: {value}"
                    )))
                }
            },
            error_code: record.error_code,
            error_message: record.error_message,
            approval_expires_at: record.approval_expires_at,
            created_at: record.created_at,
            completed_at: record.completed_at,
        })
    }
}

fn parse_status(value: &str) -> AppResult<ToolCallStatus> {
    match value {
        "requested" => Ok(ToolCallStatus::Requested),
        "waiting_approval" => Ok(ToolCallStatus::WaitingApproval),
        "running" => Ok(ToolCallStatus::Running),
        "completed" => Ok(ToolCallStatus::Completed),
        "failed" => Ok(ToolCallStatus::Failed),
        "rejected" => Ok(ToolCallStatus::Rejected),
        "cancelled" => Ok(ToolCallStatus::Cancelled),
        value => Err(AppError::Other(format!(
            "invalid tool call status: {value}"
        ))),
    }
}

pub(crate) async fn create(pool: &SqlitePool, params: CreateParams<'_>) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO tool_calls (
            id, run_id, name, arguments_json, arguments_digest,
            status, risk_level, approval_policy
        ) VALUES (?, ?, ?, ?, ?, 'requested', ?, ?)
        "#,
    )
    .bind(params.id)
    .bind(params.run_id)
    .bind(params.name)
    .bind(params.arguments_json)
    .bind(params.arguments_digest)
    .bind(params.risk_level)
    .bind(params.approval_policy)
    .execute(pool)
    .await?;
    Ok(())
}

pub(crate) async fn wait_for_approval(
    pool: &SqlitePool,
    run_id: &str,
    tool_call_id: &str,
    expires_at: &str,
) -> AppResult<()> {
    let mut transaction = pool.begin().await?;
    let tool_result = sqlx::query(
        r#"
        UPDATE tool_calls
        SET status = 'waiting_approval', approval_expires_at = ?
        WHERE id = ? AND run_id = ? AND status = 'requested' AND approval_policy = 'always'
        "#,
    )
    .bind(expires_at)
    .bind(tool_call_id)
    .bind(run_id)
    .execute(&mut *transaction)
    .await?;
    let run_result = sqlx::query(
        "UPDATE runs SET status = 'waiting_approval' WHERE id = ? AND status = 'running'",
    )
    .bind(run_id)
    .execute(&mut *transaction)
    .await?;
    ensure_one(
        tool_result.rows_affected(),
        tool_call_id,
        "wait for approval",
    )?;
    ensure_one(run_result.rows_affected(), run_id, "wait for approval")?;
    transaction.commit().await?;
    Ok(())
}

pub(crate) async fn decide(
    pool: &SqlitePool,
    run_id: &str,
    tool_call_id: &str,
    arguments_digest: &str,
    decision: ToolCallDecision,
) -> AppResult<()> {
    let mut transaction = pool.begin().await?;
    let (status, completed_at) = match decision {
        ToolCallDecision::Allow => ("requested", false),
        ToolCallDecision::Reject => ("rejected", true),
    };
    let query = if completed_at {
        r#"
        UPDATE tool_calls
        SET status = ?, decided_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
            completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ? AND run_id = ? AND arguments_digest = ?
          AND status = 'waiting_approval'
          AND approval_expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        "#
    } else {
        r#"
        UPDATE tool_calls
        SET status = ?, decided_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ? AND run_id = ? AND arguments_digest = ?
          AND status = 'waiting_approval'
          AND approval_expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        "#
    };
    let tool_result = sqlx::query(query)
        .bind(status)
        .bind(tool_call_id)
        .bind(run_id)
        .bind(arguments_digest)
        .execute(&mut *transaction)
        .await?;
    let run_result = sqlx::query(
        "UPDATE runs SET status = 'running' WHERE id = ? AND status = 'waiting_approval'",
    )
    .bind(run_id)
    .execute(&mut *transaction)
    .await?;
    ensure_one(
        tool_result.rows_affected(),
        tool_call_id,
        "record approval decision",
    )?;
    ensure_one(run_result.rows_affected(), run_id, "resume after approval")?;
    transaction.commit().await?;
    Ok(())
}

pub(crate) async fn mark_running(
    pool: &SqlitePool,
    run_id: &str,
    tool_call_id: &str,
) -> AppResult<()> {
    let result = sqlx::query(
        r#"
        UPDATE tool_calls SET status = 'running'
        WHERE id = ? AND run_id = ? AND status = 'requested'
          AND (approval_policy = 'never' OR decided_at IS NOT NULL)
          AND EXISTS (SELECT 1 FROM runs WHERE id = ? AND status = 'running')
        "#,
    )
    .bind(tool_call_id)
    .bind(run_id)
    .bind(run_id)
    .execute(pool)
    .await?;
    ensure_one(result.rows_affected(), tool_call_id, "start execution")
}

pub(crate) async fn complete(
    pool: &SqlitePool,
    tool_call_id: &str,
    result_json: &str,
) -> AppResult<()> {
    let result = sqlx::query(
        r#"
        UPDATE tool_calls
        SET status = 'completed', result_json = ?,
            completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ? AND status = 'running'
        "#,
    )
    .bind(result_json)
    .bind(tool_call_id)
    .execute(pool)
    .await?;
    ensure_one(result.rows_affected(), tool_call_id, "complete execution")
}

pub(crate) async fn fail(
    pool: &SqlitePool,
    tool_call_id: &str,
    error_code: &str,
    error_message: &str,
) -> AppResult<()> {
    let result = sqlx::query(
        r#"
        UPDATE tool_calls
        SET status = 'failed', error_code = ?, error_message = ?,
            completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ? AND status IN ('requested', 'waiting_approval', 'running')
        "#,
    )
    .bind(error_code)
    .bind(error_message)
    .bind(tool_call_id)
    .execute(pool)
    .await?;
    ensure_one(result.rows_affected(), tool_call_id, "fail execution")
}

pub(crate) async fn expire(pool: &SqlitePool, run_id: &str, tool_call_id: &str) -> AppResult<()> {
    let mut transaction = pool.begin().await?;
    let tool_result = sqlx::query(
        r#"
        UPDATE tool_calls
        SET status = 'failed', error_code = 'tool_approval_expired',
            error_message = 'tool approval expired',
            completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ? AND run_id = ? AND status = 'waiting_approval'
        "#,
    )
    .bind(tool_call_id)
    .bind(run_id)
    .execute(&mut *transaction)
    .await?;
    let run_result = sqlx::query(
        "UPDATE runs SET status = 'running' WHERE id = ? AND status = 'waiting_approval'",
    )
    .bind(run_id)
    .execute(&mut *transaction)
    .await?;
    ensure_one(tool_result.rows_affected(), tool_call_id, "expire approval")?;
    ensure_one(
        run_result.rows_affected(),
        run_id,
        "resume expired approval",
    )?;
    transaction.commit().await?;
    Ok(())
}

pub(crate) async fn list(pool: &SqlitePool, run_id: &str) -> AppResult<Vec<ToolCallSnapshot>> {
    let records = sqlx::query_as::<_, ToolCallRecord>(
        r#"
        SELECT id, run_id, name, arguments_json, arguments_digest, result_json,
               status, risk_level, approval_policy, error_code, error_message,
               approval_expires_at, created_at, completed_at
        FROM tool_calls WHERE run_id = ? ORDER BY created_at, id
        "#,
    )
    .bind(run_id)
    .fetch_all(pool)
    .await?;
    records.into_iter().map(TryInto::try_into).collect()
}

fn ensure_one(rows_affected: u64, id: &str, action: &str) -> AppResult<()> {
    if rows_affected == 1 {
        Ok(())
    } else {
        Err(AppError::RunState(format!(
            "cannot {action} for tool call {id}"
        )))
    }
}

#[cfg(test)]
mod tests {
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
                arguments_digest:
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
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
}
