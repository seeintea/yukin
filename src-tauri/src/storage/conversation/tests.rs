use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

use crate::AppError;

use super::{create, delete, find, rename};

async fn setup() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("in-memory database");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrations");
    sqlx::query(
        r#"
        INSERT INTO model_providers (
            id, provider_key, api_format, base_url, provider_alias, api_key_alias
        ) VALUES (
            'provider-1', 'deepseek', 'openai', 'https://example.com', 'default', 'key-1'
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("provider");
    pool
}

#[tokio::test]
async fn renames_and_deletes_conversation_with_related_records() {
    let pool = setup().await;
    let conversation = create(&pool).await.expect("conversation");
    let renamed = rename(&pool, &conversation.id, "项目讨论")
        .await
        .expect("rename");
    assert_eq!(renamed.title, "项目讨论");

    sqlx::query(
        r#"
        INSERT INTO runs (id, conversation_id, provider_id, model_id, status)
        VALUES ('run-1', ?, 'provider-1', 'model-1', 'completed')
        "#,
    )
    .bind(&conversation.id)
    .execute(&pool)
    .await
    .expect("run");
    sqlx::query(
        r#"
        INSERT INTO messages (id, conversation_id, run_id, role, content, status, sequence)
        VALUES ('message-1', ?, 'run-1', 'user', 'question', 'completed', 1)
        "#,
    )
    .bind(&conversation.id)
    .execute(&pool)
    .await
    .expect("message");

    delete(&pool, &conversation.id).await.expect("delete");

    assert!(matches!(
        find(&pool, &conversation.id).await,
        Err(AppError::Db(sqlx::Error::RowNotFound))
    ));
    let runs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM runs WHERE conversation_id = ?")
        .bind(&conversation.id)
        .fetch_one(&pool)
        .await
        .expect("run count");
    assert_eq!(runs, 0);
}

#[tokio::test]
async fn refuses_to_delete_conversation_with_active_run() {
    let pool = setup().await;
    let conversation = create(&pool).await.expect("conversation");
    sqlx::query(
        r#"
        INSERT INTO runs (id, conversation_id, provider_id, model_id, status)
        VALUES ('run-active', ?, 'provider-1', 'model-1', 'running')
        "#,
    )
    .bind(&conversation.id)
    .execute(&pool)
    .await
    .expect("active run");

    assert!(matches!(
        delete(&pool, &conversation.id).await,
        Err(AppError::RunState(_))
    ));
}
