//! Baseline comparison report: turns one scan-to-scan diff into a reviewable
//! document. Only fields already stored in the local database are emitted.

use crate::domain::types::Issue;
use crate::history::{ChangeKind, HistoryDiff, IssueChange};
use crate::score::ScanSnapshot;
use serde_json::json;
use std::collections::HashMap;
use std::fmt::Write;

pub fn render_markdown(
    base: &ScanSnapshot,
    target: &ScanSnapshot,
    diff: &HistoryDiff,
    base_score: Option<&crate::domain::types::ScoreSnapshot>,
    target_score: Option<&crate::domain::types::ScoreSnapshot>,
) -> String {
    let issues = issue_index(base, target);
    let count = |kind: &ChangeKind| {
        diff.issue_changes
            .iter()
            .filter(|change| same_kind(&change.kind, kind))
            .count()
    };
    let mut out = String::from("# CodeDoctor 基线对比报告\n\n");
    let _ = writeln!(out, "- 基线扫描：`{}`", base.run.id);
    let _ = writeln!(out, "- 目标扫描：`{}`", target.run.id);
    let _ = writeln!(out, "- 项目：`{}`", target.run.project_id);
    let _ = writeln!(
        out,
        "- 基线完成时间：`{}`",
        base.run.completed_at.as_deref().unwrap_or("未记录")
    );
    let _ = writeln!(
        out,
        "- 目标完成时间：`{}`",
        target.run.completed_at.as_deref().unwrap_or("未记录")
    );
    let _ = writeln!(
        out,
        "- 质量分变化：{}",
        diff.score_delta
            .map(|value| format!("{value:+.1}"))
            .unwrap_or_else(|| "未评估".into())
    );
    let _ = writeln!(
        out,
        "- 检测覆盖率变化：{:+.0}%",
        diff.coverage_delta * 100.0
    );
    if let (Some(base), Some(target)) = (base_score, target_score) {
        let _ = writeln!(
            out,
            "- 质量分：{:.1} → {:.1}",
            base.overall_score, target.overall_score
        );
    }
    if diff.rule_version_changed {
        let _ = writeln!(
            out,
            "\n> 两次扫描使用了不同规则版本（`{}` → `{}`）。新增与已解决的发现可能来自规则变化，请结合规则更新记录解读。",
            base.run.rule_version, target.run.rule_version
        );
    }
    let _ = writeln!(
        out,
        "\n## 变更汇总\n\n| 类别 | 数量 |\n| --- | ---: |\n| 新增 | {} |\n| 已解决 | {} |\n| 持续存在 | {} |\n| 严重度变化 | {} |\n| 位置变化 | {} |\n| 合计 | {} |",
        count(&ChangeKind::Added),
        count(&ChangeKind::Resolved),
        count(&ChangeKind::Persistent),
        count(&ChangeKind::SeverityChanged),
        count(&ChangeKind::LocationChanged),
        diff.issue_changes.len()
    );

    for (kind, heading) in [
        (ChangeKind::Added, "新增问题"),
        (ChangeKind::Resolved, "已解决问题"),
        (ChangeKind::SeverityChanged, "严重度变化"),
        (ChangeKind::LocationChanged, "位置变化"),
        (ChangeKind::Persistent, "持续存在的问题"),
    ] {
        let changes = diff
            .issue_changes
            .iter()
            .filter(|change| same_kind(&change.kind, &kind))
            .collect::<Vec<_>>();
        let _ = write!(out, "\n## {heading}（{}）\n\n", changes.len());
        if changes.is_empty() {
            let _ = writeln!(out, "无。\n");
            continue;
        }
        for change in changes {
            write_change(&mut out, change, &issues);
        }
    }
    out
}

pub fn render_json(
    base: &ScanSnapshot,
    target: &ScanSnapshot,
    diff: &HistoryDiff,
    base_score: Option<&crate::domain::types::ScoreSnapshot>,
    target_score: Option<&crate::domain::types::ScoreSnapshot>,
) -> Result<String, crate::error::AppError> {
    let issues = issue_index(base, target);
    let changes = diff
        .issue_changes
        .iter()
        .map(|change| {
            let baseline = change
                .baseline_issue_id
                .as_deref()
                .and_then(|id| issues.get(id))
                .copied();
            let current = change
                .current_issue_id
                .as_deref()
                .and_then(|id| issues.get(id))
                .copied();
            json!({ "change": change, "baselineIssue": baseline, "currentIssue": current })
        })
        .collect::<Vec<_>>();
    serde_json::to_string_pretty(&json!({
        "version": 1,
        "baseRun": base.run,
        "targetRun": target.run,
        "baseScore": base_score,
        "targetScore": target_score,
        "diff": diff,
        "changes": changes
    }))
    .map_err(|_| crate::error::AppError::Validation("history report could not be rendered".into()))
}

fn issue_index<'a>(
    base: &'a ScanSnapshot,
    target: &'a ScanSnapshot,
) -> HashMap<&'a str, &'a Issue> {
    base.issues
        .iter()
        .chain(target.issues.iter())
        .map(|issue| (issue.id.as_str(), issue))
        .collect()
}

fn same_kind(left: &ChangeKind, right: &ChangeKind) -> bool {
    std::mem::discriminant(left) == std::mem::discriminant(right)
}

fn kind_label(kind: &ChangeKind) -> &'static str {
    match kind {
        ChangeKind::Added => "新增",
        ChangeKind::Resolved => "已解决",
        ChangeKind::Persistent => "持续存在",
        ChangeKind::LocationChanged => "位置变化",
        ChangeKind::SeverityChanged => "严重度变化",
    }
}

fn escape_inline(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\r' => out.push_str("\\r"),
            '\n' => out.push_str("\\n"),
            '<' | '>' | '`' | '*' | '_' | '[' | ']' | '|' | '\\' => {
                out.push('\\');
                out.push(character);
            }
            character => out.push(character),
        }
    }
    out
}

fn code(value: &str) -> String {
    let value = value.replace('\r', "\\r").replace('\n', "\\n");
    if value.is_empty() {
        return "—".into();
    }
    let longest = value
        .split(|character| character != '`')
        .map(str::len)
        .max()
        .unwrap_or(0);
    let fence = "`".repeat(longest + 1);
    if value.starts_with(' ') || value.ends_with(' ') {
        format!("{fence} {value} {fence}")
    } else {
        format!("{fence}{value}{fence}")
    }
}

fn write_change(out: &mut String, change: &IssueChange, issues: &HashMap<&str, &Issue>) {
    let baseline = change
        .baseline_issue_id
        .as_deref()
        .and_then(|id| issues.get(id))
        .copied();
    let current = change
        .current_issue_id
        .as_deref()
        .and_then(|id| issues.get(id))
        .copied();
    let issue = current.or(baseline);
    let _ = writeln!(out, "### {}", kind_label(&change.kind));
    if let Some(issue) = issue {
        let _ = writeln!(
            out,
            "\n- 标题：{}\n- 位置：{}\n- 规则：{}\n- 类别：{}",
            escape_inline(&issue.title),
            code(&format!(
                "{}:{}:{}",
                issue.file_path, issue.start_line, issue.start_column
            )),
            code(&issue.rule_id),
            escape_inline(&issue.category)
        );
        if let (Some(before), Some(after)) = (change.baseline_severity, change.current_severity)
            && before != after
        {
            let _ = writeln!(out, "- 严重度：{before:?} → {after:?}");
        }
        if let (Some(before), Some(after)) = (baseline, current)
            && (before.start_line, before.start_column) != (after.start_line, after.start_column)
        {
            let _ = writeln!(out, "- 行号：{} → {}", before.start_line, after.start_line);
        }
        if !issue.recommendation.trim().is_empty() {
            let _ = writeln!(out, "- 建议：{}", escape_inline(&issue.recommendation));
        }
        if !issue.evidence.trim().is_empty() {
            let longest = issue
                .evidence
                .split(|character| character != '`')
                .map(str::len)
                .max()
                .unwrap_or(0);
            let fence = "`".repeat((longest + 1).max(3));
            let newline = if issue.evidence.ends_with('\n') {
                ""
            } else {
                "\n"
            };
            let _ = write!(out, "\n{fence}text\n{}{newline}{fence}\n", issue.evidence);
        }
    } else {
        let _ = writeln!(out, "\n- 指纹：{}", code(&change.fingerprint));
    }
    out.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::{CoverageKind, ScanRun, ScanStatus, Severity};
    use serde_json::json;

    fn issue(id: &str, fingerprint: &str, line: u32, severity: Severity) -> Issue {
        Issue {
            id: id.into(),
            rule_id: "quality.unused".into(),
            category: "quality".into(),
            severity,
            file_path: "src/app.py".into(),
            start_line: line,
            start_column: 1,
            end_line: line,
            end_column: 4,
            title: "未使用的导入".into(),
            explanation: "explain".into(),
            evidence: "import os".into(),
            impact: "impact".into(),
            recommendation: "删除导入".into(),
            source: "codedoctor".into(),
            coverage: CoverageKind::Ast,
            confidence: 0.9,
            fingerprint: fingerprint.into(),
        }
    }

    fn snap(id: &str, issues: Vec<Issue>) -> ScanSnapshot {
        ScanSnapshot {
            run: ScanRun {
                id: id.into(),
                project_id: "p".into(),
                status: ScanStatus::Completed,
                started_at: Some("1".into()),
                completed_at: Some("2".into()),
                failure_message: None,
                config: json!({}),
                rule_version: "rules-5".into(),
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
        }
    }

    #[test]
    fn markdown_lists_each_change_group_and_escapes_untrusted_text() {
        let base = snap(
            "base",
            vec![
                issue("old-resolved", "fp-resolved", 3, Severity::Medium),
                issue("old-persistent", "fp-persistent", 4, Severity::Low),
            ],
        );
        let mut added = issue("new-added", "fp-added", 9, Severity::High);
        added.title = "注入 <script>alert(1)</script> *强调*".into();
        let mut kept = issue("new-persistent", "fp-persistent", 5, Severity::Low);
        kept.severity = Severity::High;
        let target = snap("target", vec![added, kept]);
        let diff = crate::history::compare_scans(&base, &target, None, None);

        let text = render_markdown(&base, &target, &diff, None, None);

        assert!(text.starts_with("# CodeDoctor 基线对比报告"));
        assert!(text.contains("| 新增 | 1 |"));
        assert!(text.contains("| 已解决 | 1 |"));
        assert!(text.contains("| 严重度变化 | 1 |"));
        assert!(text.contains("## 新增问题（1）"));
        assert!(text.contains("## 已解决问题（1）"));
        assert!(text.contains("## 严重度变化（1）"));
        assert!(!text.contains("<script>"));
        assert!(text.contains("\\<script\\>"));
        assert!(text.contains("行号：4 → 5"));
        assert!(text.contains("Medium → High") || text.contains("High"));
        assert!(text.contains("import os"));
    }

    #[test]
    fn markdown_notes_rule_version_changes() {
        let base = snap("base", vec![]);
        let mut target = snap("target", vec![]);
        target.run.rule_version = "rules-6".into();
        let diff = crate::history::compare_scans(&base, &target, None, None);
        let text = render_markdown(&base, &target, &diff, None, None);
        assert!(text.contains("两次扫描使用了不同规则版本"));
        assert!(text.contains("rules-5"));
        assert!(text.contains("rules-6"));
        assert!(text.contains("无。"));
    }

    #[test]
    fn json_report_embeds_full_issue_records_and_diff() {
        let base = snap("base", vec![issue("old", "fp-old", 1, Severity::Low)]);
        let target = snap("target", vec![issue("new", "fp-new", 2, Severity::High)]);
        let diff = crate::history::compare_scans(&base, &target, None, None);
        let text = render_json(&base, &target, &diff, None, None).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["version"], json!(1));
        assert_eq!(value["baseRun"]["id"], json!("base"));
        assert_eq!(value["targetRun"]["id"], json!("target"));
        assert_eq!(value["diff"]["baseRunId"], json!("base"));
        let changes = value["changes"].as_array().unwrap();
        assert_eq!(changes.len(), 2);
        assert!(
            changes
                .iter()
                .any(|entry| entry["change"]["kind"] == json!("added")
                    && entry["currentIssue"]["id"] == json!("new"))
        );
        assert!(
            changes
                .iter()
                .any(|entry| entry["change"]["kind"] == json!("resolved")
                    && entry["baselineIssue"]["id"] == json!("old"))
        );
    }
}
