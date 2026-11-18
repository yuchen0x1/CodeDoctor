CREATE TABLE schema_metadata (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL
);

CREATE TABLE projects (
    id TEXT PRIMARY KEY,
    normalized_path TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE scan_runs (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id),
    status TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT,
    config TEXT NOT NULL,
    rule_version TEXT NOT NULL,
    analyzer_versions TEXT NOT NULL,
    total_files INTEGER NOT NULL DEFAULT 0,
    scanned_files INTEGER NOT NULL DEFAULT 0,
    total_issues INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE files (
    id TEXT PRIMARY KEY,
    scan_run_id TEXT NOT NULL REFERENCES scan_runs(id),
    project_id TEXT NOT NULL REFERENCES projects(id),
    relative_path TEXT NOT NULL,
    absolute_path TEXT NOT NULL,
    encoding TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    content_hash TEXT NOT NULL,
    language TEXT NOT NULL,
    parse_capability TEXT NOT NULL,
    excluded_reason TEXT,
    UNIQUE (scan_run_id, relative_path)
);

CREATE TABLE issues (
    id TEXT PRIMARY KEY,
    scan_run_id TEXT NOT NULL REFERENCES scan_runs(id),
    project_id TEXT NOT NULL REFERENCES projects(id),
    rule_id TEXT NOT NULL,
    category TEXT NOT NULL,
    severity TEXT NOT NULL,
    file_path TEXT NOT NULL,
    start_line INTEGER NOT NULL,
    start_column INTEGER NOT NULL,
    end_line INTEGER NOT NULL,
    end_column INTEGER NOT NULL,
    title TEXT NOT NULL,
    explanation TEXT NOT NULL,
    evidence TEXT NOT NULL,
    impact TEXT NOT NULL,
    recommendation TEXT NOT NULL,
    source TEXT NOT NULL,
    coverage TEXT NOT NULL,
    confidence REAL NOT NULL,
    fingerprint TEXT NOT NULL
);

CREATE TABLE architecture_edges (
    id TEXT NOT NULL,
    scan_run_id TEXT NOT NULL REFERENCES scan_runs(id),
    source_path TEXT NOT NULL,
    target_path TEXT NOT NULL,
    kind TEXT NOT NULL,
    confidence REAL NOT NULL,
    coverage TEXT NOT NULL,
    metadata TEXT NOT NULL,
    PRIMARY KEY (scan_run_id, id)
);

CREATE TABLE analyzer_runs (
    id TEXT NOT NULL,
    scan_run_id TEXT NOT NULL REFERENCES scan_runs(id),
    analyzer TEXT NOT NULL,
    status TEXT NOT NULL,
    version TEXT,
    duration_ms INTEGER,
    message TEXT,
    PRIMARY KEY (scan_run_id, id)
);

CREATE TABLE score_snapshots (
    id TEXT PRIMARY KEY,
    scan_run_id TEXT NOT NULL UNIQUE REFERENCES scan_runs(id),
    overall_score REAL NOT NULL,
    dimensions TEXT NOT NULL,
    coverage REAL NOT NULL,
    confidence REAL NOT NULL,
    rule_version TEXT NOT NULL,
    analyzer_versions TEXT NOT NULL,
    config TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE prompt_artifacts (
    id TEXT NOT NULL,
    scan_run_id TEXT NOT NULL REFERENCES scan_runs(id),
    mode TEXT NOT NULL,
    selected_issue_ids TEXT NOT NULL,
    template_version TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (scan_run_id, id)
);

CREATE INDEX idx_files_project_id ON files(project_id);
CREATE INDEX idx_files_run_id ON files(scan_run_id);
CREATE INDEX idx_files_relative_path ON files(relative_path);
CREATE INDEX idx_scan_runs_project_id ON scan_runs(project_id);
CREATE INDEX idx_issues_project_id ON issues(project_id);
CREATE INDEX idx_issues_run_id ON issues(scan_run_id);
CREATE INDEX idx_issues_fingerprint ON issues(fingerprint);
CREATE INDEX idx_issues_file_path ON issues(file_path);
CREATE INDEX idx_issues_severity ON issues(severity);
CREATE INDEX idx_issues_category ON issues(category);
CREATE INDEX idx_edges_run_id ON architecture_edges(scan_run_id);
CREATE INDEX idx_analyzer_runs_run_id ON analyzer_runs(scan_run_id);
CREATE INDEX idx_prompts_run_id ON prompt_artifacts(scan_run_id);
