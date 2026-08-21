CREATE TABLE message_attachments (
    message_id TEXT NOT NULL REFERENCES messages (id) ON DELETE CASCADE,
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    size INTEGER NOT NULL CHECK (size >= 0),
    PRIMARY KEY (message_id, name)
) STRICT;
