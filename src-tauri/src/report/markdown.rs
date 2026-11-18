use crate::domain::types::ScoreSnapshot;
use crate::score::{ScanSnapshot, WEIGHTS};
use std::fmt::Write;

use super::{quality_score, summary_lines};

pub fn render(snapshot: &ScanSnapshot, score: &ScoreSnapshot) -> String {
    render_document(snapshot, score, true)
}

pub fn render_text(snapshot: &ScanSnapshot, score: &ScoreSnapshot) -> String {
    render_document(snapshot, score, false)
}

fn render_document(snapshot: &ScanSnapshot, score: &ScoreSnapshot, markdown: bool) -> String {
    let heading = if markdown { "# " } else { "" };
    let section_heading = if markdown { "## " } else { "" };
    let issue_heading = if markdown { "### " } else { "" };
    let code = |value: &str| {
        if !markdown {
            return value.to_owned();
        }
        let longest_run = value
            .split(|character| character != '`')
            .map(str::len)
            .max()
            .unwrap_or(0);
        let fence = "`".repeat(longest_run + 1);
        if longest_run > 0 {
            format!("{fence} {value} {fence}")
        } else {
            format!("{fence}{value}{fence}")
        }
    };
    let mut out = format!(
        "{heading}CodeDoctor 扫描报告\n\n- 扫描：{}\n- 项目：{}\n- 状态：{:?}\n- 开始时间：{}\n- 完成时间：{}\n- 规则版本：{}\n- 分析器版本：{}\n- 文件数：{}\n- 检测覆盖率：{:.0}%\n- 评分可信度：{:.0}%\n- 质量分：{}\n",
        code(&snapshot.run.id),
        code(&snapshot.run.project_id),
        snapshot.run.status,
        code(snapshot.run.started_at.as_deref().unwrap_or("未记录")),
        code(snapshot.run.completed_at.as_deref().unwrap_or("未完成")),
        code(&score.rule_version),
        code(&score.analyzer_versions.to_string()),
        snapshot.file_count,
        score.coverage * 100.0,
        score.confidence * 100.0,
        quality_score(score),
    );
    for line in summary_lines(score) {
        let _ = writeln!(out, "- {line}");
    }
    let _ = writeln!(out, "\n{section_heading}评分维度\n");

    for (name, weight) in WEIGHTS {
        let item = &score.dimensions["items"][name];
        let value = item["score"]
            .as_f64()
            .map(|value| format!("{value:.1}"))
            .unwrap_or_else(|| "未评估".to_owned());
        let coverage = item["evidenceCoverage"].as_f64().unwrap_or(0.0) * 100.0;
        let _ = writeln!(
            out,
            "- {}：{value}（权重 {weight}，证据覆盖 {coverage:.0}%）",
            code(name)
        );
    }
    let _ = writeln!(out, "\n{section_heading}问题\n");
    for issue in &snapshot.issues {
        let location = format!(
            "{}:{}:{}-{}:{}",
            issue.file_path, issue.start_line, issue.start_column, issue.end_line, issue.end_column
        );
        let _ = writeln!(
            out,
            "{issue_heading}{} {}\n\n- 严重度：{:?}\n- 位置：{}\n- 来源：{}\n- 覆盖：{:?}，可信度：{:.0}%\n- 证据：{}\n- 说明：{}\n- 影响：{}\n- 建议：{}\n",
            issue.title,
            code(&issue.id),
            issue.severity,
            code(&location),
            code(&issue.source),
            issue.coverage,
            issue.confidence * 100.0,
            code(&issue.evidence),
            issue.explanation,
            issue.impact,
            issue.recommendation,
        );
    }
    if snapshot.issues.is_empty() {
        out.push_str("当前扫描没有发现问题，仍需结合覆盖率理解结果。\n");
    }
    out
}
