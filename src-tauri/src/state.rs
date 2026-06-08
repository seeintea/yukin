use std::{collections::HashMap, path::PathBuf};

use tauri::AppHandle;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use crate::AppResult;

pub struct AppState {
    pub workspace: RwLock<Option<PathBuf>>,
    pub db: Option<sqlx::SqlitePool>,
    pub http: reqwest::Client,
    pub runs: RwLock<HashMap<String, CancellationToken>>,
}

impl AppState {
    pub async fn new(_app: &AppHandle) -> AppResult<Self> {
        unimplemented!()
    }
}
