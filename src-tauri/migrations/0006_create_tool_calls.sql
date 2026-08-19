CREATE TABLE tool_calls (
    id TEXT PRIMARY KEY NOT NULL,
    run_id TEXT NOT NULL REFERENCES runs (id) ON DELETE CASCADE,
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    arguments_json TEXT NOT NULL,
    arguments_digest TEXT NOT NULL CHECK (length(arguments_digest) = 64),
    result_json TEXT,
    status TEXT NOT NULL CHECK (
        status IN (
            'requested', 'waiting_approval', 'running', 'completed',
            'failed', 'rejected', 'cancelled'
        )
    ),
    risk_level TEXT NOT NULL CHECK (risk_level IN ('read_only', 'write')),
    approval_policy TEXT NOT NULL CHECK (approval_policy IN ('never', 'always')),
    error_code TEXT,
    error_message TEXT,
    approval_expires_at TEXT,
    decided_at TEXT,
    created_at TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    completed_at TEXT
) STRICT;

CREATE INDEX tool_calls_run_id_created_at
    ON tool_calls (run_id, created_at);
