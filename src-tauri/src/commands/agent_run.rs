use std::panic::AssertUnwindSafe;

use futures_util::FutureExt;
use tauri::{ipc::Channel, State};

use crate::{
    diagnostics::result::LogError,
    protocol::agent_run::{
        CancelRequest, Event, Snapshot, SnapshotRequest, StartRequest, StartResponse,
        ToolCallDecideRequest,
    },
    storage::{model_response, tool_call},
    workflows::agent_run,
    AppError, AppResult, AppState,
};

#[tauri::command]
pub async fn agent_run_start(
    state: State<'_, AppState>,
    request: StartRequest,
    events: Channel<Event>,
) -> AppResult<StartResponse> {
    let conversation_id = request.conversation_id.clone();
    let prepared = agent_run::prepare(state.db(), request)
        .await
        .log_error("agent_run_start")?;
    let response = prepared.response.clone();
    let cancellation = state.active_runs().register(response.run_id.clone());
    let active_runs = state.active_runs().clone();
    let run_handles = active_runs.clone();
    let run_id = response.run_id.clone();
    let pool = state.db().clone();
    let panic_pool = pool.clone();
    let client = state.http().clone();
    let tool_data_dir = state.tool_data_dir().to_path_buf();
    let event_run_id = response.run_id.clone();
    let event_sink = Box::new(move |event| {
        if let Err(error) = events.send(event) {
            tracing::debug!(%error, run_id = %event_run_id, "agent run event receiver unavailable");
        }
    });

    tauri::async_runtime::spawn(async move {
        let outcome = AssertUnwindSafe(agent_run::execute(
            pool,
            client,
            prepared,
            cancellation,
            run_handles,
            tool_data_dir,
            event_sink,
        ))
        .catch_unwind()
        .await;
        if outcome.is_err() {
            tracing::error!(
                %conversation_id,
                %run_id,
                "agent run background task panicked"
            );
            if let Err(error) = model_response::fail_panicked(&panic_pool, &run_id).await {
                tracing::error!(
                    %conversation_id,
                    %run_id,
                    error_code = error.code(),
                    error = %error,
                    "failed to persist panicked agent run"
                );
            }
        }
        active_runs.remove(&run_id);
    });

    Ok(response)
}

#[tauri::command]
pub async fn tool_call_decide(
    state: State<'_, AppState>,
    request: ToolCallDecideRequest,
) -> AppResult<()> {
    async {
        if !state
            .active_runs()
            .is_waiting_for_approval(&request.run_id, &request.tool_call_id)
        {
            return Err(AppError::RunState(
                "tool call is not waiting for approval".into(),
            ));
        }
        tool_call::decide(
            state.db(),
            &request.run_id,
            &request.tool_call_id,
            &request.arguments_digest,
            request.decision,
        )
        .await?;
        if state
            .active_runs()
            .decide(&request.run_id, &request.tool_call_id, request.decision)
        {
            Ok(())
        } else {
            Err(AppError::RunState(
                "tool approval waiter is no longer active".into(),
            ))
        }
    }
    .await
    .log_error("tool_call_decide")
}

#[tauri::command]
pub async fn agent_run_snapshot(
    state: State<'_, AppState>,
    request: SnapshotRequest,
) -> AppResult<Snapshot> {
    model_response::snapshot(state.db(), &request.run_id)
        .await
        .log_error("agent_run_snapshot")
}

#[tauri::command]
pub async fn agent_run_cancel(state: State<'_, AppState>, request: CancelRequest) -> AppResult<()> {
    let result = if state.active_runs().cancel(&request.run_id) {
        Ok(())
    } else {
        Err(AppError::RunState("agent run is not active".into()))
    };
    result.log_error("agent_run_cancel")
}
