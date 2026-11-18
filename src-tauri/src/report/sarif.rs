use crate::domain::types::{Issue, Severity};
use crate::error::AppError;
use crate::score::ScanSnapshot;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fmt::Write;

pub fn render(snapshot: &ScanSnapshot) -> Result<String, AppError> {
    let mut rules = BTreeMap::new();
    let mut results = Vec::with_capacity(snapshot.issues.len());
    for issue in &snapshot.issues {
        rules
            .entry(&issue.rule_id)
            .or_insert_with(|| json!({"id": issue.rule_id}));
        results.push(result(issue));
    }
    serde_json::to_string_pretty(&json!({
        "$schema": "https://docs.oasis-open.org/sarif/sarif/v2.1.0/errata01/os/schemas/sarif-schema-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": { "driver": {
                "name": "CodeDoctor",
                "semanticVersion": env!("CARGO_PKG_VERSION"),
                "informationUri": "https://github.com/yuchen0x1/CodeDoctor",
                "rules": rules.into_values().collect::<Vec<_>>(),
            } },
            "results": results,
        }],
    }))
    .map_err(|_| AppError::Validation("SARIF report could not be serialized".to_owned()))
}

fn result(issue: &Issue) -> Value {
    let level = match issue.severity {
        Severity::Critical | Severity::High => "error",
        Severity::Medium => "warning",
        Severity::Low | Severity::Info => "note",
    };
    let mut value = json!({
        "ruleId": issue.rule_id,
        "level": level,
        "message": { "text": issue.title },
        "partialFingerprints": { "codedoctorIssue/v1": issue.fingerprint },
        "properties": { "severity": issue.severity, "category": issue.category, "confidence": issue.confidence },
    });
    if let Some(uri) = encode_relative_uri(&issue.file_path) {
        value["locations"] = json!([{
            "physicalLocation": {
                "artifactLocation": { "uri": uri },
                "region": { "startLine": issue.start_line.max(1) },
            },
        }]);
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
