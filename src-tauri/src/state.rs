use reqwest::Client;
use sqlx::SqlitePool;
use tauri::AppHandle;

use crate::{storage::database, AppResult};

pub struct AppState {
    db: SqlitePool,
    http: Client,
}

impl AppState {
    pub async fn new(app: &AppHandle) -> AppResult<Self> {
        let db = database::connect(app).await?;
        let http = Client::new();
        Ok(Self { db, http })
    }

    pub fn db(&self) -> &SqlitePool {
        &self.db
    }

    pub fn http(&self) -> &Client {
        &self.http
    }
}
