//! 纯数据库层 - memory 表 CRUD + FTS5 搜索
//!
//! 设计原则(详见 docs/plan/learning-notes/C3-sqlx-and-dto-and-tests.md):
//! - 无 #[tauri::command],无 State<AppState>,可独立测试
//! - 操作函数收 `&SqlitePool`,由 commands 层注入
//! - DTO 三类型分离: Input(写) / Update(patch) / Row(读)

use serde::{Deserialize, Serialize};

// ============ Types ============

/// memory.kind 字段的合法值。
///
/// - 序列化为 snake_case 字符串(`user` / `feedback` / `project` / `reference`),
///   与 schema CHECK 约束 + 前端 JSON 对齐
/// - sqlx 端通过 `as_str()` 显式转字符串后 `bind`(不实现 sqlx::Type 以避免反向 decode 复杂度)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    User,
    Feedback,
    Project,
    Reference,
}

impl MemoryKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            MemoryKind::User => "user",
            MemoryKind::Feedback => "feedback",
            MemoryKind::Project => "project",
            MemoryKind::Reference => "reference",
        }
    }
}

/// 创建一条 memory 时的输入(用户 → 后端)。
///
/// 不含 id / created_at / updated_at —— 前者由 `Uuid::new_v4()` 在 save 内生成,
/// 后两者由 SQLite `DEFAULT (datetime('now'))` 自动填。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySaveInput {
    pub name: String,
    pub kind: MemoryKind,
    pub content: String,
    pub description: Option<String>,
    /// JSON 任意结构;db 层 `serde_json::to_string` 后存为 TEXT。`None` → `"{}"`
    pub metadata: Option<serde_json::Value>,
    /// `None` 表示全局 memory(跨 workspace 可见)
    pub workspace: Option<String>,
}

/// 更新一条 memory 时的 patch(用户 → 后端)。
///
/// 全字段 `Option`:`Some(v)` 表示要改成 `v`,`None` 表示不动。
/// SQL 用 `COALESCE(?, original)` 模式实现部分更新。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryUpdate {
    pub name: Option<String>,
    pub description: Option<String>,
    pub content: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// 从 db 读出的完整 memory 行(后端 → 用户)。
///
/// - `kind` 是 String(不是 `MemoryKind`):避免实现 sqlx Decode/Type;
///   db CHECK 已保证只有 4 个合法值,前端 TS 端用 union type 收
/// - `metadata` 是 JSON 字符串:前端自行 `JSON.parse`
/// - `created_at` / `updated_at` 是 String:格式 `YYYY-MM-DD HH:MM:SS`(SQLite `datetime('now')`),
///   字典序 = 时间序,排序/范围过滤直接走 SQL,Rust 端无需 chrono
#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct MemoryRow {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub description: Option<String>,
    pub content: String,
    pub metadata: String,
    pub workspace: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
