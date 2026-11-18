#[cfg(test)]
mod tests {
    use crate::domain::types::{
        ArchitectureEdge, CoverageKind, FileRecord, Issue, IssueFingerprint, ProjectRecord,
        PromptArtifact, PromptMode, ScanRun, ScanStatus, ScoreSnapshot, Severity,
    };
    use crate::storage::{IssueDiffStatus, Storage};
    use serde_json::json;

    #[test]
    fn migration_and_repository_round_trip_scan_data() {
        let mut storage = Storage::in_memory().unwrap();
        assert_eq!(storage.schema_version().unwrap(), 2);

        storage.insert_project(&test_project()).unwrap();
        storage
            .create_scan_run(&test_scan_run("run-1", ScanStatus::Running))
            .unwrap();
        storage.upsert_file(&test_file("run-1")).unwrap();
        storage
            .insert_issue("run-1", &test_issue("issue-1", "unused import os"))
            .unwrap();
        storage
            .insert_edge("run-1", &test_edge("src/a.py", "src/b.py"))
            .unwrap();
        storage
            .save_prompt("run-1", &test_prompt("prompt-1"))
            .unwrap();
        storage.save_score("run-1", &test_score("score-1")).unwrap();

        let score = storage.get_score("run-1").unwrap().unwrap();
        assert_eq!(score.id, "score-1");
        assert_eq!(score.overall_score, 82.5);

        let issues = storage.list_issues("run-1").unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "rule.unused-import");
        assert_eq!(issues[0].severity, Severity::Medium);
    }

    #[test]
    fn get_project_returns_the_persisted_project_record() {
        let mut storage = Storage::in_memory().unwrap();
        storage.insert_project(&test_project()).unwrap();

        let project = storage.get_project("project-1").unwrap().unwrap();
        assert_eq!(project.id, "project-1");
        assert_eq!(project.normalized_path, "C:/projects/example");
        assert!(storage.get_project("missing-project").unwrap().is_none());
    }

    #[test]
    fn get_file_for_run_matches_both_run_id_and_relative_path() {
        let mut storage = Storage::in_memory().unwrap();
        storage.insert_project(&test_project()).unwrap();
        storage
            .create_scan_run(&test_scan_run("run-1", ScanStatus::Completed))
            .unwrap();
        storage
            .create_scan_run(&test_scan_run("run-2", ScanStatus::Completed))
            .unwrap();
        storage.upsert_file(&test_file("run-1")).unwrap();
        storage.upsert_file(&test_file("run-2")).unwrap();

        let first = storage
            .get_file_for_run("run-1", "src/example.py")
            .unwrap()
            .unwrap();
        let second = storage
            .get_file_for_run("run-2", "src/example.py")
            .unwrap()
            .unwrap();

        assert_eq!(first.scan_run_id, "run-1");
        assert_eq!(second.scan_run_id, "run-2");
        assert!(
            storage
                .get_file_for_run("run-1", "src/missing.py")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn compare_issue_fingerprints_reports_added_resolved_and_changed_issues() {
        let mut storage = Storage::in_memory().unwrap();
        storage.insert_project(&test_project()).unwrap();
        storage
            .create_scan_run(&test_scan_run("baseline", ScanStatus::Completed))
            .unwrap();
        storage
            .create_scan_run(&test_scan_run("current", ScanStatus::Completed))
            .unwrap();

        let mut baseline = test_issue("baseline-persistent", "unused import os");
        baseline.severity = Severity::Medium;
        let baseline_resolved = test_issue("baseline-resolved", "duplicate helper function");
        storage.insert_issue("baseline", &baseline).unwrap();
        storage
            .insert_issue("baseline", &baseline_resolved)
            .unwrap();

        let mut current = test_issue("current-persistent", "unused import os");
        current.severity = Severity::High;
        let current_added = test_issue("current-added", "deeply nested condition");
        storage.insert_issue("current", &current).unwrap();
        storage.insert_issue("current", &current_added).unwrap();

        let diffs = storage
            .compare_issue_fingerprints("baseline", "current")
            .unwrap();
        let by_fingerprint: std::collections::HashMap<_, _> = diffs
            .into_iter()
            .map(|diff| (diff.fingerprint.clone(), diff))
            .collect();

        let persistent = by_fingerprint
            .get(&IssueFingerprint::for_issue(&baseline))
            .unwrap();
        assert!(matches!(
            persistent.status,
            IssueDiffStatus::SeverityChanged
        ));
        assert_eq!(
            persistent.current_issue_id.as_deref(),
            Some("current-persistent")
        );

        assert!(by_fingerprint.contains_key(&IssueFingerprint::for_issue(&current_added)));
        assert!(by_fingerprint.contains_key(&IssueFingerprint::for_issue(&baseline_resolved)));
    }

    #[test]
    fn phase_batch_rolls_back_all_records_on_failure() {
        let mut storage = Storage::in_memory().unwrap();
        storage.insert_project(&test_project()).unwrap();
        storage
            .create_scan_run(&test_scan_run("run-1", ScanStatus::Running))
            .unwrap();

        let issue = test_issue("issue-1", "unused import os");
        let result = storage.with_phase(|phase| {
            phase.upsert_file(&test_file("run-1"))?;
            phase.insert_issue("run-1", &issue)?;
            phase.insert_issue("run-1", &issue)?;
            Ok(())
        });

        assert!(result.is_err());
        let file_count: i64 = storage
            .conn
            .query_row(
                "SELECT COUNT(*) FROM files WHERE scan_run_id = 'run-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(file_count, 0);
        assert!(storage.list_issues("run-1").unwrap().is_empty());
    }

    #[test]
    fn upsert_file_rejects_a_project_id_that_does_not_match_the_scan_run() {
        let mut storage = Storage::in_memory().unwrap();
        storage.insert_project(&test_project()).unwrap();
        storage
            .create_scan_run(&test_scan_run("run-1", ScanStatus::Running))
            .unwrap();

        let mut foreign_file = test_file("run-1");
        foreign_file.project_id = "project-other".to_owned();

        assert!(storage.upsert_file(&foreign_file).is_err());
    }

    #[test]
    fn insert_issue_derives_fingerprint_instead_of_trusting_caller_value() {
        let mut storage = Storage::in_memory().unwrap();
        storage.insert_project(&test_project()).unwrap();
        storage
            .create_scan_run(&test_scan_run("run-1", ScanStatus::Running))
            .unwrap();

        let mut issue = test_issue("issue-1", "unused import os");
        issue.fingerprint = "caller-supplied-fingerprint".to_owned();
        let expected = IssueFingerprint::for_issue(&issue);
        storage.insert_issue("run-1", &issue).unwrap();

        let stored = storage.list_issues("run-1").unwrap();
        assert_eq!(stored[0].fingerprint, expected);
    }

    #[test]
    fn comparison_ignores_a_stale_persisted_fingerprint() {
        let mut storage = Storage::in_memory().unwrap();
        storage.insert_project(&test_project()).unwrap();
        storage
            .create_scan_run(&test_scan_run("baseline", ScanStatus::Completed))
            .unwrap();
        storage
            .create_scan_run(&test_scan_run("current", ScanStatus::Completed))
            .unwrap();

        storage
            .insert_issue(
                "baseline",
                &test_issue("baseline-issue", "unused import os"),
            )
            .unwrap();
        storage
            .insert_issue("current", &test_issue("current-issue", "unused import os"))
            .unwrap();
        storage
            .conn
            .execute(
                "UPDATE issues SET fingerprint = 'stale-history-value' WHERE scan_run_id = 'baseline'",
                [],
            )
            .unwrap();

        let diffs = storage
            .compare_issue_fingerprints("baseline", "current")
            .unwrap();
        assert_eq!(diffs.len(), 1);
        assert!(matches!(diffs[0].status, IssueDiffStatus::Persistent));
    }

    fn test_project() -> ProjectRecord {
        ProjectRecord {
            id: "project-1".to_owned(),
            name: "Example".to_owned(),
            normalized_path: "C:/projects/example".to_owned(),
            created_at: "2026-11-17T00:00:00Z".to_owned(),
        }
    }

    fn test_scan_run(id: &str, status: ScanStatus) -> ScanRun {
        ScanRun {
            id: id.to_owned(),
            project_id: "project-1".to_owned(),
            status,
            started_at: Some("2026-11-17T00:00:00Z".to_owned()),
            completed_at: None,
            config: json!({"mode": "full"}),
            rule_version: "rules-1".to_owned(),
            analyzer_versions: json!({}),
            total_files: 10,
            scanned_files: 10,
            total_issues: 2,
        }
    }

    fn test_file(scan_run_id: &str) -> FileRecord {
        FileRecord {
            id: format!("file-{scan_run_id}"),
            scan_run_id: scan_run_id.to_owned(),
            project_id: "project-1".to_owned(),
            relative_path: "src/example.py".to_owned(),
            absolute_path: "C:/projects/example/src/example.py".to_owned(),
            encoding: "UTF-8".to_owned(),
            size_bytes: 128,
            content_hash: "hash-1".to_owned(),
            language: "python".to_owned(),
            parse_capability: CoverageKind::Ast,
            excluded_reason: None,
        }
    }

    fn test_issue(id: &str, evidence: &str) -> Issue {
        Issue {
            id: id.to_owned(),
            rule_id: "rule.unused-import".to_owned(),
            category: "quality".to_owned(),
            severity: Severity::Medium,
            file_path: "src/example.py".to_owned(),
            start_line: 7,
            start_column: 3,
            end_line: 7,
            end_column: 18,
            title: "Unused import".to_owned(),
            explanation: "The import is never referenced.".to_owned(),
            evidence: evidence.to_owned(),
            impact: "Dependency surface is harder to reason about.".to_owned(),
            recommendation: "Remove the unused import.".to_owned(),
            source: "codedoctor".to_owned(),
            coverage: CoverageKind::Ast,
            confidence: 0.94,
            fingerprint: String::new(),
        }
    }

    fn test_edge(source: &str, target: &str) -> ArchitectureEdge {
        ArchitectureEdge {
            id: format!("{source}->{target}"),
            source_path: source.to_owned(),
            target_path: target.to_owned(),
            kind: "import".to_owned(),
            confidence: 0.9,
            coverage: CoverageKind::Ast,
            metadata: json!({}),
        }
    }

    fn test_prompt(id: &str) -> PromptArtifact {
        PromptArtifact {
            id: id.to_owned(),
            mode: PromptMode::Comprehensive,
            selected_issue_ids: vec!["issue-1".to_owned()],
            template_version: "prompt-1".to_owned(),
            content: "治理提示词".to_owned(),
            created_at: "2026-11-17T00:00:00Z".to_owned(),
        }
    }

    fn test_score(id: &str) -> ScoreSnapshot {
        ScoreSnapshot {
            id: id.to_owned(),
            overall_score: 82.5,
            dimensions: json!({"readability": 80}),
            coverage: 0.85,
            confidence: 0.9,
            rule_version: "rules-1".to_owned(),
            analyzer_versions: json!({}),
            config: json!({"mode": "full"}),
            created_at: "2026-11-17T00:00:00Z".to_owned(),
        }
    }
}
mod migrations;

use crate::domain::types::{
    AnalyzerRun, AnalyzerStatus, ArchitectureEdge, CoverageKind, FileRecord, Issue,
    IssueFingerprint, ProjectRecord, PromptArtifact, PromptMode, ScanRun, ScanStatus,
    ScoreSnapshot, Severity,
};
use crate::error::AppError;
pub use migrations::MIGRATION_V1;
use rusqlite::{Connection, Transaction, params};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IssueDiffStatus {
    Added,
    Resolved,
    Persistent,
    SeverityChanged,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IssueDiff {
    pub fingerprint: String,
    pub status: IssueDiffStatus,
    pub baseline_issue_id: Option<String>,
    pub current_issue_id: Option<String>,
    pub baseline_severity: Option<Severity>,
    pub current_severity: Option<Severity>,
}

#[cfg(any(feature = "desktop", test))]
pub(crate) struct IssueLocation {
    pub file_path: String,
    pub start_line: u32,
    pub end_line: u32,
    pub rule_id: String,
}

pub struct Storage {
    conn: Connection,
}

/// A transaction spanning one complete scan phase.
///
/// Dropping this handle without a successful return from [`Storage::with_phase`]
/// rolls back every write performed through the handle.
pub struct PhaseTransaction<'conn> {
    transaction: Transaction<'conn>,
}

impl Storage {
    pub fn open(path: &str) -> Result<Self, AppError> {
        Self::new(Connection::open(path)?)
    }

    pub fn in_memory() -> Result<Self, AppError> {
        Self::new(Connection::open_in_memory()?)
    }

    #[cfg(all(windows, feature = "desktop"))]
    pub fn backup(&self, destination: &std::path::Path) -> Result<(), AppError> {
        self.conn.backup(
            rusqlite::MAIN_DB,
            destination,
            rusqlite::backup::NO_PROGRESS,
        )?;
        Ok(())
    }

    fn new(mut conn: Connection) -> Result<Self, AppError> {
        migrations::run(&mut conn)?;
        Ok(Self { conn })
    }

    pub fn schema_version(&self) -> Result<i64, AppError> {
        let version = self.conn.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_metadata",
            [],
            |row| row.get(0),
        )?;
        Ok(version)
    }

    pub fn count_files_for_project(&self, project_id: &str) -> Result<u64, AppError> {
        let count = self.conn.query_row(
            "SELECT COUNT(*) FROM files WHERE project_id = ?1",
            params![project_id],
            |row| row.get::<_, i64>(0),
        )?;
        Ok(count as u64)
    }

    pub fn count_files_for_run(&self, run_id: &str) -> Result<u32, AppError> {
        let count = self.conn.query_row(
            "SELECT COUNT(*) FROM files WHERE scan_run_id = ?1",
            params![run_id],
            |row| row.get::<_, i64>(0),
        )?;
        Ok(count.max(0) as u32)
    }

    pub fn count_source_files_for_run(&self, run_id: &str) -> Result<u32, AppError> {
        let count = self.conn.query_row(
            "SELECT COUNT(*) FROM files WHERE scan_run_id = ?1 AND excluded_reason IS NULL",
            params![run_id],
            |row| row.get::<_, i64>(0),
        )?;
        Ok(count.max(0) as u32)
    }

    pub fn with_phase<T, F>(&mut self, operation: F) -> Result<T, AppError>
    where
        F: FnOnce(&mut PhaseTransaction<'_>) -> Result<T, AppError>,
    {
        let transaction = self.conn.transaction()?;
        let mut phase = PhaseTransaction { transaction };
        let value = operation(&mut phase)?;
        phase.transaction.commit()?;
        Ok(value)
    }

    pub fn insert_project(&mut self, project: &ProjectRecord) -> Result<(), AppError> {
        let transaction = self.conn.transaction()?;
        transaction.execute(
            "INSERT INTO projects (id, normalized_path, name, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![
                project.id,
                project.normalized_path,
                project.name,
                project.created_at
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn create_scan_run(&mut self, run: &ScanRun) -> Result<(), AppError> {
        let transaction = self.conn.transaction()?;
        transaction.execute(
            "INSERT INTO scan_runs
             (id, project_id, status, started_at, completed_at, config, rule_version,
              analyzer_versions, total_files, scanned_files, total_issues)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                run.id,
                run.project_id,
                scan_status_name(&run.status),
                run.started_at,
                run.completed_at,
                run.config.to_string(),
                run.rule_version,
                run.analyzer_versions.to_string(),
                run.total_files,
                run.scanned_files,
                run.total_issues,
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn upsert_file(&mut self, file: &FileRecord) -> Result<(), AppError> {
        self.with_phase(|phase| phase.upsert_file(file))
    }

    pub fn insert_issue(&mut self, run_id: &str, issue: &Issue) -> Result<(), AppError> {
        self.with_phase(|phase| phase.insert_issue(run_id, issue))
    }

    pub fn insert_edge(&mut self, run_id: &str, edge: &ArchitectureEdge) -> Result<(), AppError> {
        self.with_phase(|phase| phase.insert_edge(run_id, edge))
    }

    pub fn save_score(&mut self, run_id: &str, score: &ScoreSnapshot) -> Result<(), AppError> {
        self.with_phase(|phase| phase.save_score(run_id, score))
    }

    pub fn get_score(&self, run_id: &str) -> Result<Option<ScoreSnapshot>, AppError> {
        let mut statement = self.conn.prepare(
            "SELECT id, overall_score, dimensions, coverage, confidence,
                    rule_version, analyzer_versions, config, created_at
             FROM score_snapshots WHERE scan_run_id = ?1",
        )?;
        let mut rows = statement.query(params![run_id])?;
        rows.next()?
            .map(row_to_score)
            .transpose()
            .map_err(AppError::from)
    }

    pub fn save_prompt(&mut self, run_id: &str, prompt: &PromptArtifact) -> Result<(), AppError> {
        self.with_phase(|phase| phase.save_prompt(run_id, prompt))
    }

    /// Reads a stored preview's text and creation time for the given scan.
    pub fn get_prompt_content(
        &self,
        run_id: &str,
        prompt_id: &str,
    ) -> Result<Option<(String, String)>, AppError> {
        let mut statement = self.conn.prepare(
            "SELECT content, created_at FROM prompt_artifacts WHERE scan_run_id = ?1 AND id = ?2",
        )?;
        let mut rows = statement.query(params![run_id, prompt_id])?;
        rows.next()?
            .map(|row| -> rusqlite::Result<_> { Ok((row.get(0)?, row.get(1)?)) })
            .transpose()
            .map_err(AppError::from)
    }

    pub fn list_issues(&self, run_id: &str) -> Result<Vec<Issue>, AppError> {
        let mut statement = self.conn.prepare(
            "SELECT id, rule_id, category, severity, file_path, start_line, start_column,
                    end_line, end_column, title, explanation, evidence, impact,
                    recommendation, source, coverage, confidence, fingerprint
             FROM issues WHERE scan_run_id = ?1 ORDER BY file_path, start_line, id",
        )?;
        let issues = statement
            .query_map(params![run_id], row_to_issue)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(issues)
    }

    pub fn list_files_for_run(&self, run_id: &str) -> Result<Vec<FileRecord>, AppError> {
        let mut statement = self.conn.prepare(
            "SELECT id, scan_run_id, project_id, relative_path, absolute_path, encoding,
                    size_bytes, content_hash, language, parse_capability, excluded_reason
             FROM files WHERE scan_run_id = ?1 ORDER BY relative_path",
        )?;
        let files = statement
            .query_map(params![run_id], row_to_file)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(files)
    }

    pub fn get_file_for_run(
        &self,
        run_id: &str,
        relative_path: &str,
    ) -> Result<Option<FileRecord>, AppError> {
        let mut statement = self.conn.prepare(
            "SELECT id, scan_run_id, project_id, relative_path, absolute_path, encoding,
                    size_bytes, content_hash, language, parse_capability, excluded_reason
             FROM files WHERE scan_run_id = ?1 AND relative_path = ?2",
        )?;
        let mut rows = statement.query(params![run_id, relative_path])?;
        rows.next()?
            .map(row_to_file)
            .transpose()
            .map_err(AppError::from)
    }

    #[cfg(any(feature = "desktop", test))]
    pub(crate) fn get_issue_location(
        &self,
        run_id: &str,
        issue_id: &str,
    ) -> Result<Option<IssueLocation>, AppError> {
        let mut statement = self.conn.prepare(
            "SELECT file_path, start_line, end_line, rule_id
             FROM issues WHERE scan_run_id = ?1 AND id = ?2",
        )?;
        let mut rows = statement.query(params![run_id, issue_id])?;
        rows.next()?
            .map(|row| -> rusqlite::Result<IssueLocation> {
                Ok(IssueLocation {
                    file_path: row.get(0)?,
                    start_line: row.get(1)?,
                    end_line: row.get(2)?,
                    rule_id: row.get(3)?,
                })
            })
            .transpose()
            .map_err(AppError::from)
    }

    /// Filtered, paginated issue query for the results UI.
    pub fn list_issues_filtered(
        &self,
        run_id: &str,
        filter: &IssueFilter,
    ) -> Result<(Vec<Issue>, i64), AppError> {
        let mut clauses = vec!["scan_run_id = ?1".to_owned()];
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(run_id.to_owned())];
        if let Some(severity) = &filter.severity {
            params_vec.push(Box::new(severity.clone()));
            clauses.push(format!("severity = ?{}", params_vec.len()));
        }
        if let Some(category) = &filter.category {
            params_vec.push(Box::new(category.clone()));
            clauses.push(format!("category = ?{}", params_vec.len()));
        }
        if let Some(file) = &filter.file_path {
            params_vec.push(Box::new(file.clone()));
            clauses.push(format!("file_path = ?{}", params_vec.len()));
        }
        if let Some(search) = &filter.search {
            params_vec.push(Box::new(format!("%{search}%")));
            clauses.push(format!(
                "(title LIKE ?{} OR explanation LIKE ?{} OR file_path LIKE ?{} OR rule_id LIKE ?{})",
                params_vec.len(),
                params_vec.len(),
                params_vec.len(),
                params_vec.len()
            ));
        }
        let where_clause = clauses.join(" AND ");
        let total: i64 = self.conn.query_row(
            &format!("SELECT COUNT(*) FROM issues WHERE {where_clause}"),
            rusqlite::params_from_iter(params_vec.iter().map(|p| p.as_ref())),
            |row| row.get(0),
        )?;
        let limit = filter.limit.unwrap_or(200).min(1000);
        let offset = filter.offset.unwrap_or(0);
        params_vec.push(Box::new(limit as i64));
        params_vec.push(Box::new(offset as i64));
        let mut statement = self.conn.prepare(&format!(
            "SELECT id, rule_id, category, severity, file_path, start_line, start_column,
                    end_line, end_column, title, explanation, evidence, impact,
                    recommendation, source, coverage, confidence, fingerprint
             FROM issues WHERE {where_clause}
             ORDER BY file_path, start_line, id LIMIT ?{} OFFSET ?{}",
            params_vec.len() - 1,
            params_vec.len()
        ))?;
        let issues = statement
            .query_map(
                rusqlite::params_from_iter(params_vec.iter().map(|p| p.as_ref())),
                row_to_issue,
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok((issues, total))
    }

    pub fn get_scan_run(&self, run_id: &str) -> Result<Option<ScanRun>, AppError> {
        let mut statement = self.conn.prepare(
            "SELECT id, project_id, status, started_at, completed_at, config, rule_version,
                    analyzer_versions, total_files, scanned_files, total_issues
             FROM scan_runs WHERE id = ?1",
        )?;
        let mut rows = statement
            .query_map(params![run_id], row_to_scan_run)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows.pop())
    }

    pub fn list_scan_runs(&self, project_id: &str) -> Result<Vec<ScanRun>, AppError> {
        let mut statement = self.conn.prepare(
            "SELECT id, project_id, status, started_at, completed_at, config, rule_version,
                    analyzer_versions, total_files, scanned_files, total_issues
             FROM scan_runs WHERE project_id = ?1 AND config != '{\"phase\":\"index\"}'
             ORDER BY started_at DESC",
        )?;
        let runs = statement
            .query_map(params![project_id], row_to_scan_run)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(runs)
    }

    pub fn list_projects(&self) -> Result<Vec<ProjectRecord>, AppError> {
        let mut statement = self.conn.prepare(
            "SELECT id, normalized_path, name, created_at FROM projects ORDER BY created_at DESC",
        )?;
        let projects = statement
            .query_map([], |row| {
                Ok(ProjectRecord {
                    id: row.get(0)?,
                    normalized_path: row.get(1)?,
                    name: row.get(2)?,
                    created_at: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(projects)
    }

    pub fn get_project(&self, project_id: &str) -> Result<Option<ProjectRecord>, AppError> {
        let mut statement = self
            .conn
            .prepare("SELECT id, normalized_path, name, created_at FROM projects WHERE id = ?1")?;
        let mut rows = statement.query(params![project_id])?;
        rows.next()?
            .map(row_to_project)
            .transpose()
            .map_err(AppError::from)
    }

    pub fn transition_scan_status(
        &mut self,
        run_id: &str,
        previous: ScanStatus,
        next: ScanStatus,
    ) -> Result<bool, AppError> {
        Ok(self.conn.execute(
            "UPDATE scan_runs SET status = ?3
             WHERE id = ?1 AND status = ?2 AND completed_at IS NULL",
            params![run_id, scan_status_name(&previous), scan_status_name(&next)],
        )? == 1)
    }

    pub fn has_active_scan(&self, project_id: &str) -> Result<bool, AppError> {
        Ok(self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM scan_runs WHERE project_id = ?1
             AND status IN ('queued', 'running', 'paused', 'canceling') AND completed_at IS NULL)",
            params![project_id],
            |row| row.get(0),
        )?)
    }

    pub fn finish_scan_run(
        &mut self,
        run_id: &str,
        status: ScanStatus,
        completed_at: &str,
        total_files: u32,
        scanned_files: u32,
    ) -> Result<u32, AppError> {
        Ok(self.conn.query_row(
            "UPDATE scan_runs SET status = ?2, completed_at = ?3,
                total_files = ?4, scanned_files = ?5,
                total_issues = (SELECT COUNT(*) FROM issues WHERE scan_run_id = ?1)
             WHERE id = ?1 AND completed_at IS NULL RETURNING total_issues",
            params![
                run_id,
                scan_status_name(&status),
                completed_at,
                total_files,
                scanned_files
            ],
            |row| row.get(0),
        )?)
    }

    pub fn recover_interrupted_scans(&mut self, recovered_at: &str) -> Result<(), AppError> {
        self.conn.execute(
            "UPDATE scan_runs SET
                config = CASE WHEN completed_at IS NULL THEN
                    json_set(CASE WHEN json_valid(config) THEN
                        CASE WHEN json_type(config) = 'object' THEN config ELSE '{}' END
                        ELSE '{}' END, '$.interrupted', json('true'))
                    ELSE config END,
                status = CASE WHEN status = 'canceling' THEN 'canceled' ELSE 'failed' END,
                completed_at = COALESCE(completed_at, ?1),
                total_files = MAX(total_files, (SELECT COUNT(*) FROM files
                    WHERE scan_run_id = scan_runs.id AND (excluded_reason IS NULL OR size_bytes > 0))),
                scanned_files = MAX(scanned_files, (SELECT COUNT(*) FROM files
                    WHERE scan_run_id = scan_runs.id AND (excluded_reason IS NULL OR size_bytes > 0))),
                total_issues = (SELECT COUNT(*) FROM issues WHERE scan_run_id = scan_runs.id)
             WHERE status IN ('queued', 'running', 'paused', 'canceling')",
            params![recovered_at],
        )?;
        Ok(())
    }

    pub fn list_analyzer_runs_for_run(&self, run_id: &str) -> Result<Vec<AnalyzerRun>, AppError> {
        let mut statement = self.conn.prepare(
            "SELECT id, analyzer, status, version, duration_ms, message
             FROM analyzer_runs WHERE scan_run_id = ?1 ORDER BY analyzer",
        )?;
        let results = statement.query_map(params![run_id], |row| {
            let status = match row.get::<_, String>(2)?.as_str() {
                "available" => AnalyzerStatus::Available,
                "executed" => AnalyzerStatus::Executed,
                "failed" => AnalyzerStatus::Failed,
                "skipped" => AnalyzerStatus::Skipped,
                "unavailable" => AnalyzerStatus::Unavailable,
                _ => return Err(rusqlite::Error::InvalidQuery),
            };
            Ok(AnalyzerRun {
                id: row.get(0)?,
                analyzer: row.get(1)?,
                status,
                version: row.get(3)?,
                duration_ms: row
                    .get::<_, Option<i64>>(4)?
                    .and_then(|value| value.try_into().ok()),
                message: row.get(5)?,
            })
        })?;
        Ok(results.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn insert_analyzer_run(
        &mut self,
        run_id: &str,
        analyzer_run: &AnalyzerRun,
    ) -> Result<(), AppError> {
        self.with_phase(|phase| phase.insert_analyzer_run(run_id, analyzer_run))
    }

    pub fn list_edges(&self, run_id: &str) -> Result<Vec<ArchitectureEdge>, AppError> {
        let mut statement = self.conn.prepare(
            "SELECT id, source_path, target_path, kind, confidence, coverage, metadata
             FROM architecture_edges WHERE scan_run_id = ?1 ORDER BY source_path, target_path",
        )?;
        let edges = statement
            .query_map(params![run_id], |row| {
                let coverage_name: String = row.get(5)?;
                Ok(ArchitectureEdge {
                    id: row.get(0)?,
                    source_path: row.get(1)?,
                    target_path: row.get(2)?,
                    kind: row.get(3)?,
                    confidence: row.get(4)?,
                    coverage: coverage_from_name(&coverage_name).map_err(|_| {
                        rusqlite::Error::InvalidColumnType(
                            5,
                            coverage_name.clone(),
                            rusqlite::types::Type::Text,
                        )
                    })?,
                    metadata: serde_json::from_str(&row.get::<_, String>(6)?)
                        .unwrap_or(serde_json::json!({})),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(edges)
    }

    /// Rule-result cache: unchanged files reuse persisted issues instead of
    /// re-running the rule engine.
    pub fn get_cached_issues(&self, cache_key: &str) -> Result<Option<Vec<Issue>>, AppError> {
        let json: Option<String> = self
            .conn
            .query_row(
                "SELECT issues_json FROM scan_cache WHERE cache_key = ?1",
                params![cache_key],
                |row| row.get(0),
            )
            .ok();
        match json {
            Some(text) => Ok(serde_json::from_str(&text).ok()),
            None => Ok(None),
        }
    }

    pub fn put_cached_issues(
        &mut self,
        cache_key: &str,
        project_id: &str,
        issues: &[Issue],
    ) -> Result<(), AppError> {
        let json = serde_json::to_string(issues)
            .map_err(|_| AppError::Validation("cache payload could not be stored".to_owned()))?;
        self.conn.execute(
            "INSERT INTO scan_cache (cache_key, project_id, issues_json, created_at)
             VALUES (?1, ?2, ?3, datetime('now'))
             ON CONFLICT(cache_key) DO UPDATE SET issues_json = excluded.issues_json",
            params![cache_key, project_id, json],
        )?;
        Ok(())
    }

    pub fn compare_issue_fingerprints(
        &self,
        baseline_run_id: &str,
        current_run_id: &str,
    ) -> Result<Vec<IssueDiff>, AppError> {
        let baseline = self.issues_by_fingerprint(baseline_run_id)?;
        let current = self.issues_by_fingerprint(current_run_id)?;
        let mut fingerprints: Vec<_> = baseline.keys().chain(current.keys()).cloned().collect();
        fingerprints.sort();
        fingerprints.dedup();

        let mut diffs = Vec::new();
        for fingerprint in fingerprints {
            let baseline_issue = baseline.get(&fingerprint);
            let current_issue = current.get(&fingerprint);
            let status = match (baseline_issue, current_issue) {
                (None, Some(_)) => IssueDiffStatus::Added,
                (Some(_), None) => IssueDiffStatus::Resolved,
                (Some(old), Some(new)) if old.severity != new.severity => {
                    IssueDiffStatus::SeverityChanged
                }
                (Some(_), Some(_)) => IssueDiffStatus::Persistent,
                (None, None) => continue,
            };
            diffs.push(IssueDiff {
                fingerprint,
                status,
                baseline_issue_id: baseline_issue.map(|issue| issue.id.clone()),
                current_issue_id: current_issue.map(|issue| issue.id.clone()),
                baseline_severity: baseline_issue.map(|issue| issue.severity),
                current_severity: current_issue.map(|issue| issue.severity),
            });
        }
        Ok(diffs)
    }

    fn issues_by_fingerprint(
        &self,
        run_id: &str,
    ) -> Result<std::collections::HashMap<String, Issue>, AppError> {
        let issues = self.list_issues(run_id)?;
        let mut map = std::collections::HashMap::new();
        for issue in issues {
            let fingerprint = IssueFingerprint::for_issue(&issue);
            map.entry(fingerprint).or_insert(issue);
        }
        Ok(map)
    }
}

impl<'conn> PhaseTransaction<'conn> {
    pub(crate) fn set_indexed_project_context(
        &mut self,
        run_id: &str,
        context: &serde_json::Value,
    ) -> Result<(), AppError> {
        self.transaction.execute(
            "UPDATE scan_runs SET config = json_set(config, '$.indexedProjectContext', json(?2)) WHERE id = ?1",
            params![run_id, context.to_string()],
        )?;
        Ok(())
    }

    pub fn set_analyzer_versions(
        &mut self,
        run_id: &str,
        versions: &serde_json::Value,
    ) -> Result<(), AppError> {
        self.transaction.execute(
            "UPDATE scan_runs SET analyzer_versions = ?2 WHERE id = ?1",
            params![run_id, versions.to_string()],
        )?;
        Ok(())
    }

    pub fn create_scan_run(&mut self, run: &ScanRun) -> Result<(), AppError> {
        self.transaction.execute(
            "INSERT INTO scan_runs
             (id, project_id, status, started_at, completed_at, config, rule_version,
              analyzer_versions, total_files, scanned_files, total_issues)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                run.id,
                run.project_id,
                scan_status_name(&run.status),
                run.started_at,
                run.completed_at,
                run.config.to_string(),
                run.rule_version,
                run.analyzer_versions.to_string(),
                run.total_files,
                run.scanned_files,
                run.total_issues,
            ],
        )?;
        Ok(())
    }

    pub fn upsert_file(&mut self, file: &FileRecord) -> Result<(), AppError> {
        let run_project_id: String = self.transaction.query_row(
            "SELECT project_id FROM scan_runs WHERE id = ?1",
            params![file.scan_run_id],
            |row| row.get(0),
        )?;
        if run_project_id != file.project_id {
            return Err(AppError::Validation(
                "file project does not match scan run project".to_owned(),
            ));
        }

        self.transaction.execute(
            "INSERT INTO files
             (id, scan_run_id, project_id, relative_path, absolute_path, encoding,
              size_bytes, content_hash, language, parse_capability, excluded_reason)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(scan_run_id, relative_path) DO UPDATE SET
               id = excluded.id,
               project_id = excluded.project_id,
               absolute_path = excluded.absolute_path,
               encoding = excluded.encoding,
               size_bytes = excluded.size_bytes,
               content_hash = excluded.content_hash,
               language = excluded.language,
               parse_capability = excluded.parse_capability,
               excluded_reason = excluded.excluded_reason",
            params![
                file.id,
                file.scan_run_id,
                run_project_id,
                file.relative_path,
                file.absolute_path,
                file.encoding,
                file.size_bytes as i64,
                file.content_hash,
                file.language,
                coverage_name(&file.parse_capability),
                file.excluded_reason,
            ],
        )?;
        Ok(())
    }

    pub fn insert_issue(&mut self, run_id: &str, issue: &Issue) -> Result<(), AppError> {
        let project_id: String = self.transaction.query_row(
            "SELECT project_id FROM scan_runs WHERE id = ?1",
            params![run_id],
            |row| row.get(0),
        )?;
        let fingerprint = IssueFingerprint::for_issue(issue);
        self.transaction.execute(
            "INSERT INTO issues
             (id, scan_run_id, project_id, rule_id, category, severity, file_path,
              start_line, start_column, end_line, end_column, title, explanation,
              evidence, impact, recommendation, source, coverage, confidence, fingerprint)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                     ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
            params![
                issue.id,
                run_id,
                project_id,
                issue.rule_id,
                issue.category,
                severity_name(&issue.severity),
                issue.file_path,
                issue.start_line,
                issue.start_column,
                issue.end_line,
                issue.end_column,
                issue.title,
                issue.explanation,
                crate::rules::safe_issue_evidence(&issue.rule_id, &issue.evidence),
                issue.impact,
                issue.recommendation,
                issue.source,
                coverage_name(&issue.coverage),
                issue.confidence,
                fingerprint,
            ],
        )?;
        Ok(())
    }

    pub fn insert_edge(&mut self, run_id: &str, edge: &ArchitectureEdge) -> Result<(), AppError> {
        self.transaction.execute(
            "INSERT INTO architecture_edges
             (id, scan_run_id, source_path, target_path, kind, confidence, coverage, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                edge.id,
                run_id,
                edge.source_path,
                edge.target_path,
                edge.kind,
                edge.confidence,
                coverage_name(&edge.coverage),
                edge.metadata.to_string(),
            ],
        )?;
        Ok(())
    }

    pub fn save_score(&mut self, run_id: &str, score: &ScoreSnapshot) -> Result<(), AppError> {
        self.transaction.execute(
            "INSERT INTO score_snapshots
             (id, scan_run_id, overall_score, dimensions, coverage, confidence,
              rule_version, analyzer_versions, config, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(scan_run_id) DO UPDATE SET
               id = excluded.id,
               overall_score = excluded.overall_score,
               dimensions = excluded.dimensions,
               coverage = excluded.coverage,
               confidence = excluded.confidence,
               rule_version = excluded.rule_version,
               analyzer_versions = excluded.analyzer_versions,
               config = excluded.config,
               created_at = excluded.created_at",
            params![
                score.id,
                run_id,
                score.overall_score,
                score.dimensions.to_string(),
                score.coverage,
                score.confidence,
                score.rule_version,
                score.analyzer_versions.to_string(),
                score.config.to_string(),
                score.created_at,
            ],
        )?;
        Ok(())
    }

    pub fn insert_analyzer_run(
        &mut self,
        run_id: &str,
        analyzer_run: &AnalyzerRun,
    ) -> Result<(), AppError> {
        self.transaction.execute(
            "INSERT INTO analyzer_runs
             (id, scan_run_id, analyzer, status, version, duration_ms, message)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                analyzer_run.id,
                run_id,
                analyzer_run.analyzer,
                analyzer_status_name(&analyzer_run.status),
                analyzer_run.version,
                analyzer_run.duration_ms.map(|value| value as i64),
                analyzer_run.message,
            ],
        )?;
        Ok(())
    }

    pub fn save_prompt(&mut self, run_id: &str, prompt: &PromptArtifact) -> Result<(), AppError> {
        let selected_issue_ids =
            serde_json::to_string(&prompt.selected_issue_ids).map_err(|_| {
                AppError::Validation("prompt issue list could not be stored".to_owned())
            })?;
        self.transaction.execute(
            "INSERT INTO prompt_artifacts
             (id, scan_run_id, mode, selected_issue_ids, template_version, content, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(scan_run_id, id) DO NOTHING",
            params![
                prompt.id,
                run_id,
                prompt_mode_name(&prompt.mode),
                selected_issue_ids,
                prompt.template_version,
                prompt.content,
                prompt.created_at,
            ],
        )?;
        Ok(())
    }
}

fn scan_status_name(status: &ScanStatus) -> &'static str {
    match status {
        ScanStatus::Queued => "queued",
        ScanStatus::Running => "running",
        ScanStatus::Paused => "paused",
        ScanStatus::Canceling => "canceling",
        ScanStatus::Canceled => "canceled",
        ScanStatus::Completed => "completed",
        ScanStatus::Failed => "failed",
    }
}

fn severity_name(severity: &Severity) -> &'static str {
    match severity {
        Severity::Critical => "critical",
        Severity::High => "high",
        Severity::Medium => "medium",
        Severity::Low => "low",
        Severity::Info => "info",
    }
}

fn severity_from_name(name: &str) -> Result<Severity, AppError> {
    match name {
        "critical" => Ok(Severity::Critical),
        "high" => Ok(Severity::High),
        "medium" => Ok(Severity::Medium),
        "low" => Ok(Severity::Low),
        "info" => Ok(Severity::Info),
        _ => Err(AppError::Database("stored severity is invalid".to_owned())),
    }
}

fn coverage_name(coverage: &CoverageKind) -> &'static str {
    match coverage {
        CoverageKind::Ast => "ast",
        CoverageKind::Lexical => "lexical",
        CoverageKind::Structural => "structural",
        CoverageKind::NotCovered => "notcovered",
    }
}

fn coverage_from_name(name: &str) -> Result<CoverageKind, AppError> {
    match name {
        "ast" => Ok(CoverageKind::Ast),
        "lexical" => Ok(CoverageKind::Lexical),
        "structural" => Ok(CoverageKind::Structural),
        "notcovered" => Ok(CoverageKind::NotCovered),
        _ => Err(AppError::Database("stored coverage is invalid".to_owned())),
    }
}

fn prompt_mode_name(mode: &PromptMode) -> &'static str {
    match mode {
        PromptMode::Comprehensive => "comprehensive",
        PromptMode::Focused => "focused",
        PromptMode::Targeted => "targeted",
    }
}

fn analyzer_status_name(status: &AnalyzerStatus) -> &'static str {
    match status {
        AnalyzerStatus::Available => "available",
        AnalyzerStatus::Executed => "executed",
        AnalyzerStatus::Failed => "failed",
        AnalyzerStatus::Skipped => "skipped",
        AnalyzerStatus::Unavailable => "unavailable",
    }
}

/// Filter parameters for the paginated issue query.
#[derive(Debug, Clone, Default)]
pub struct IssueFilter {
    pub severity: Option<String>,
    pub category: Option<String>,
    pub file_path: Option<String>,
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

fn row_to_issue(row: &rusqlite::Row<'_>) -> rusqlite::Result<Issue> {
    let severity_name: String = row.get(3)?;
    let coverage_name: String = row.get(15)?;
    let mut issue = Issue {
        id: row.get(0)?,
        rule_id: row.get(1)?,
        category: row.get(2)?,
        severity: severity_from_name(&severity_name).map_err(|_| {
            rusqlite::Error::InvalidColumnType(
                3,
                severity_name.clone(),
                rusqlite::types::Type::Text,
            )
        })?,
        file_path: row.get(4)?,
        start_line: row.get(5)?,
        start_column: row.get(6)?,
        end_line: row.get(7)?,
        end_column: row.get(8)?,
        title: row.get(9)?,
        explanation: row.get(10)?,
        evidence: row.get(11)?,
        impact: row.get(12)?,
        recommendation: row.get(13)?,
        source: row.get(14)?,
        coverage: coverage_from_name(&coverage_name).map_err(|_| {
            rusqlite::Error::InvalidColumnType(
                15,
                coverage_name.clone(),
                rusqlite::types::Type::Text,
            )
        })?,
        confidence: row.get(16)?,
        fingerprint: String::new(),
    };
    issue.evidence = crate::rules::safe_issue_evidence(&issue.rule_id, &issue.evidence).to_owned();
    issue.fingerprint = IssueFingerprint::for_issue(&issue);
    Ok(issue)
}

fn row_to_scan_run(row: &rusqlite::Row<'_>) -> rusqlite::Result<ScanRun> {
    let status_name: String = row.get(2)?;
    let status = scan_status_from_name(&status_name).map_err(|_| {
        rusqlite::Error::InvalidColumnType(2, status_name.clone(), rusqlite::types::Type::Text)
    })?;
    Ok(ScanRun {
        id: row.get(0)?,
        project_id: row.get(1)?,
        status,
        started_at: row.get(3)?,
        completed_at: row.get(4)?,
        config: serde_json::from_str(&row.get::<_, String>(5)?).unwrap_or(serde_json::json!({})),
        rule_version: row.get(6)?,
        analyzer_versions: serde_json::from_str(&row.get::<_, String>(7)?)
            .unwrap_or(serde_json::json!({})),
        total_files: row.get(8)?,
        scanned_files: row.get(9)?,
        total_issues: row.get(10)?,
    })
}

fn row_to_project(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectRecord> {
    Ok(ProjectRecord {
        id: row.get(0)?,
        normalized_path: row.get(1)?,
        name: row.get(2)?,
        created_at: row.get(3)?,
    })
}

fn row_to_file(row: &rusqlite::Row<'_>) -> rusqlite::Result<FileRecord> {
    let coverage_name: String = row.get(9)?;
    Ok(FileRecord {
        id: row.get(0)?,
        scan_run_id: row.get(1)?,
        project_id: row.get(2)?,
        relative_path: row.get(3)?,
        absolute_path: row.get(4)?,
        encoding: row.get(5)?,
        size_bytes: row.get::<_, i64>(6)?.max(0) as u64,
        content_hash: row.get(7)?,
        language: row.get(8)?,
        parse_capability: coverage_from_name(&coverage_name).map_err(|_| {
            rusqlite::Error::InvalidColumnType(9, coverage_name, rusqlite::types::Type::Text)
        })?,
        excluded_reason: row.get(10)?,
    })
}

fn row_to_score(row: &rusqlite::Row<'_>) -> rusqlite::Result<ScoreSnapshot> {
    Ok(ScoreSnapshot {
        id: row.get(0)?,
        overall_score: row.get(1)?,
        dimensions: serde_json::from_str(&row.get::<_, String>(2)?)
            .unwrap_or(serde_json::json!({})),
        coverage: row.get(3)?,
        confidence: row.get(4)?,
        rule_version: row.get(5)?,
        analyzer_versions: serde_json::from_str(&row.get::<_, String>(6)?)
            .unwrap_or(serde_json::json!({})),
        config: serde_json::from_str(&row.get::<_, String>(7)?).unwrap_or(serde_json::json!({})),
        created_at: row.get(8)?,
    })
}

fn scan_status_from_name(name: &str) -> Result<ScanStatus, AppError> {
    match name {
        "queued" => Ok(ScanStatus::Queued),
        "running" => Ok(ScanStatus::Running),
        "paused" => Ok(ScanStatus::Paused),
        "canceling" => Ok(ScanStatus::Canceling),
        "canceled" => Ok(ScanStatus::Canceled),
        "completed" => Ok(ScanStatus::Completed),
        "failed" => Ok(ScanStatus::Failed),
        _ => Err(AppError::Database(
            "stored scan status is invalid".to_owned(),
        )),
    }
}
