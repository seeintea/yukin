use std::{fs, path::PathBuf};

use sqlx::SqlitePool;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;

use crate::{
    protocol::imported_skill::{ImportedSkill, SourceKind},
    storage::imported_skill::{self, CreateParams},
    AppError, AppResult,
};

use super::package_import;

pub async fn import_directory(
    app: AppHandle,
    pool: &SqlitePool,
) -> AppResult<Option<ImportedSkill>> {
    let Some(source) = pick_directory(app.clone()).await? else {
        return Ok(None);
    };
    import(app, pool, source, SourceKind::Directory)
        .await
        .map(Some)
}

pub async fn import_archive(app: AppHandle, pool: &SqlitePool) -> AppResult<Option<ImportedSkill>> {
    let Some(source) = pick_archive(app.clone()).await? else {
        return Ok(None);
    };
    import(app, pool, source, SourceKind::Archive)
        .await
        .map(Some)
}

async fn import(
    app: AppHandle,
    pool: &SqlitePool,
    source: PathBuf,
    source_kind: SourceKind,
) -> AppResult<ImportedSkill> {
    let id = Uuid::now_v7().to_string();
    let base = app.path().app_data_dir()?.join("skills");
    let staging = base.join(format!(".import-{id}"));
    let destination = base.join(&id);
    let prepared = tauri::async_runtime::spawn_blocking(move || {
        let result = (|| {
            fs::create_dir_all(&base)?;
            match source_kind {
                SourceKind::Directory => package_import::copy_directory(&source, &staging)?,
                SourceKind::Archive => package_import::extract_zip(&source, &staging)?,
            }
            let skill_path = package_import::find_unique_file(&staging, "SKILL.md")?;
            let content = package_import::read_metadata(&skill_path)?;
            let metadata = parse_frontmatter(&content)?;
            let digest = package_import::digest_directory(&staging)?;
            fs::rename(&staging, &destination)?;
            Ok::<_, AppError>((metadata, digest, destination))
        })();
        if result.is_err() {
            let _ = fs::remove_dir_all(&staging);
        }
        result
    })
    .await
    .map_err(|error| AppError::Other(format!("skill import task failed: {error}")))??;

    let (metadata, content_digest, destination) = prepared;
    let result = imported_skill::create(
        pool,
        CreateParams {
            id,
            name: metadata.name,
            description: metadata.description,
            source_kind,
            managed_path: destination.to_string_lossy().into_owned(),
            content_digest,
        },
    )
    .await;
    if result.is_err() {
        let _ = fs::remove_dir_all(destination);
    }
    let skill = result?;
    tracing::info!(
        skill_id = %skill.id,
        skill_name = %skill.name,
        source_kind = skill.source_kind.as_str(),
        "imported skill stored"
    );
    Ok(skill)
}

pub async fn delete(app: AppHandle, pool: &SqlitePool, id: &str) -> AppResult<()> {
    let managed_path = PathBuf::from(imported_skill::managed_path(pool, id).await?);
    imported_skill::delete(pool, id).await?;
    remove_managed_directory(&app, "skills", managed_path);
    tracing::info!(skill_id = %id, "imported skill deleted");
    Ok(())
}

fn remove_managed_directory(app: &AppHandle, kind: &str, path: PathBuf) {
    let Ok(base) = app.path().app_data_dir().map(|path| path.join(kind)) else {
        return;
    };
    if path.parent() == Some(base.as_path()) {
        if let Err(error) = fs::remove_dir_all(&path) {
            tracing::warn!(?path, %error, "failed to remove managed package directory");
        }
    }
}

async fn pick_directory(app: AppHandle) -> AppResult<Option<PathBuf>> {
    tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .blocking_pick_folder()
            .map(|path| path.into_path())
    })
    .await
    .map_err(|error| AppError::Other(format!("skill directory dialog failed: {error}")))?
    .transpose()
    .map_err(|_| AppError::Validation("selected skill directory path is unsupported".into()))
}

async fn pick_archive(app: AppHandle) -> AppResult<Option<PathBuf>> {
    tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .add_filter("Skill archive", &["zip"])
            .blocking_pick_file()
            .map(|path| path.into_path())
    })
    .await
    .map_err(|error| AppError::Other(format!("skill archive dialog failed: {error}")))?
    .transpose()
    .map_err(|_| AppError::Validation("selected skill archive path is unsupported".into()))
}

struct SkillMetadata {
    name: String,
    description: String,
}

fn parse_frontmatter(content: &str) -> AppResult<SkillMetadata> {
    let mut lines = content.lines();
    if lines.next().map(str::trim) != Some("---") {
        return Err(AppError::Validation(
            "SKILL.md must start with YAML frontmatter".into(),
        ));
    }
    let mut name = None;
    let mut description = None;
    let mut closed = false;
    for line in lines {
        let line = line.trim();
        if line == "---" {
            closed = true;
            break;
        }
        if let Some(value) = line.strip_prefix("name:") {
            name = Some(clean_value(value));
        } else if let Some(value) = line.strip_prefix("description:") {
            description = Some(clean_value(value));
        }
    }
    if !closed {
        return Err(AppError::Validation(
            "SKILL.md YAML frontmatter is not closed".into(),
        ));
    }
    let name = name
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Validation("SKILL.md frontmatter must contain a name".into()))?;
    if name.len() > 128 {
        return Err(AppError::Validation("skill name is too long".into()));
    }
    let description = description
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::Validation("SKILL.md frontmatter must contain a description".into())
        })?;
    Ok(SkillMetadata { name, description })
}

fn clean_value(value: &str) -> String {
    value
        .trim()
        .trim_matches(|character| character == '\'' || character == '"')
        .trim()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::parse_frontmatter;

    #[test]
    fn parses_skill_frontmatter() {
        let metadata = parse_frontmatter(
            "---\nname: local-notes\ndescription: 'Organize local notes'\n---\n\nInstructions",
        )
        .expect("valid skill metadata");

        assert_eq!(metadata.name, "local-notes");
        assert_eq!(metadata.description, "Organize local notes");
    }

    #[test]
    fn rejects_unclosed_frontmatter() {
        let error = parse_frontmatter("---\nname: broken\ndescription: Missing delimiter")
            .err()
            .expect("invalid skill metadata");

        assert!(error.to_string().contains("not closed"));
    }
}
