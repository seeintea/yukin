use tauri::{ipc::Channel, State};

use crate::{
    protocol::agent_run::{
        CancelRequest, Event, Snapshot, SnapshotRequest, StartRequest, StartResponse,
    },
    storage::model_response,
    workflows::agent_run,
    AppError, AppResult, AppState,
};

#[tauri::command]
pub async fn agent_run_start(
    state: State<'_, AppState>,
    request: StartRequest,
    events: Channel<Event>,
) -> AppResult<StartResponse> {
    let prepared = agent_run::prepare(state.db(), request).await?;
    let response = prepared.response.clone();
    let cancellation = state.active_runs().register(response.run_id.clone());
    let active_runs = state.active_runs().clone();
    let run_id = response.run_id.clone();
    let pool = state.db().clone();
    let client = state.http().clone();
    let event_sink = Box::new(move |event| {
        if let Err(error) = events.send(event) {
            tracing::debug!(%error, "agent run event receiver unavailable");
        }
    });

    tauri::async_runtime::spawn(async move {
        agent_run::execute(pool, client, prepared, cancellation, event_sink).await;
        active_runs.remove(&run_id);
    });

    Ok(response)
}

#[tauri::command]
pub async fn agent_run_snapshot(
    state: State<'_, AppState>,
    request: SnapshotRequest,
) -> AppResult<Snapshot> {
    model_response::snapshot(state.db(), &request.run_id).await
}

#[tauri::command]
pub async fn agent_run_cancel(state: State<'_, AppState>, request: CancelRequest) -> AppResult<()> {
    if state.active_runs().cancel(&request.run_id) {
        Ok(())
    } else {
        Err(AppError::RunState("agent run is not active".into()))
    }
}
