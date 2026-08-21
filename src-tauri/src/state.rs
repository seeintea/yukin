use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use reqwest::Client;
use sqlx::SqlitePool;
use tauri::{AppHandle, Manager};
use tokio::sync::{oneshot, watch};

use crate::{
    files::SelectedFiles,
    protocol::agent_run::ToolCallDecision,
    storage::{database, model_response},
    AppResult,
};

type ApprovalKey = (String, String);
type ApprovalSenders = HashMap<ApprovalKey, oneshot::Sender<ToolCallDecision>>;

#[derive(Clone, Default)]
pub struct ActiveRuns {
    senders: Arc<Mutex<HashMap<String, watch::Sender<bool>>>>,
    approvals: Arc<Mutex<ApprovalSenders>>,
}

impl ActiveRuns {
    pub fn register(&self, run_id: String) -> watch::Receiver<bool> {
        let (sender, receiver) = watch::channel(false);
        self.senders
            .lock()
            .expect("active run registry lock")
            .insert(run_id, sender);
        receiver
    }

    pub fn cancel(&self, run_id: &str) -> bool {
        self.senders
            .lock()
            .expect("active run registry lock")
            .get(run_id)
            .is_some_and(|sender| sender.send(true).is_ok())
    }

    pub fn wait_for_approval(
        &self,
        run_id: String,
        tool_call_id: String,
    ) -> oneshot::Receiver<ToolCallDecision> {
        let (sender, receiver) = oneshot::channel();
        self.approvals
            .lock()
            .expect("tool approval registry lock")
            .insert((run_id, tool_call_id), sender);
        receiver
    }

    pub fn decide(&self, run_id: &str, tool_call_id: &str, decision: ToolCallDecision) -> bool {
        self.approvals
            .lock()
            .expect("tool approval registry lock")
            .remove(&(run_id.into(), tool_call_id.into()))
            .is_some_and(|sender| sender.send(decision).is_ok())
    }

    pub fn is_waiting_for_approval(&self, run_id: &str, tool_call_id: &str) -> bool {
        self.approvals
            .lock()
            .expect("tool approval registry lock")
            .contains_key(&(run_id.into(), tool_call_id.into()))
    }

    pub fn remove(&self, run_id: &str) {
        self.senders
            .lock()
            .expect("active run registry lock")
            .remove(run_id);
        self.approvals
            .lock()
            .expect("tool approval registry lock")
            .retain(|(registered_run_id, _), _| registered_run_id != run_id);
    }
}

pub struct AppState {
    db: SqlitePool,
    http: Client,
    active_runs: ActiveRuns,
    tool_data_dir: PathBuf,
    selected_files: SelectedFiles,
}

impl AppState {
    pub async fn new(app: &AppHandle) -> AppResult<Self> {
        let db = database::connect(app).await?;
        let tool_data_dir = app.path().app_data_dir()?.join("agent-files");
        let interrupted_count = model_response::recover_interrupted(&db).await?;
        if interrupted_count > 0 {
            tracing::warn!(interrupted_count, "recovered interrupted agent runs");
        }
        let http = Client::new();
        Ok(Self {
            db,
            http,
            active_runs: ActiveRuns::default(),
            tool_data_dir,
            selected_files: SelectedFiles::default(),
        })
    }

    pub fn db(&self) -> &SqlitePool {
        &self.db
    }

    pub fn http(&self) -> &Client {
        &self.http
    }

    pub fn active_runs(&self) -> &ActiveRuns {
        &self.active_runs
    }

    pub fn tool_data_dir(&self) -> &Path {
        &self.tool_data_dir
    }

    pub(crate) fn selected_files(&self) -> &SelectedFiles {
        &self.selected_files
    }
}

#[cfg(test)]
mod tests {
    use crate::protocol::agent_run::ToolCallDecision;

    use super::ActiveRuns;

    #[tokio::test]
    async fn delivers_approval_to_the_matching_run_and_tool_call() {
        let runs = ActiveRuns::default();
        let approval = runs.wait_for_approval("run-1".into(), "tool-1".into());

        assert!(runs.is_waiting_for_approval("run-1", "tool-1"));
        assert!(runs.decide("run-1", "tool-1", ToolCallDecision::Allow));
        assert_eq!(
            approval.await.expect("approval decision"),
            ToolCallDecision::Allow
        );
        assert!(!runs.is_waiting_for_approval("run-1", "tool-1"));
    }
}
