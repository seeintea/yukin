CREATE TABLE run_skills (
    run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
    skill_id TEXT NOT NULL,
    skill_version TEXT NOT NULL,
    PRIMARY KEY (run_id, skill_id)
) STRICT;
