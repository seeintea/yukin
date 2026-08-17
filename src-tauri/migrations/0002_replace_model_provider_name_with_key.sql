CREATE TABLE model_providers_new (
    id TEXT PRIMARY KEY NOT NULL,
    provider_key TEXT NOT NULL CHECK (length(trim(provider_key)) > 0),
    api_format TEXT NOT NULL CHECK (length(trim(api_format)) > 0),
    base_url TEXT NOT NULL CHECK (length(trim(base_url)) > 0),
    provider_alias TEXT NOT NULL COLLATE NOCASE
        CHECK (length(trim(provider_alias)) > 0),
    api_key_alias TEXT NOT NULL
        CHECK (length(trim(api_key_alias)) > 0),
    created_at TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL
        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    deleted_at TEXT
) STRICT;

INSERT INTO model_providers_new (
    id,
    provider_key,
    api_format,
    base_url,
    provider_alias,
    api_key_alias,
    created_at,
    updated_at,
    deleted_at
)
SELECT
    id,
    'deepseek',
    api_format,
    base_url,
    provider_alias,
    api_key_alias,
    created_at,
    updated_at,
    deleted_at
FROM model_providers;

DROP TABLE model_providers;
ALTER TABLE model_providers_new RENAME TO model_providers;

CREATE UNIQUE INDEX model_providers_provider_alias_active_unique
    ON model_providers (provider_alias)
    WHERE deleted_at IS NULL;

CREATE UNIQUE INDEX model_providers_api_key_alias_active_unique
    ON model_providers (api_key_alias)
    WHERE deleted_at IS NULL;
