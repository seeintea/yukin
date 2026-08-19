ALTER TABLE messages RENAME TO messages_old;
ALTER TABLE runs RENAME TO runs_old;

CREATE TABLE runs (
    id TEXT PRIMARY KEY NOT NULL,
    conversation_id TEXT NOT NULL REFERENCES conversations (id) ON DELETE CASCADE,
    provider_id TEXT NOT NULL REFERENCES model_providers (id),
    model_id TEXT NOT NULL CHECK (length(trim(model_id)) > 0),
    reasoning_effort TEXT,
    status TEXT NOT NULL CHECK (
        status IN ('pending', 'running', 'waiting_approval', 'completed', 'failed', 'cancelled')
    ),
    error_code TEXT,
    error_message TEXT,
    prompt_tokens INTEGER,
    completion_tokens INTEGER,
    total_tokens INTEGER,
    created_at TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    started_at TEXT,
    completed_at TEXT
) STRICT;

INSERT INTO runs (
    id, conversation_id, provider_id, model_id, reasoning_effort, status,
    error_message, prompt_tokens, completion_tokens, total_tokens,
    created_at, started_at, completed_at
)
SELECT
    id, conversation_id, provider_id, model_id, reasoning_effort, status,
    error_message, prompt_tokens, completion_tokens, total_tokens,
    created_at, created_at, completed_at
FROM runs_old;

CREATE TABLE messages (
    id TEXT PRIMARY KEY NOT NULL,
    conversation_id TEXT NOT NULL REFERENCES conversations (id) ON DELETE CASCADE,
    run_id TEXT REFERENCES runs (id) ON DELETE SET NULL,
    role TEXT NOT NULL CHECK (role IN ('user', 'assistant', 'tool')),
    content TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('streaming', 'completed', 'failed', 'cancelled')),
    sequence INTEGER NOT NULL CHECK (sequence > 0),
    created_at TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (conversation_id, sequence)
) STRICT;

INSERT INTO messages (
    id, conversation_id, run_id, role, content, status, sequence, created_at, updated_at
)
SELECT
    id, conversation_id, run_id, role, content, status, sequence, created_at, updated_at
FROM messages_old;

DROP TABLE messages_old;
DROP TABLE runs_old;

CREATE INDEX runs_conversation_id_created_at
    ON runs (conversation_id, created_at);

CREATE INDEX messages_conversation_id_sequence
    ON messages (conversation_id, sequence);
