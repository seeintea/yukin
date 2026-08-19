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
mod tests;
