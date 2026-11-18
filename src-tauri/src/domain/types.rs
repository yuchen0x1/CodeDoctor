#[cfg(test)]
mod tests {
    use crate::domain::types::{
        CoverageKind, Issue, IssueFingerprint, ScanRun, ScanStatus, Severity,
    };
    use serde_json::{Value, json};

    #[test]
    fn severity_serializes_with_stable_lowercase_values() {
        let cases = [
            (Severity::Critical, "critical"),
            (Severity::High, "high"),
            (Severity::Medium, "medium"),
            (Severity::Low, "low"),
            (Severity::Info, "info"),
        ];

        for (value, expected) in cases {
            let serialized = serde_json::to_value(value).unwrap();
            assert_eq!(serialized, Value::String(expected.to_owned()));
        }
    }

    #[test]
    fn scan_status_serializes_with_stable_lowercase_values() {
        let cases = [
            (ScanStatus::Queued, "queued"),
            (ScanStatus::Running, "running"),
            (ScanStatus::Paused, "paused"),
            (ScanStatus::Canceling, "canceling"),
            (ScanStatus::Canceled, "canceled"),
            (ScanStatus::Completed, "completed"),
            (ScanStatus::Failed, "failed"),
        ];

        for (value, expected) in cases {
            let serialized = serde_json::to_value(value).unwrap();
            assert_eq!(serialized, Value::String(expected.to_owned()));
        }
    }

    #[test]
    fn equal_issue_content_produces_the_same_fingerprint() {
        let first = test_issue("same evidence");
        let second = test_issue("same evidence");

        assert_eq!(
            IssueFingerprint::for_issue(&first),
            IssueFingerprint::for_issue(&second)
        );
    }

    #[test]
    fn changed_location_changes_the_issue_fingerprint() {
        let mut changed = test_issue("same evidence");
        changed.start_line = 20;

        assert_ne!(
            IssueFingerprint::for_issue(&test_issue("same evidence")),
            IssueFingerprint::for_issue(&changed)
        );
    }

    #[test]
    fn changed_rule_changes_the_issue_fingerprint() {
        let mut changed = test_issue("same evidence");
        changed.rule_id = "other.rule".to_owned();

        assert_ne!(
            IssueFingerprint::for_issue(&test_issue("same evidence")),
            IssueFingerprint::for_issue(&changed)
        );
    }

    #[test]
    fn normalized_evidence_ignores_insignificant_whitespace() {
        let compact = test_issue("unused import os");
        let formatted = test_issue("  unused   import\tos  ");

        assert_eq!(
            IssueFingerprint::for_issue(&compact),
            IssueFingerprint::for_issue(&formatted)
        );
    }

    #[test]
    fn issue_json_uses_camel_case_fields() {
        let issue = test_issue("unused import");
        let serialized: Value = serde_json::to_value(issue).unwrap();

        assert_eq!(serialized["ruleId"], json!("rule.unused-import"));
        assert_eq!(serialized["filePath"], json!("src/example.py"));
        assert_eq!(serialized["startLine"], json!(7));
        assert_eq!(serialized["coverage"], json!("ast"));
    }

    #[test]
    fn scan_run_json_uses_camel_case_fields() {
        let run = ScanRun {
            id: "run-1".to_owned(),
            project_id: "project-1".to_owned(),
            status: ScanStatus::Running,
            started_at: Some("2026-11-17T00:00:00Z".to_owned()),
            completed_at: None,
            config: json!({"mode": "full"}),
            rule_version: "rules-1".to_owned(),
            analyzer_versions: json!({}),
            total_files: 10,
            scanned_files: 3,
            total_issues: 1,
        };
        let serialized: Value = serde_json::to_value(run).unwrap();

        assert_eq!(serialized["projectId"], json!("project-1"));
        assert_eq!(serialized["startedAt"], json!("2026-11-17T00:00:00Z"));
        assert_eq!(serialized["analyzerVersions"], json!({}));
    }

    fn test_issue(evidence: &str) -> Issue {
        Issue {
            id: "issue-1".to_owned(),
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
            impact: "This makes the dependency surface harder to reason about.".to_owned(),
            recommendation: "Remove the unused import.".to_owned(),
            source: "codedoctor".to_owned(),
            coverage: CoverageKind::Ast,
            confidence: 0.94,
            fingerprint: String::new(),
        }
    }
}
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScanStatus {
    Queued,
    Running,
    Paused,
    Canceling,
    Canceled,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CoverageKind {
    Ast,
    Lexical,
    Structural,
    NotCovered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnalyzerStatus {
    Available,
    Executed,
    Failed,
    Skipped,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PromptMode {
    Comprehensive,
    Focused,
    Targeted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRecord {
    pub id: String,
    pub name: String,
    pub normalized_path: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanRun {
    pub id: String,
    pub project_id: String,
    pub status: ScanStatus,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub config: Value,
    pub rule_version: String,
    pub analyzer_versions: Value,
    pub total_files: u32,
    pub scanned_files: u32,
    pub total_issues: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRecord {
    pub id: String,
    pub scan_run_id: String,
    pub project_id: String,
    pub relative_path: String,
    pub absolute_path: String,
    pub encoding: String,
    pub size_bytes: u64,
    pub content_hash: String,
    pub language: String,
    pub parse_capability: CoverageKind,
    pub excluded_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub id: String,
    pub rule_id: String,
    pub category: String,
    pub severity: Severity,
    pub file_path: String,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub title: String,
    pub explanation: String,
    pub evidence: String,
    pub impact: String,
    pub recommendation: String,
    pub source: String,
    pub coverage: CoverageKind,
    pub confidence: f32,
    pub fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureEdge {
    pub id: String,
    pub source_path: String,
    pub target_path: String,
    pub kind: String,
    pub confidence: f32,
    pub coverage: CoverageKind,
    pub metadata: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzerRun {
    pub id: String,
    pub analyzer: String,
    pub status: AnalyzerStatus,
    pub version: Option<String>,
    pub duration_ms: Option<u64>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreSnapshot {
    pub id: String,
    pub overall_score: f32,
    pub dimensions: Value,
    pub coverage: f32,
    pub confidence: f32,
    pub rule_version: String,
    pub analyzer_versions: Value,
    pub config: Value,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptArtifact {
    pub id: String,
    pub mode: PromptMode,
    pub selected_issue_ids: Vec<String>,
    pub template_version: String,
    pub content: String,
    pub created_at: String,
}

pub struct IssueFingerprint;

impl IssueFingerprint {
    pub fn for_issue(issue: &Issue) -> String {
        let normalized_evidence =
            crate::rules::safe_issue_evidence(&issue.rule_id, &issue.evidence)
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
        let material = format!(
            "{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
            issue.rule_id,
            issue.file_path,
            issue.start_line,
            issue.start_column,
            issue.end_line,
            issue.end_column,
            normalized_evidence,
        );
        format!("{:x}", Sha256::digest(material.as_bytes()))
    }
}
