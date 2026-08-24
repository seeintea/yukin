CREATE TABLE imported_skills (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL COLLATE NOCASE CHECK (length(trim(name)) > 0),
    description TEXT NOT NULL,
    source_kind TEXT NOT NULL CHECK (source_kind IN ('directory', 'archive')),
    managed_path TEXT NOT NULL,
    content_digest TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    deleted_at TEXT
) STRICT;

CREATE UNIQUE INDEX imported_skills_name_active_unique
    ON imported_skills (name)
    WHERE deleted_at IS NULL;

CREATE TABLE mcp_servers (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL COLLATE NOCASE CHECK (length(trim(name)) > 0),
    display_name TEXT,
    version TEXT NOT NULL CHECK (length(trim(version)) > 0),
    description TEXT NOT NULL,
    author_name TEXT NOT NULL,
    server_type TEXT NOT NULL CHECK (server_type IN ('node', 'python', 'binary', 'uv')),
    managed_path TEXT NOT NULL,
    manifest_json TEXT NOT NULL CHECK (json_valid(manifest_json)),
    enabled INTEGER NOT NULL DEFAULT 0 CHECK (enabled IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    deleted_at TEXT
) STRICT;

CREATE UNIQUE INDEX mcp_servers_name_active_unique
    ON mcp_servers (name)
    WHERE deleted_at IS NULL;
