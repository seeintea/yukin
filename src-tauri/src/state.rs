use sqlx::SqlitePool;
use tauri::AppHandle;

use crate::{storage::database, AppResult};

pub struct AppState {
    db: SqlitePool,
}

impl AppState {
    pub async fn new(app: &AppHandle) -> AppResult<Self> {
        let db = database::connect(app).await?;
        Ok(Self { db })
    }

    pub fn db(&self) -> &SqlitePool {
        &self.db
    }
}
