//! Fixture-backed integration tests for the rule families. Every assertion
//! checks rule ID, position, or evidence — not just counts.

use super::config::RuleConfig;
use super::runner::run_on_file;
use crate::domain::types::{CoverageKind, FileRecord, Issue};

fn file(language: &str, path: &str) -> FileRecord {
    FileRecord {
        id: "t".into(),
        scan_run_id: "run".into(),
        project_id: "p".into(),
        relative_path: path.into(),
        absolute_path: path.into(),
        encoding: "utf-8".into(),
        size_bytes: 0,
        content_hash: String::new(),
        language: language.into(),
        parse_capability: CoverageKind::NotCovered,
        excluded_reason: None,
    }
}

fn run(language: &str, path: &str, text: &str) -> Vec<Issue> {
    run_on_file(&file(language, path), text, &RuleConfig::default())
}

fn ids_of(issues: &[Issue]) -> Vec<&str> {
    issues.iter().map(|i| i.rule_id.as_str()).collect()
}

#[test]
fn python_unreachable_statements_require_a_direct_terminal_sibling() {
    let text = r#"def completed(flag):
    if flag:
        return 0
    still_runs()
    return 1
    trailing_first()
    trailing_second()

def failed():
    raise ValueError("error")
    cleanup()
"#;
    let issues = run("Python", "src/dead_code.py", text);
    let unreachable: Vec<_> = issues
        .iter()
        .filter(|issue| issue.rule_id == "quality.unreachable-statement")
        .collect();
    assert_eq!(
        unreachable
            .iter()
            .map(|issue| issue.start_line)
            .collect::<Vec<_>>(),
        vec![6, 7, 11]
    );
    assert!(unreachable[0].evidence.contains("第 5 行的 `return`"));
    assert!(unreachable[2].evidence.contains("第 10 行的 `raise`"));
    assert_eq!(unreachable[0].start_column, 5);
    assert_eq!(unreachable[0].coverage, CoverageKind::Ast);
}

#[test]
fn python_unused_variables_include_only_simple_locals_without_other_references() {
    let text = r#"module_level_only = 1

def read():
    spare = calculate()
    live = calculate()
    _deliberate = 1
    closure = 4
    captured = lambda: closure
    formatted = 5
    return live, captured, f'{formatted}'

def dynamically_used():
    visible = 0
    return locals()

def with_unpack():
    first, second = values()
    return second

def set_global():
    global module_level_only
    module_level_only = 2

def reordered():
    K = 5
    return K
"#;
    let issues = run("Python", "src/locals.py", text);
    let unused: Vec<_> = issues
        .iter()
        .filter(|issue| issue.rule_id == "quality.unused-local-variable")
        .collect();
    assert_eq!(unused.len(), 1, "{unused:?}");
    assert_eq!(unused[0].start_line, 4);
    assert_eq!((unused[0].start_column, unused[0].end_column), (5, 10));
    assert!(unused[0].evidence.contains("`spare`"));
    assert!(unused[0].recommendation.contains("副作用"));
}

#[test]
fn python_quality_fixture_hits_expected_rules_and_lines() {
    let text = include_str!("../../../fixtures/rules/quality.py");
    let issues = run("Python", "src/quality.py", text);
    let ids = ids_of(&issues);

    assert!(ids.contains(&"quality.unused-import"), "ids={ids:?}");
    assert!(ids.contains(&"quality.unused-parameter"));
    assert!(ids.contains(&"quality.empty-catch"));
    assert!(ids.contains(&"quality.debug-output"));
    assert!(ids.contains(&"complexity.deep-nesting"));
    assert!(ids.contains(&"quality.magic-number"));
    assert!(ids.contains(&"basic.naming"), "ids={ids:?}");
    assert!(ids.contains(&"comments.stale-marker"));

    // deep-nesting anchors on the enclosing function
    let nesting = issues
        .iter()
        .find(|i| i.rule_id == "complexity.deep-nesting")
        .unwrap();
    assert_eq!(nesting.start_line, 22);

    // debug output points at the print line
    let debug = issues
        .iter()
        .find(|i| i.rule_id == "quality.debug-output")
        .unwrap();
    assert_eq!(debug.start_line, 5);
    assert!(debug.evidence.contains("print"));

    // magic number carries the literal as evidence
    let magic = issues
        .iter()
        .find(|i| i.rule_id == "quality.magic-number")
        .unwrap();
    assert!(magic.evidence.contains("999999"));
}

#[test]
fn comments_fixture_governs_pollution() {
    let text = include_str!("../../../fixtures/rules/comments.js");
    let issues = run("JavaScript", "src/comments.js", text);
    let ids = ids_of(&issues);

    assert!(ids.contains(&"comments.commented-out-code"));
    assert!(ids.contains(&"comments.obvious"));
    assert!(ids.contains(&"comments.duplicate-explanation"));
    assert!(ids.contains(&"comments.stale-marker"));

    // commented-out `if` block keeps its line position
    let commented = issues
        .iter()
        .find(|i| i.rule_id == "comments.commented-out-code")
        .unwrap();
    assert_eq!(commented.start_line, 5);
    assert!(commented.evidence.contains("if (data.length"));
}

#[test]
fn security_fixture_reports_potential_risks_only() {
    let text = include_str!("../../../fixtures/rules/security_sample.py");
    let issues = run("Python", "src/security_sample.py", text);
    let ids = ids_of(&issues);

    assert!(ids.contains(&"security.hardcoded-secret"));
    assert!(ids.contains(&"security.dangerous-exec"));
    assert!(ids.contains(&"security.insecure-deserialization"));
    assert!(ids.contains(&"security.sql-concat"));
    assert!(ids.contains(&"security.weak-random"));

    let secret = issues
        .iter()
        .find(|i| i.rule_id == "security.hardcoded-secret")
        .unwrap();
    assert_eq!(secret.start_line, 4);
    assert_eq!(secret.evidence, "疑似凭据已隐藏");

    // every security finding stays worded as a potential risk
    for issue in issues.iter().filter(|i| i.category == "security") {
        assert!(
            issue.explanation.contains("待人工确认") || issue.explanation.contains("潜在"),
            "security wording must stay tentative: {}",
            issue.explanation
        );
    }
}

#[test]
fn long_line_rule_applies_to_lexical_and_grammar_languages() {
    let text = include_str!("../../../fixtures/rules/longline.go");
    let issues = run("Go", "pkg/longline.go", text);
    assert!(ids_of(&issues).contains(&"quality.long-line"));
    let long = issues
        .iter()
        .find(|i| i.rule_id == "quality.long-line")
        .unwrap();
    assert_eq!(long.start_line, 3);
}

#[test]
fn lexical_language_only_gets_pattern_rules() {
    // PowerShell: no grammar → no complexity/naming rules, but long-line and
    // security patterns still apply.
    let text = format!(
        "param($a1)\n$sticky = '{}'\nfunction Start-Work {{ {}\n",
        "x".repeat(140),
        "#".repeat(30)
    );
    let issues = run("PowerShell", "scripts/run.ps1", &text);
    let ids = ids_of(&issues);
    assert!(ids.contains(&"quality.long-line"));
    assert!(!ids.contains(&"complexity.long-function"));
    assert!(!ids.contains(&"basic.naming"));
}

#[test]
fn unused_import_respects_dynamic_and_wildcards() {
    let text = r#"import numpy as np
import json
import json as j

def use():
    return json.dumps({})
"#;
    let issues = run("Python", "m.py", text);
    let unused: Vec<&str> = issues
        .iter()
        .filter(|i| i.rule_id == "quality.unused-import")
        .map(|i| i.evidence.as_str())
        .collect();
    // json is used; alias np and j are not
    assert_eq!(unused.len(), 2, "{unused:?}");
}
