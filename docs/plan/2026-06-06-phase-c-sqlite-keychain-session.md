# Phase C — SQLite + Keychain + Session 命令

> 创建日期: 2026-06-06
> 目标: SQLite schema 建好,`memory_*` / `key_*` / `session_*` 全实现,数据库 + Keychain 可见数据。

## 前置
- Phase B 完成(模块骨架就位)

## 步骤

1. **创建迁移** `src-tauri/migrations/0001_init.sql`:
   ```sql
   CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);

   CREATE TABLE providers (
     name TEXT PRIMARY KEY, has_key INT NOT NULL DEFAULT 0,
     default_model TEXT,
     created_at TEXT NOT NULL DEFAULT (datetime('now')),
     updated_at TEXT NOT NULL DEFAULT (datetime('now'))
   );

   CREATE TABLE memory (
     id TEXT PRIMARY KEY,
     name TEXT NOT NULL,
     kind TEXT NOT NULL CHECK (kind IN ('user','feedback','project','reference')),
     description TEXT,
     content TEXT NOT NULL,
     metadata TEXT NOT NULL DEFAULT '{}',
     workspace TEXT,                              -- NULL = 全局
     created_at TEXT NOT NULL DEFAULT (datetime('now')),
     updated_at TEXT NOT NULL DEFAULT (datetime('now'))
   );
   CREATE INDEX memory_kind_idx      ON memory(kind);
   CREATE INDEX memory_workspace_idx ON memory(workspace);

   CREATE VIRTUAL TABLE memory_fts USING fts5(
     name, description, content,
     content='memory', content_rowid='rowid',
     tokenize='unicode61 remove_diacritics 2'
   );
   -- INSERT / UPDATE / DELETE 三个同步 trigger

   CREATE TABLE sessions (
     id TEXT PRIMARY KEY,
     title TEXT NOT NULL,
     workspace_path TEXT,
     provider TEXT,
     model TEXT,
     created_at TEXT NOT NULL DEFAULT (datetime('now')),
     updated_at TEXT NOT NULL DEFAULT (datetime('now'))
   );

   CREATE TABLE messages (
     id TEXT PRIMARY KEY,
     session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
     role TEXT NOT NULL CHECK (role IN ('system','user','assistant','tool')),
     content TEXT NOT NULL,         -- JSON (Anthropic Messages 格式)
     tool_calls TEXT,
     tool_results TEXT,
     step_index INTEGER,
     created_at TEXT NOT NULL DEFAULT (datetime('now'))
   );
   CREATE INDEX messages_session_idx ON messages(session_id, created_at);
   ```

2. **实现 `AppState::new`** (`state.rs`):
   - `app.path().app_data_dir()?.join("yukin.db")`
   - 确保父目录存在
   - `SqlitePoolOptions::new().max_connections(1).connect(&format!("sqlite://{}?mode=rwc", path.display())).await?`
   - `sqlx::migrate!("./migrations").run(&pool).await?`
   - PRAGMA WAL: `sqlx::query("PRAGMA journal_mode=WAL").execute(&pool).await?`
   - 从 `settings.workspace_path` 回填 `workspace`
   - 返回 `AppState { workspace, db: pool, http: reqwest::Client::new(), runs: Default::default() }`

3. **`commands/memory.rs`** 完整实现:
   ```rust
   #[tauri::command]
   pub async fn memory_save(input: MemorySaveInput, state: State<'_, AppState>) -> AppResult<MemoryRow> {
       let id = Uuid::new_v4().to_string();
       sqlx::query("INSERT INTO memory (id,name,kind,description,content,metadata,workspace) VALUES (?,?,?,?,?,?,?)")
           .bind(&id).bind(&input.name).bind(input.kind.as_str())
           .bind(&input.description).bind(&input.content)
           .bind(serde_json::to_string(&input.metadata)?)
           .bind(&input.workspace)
           .execute(&state.db).await?;
       // 查回完整行
       fetch_memory(&state.db, &id).await
   }

   #[tauri::command]
   pub async fn memory_recall(query: String, limit: Option<i64>, kind: Option<MemoryKind>, state: State<'_, AppState>) -> AppResult<Vec<MemoryRow>> {
       // FTS5 + 可选 kind 过滤
       sqlx::query_as::<_, MemoryRow>(r#"
           SELECT m.* FROM memory m
           JOIN memory_fts f ON f.rowid = m.rowid
           WHERE memory_fts MATCH ?1
             AND (?3 IS NULL OR m.kind = ?3)
           ORDER BY rank LIMIT ?2
       "#).bind(&query).bind(limit.unwrap_or(8)).bind(kind.map(|k|k.as_str()))
         .fetch_all(&state.db).await.map_err(Into::into)
   }

   // memory_list / memory_delete / memory_update 类似
   ```

4. **`commands/keychain.rs`** 完整实现:
   ```rust
   const SERVICE: &str = "xyz.yukin.agent";

   #[tauri::command]
   pub async fn key_set(provider: String, key: String, state: State<'_, AppState>) -> AppResult<()> {
       tokio::task::spawn_blocking(move || {
           keyring::Entry::new(SERVICE, &provider)?.set_password(&key)
       }).await.map_err(|e| AppError::Other(e.to_string()))??;
       // upsert providers 表
       sqlx::query("INSERT INTO providers (name,has_key) VALUES (?,1)
                    ON CONFLICT(name) DO UPDATE SET has_key=1, updated_at=datetime('now')")
           .bind(&provider).execute(&state.db).await?;
       Ok(())
   }

   #[tauri::command]
   pub async fn key_get(provider: String) -> AppResult<Option<String>> {
       tokio::task::spawn_blocking(move || {
           match keyring::Entry::new(SERVICE, &provider)?.get_password() {
               Ok(k) => Ok(Some(k)),
               Err(keyring::Error::NoEntry) => Ok(None),
               Err(e) => Err(e.into()),
           }
       }).await.map_err(|e| AppError::Other(e.to_string()))?
   }
   // key_delete / key_list_providers 类似
   ```

5. **`commands/session.rs`** 完整实现:
   - `session_create(title)` → uuid + INSERT
   - `session_list` → SELECT 排序 desc,limit 100
   - `session_update(id, patch)` → UPDATE 部分字段
   - `session_delete(id)` → DELETE(FK cascade 自动清 messages)
   - `session_append_message(session_id, msg)` → INSERT messages
   - `session_load_messages(session_id)` → SELECT 按 created_at 升序

## 关键文件
- `src-tauri/migrations/0001_init.sql`(新)
- `src-tauri/src/state.rs`(实现 `new`)
- `src-tauri/src/commands/memory.rs`(实现)
- `src-tauri/src/commands/keychain.rs`(实现)
- `src-tauri/src/commands/session.rs`(实现)

## 验证
- [ ] `~/Library/Application Support/xyz.yukin.agent/yukin.db` 启动后存在
- [ ] devtools: `invoke('memory_save', {input:{name:"t",kind:"user",content:"hello",metadata:{}}})` 返回 id
- [ ] devtools: `invoke('memory_recall', {query:"hello"})` 返回数组
- [ ] DB Browser 打开 yukin.db 验证 memory 行
- [ ] `invoke('key_set', {provider:"anthropic", key:"sk-ant-test"})` 无错
- [ ] Keychain Access 搜 "xyz.yukin.agent" 显示条目 "anthropic"
- [ ] `invoke('key_get', {provider:"anthropic"})` 返回 "sk-ant-test"
- [ ] `invoke('session_create', {title:"test"})` 返回 session 对象

## 风险/陷阱
- FTS5 中文 tokenizer: `unicode61 remove_diacritics 2` 对中文已可基本切词;问题严重再换 `trigram`(SQLite ≥3.34)
- `keyring` 在 spawn_blocking 内运行(它是同步 API)
- Keychain 在 headless 环境(CI)不可用;开发在 mac desktop OK