use std::{collections::HashMap, path::PathBuf};

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};
use tauri::{AppHandle, Manager};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use crate::AppResult;

pub struct AppState {
    pub workspace: RwLock<Option<PathBuf>>,
    pub db: SqlitePool,
    pub http: reqwest::Client,
    pub runs: RwLock<HashMap<String, CancellationToken>>,
}

impl AppState {
    pub async fn new(app: &AppHandle) -> AppResult<Self> {
        let pool = Self::open_db(app).await?;
        Self::run_migrations(&pool).await?;
        let workspace = Self::load_workspace(&pool).await?;

        Ok(Self {
            workspace: RwLock::new(workspace),
            db: pool,
            http: reqwest::Client::new(),
            runs: RwLock::new(HashMap::new()),
        })
    }

    async fn open_db(app: &AppHandle) -> AppResult<sqlx::SqlitePool> {
        let data_dir = app.path().app_data_dir()?;
        std::fs::create_dir_all(&data_dir)?;
        let db_path = data_dir.join("yukin.db");
        tracing::info!(?db_path, "opening db");

        let options = SqliteConnectOptions::new()
            .filename(&db_path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(std::time::Duration::from_secs(5));

        let pool: sqlx::Pool<sqlx::Sqlite> = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;

        Ok(pool)
    }

    async fn run_migrations(pool: &sqlx::SqlitePool) -> AppResult<()> {
        sqlx::migrate!("./migrations").run(pool).await?;
        tracing::info!("migrations applied");
        Ok(())
    }

    async fn load_workspace(pool: &sqlx::SqlitePool) -> AppResult<Option<PathBuf>> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT value FROM settings WHERE key = 'workspace_path'")
                .fetch_optional(pool)
                .await?;

        Ok(row.map(|(v,)| PathBuf::from(v)))
    }
}
