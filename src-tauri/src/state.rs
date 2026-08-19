use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use reqwest::Client;
use sqlx::SqlitePool;
use tauri::AppHandle;
use tokio::sync::watch;

use crate::{
    storage::{database, model_response},
    AppResult,
};

#[derive(Clone, Default)]
pub struct ActiveRuns {
    senders: Arc<Mutex<HashMap<String, watch::Sender<bool>>>>,
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

    pub fn remove(&self, run_id: &str) {
        self.senders
            .lock()
            .expect("active run registry lock")
            .remove(run_id);
    }
}

pub struct AppState {
    db: SqlitePool,
    http: Client,
    active_runs: ActiveRuns,
}

impl AppState {
    pub async fn new(app: &AppHandle) -> AppResult<Self> {
        let db = database::connect(app).await?;
        let interrupted_count = model_response::recover_interrupted(&db).await?;
        if interrupted_count > 0 {
            tracing::warn!(interrupted_count, "recovered interrupted agent runs");
        }
        let http = Client::new();
        Ok(Self {
            db,
            http,
            active_runs: ActiveRuns::default(),
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
}
