use crate::AppResult;
use sqlx::SqlitePool;

pub async fn upsert_has_key(pool: &SqlitePool, provider: &str, has: bool) -> AppResult<()> {
    let v: i64 = if has { 1 } else { 0 };

    sqlx::query!(
        "INSERT INTO providers (name, has_key) VALUES (?, ?) \
    ON CONFLICT(name) DO UPDATE SET \
    has_key = excluded.has_key, updated_at = datetime('now')",
        provider,
        v
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn list_providers_with_key(pool: &SqlitePool) -> AppResult<Vec<String>> {
    let rows = sqlx::query!(r#"SELECT name as "name!" FROM providers WHERE has_key = 1"#)
        .fetch_all(pool)
        .await?;

    Ok(rows.into_iter().map(|r| r.name).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test(migrations = "./migrations")]
    async fn upsert_then_list(pool: SqlitePool) {
        upsert_has_key(&pool, "anthropic", true).await.unwrap();
        upsert_has_key(&pool, "openai", false).await.unwrap();
        upsert_has_key(&pool, "google", true).await.unwrap();

        let mut list = list_providers_with_key(&pool).await.unwrap();
        list.sort();
        assert_eq!(list, vec!["anthropic", "google"]);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn upsert_overwrites(pool: SqlitePool) {
        upsert_has_key(&pool, "anthropic", true).await.unwrap();
        upsert_has_key(&pool, "anthropic", false).await.unwrap();
        let list = list_providers_with_key(&pool).await.unwrap();
        assert_eq!(list, Vec::<String>::new());
    }
}

