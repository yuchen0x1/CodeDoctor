use super::{quality_score, summary_lines};
use crate::domain::types::ScoreSnapshot;
use crate::score::{ScanSnapshot, WEIGHTS};
fn esc(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
pub fn render(snapshot: &ScanSnapshot, score: &ScoreSnapshot) -> String {
    let mut out = format!(
        "<!doctype html><meta charset=\"utf-8\"><title>CodeDoctor 报告</title><main><h1>CodeDoctor 扫描报告</h1><p>扫描 <code>{}</code> · 项目 <code>{}</code> · 状态 <code>{:?}</code></p><p>开始：<code>{}</code> · 完成：<code>{}</code></p><p>规则版本 <code>{}</code> · 分析器版本 <code>{}</code></p><p>文件 {} · 检测覆盖率 {:.0}% · 评分可信度 {:.0}% · 质量分 {}</p><ul>",
        esc(&snapshot.run.id),
        esc(&snapshot.run.project_id),
        snapshot.run.status,
        esc(snapshot.run.started_at.as_deref().unwrap_or("未记录")),
        esc(snapshot.run.completed_at.as_deref().unwrap_or("未完成")),
        esc(&score.rule_version),
        esc(&score.analyzer_versions.to_string()),
        snapshot.file_count,
        score.coverage * 100.0,
        score.confidence * 100.0,
        quality_score(score),
    );
    for line in summary_lines(score) {
        out.push_str(&format!("<li>{}</li>", esc(&line)));
    }
    out.push_str("</ul><section><h2>评分维度</h2><ul>");
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
        out.push_str(&format!(
            "<li><code>{}</code>：{} · 权重 {} · 证据覆盖 {:.0}%</li>",
            esc(name),
            score_text,
            weight,
            coverage * 100.0
        ));
    }
    out.push_str("</ul><h2>问题</h2>");
    for issue in &snapshot.issues {
        out.push_str(&format!("<article><h3>{}</h3><p><code>{}</code> · {:?} · <code>{}:{}:{}-{}:{}</code></p><p>来源：<code>{}</code> · 覆盖：<code>{:?}</code> · 可信度：{:.0}%</p><p>证据：<code>{}</code></p><p>{}</p><p>影响：{}</p><p>建议：{}</p></article>",esc(&issue.title),esc(&issue.id),issue.severity,esc(&issue.file_path),issue.start_line,issue.start_column,issue.end_line,issue.end_column,esc(&issue.source),issue.coverage,issue.confidence * 100.0,esc(&issue.evidence),esc(&issue.explanation),esc(&issue.impact),esc(&issue.recommendation)));
    }
    if snapshot.issues.is_empty() {
        out.push_str("<p>当前扫描没有发现问题，仍需结合覆盖率理解结果。</p>");
    }
    out.push_str("</section></main>");
    out
}
