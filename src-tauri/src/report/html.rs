use super::{quality_score, summary_lines};
use crate::domain::types::{CoverageKind, ScoreSnapshot, Severity};
use crate::score::{ScanSnapshot, WEIGHTS};

const REPORT_CSS: &str = include_str!("report.css");

fn esc(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn severity_class(severity: Severity) -> &'static str {
    match severity {
        Severity::Critical => "sev-critical",
        Severity::High => "sev-high",
        Severity::Medium => "sev-medium",
        Severity::Low => "sev-low",
        Severity::Info => "sev-info",
    }
}

fn severity_label(severity: Severity) -> &'static str {
    match severity {
        Severity::Critical => "严重",
        Severity::High => "高",
        Severity::Medium => "中",
        Severity::Low => "低",
        Severity::Info => "提示",
    }
}

fn coverage_label(coverage: CoverageKind) -> &'static str {
    match coverage {
        CoverageKind::Ast => "AST",
        CoverageKind::Lexical => "词法",
        CoverageKind::Structural => "结构",
        CoverageKind::NotCovered => "未覆盖",
    }
}

pub fn render(snapshot: &ScanSnapshot, score: &ScoreSnapshot) -> String {
    let mut out = String::from(
        "<!doctype html>\n<html lang=\"zh-CN\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <meta name=\"color-scheme\" content=\"light dark\">\n\
         <title>CodeDoctor 扫描报告 - ",
    );
    out.push_str(&esc(&snapshot.run.project_id));
    out.push_str(" / ");
    out.push_str(&esc(&snapshot.run.id));
    out.push_str("</title>\n<style>\n");
    out.push_str(REPORT_CSS);
    out.push_str("</style>\n</head>\n<body>\n<main>\n<header><h1>CodeDoctor 扫描报告</h1>\n<p class=\"meta\">扫描 <code>");
    out.push_str(&esc(&snapshot.run.id));
    out.push_str("</code> · 项目 <code>");
    out.push_str(&esc(&snapshot.run.project_id));
    out.push_str("</code> · 状态 <code class=\"status");
    let status_failed = format!("{:?}", snapshot.run.status).eq("Failed");
    if status_failed {
        out.push_str(" status-failed");
    }
    out.push_str("\">");
    out.push_str(&esc(&format!("{:?}", snapshot.run.status)));
    out.push_str("</code></p>\n<p class=\"meta\">开始：<code>");
    out.push_str(&esc(snapshot.run.started_at.as_deref().unwrap_or("未记录")));
    out.push_str("</code> · 完成：<code>");
    out.push_str(&esc(snapshot
        .run
        .completed_at
        .as_deref()
        .unwrap_or("未完成")));
    out.push_str("</code></p>\n<p class=\"meta\">规则版本 <code>");
    out.push_str(&esc(&score.rule_version));
    out.push_str("</code> · 分析器版本 <code>");
    out.push_str(&esc(&score.analyzer_versions.to_string()));
    out.push_str("</code></p>\n<p>文件 <strong>");
    out.push_str(&snapshot.file_count.to_string());
    out.push_str("</strong> · 问题 <strong>");
    out.push_str(&snapshot.issues.len().to_string());
    out.push_str("</strong> · 检测覆盖率 <strong>");
    out.push_str(&format!("{:.0}", score.coverage * 100.0));
    out.push_str("%</strong> · 评分可信度 <strong>");
    out.push_str(&format!("{:.0}", score.confidence * 100.0));
    out.push_str("%</strong> · 质量分 <strong>");
    out.push_str(&esc(&quality_score(score)));
    out.push_str("</strong></p>\n</header>\n<section aria-labelledby=\"summary-heading\">\n<h2 id=\"summary-heading\">结果摘要</h2>\n<ul>");
    for line in summary_lines(score) {
        out.push_str("<li>");
        out.push_str(&esc(&line));
        out.push_str("</li>");
    }
    out.push_str("</ul>\n</section>\n<section aria-labelledby=\"dimensions-heading\">\n<h2 id=\"dimensions-heading\">评分维度</h2>\n<ul>");
    for (name, weight) in WEIGHTS {
        let item = score
            .dimensions
            .get("items")
            .and_then(|items| items.get(name));
        let value = item
            .and_then(|item| item.get("score"))
            .and_then(|value| value.as_f64());
        let coverage = item
            .and_then(|item| item.get("evidenceCoverage"))
            .and_then(|value| value.as_f64())
            .unwrap_or(0.0);
        let score_text = value
            .map(|value| format!("{value:.1}"))
            .unwrap_or_else(|| "未评估".to_owned());
        out.push_str("<li><code>");
        out.push_str(&esc(name));
        out.push_str("</code>：<strong>");
        out.push_str(&esc(&score_text));
        out.push_str("</strong> · 权重 ");
        out.push_str(&weight.to_string());
        out.push_str(" · 证据覆盖 ");
        out.push_str(&format!("{:.0}", coverage * 100.0));
        out.push_str("%</li>");
    }
    out.push_str("</ul>\n</section>\n<section aria-labelledby=\"issues-heading\">\n<h2 id=\"issues-heading\">问题</h2>");
    for issue in &snapshot.issues {
        out.push_str("\n<article>\n<h3>");
        out.push_str(&esc(&issue.title));
        out.push_str("</h3>\n<p><span class=\"sev ");
        out.push_str(severity_class(issue.severity));
        out.push_str("\">");
        out.push_str(severity_label(issue.severity));
        out.push_str("</span> <code>");
        out.push_str(&esc(&issue.id));
        out.push_str("</code> · <code>");
        out.push_str(&esc(&issue.rule_id));
        out.push_str("</code> · 分类 <code>");
        out.push_str(&esc(&issue.category));
        out.push_str("</code></p>\n<p class=\"meta\">文件 <code>");
        out.push_str(&esc(&issue.file_path));
        out.push_str("</code> · 行列 <code>");
        out.push_str(&format!(
            "{}:{}, {}:{}",
            issue.start_line, issue.start_column, issue.end_line, issue.end_column
        ));
        out.push_str("</code></p>\n<p class=\"meta\">来源 <code>");
        out.push_str(&esc(&issue.source));
        out.push_str("</code> · 覆盖 <code>");
        out.push_str(coverage_label(issue.coverage));
        out.push_str("</code> · 可信度 ");
        out.push_str(&format!("{:.0}", issue.confidence * 100.0));
        out.push_str("%</p>\n<p>证据：</p>\n<pre><code>");
        out.push_str(&esc(&issue.evidence));
        out.push_str("</code></pre>\n<p>说明：");
        out.push_str(&esc(&issue.explanation));
        out.push_str("</p>\n<p>影响：");
        out.push_str(&esc(&issue.impact));
        out.push_str("</p>\n<p>建议：");
        out.push_str(&esc(&issue.recommendation));
        out.push_str("</p>\n</article>");
    }
    if snapshot.issues.is_empty() {
        out.push_str("\n<p class=\"empty\">当前扫描没有发现问题，仍需结合覆盖率理解结果。</p>");
    }
    out.push_str("\n</section>\n</main>\n</body>\n</html>\n");
    out
}
