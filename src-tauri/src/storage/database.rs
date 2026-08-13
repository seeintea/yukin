use std::{fs::create_dir_all, time::Duration};

use sqlx::{
    migrate,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    SqlitePool,
};
use tauri::{AppHandle, Manager};

use crate::AppResult;

pub async fn connect(app: &AppHandle) -> AppResult<SqlitePool> {
    let data_dir = app.path().app_data_dir()?;
    create_dir_all(&data_dir)?;

    let db_path = data_dir.join("yukin.db");
    tracing::info!(?db_path, "opening db");

    let options = SqliteConnectOptions::new()
        .filename(&db_path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    migrate!("./migrations").run(&pool).await?;
    tracing::info!("sql migrations applied");

    Ok(pool)
}
