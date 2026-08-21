CREATE TABLE message_directory_scopes (
    message_id TEXT NOT NULL REFERENCES messages (id) ON DELETE CASCADE,
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    PRIMARY KEY (message_id, name)
) STRICT;
