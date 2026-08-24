ALTER TABLE mcp_servers
ADD COLUMN source_kind TEXT NOT NULL DEFAULT 'bundle'
CHECK (source_kind IN ('bundle', 'command'));

ALTER TABLE mcp_servers
ADD COLUMN command TEXT;

ALTER TABLE mcp_servers
ADD COLUMN args_json TEXT NOT NULL DEFAULT '[]'
CHECK (json_valid(args_json) AND json_type(args_json) = 'array');
