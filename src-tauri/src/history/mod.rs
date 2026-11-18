//! Scan-to-scan comparison based on issue fingerprints and persisted metadata.
use crate::domain::types::{Issue, ScoreSnapshot, Severity};
use crate::score::ScanSnapshot;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChangeKind {
    Added,
    Resolved,
    Persistent,
    LocationChanged,
    #[serde(alias = "severitychanged")]
    SeverityChanged,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueChange {
    pub fingerprint: String,
    pub kind: ChangeKind,
    pub baseline_issue_id: Option<String>,
    pub current_issue_id: Option<String>,
    pub baseline_severity: Option<Severity>,
    pub current_severity: Option<Severity>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryDiff {
    pub base_run_id: String,
    pub target_run_id: String,
    pub issue_changes: Vec<IssueChange>,
    pub score_delta: Option<f32>,
    pub rule_version_changed: bool,
    pub coverage_delta: f32,
}

pub fn compare_scans(
    base: &ScanSnapshot,
    target: &ScanSnapshot,
    base_score: Option<&ScoreSnapshot>,
    target_score: Option<&ScoreSnapshot>,
) -> HistoryDiff {
    let old = by_fp(&base.issues);
    let new = by_fp(&target.issues);
    let mut keys = BTreeSet::new();
    keys.extend(old.keys().cloned());
    keys.extend(new.keys().cloned());
    let old_only = unique_unmatched(&old, &new);
    let new_only = unique_unmatched(&new, &old);
    let mut moves = BTreeMap::new();
    let mut matched_targets = BTreeSet::new();
    for (key, from) in old_only {
        if let (Some(from), Some(Some(to))) = (from, new_only.get(&key)) {
            moves.insert(from, *to);
            matched_targets.insert(*to);
        }
    }
    let mut changes = vec![];
    for fp in keys {
        if matched_targets.contains(fp.as_str()) {
            continue;
        }
        let a = old.get(&fp);
        let moved = moves.get(fp.as_str());
        let b = new.get(moved.copied().unwrap_or(&fp));
        let kind = match (a, b) {
            (None, Some(_)) => ChangeKind::Added,
            (Some(_), None) => ChangeKind::Resolved,
            (Some(x), Some(y)) if x.severity != y.severity => ChangeKind::SeverityChanged,
            (Some(_), Some(_)) if moved.is_some() => ChangeKind::LocationChanged,
            (Some(_), Some(_)) => ChangeKind::Persistent,
            _ => continue,
        };
        changes.push(IssueChange {
            fingerprint: fp,
            kind,
            baseline_issue_id: a.map(|i| i.id.clone()),
            current_issue_id: b.map(|i| i.id.clone()),
            baseline_severity: a.map(|i| i.severity),
            current_severity: b.map(|i| i.severity),
        });
    }
    let baseline_calculated = base_score
        .is_none()
        .then(|| crate::score::calculate_score(base));
    let target_calculated = target_score
        .is_none()
        .then(|| crate::score::calculate_score(target));
    let base_score = base_score
        .or(baseline_calculated.as_ref())
        .expect("baseline score is present");
    let target_score = target_score
        .or(target_calculated.as_ref())
        .expect("target score is present");
    HistoryDiff {
        base_run_id: base.run.id.clone(),
        target_run_id: target.run.id.clone(),
        issue_changes: changes,
        score_delta: (base_score.coverage > 0.0 && target_score.coverage > 0.0)
            .then_some(target_score.overall_score - base_score.overall_score),
        rule_version_changed: base.run.rule_version != target.run.rule_version,
        coverage_delta: target_score.coverage - base_score.coverage,
    }
}
type LocationKey<'a> = (&'a str, &'a str, &'a str, String);

fn unique_unmatched<'a>(
    scans: &'a std::collections::HashMap<String, &Issue>,
    other: &std::collections::HashMap<String, &Issue>,
) -> BTreeMap<LocationKey<'a>, Option<&'a str>> {
    let mut keys = BTreeMap::new();
    for (fingerprint, issue) in scans {
        if other.contains_key(fingerprint) || issue.evidence.trim().is_empty() {
            continue;
        }
        let key = (
            issue.file_path.as_str(),
            issue.rule_id.as_str(),
            issue.source.as_str(),
            issue
                .evidence
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
        );
        keys.entry(key)
            .and_modify(|existing| *existing = None)
            .or_insert(Some(fingerprint.as_str()));
    }
    keys
}
fn by_fp(issues: &[Issue]) -> std::collections::HashMap<String, &Issue> {
    issues
        .iter()
        .map(|i| {
            (
                if i.fingerprint.is_empty() {
                    crate::domain::types::IssueFingerprint::for_issue(i)
                } else {
                    i.fingerprint.clone()
                },
                i,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::{CoverageKind, ScanRun, ScanStatus};
    use serde_json::json;
    fn snap(id: &str, sev: Severity) -> ScanSnapshot {
        let i = Issue {
            id: "i".into(),
            rule_id: "quality.x".into(),
            category: "quality".into(),
            severity: sev,
            file_path: "a.py".into(),
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 2,
            title: "x".into(),
            explanation: "e".into(),
            evidence: "e".into(),
            impact: "i".into(),
            recommendation: "r".into(),
            source: "s".into(),
            coverage: CoverageKind::Ast,
            confidence: 1.,
            fingerprint: "same".into(),
        };
        ScanSnapshot {
            run: ScanRun {
                id: id.into(),
                project_id: "p".into(),
                status: ScanStatus::Completed,
                started_at: None,
                completed_at: None,
                config: json!({}),
                rule_version: "r".into(),
                analyzer_versions: json!({}),
                total_files: 1,
                scanned_files: 1,
                total_issues: 1,
            },
            issues: vec![i],
            edges: vec![],
            file_count: 1,
            source_file_count: 1,
            evidence: Default::default(),
        }
    }
    #[test]
    fn detects_severity_and_score_changes() {
        let a = snap("a", Severity::Low);
        let b = snap("b", Severity::High);
        let base_score = ScoreSnapshot {
            id: "score-a".into(),
            overall_score: 60.0,
            dimensions: serde_json::json!({}),
            coverage: 0.5,
            confidence: 0.5,
            rule_version: "r".into(),
            analyzer_versions: serde_json::json!({}),
            config: serde_json::json!({}),
            created_at: "1".into(),
        };
        let target_score = ScoreSnapshot {
            overall_score: 72.5,
            id: "score-b".into(),
            ..base_score.clone()
        };
        let d = compare_scans(&a, &b, Some(&base_score), Some(&target_score));
        assert!(matches!(
            d.issue_changes[0].kind,
            ChangeKind::SeverityChanged
        ));
        assert_eq!(d.coverage_delta, 0.);
        assert_eq!(d.score_delta, Some(12.5));
    }

    #[test]
    fn computes_score_delta_when_snapshots_were_not_persisted() {
        let baseline = snap("baseline", Severity::Low);
        let target = snap("target", Severity::High);
        let diff = compare_scans(&baseline, &target, None, None);

        assert!(diff.score_delta.is_some_and(|delta| delta < 0.0));
    }

    #[test]
    fn uniquely_matching_evidence_after_a_line_shift_is_a_location_change() {
        let baseline = snap("baseline", Severity::Low);
        let mut target = snap("target", Severity::Low);
        target.issues[0].start_line = 5;
        target.issues[0].end_line = 5;
        target.issues[0].fingerprint = "moved".into();
        let diff = compare_scans(&baseline, &target, None, None);
        assert_eq!(diff.issue_changes.len(), 1);
        assert!(matches!(
            diff.issue_changes[0].kind,
            ChangeKind::LocationChanged
        ));
        assert_eq!(
            serde_json::to_value(ChangeKind::SeverityChanged).unwrap(),
            json!("severityChanged")
        );
    }
}
