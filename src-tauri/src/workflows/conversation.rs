use sqlx::SqlitePool;

use crate::{
    protocol::conversation::{Conversation, RenameRequest},
    storage::conversation,
    AppError, AppResult,
};

const MAX_TITLE_CHARS: usize = 120;

pub async fn rename(pool: &SqlitePool, request: RenameRequest) -> AppResult<Conversation> {
    let title = normalize_title(&request.title)?;
    conversation::rename(pool, &request.id, title).await
}

pub async fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    conversation::delete(pool, id).await
}

fn normalize_title(title: &str) -> AppResult<&str> {
    let title = title.trim();
    if title.is_empty() {
        return Err(AppError::Validation(
            "conversation title must not be empty".into(),
        ));
    }
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(AppError::Validation(format!(
            "conversation title must not exceed {MAX_TITLE_CHARS} characters"
        )));
    }
    Ok(title)
}

#[cfg(test)]
mod tests {
    use crate::AppError;

    use super::normalize_title;

    #[test]
    fn normalizes_and_validates_conversation_title() {
        assert_eq!(normalize_title("  标题  ").expect("title"), "标题");
        assert!(matches!(
            normalize_title("   "),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            normalize_title(&"a".repeat(121)),
            Err(AppError::Validation(_))
        ));
    }
}
