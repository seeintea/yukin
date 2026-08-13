CREATE TABLE model_providers (
    id TEXT PRIMARY KEY NOT NULL,
    provider_name TEXT NOT NULL CHECK (length(trim(provider_name)) > 0),
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

CREATE UNIQUE INDEX model_providers_provider_alias_active_unique
    ON model_providers (provider_alias)
    WHERE deleted_at IS NULL;

CREATE UNIQUE INDEX model_providers_api_key_alias_active_unique
    ON model_providers (api_key_alias)
    WHERE deleted_at IS NULL;
