use crate::domain::types::{Issue, Severity};
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
    for (index, value) in rule_indexes.values_mut().enumerate() {
        *value = index;
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
        // Persisted columns can be byte offsets, not SARIF character columns.
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
    use crate::domain::types::{CoverageKind, ScanRun, ScanStatus};

    fn issue(rule_id: &str, severity: Severity, file_path: &str) -> Issue {
        Issue {
            id: String::new(),
            rule_id: rule_id.to_owned(),
            category: "quality".to_owned(),
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
            source: "builtin".to_owned(),
            coverage: CoverageKind::Ast,
            confidence: 0.8,
            fingerprint: "fp-1".to_owned(),
        }
    }

    fn render_run(issues: Vec<Issue>) -> Value {
        let snapshot = ScanSnapshot {
            run: ScanRun {
                id: "run".into(),
                project_id: "project".into(),
                status: ScanStatus::Completed,
                started_at: None,
                completed_at: None,
                config: json!({}),
                rule_version: "test".into(),
                analyzer_versions: json!({}),
                total_files: 1,
                scanned_files: 1,
                total_issues: issues.len() as u32,
            },
            issues,
            edges: vec![],
            file_count: 1,
            source_file_count: 1,
            evidence: Default::default(),
        };
        let log: Value = serde_json::from_str(&render(&snapshot).unwrap()).unwrap();
        assert_eq!(log["version"], "2.1.0");
        log["runs"][0].clone()
    }

    #[test]
    fn results_reference_the_sorted_deduplicated_rule_table() {
        let run = render_run(vec![
            issue("z.last", Severity::High, "src/测 试.py"),
            issue("a.first", Severity::Medium, "src/main.py"),
            issue("z.last", Severity::Info, "src/other.py"),
        ]);
        assert_eq!(
            run["tool"]["driver"]["rules"],
            json!([{ "id": "a.first" }, { "id": "z.last" }])
        );
        for (result, (index, level)) in run["results"].as_array().unwrap().iter().zip([
            (1, "error"),
            (0, "warning"),
            (1, "note"),
        ]) {
            assert_eq!(result["ruleIndex"], index);
            assert_eq!(result["level"], level);
            assert_eq!(
                result["ruleId"],
                run["tool"]["driver"]["rules"][index as usize]["id"]
            );
        }
        let first = &run["results"][0];
        assert_eq!(first["partialFingerprints"]["codedoctorIssue/v1"], "fp-1");
        let physical = &first["locations"][0]["physicalLocation"];
        assert_eq!(
            physical["artifactLocation"]["uri"],
            "src/%E6%B5%8B%20%E8%AF%95.py"
        );
        assert_eq!(physical["region"], json!({ "startLine": 12 }));
    }

    #[test]
    fn missing_evidence_does_not_create_a_location_or_fingerprint() {
        let mut file_only = issue("a", Severity::Low, "src/file.py");
        file_only.start_line = 0;
        file_only.fingerprint.clear();
        let run = render_run(vec![file_only, issue("b", Severity::Critical, "")]);
        let result = &run["results"][0];
        assert!(result.get("partialFingerprints").is_none());
        assert_eq!(
            result["locations"][0]["physicalLocation"],
            json!({ "artifactLocation": { "uri": "src/file.py" } })
        );
        assert!(run["results"][1].get("locations").is_none());
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
