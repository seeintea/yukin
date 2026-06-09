-- 0001_init.sql
-- Phase C / C1: 初始 schema —— 5 张表 + FTS5 虚拟表 + 3 个同步 trigger + 3 个 index
--
-- 设计依据:
--   docs/plan/2026-06-06-phase-c-sqlite-keychain-session.md (架构定义)
--   docs/plan/learning-notes/C1-sqlite-schema-and-fts5.md   (字段语义 + FTS5 决策)
--
-- 约定:
--   * datetime 用 TEXT + datetime('now') (无 T 无时区,C3 会撞 chrono 解析格式问题)
--   * BOOLEAN 用 INT (SQLite 没有原生 boolean)
--   * id 用 TEXT (uuid v4),不用自增,跨设备/导出友好
--   * 外键 cascade 需要每次连接打开 PRAGMA foreign_keys=ON (C2 在 AppState::new 里加)

------------------------------------------------------------------------
-- settings — 全局 K-V 配置
------------------------------------------------------------------------
CREATE TABLE settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

------------------------------------------------------------------------
-- providers — LLM provider 注册表 (key 本身在 keychain,这里只存元数据)
------------------------------------------------------------------------
CREATE TABLE providers (
  name          TEXT PRIMARY KEY,
  has_key       INT  NOT NULL DEFAULT 0,
  default_model TEXT,
  created_at    TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

------------------------------------------------------------------------
-- memory — 用户记忆条目 (跨会话持久化)
------------------------------------------------------------------------
CREATE TABLE memory (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL,
  kind        TEXT NOT NULL CHECK (kind IN ('user','feedback','project','reference')),
  description TEXT,
  content     TEXT NOT NULL,
  metadata    TEXT NOT NULL DEFAULT '{}',
  workspace   TEXT,
  created_at  TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX memory_kind_idx      ON memory(kind);
CREATE INDEX memory_workspace_idx ON memory(workspace);

------------------------------------------------------------------------
-- memory_fts — FTS5 全文索引 (外部内容模式)
------------------------------------------------------------------------
CREATE VIRTUAL TABLE memory_fts USING fts5(
  name, description, content,
  content='memory',
  content_rowid='rowid',
  tokenize='unicode61 remove_diacritics 2'
);

-- INSERT 同步:新插入的 memory 行 → 索引
CREATE TRIGGER memory_ai AFTER INSERT ON memory BEGIN
  INSERT INTO memory_fts(rowid, name, description, content)
  VALUES (new.rowid, new.name, new.description, new.content);
END;

-- DELETE 同步:用 FTS5 'delete' 特殊命令
CREATE TRIGGER memory_ad AFTER DELETE ON memory BEGIN
  INSERT INTO memory_fts(memory_fts, rowid, name, description, content)
  VALUES ('delete', old.rowid, old.name, old.description, old.content);
END;

-- UPDATE 同步:先 'delete' 旧索引,再 insert 新索引
CREATE TRIGGER memory_au AFTER UPDATE ON memory BEGIN
  INSERT INTO memory_fts(memory_fts, rowid, name, description, content)
  VALUES ('delete', old.rowid, old.name, old.description, old.content);
  INSERT INTO memory_fts(rowid, name, description, content)
  VALUES (new.rowid, new.name, new.description, new.content);
END;

------------------------------------------------------------------------
-- sessions — 对话会话
------------------------------------------------------------------------
CREATE TABLE sessions (
  id             TEXT PRIMARY KEY,
  title          TEXT NOT NULL,
  workspace_path TEXT,
  provider       TEXT,
  model          TEXT,
  created_at     TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at     TEXT NOT NULL DEFAULT (datetime('now'))
);

------------------------------------------------------------------------
-- messages — 单条消息 (1:N to sessions, cascade on delete)
------------------------------------------------------------------------
CREATE TABLE messages (
  id           TEXT PRIMARY KEY,
  session_id   TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
  role         TEXT NOT NULL CHECK (role IN ('system','user','assistant','tool')),
  content      TEXT NOT NULL,     -- JSON (Anthropic Messages 格式)
  tool_calls   TEXT,              -- JSON, Phase G 填
  tool_results TEXT,              -- JSON, Phase G 填
  step_index   INTEGER,
  created_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX messages_session_idx ON messages(session_id, created_at);
