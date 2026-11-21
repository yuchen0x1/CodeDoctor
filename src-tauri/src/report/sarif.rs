use crate::domain::types::{Issue, ScanStatus, Severity};
use crate::error::AppError;
use crate::score::ScanSnapshot;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fmt::Write;

pub fn render(snapshot: &ScanSnapshot) -> Result<String, AppError> {
    let mut rule_indexes: BTreeMap<&str, usize> = snapshot
        .issues
        .iter()
        .map(|issue| (issue.rule_id.as_str(), 0))
        .collect();
    for (index, slot) in rule_indexes.values_mut().enumerate() {
        *slot = index;
    }
    let rules: Vec<_> = rule_indexes.keys().map(|id| json!({"id": id})).collect();
    let results: Vec<_> = snapshot
        .issues
        .iter()
        .map(|issue| result(issue, rule_indexes[issue.rule_id.as_str()]))
        .collect();
    serde_json::to_string_pretty(&json!({
        "$schema": "https://docs.oasis-open.org/sarif/sarif/v2.1.0/errata01/os/schemas/sarif-schema-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": { "driver": {
                "name": "CodeDoctor",
                "semanticVersion": env!("CARGO_PKG_VERSION"),
                "informationUri": "https://github.com/yuchen0x1/CodeDoctor",
                "rules": rules,
            } },
            "invocations": [{ "executionSuccessful": snapshot.run.status == ScanStatus::Completed }],
            "properties": { "ruleVersion": snapshot.run.rule_version },
            "results": results,
        }],
    }))
    .map_err(|_| AppError::Validation("SARIF report could not be serialized".to_owned()))
}

fn result(issue: &Issue, rule_index: usize) -> Value {
    let level = match issue.severity {
        Severity::Critical | Severity::High => "error",
        Severity::Medium => "warning",
        Severity::Low | Severity::Info => "note",
    };
    let mut value = json!({
        "ruleId": issue.rule_id,
        "ruleIndex": rule_index,
        "level": level,
        "message": { "text": issue.title },
        "properties": { "severity": issue.severity, "category": issue.category, "confidence": issue.confidence },
    });
    if !issue.fingerprint.is_empty() {
        value["partialFingerprints"] = json!({ "codedoctorIssue/v1": issue.fingerprint });
    }
    if let Some(uri) = encode_relative_uri(&issue.file_path) {
        let mut physical = json!({ "artifactLocation": { "uri": uri } });
        // A missing line is not line 1, and stored UTF-8 byte columns are not SARIF columns.
        if issue.start_line > 0 {
            physical["region"] = json!({ "startLine": issue.start_line });
        }
        value["locations"] = json!([{ "physicalLocation": physical }]);
    }
    value
}

fn encode_relative_uri(path: &str) -> Option<String> {
    if path.contains(['\\', ':']) || path.split('/').any(|part| matches!(part, "" | "." | "..")) {
        return None;
    }
    let mut uri = String::with_capacity(path.len());
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~/".contains(&byte) {
            uri.push(char::from(byte));
        } else {
            let _ = write!(uri, "%{byte:02X}");
        }
    }
    Some(uri)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::{CoverageKind, ScanRun};

    fn sample_issue(rule_id: &str, severity: Severity, file_path: &str) -> Issue {
        Issue {
            id: format!("issue-{rule_id}-{file_path}"),
            rule_id: rule_id.to_owned(),
            category: "security".to_owned(),
            severity,
            file_path: file_path.to_owned(),
            start_line: 12,
            start_column: 3,
            end_line: 14,
            end_column: 9,
            title: "发现的问题".to_owned(),
            explanation: String::new(),
            evidence: String::new(),
            impact: String::new(),
            recommendation: String::new(),
            source: "rule".to_owned(),
            coverage: CoverageKind::Ast,
            confidence: 0.8,
            fingerprint: "fp-1".to_owned(),
        }
    }

    fn snapshot(issues: Vec<Issue>) -> ScanSnapshot {
        ScanSnapshot {
            run: ScanRun {
                id: "run".into(),
                project_id: "project".into(),
                status: ScanStatus::Completed,
                started_at: None,
                completed_at: None,
                failure_message: None,
                config: json!({}),
                rule_version: "test-rules".into(),
                analyzer_versions: json!({}),
                total_files: 1,
                scanned_files: 1,
                total_issues: issues.len() as u32,
            },
            issues,
            edges: Vec::new(),
            file_count: 1,
            source_file_count: 1,
            evidence: Default::default(),
        }
    }

    fn render_issues(issues: Vec<Issue>) -> Value {
        serde_json::from_str(&render(&snapshot(issues)).expect("sarif render")).expect("sarif json")
    }

    #[test]
    fn scan_failure_diagnostics_are_not_exported_to_sarif() {
        let mut snapshot = snapshot(Vec::new());
        snapshot.run.status = ScanStatus::Failed;
        snapshot.run.failure_message = Some("private diagnostic sentinel".into());
        let text = render(&snapshot).unwrap();
        assert!(!text.contains("private diagnostic sentinel"));
        assert!(!text.contains("failureMessage"));
        let log: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            log["runs"][0]["invocations"][0]["executionSuccessful"],
            false
        );
    }

    #[test]
    fn results_reference_sorted_unique_rules_without_reordering_findings() {
        let log = render_issues(vec![
            sample_issue("z.rule", Severity::High, "src/测 试#%.py"),
            sample_issue("a.rule", Severity::Medium, "src/main.py"),
            sample_issue("z.rule", Severity::Info, "src/other.py"),
        ]);
        assert_eq!(log["version"], "2.1.0");
        let run = &log["runs"][0];
        let rules = &run["tool"]["driver"]["rules"];
        assert_eq!(rules, &json!([{ "id": "a.rule" }, { "id": "z.rule" }]));
        for (result, (index, level)) in run["results"].as_array().unwrap().iter().zip([
            (1, "error"),
            (0, "warning"),
            (1, "note"),
        ]) {
            assert_eq!(result["ruleIndex"], index);
            assert_eq!(result["ruleId"], rules[index as usize]["id"]);
            assert_eq!(result["level"], level);
        }
        let first = &run["results"][0];
        assert_eq!(first["partialFingerprints"]["codedoctorIssue/v1"], "fp-1");
        let physical = &first["locations"][0]["physicalLocation"];
        assert_eq!(
            physical["artifactLocation"]["uri"],
            "src/%E6%B5%8B%20%E8%AF%95%23%25.py"
        );
        assert_eq!(physical["region"], json!({ "startLine": 12 }));
        assert_eq!(run["properties"]["ruleVersion"], "test-rules");
    }

    #[test]
    fn all_severities_map_to_sarif_levels() {
        for (severity, level) in [
            (Severity::Critical, "error"),
            (Severity::High, "error"),
            (Severity::Medium, "warning"),
            (Severity::Low, "note"),
            (Severity::Info, "note"),
        ] {
            let log = render_issues(vec![sample_issue("rule", severity, "src/main.py")]);
            assert_eq!(log["runs"][0]["results"][0]["level"], level);
        }
    }

    #[test]
    fn missing_location_and_fingerprint_are_not_fabricated() {
        let mut file_only = sample_issue("a", Severity::Low, "src/file.py");
        file_only.start_line = 0;
        file_only.fingerprint.clear();
        let log = render_issues(vec![
            file_only,
            sample_issue("b", Severity::Critical, "../secret.py"),
        ]);
        let results = &log["runs"][0]["results"];
        assert!(results[0].get("partialFingerprints").is_none());
        assert_eq!(
            results[0]["locations"][0]["physicalLocation"],
            json!({ "artifactLocation": { "uri": "src/file.py" } })
        );
        assert!(results[1].get("locations").is_none());
    }

    #[test]
    fn invocation_success_requires_a_completed_scan_even_with_no_findings() {
        for status in [
            ScanStatus::Queued,
            ScanStatus::Running,
            ScanStatus::Paused,
            ScanStatus::Canceling,
            ScanStatus::Canceled,
            ScanStatus::Failed,
            ScanStatus::Completed,
        ] {
            let mut scan = snapshot(Vec::new());
            scan.run.status = status;
            let text = render(&scan).unwrap();
            assert_eq!(text, render(&scan).unwrap());
            let log: Value = serde_json::from_str(&text).unwrap();
            let run = &log["runs"][0];
            assert_eq!(
                run["invocations"][0]["executionSuccessful"],
                status == ScanStatus::Completed
            );
            assert_eq!(run["tool"]["driver"]["rules"], json!([]));
            assert_eq!(run["results"], json!([]));
        }
    }

    #[test]
    fn sarif_does_not_embed_source_evidence_or_scan_configuration() {
        let mut issue = sample_issue("security.hardcoded_secret", Severity::High, "src/key.py");
        issue.evidence = "password = 'private-value'".into();
        let mut scan = snapshot(vec![issue]);
        scan.run.config = json!({ "path": "C:/private/project", "token": "private-token" });
        let text = render(&scan).unwrap();
        assert!(!text.contains("private-value"));
        assert!(!text.contains("private-token"));
        assert!(!text.contains("C:/private"));
    }

    #[test]
    fn artifact_uris_are_encoded_and_relative() {
        assert_eq!(
            encode_relative_uri("src/测 试#%.py").as_deref(),
            Some("src/%E6%B5%8B%20%E8%AF%95%23%25.py")
        );
        for invalid in [
            "",
            "../other.py",
            "project/../other.py",
            "/tmp/main.py",
            "C:\\repo\\main.py",
        ] {
            assert_eq!(encode_relative_uri(invalid), None);
        }
    }
}
