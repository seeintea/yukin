//! memory 命令薄壳 —— 只做 DTO 转换 + state 解包,实际 SQL 在 db::memory。

use tauri::State;

use crate::db::memory::{self, MemoryRow, MemorySaveInput, MemoryUpdate};
use crate::{AppResult, AppState};

#[tauri::command]
pub async fn memory_save(
    input: MemorySaveInput,
    state: State<'_, AppState>,
) -> AppResult<MemoryRow> {
    memory::save(&state.db, input).await
}

#[tauri::command]
pub async fn memory_recall(
    query: String,
    limit: Option<i64>,
    kind: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<Vec<MemoryRow>> {
    memory::recall(&state.db, &query, limit.unwrap_or(8), kind.as_deref()).await
}

#[tauri::command]
pub async fn memory_list(
    kind: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<Vec<MemoryRow>> {
    memory::list(&state.db, kind.as_deref()).await
}

#[tauri::command]
pub async fn memory_delete(id: String, state: State<'_, AppState>) -> AppResult<()> {
    memory::delete(&state.db, &id).await
}

#[tauri::command]
pub async fn memory_update(
    id: String,
    patch: MemoryUpdate,
    state: State<'_, AppState>,
) -> AppResult<MemoryRow> {
    memory::update(&state.db, &id, patch).await
}
