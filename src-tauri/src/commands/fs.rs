//! 文件系统命令 —— 全部走 path_safety::resolve_within 防穿越,
//! 严格限制在用户选定的 workspace 内。

use std::path::PathBuf;

use serde::Serialize;
use tauri::State;

use crate::path_safety;
use crate::{AppError, AppResult, AppState};

const READ_TRUNCATE_BYTES: usize = 200_000;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FsReadResult {
    pub content: String,
    pub truncated: bool,
    pub original_size: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirEntry {
    pub name: String,
    pub path: String, // 相对 workspace 的路径
    pub is_dir: bool,
    pub is_file: bool,
    pub size: Option<u64>,
}

/// 拿当前 workspace 根目录,未设置时报 NoWorkspace。
async fn workspace_root(state: &AppState) -> AppResult<PathBuf> {
    state
        .workspace
        .read()
        .await
        .clone()
        .ok_or(AppError::NoWorkspace)
}

#[tauri::command]
pub async fn fs_read(path: String, state: State<'_, AppState>) -> AppResult<FsReadResult> {
    let root = workspace_root(&state).await?;
    let abs = path_safety::resolve_within(&root, &path)?;

    let bytes = tokio::fs::read(&abs).await?;
    let original_size = bytes.len();
    let (data, truncated) = if bytes.len() > READ_TRUNCATE_BYTES {
        (
            String::from_utf8_lossy(&bytes[..READ_TRUNCATE_BYTES]).to_string(),
            true,
        )
    } else {
        (String::from_utf8_lossy(&bytes).to_string(), false)
    };

    Ok(FsReadResult {
        content: data,
        truncated,
        original_size,
    })
}

#[tauri::command]
pub async fn fs_write(
    path: String,
    content: String,
    create_dirs: Option<bool>,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let root = workspace_root(&state).await?;
    let abs = path_safety::resolve_within(&root, &path)?;

    if create_dirs.unwrap_or(true) {
        if let Some(parent) = abs.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
    }

    tokio::fs::write(&abs, content).await?;
    Ok(())
}

#[tauri::command]
pub async fn fs_edit(
    path: String,
    search: String,
    replace: String,
    all: Option<bool>,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let root = workspace_root(&state).await?;
    let abs = path_safety::resolve_within(&root, &path)?;

    let original = tokio::fs::read_to_string(&abs).await?;
    let updated = if all.unwrap_or(false) {
        original.replace(&search, &replace)
    } else {
        // 只替换第一处:用 replacen(_, _, 1)
        original.replacen(&search, &replace, 1)
    };

    if updated == original {
        return Err(AppError::Other(format!(
            "fs_edit: search string not found in {path}"
        )));
    }

    tokio::fs::write(&abs, updated).await?;
    Ok(())
}

#[tauri::command]
pub async fn fs_list_dir(path: String, state: State<'_, AppState>) -> AppResult<Vec<DirEntry>> {
    let root = workspace_root(&state).await?;
    let abs = path_safety::resolve_within(&root, &path)?;

    let mut entries = Vec::new();
    let mut rd = tokio::fs::read_dir(&abs).await?;

    while let Some(entry) = rd.next_entry().await? {
        let file_type = entry.file_type().await?;
        let meta = entry.metadata().await.ok();

        let name = entry.file_name().to_string_lossy().to_string();
        // 相对 root 的路径,前端展示用
        let rel = entry
            .path()
            .strip_prefix(&root)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| name.clone());

        entries.push(DirEntry {
            name,
            path: rel,
            is_dir: file_type.is_dir(),
            is_file: file_type.is_file(),
            size: meta.map(|m| m.len()),
        });
    }

    // 按 name 排序便于前端稳定渲染
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

#[tauri::command]
pub async fn fs_glob(pattern: String, state: State<'_, AppState>) -> AppResult<Vec<String>> {
    let root = workspace_root(&state).await?;

    // glob pattern 拼到 root 下,然后过滤结果必须在 root 内(防止 pattern 本身含 ..)
    let full_pattern = root.join(&pattern);
    let full_pattern_str = full_pattern.to_string_lossy().to_string();

    let mut results = Vec::new();
    for entry in glob::glob(&full_pattern_str)
        .map_err(|e| AppError::Other(format!("glob pattern: {e}")))?
    {
        let path = entry.map_err(|e| AppError::Other(format!("glob entry: {e}")))?;
        // 安全网:每个结果再过一遍 resolve_within
        let canon = match path.canonicalize() {
            Ok(p) => p,
            Err(_) => continue, // glob 命中但已被删,跳过
        };
        if !canon.starts_with(&root) {
            continue;
        }
        if let Ok(rel) = canon.strip_prefix(&root) {
            results.push(rel.to_string_lossy().to_string());
        }
    }

    results.sort();
    Ok(results)
}

#[tauri::command]
pub async fn fs_exists(path: String, state: State<'_, AppState>) -> AppResult<bool> {
    let root = workspace_root(&state).await?;
    // exists 检查不能用 resolve_within(它对不存在的文件会走 fallback 路径)
    // 改用直接判断 + 防穿越
    let abs = path_safety::resolve_within(&root, &path)?;
    Ok(tokio::fs::try_exists(&abs).await?)
}
