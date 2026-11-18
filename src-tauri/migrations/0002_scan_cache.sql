CREATE TABLE scan_cache (
    cache_key TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id),
    issues_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_scan_cache_project ON scan_cache(project_id);
