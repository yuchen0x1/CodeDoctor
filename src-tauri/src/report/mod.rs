//! Deterministic report rendering from one scan snapshot.
pub mod html;
pub mod json;
pub mod markdown;
mod sarif;

use crate::domain::types::ScoreSnapshot;
use crate::score::ScanSnapshot;
#[cfg(test)]
use crate::score::calculate_score;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReportFormat {
    Json,
    Markdown,
    Txt,
    Html,
    Sarif,
}

#[cfg(test)]
fn render(snapshot: &ScanSnapshot, format: ReportFormat) -> Result<String, crate::error::AppError> {
    let score = calculate_score(snapshot);
    render_with_score(snapshot, &score, format)
}

pub fn render_with_score(
    snapshot: &ScanSnapshot,
    score: &ScoreSnapshot,
    format: ReportFormat,
) -> Result<String, crate::error::AppError> {
    match format {
        ReportFormat::Json => json::render(snapshot, score),
        ReportFormat::Markdown => Ok(markdown::render(snapshot, score)),
        ReportFormat::Txt => Ok(markdown::render_text(snapshot, score)),
        ReportFormat::Html => Ok(html::render(snapshot, score)),
        ReportFormat::Sarif => sarif::render(snapshot),
    }
}

pub(crate) fn quality_score(score: &ScoreSnapshot) -> String {
    if score.confidence > 0.0 {
        format!("{:.1}", score.overall_score)
    } else {
        "未评估".into()
    }
}

fn summary_lines(score: &ScoreSnapshot) -> Vec<String> {
    let dimensions = &score.dimensions;
    let metrics = &dimensions["metrics"];
    let mut lines = vec![format!(
        "已评估维度平均分：{}",
        dimensions["assessedAverage"]
            .as_f64()
            .map(|value| format!("{value:.1}"))
            .unwrap_or_else(|| "未评估".into())
    )];
    let coverage = &metrics["fileCoverage"];
    if coverage.is_object() {
        lines.push(format!(
            "解析结果：AST {}，结构 {}，词法 {}，未覆盖 {}",
            coverage["astFiles"],
            coverage["structuralFiles"],
            coverage["lexicalFiles"],
            coverage["unparsedFiles"],
        ));
    } else {
        lines.push("此快照的覆盖依据为已记录的发现。".into());
    }
    if let Some(ratio) = metrics["duplicateFileRatio"].as_f64() {
        lines.push(format!(
            "含重复片段的已检测文本文件：{}/{}（{:.1}%）",
            metrics["duplicateFileCount"],
            metrics["duplicateEligibleFileCount"],
            ratio * 100.0,
        ));
    }
    if let Some(count) = metrics["highComplexityFileCount"].as_u64() {
        lines.push(format!("触发已启用复杂度阈值的文件：{count}"));
    }
    if let (Some(modules), Some(dependencies)) = (
        metrics["riskModuleCount"].as_u64(),
        metrics["dependencyEdgeCount"].as_u64(),
    ) {
        lines.push(format!(
            "已解析依赖：{dependencies} 条，涉及风险模块 {modules} 个"
        ));
    }
    if let Some(analyzers) = dimensions["analyzerRuns"]
        .as_array()
        .filter(|list| !list.is_empty())
    {
        let statuses = analyzers
            .iter()
            .filter_map(|analyzer| {
                Some(format!(
                    "{} {}",
                    analyzer["analyzer"].as_str()?,
                    match analyzer["status"].as_str()? {
                        "executed" => "已执行",
                        "failed" => "失败",
                        "skipped" => "已跳过",
                        "unavailable" => "不可用",
                        "available" => "可用",
                        _ => return None,
                    }
                ))
            })
            .collect::<Vec<_>>();
        if !statuses.is_empty() {
            lines.push(format!("分析器状态：{}", statuses.join("，")));
        }
    }
    let debt = &dimensions["debt"];
    if let (Some(min), Some(max)) = (debt["minUnits"].as_u64(), debt["maxUnits"].as_u64()) {
        let factors = &debt["factors"];
        lines.push(format!(
            "已发现问题的技术债务：{min}–{max} 个处理单元（按相同位置合并至逐项处理；严重 {}，高 {}，中 {}，低 {}）",
            factors["critical"], factors["high"], factors["medium"], factors["low"],
        ));
    } else {
        lines.push("技术债务：未评估".into());
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::{CoverageKind, Issue, ScanRun, ScanStatus, Severity};
    use serde_json::json;
    fn snapshot() -> ScanSnapshot {
        let issue = Issue {
            id: "i<&".into(),
            rule_id: "security.secret".into(),
            category: "security".into(),
            severity: Severity::High,
            file_path: "src/<unsafe>.py".into(),
            start_line: 4,
            start_column: 1,
            end_line: 4,
            end_column: 6,
            title: "危险 <tag>".into(),
            explanation: "explain & detail".into(),
            evidence: "password = 'x'".into(),
            impact: "impact".into(),
            recommendation: "fix".into(),
            source: "codedoctor".into(),
            coverage: CoverageKind::Ast,
            confidence: 0.9,
            fingerprint: "f".into(),
        };
        ScanSnapshot {
            run: ScanRun {
                id: "run".into(),
                project_id: "p".into(),
                status: ScanStatus::Completed,
                started_at: Some("1".into()),
                completed_at: Some("2".into()),
                failure_message: None,
                config: json!({}),
                rule_version: "rules-1".into(),
                analyzer_versions: json!({}),
                total_files: 1,
                scanned_files: 1,
                total_issues: 1,
            },
            issues: vec![issue],
            edges: vec![],
            file_count: 1,
            source_file_count: 1,
            evidence: Default::default(),
        }
    }
    #[test]
    fn all_formats_include_real_issue_evidence() {
        let s = snapshot();
        for format in [
            ReportFormat::Json,
            ReportFormat::Markdown,
            ReportFormat::Txt,
        ] {
            let text = render(&s, format).unwrap();
            assert!(text.contains("i<&"));
            assert!(text.contains("src/<unsafe>.py"));
            assert!(text.contains("assessedAverage") || text.contains("已评估维度平均分"));
        }
        let html = render(&s, ReportFormat::Html).unwrap();
        assert!(html.contains("i&lt;&amp;"));
        assert!(html.contains("src/&lt;unsafe&gt;.py"));
    }
    #[test]
    fn html_escapes_user_data() {
        let text = render(&snapshot(), ReportFormat::Html).unwrap();
        assert!(text.contains("&lt;unsafe&gt;"));
        assert!(!text.contains("<unsafe>"));
    }

    #[test]
    fn html_is_a_standalone_zh_cn_document() {
        let text = render(&snapshot(), ReportFormat::Html).unwrap();
        assert!(text.starts_with("<!doctype html>\n<html lang=\"zh-CN\">"));
        assert!(text.contains("<meta charset=\"utf-8\">"));
        assert!(
            text.contains(
                "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">"
            )
        );
        assert!(text.contains("<meta name=\"color-scheme\" content=\"light dark\">"));
        assert!(text.contains("<title>CodeDoctor 扫描报告 - p / run</title>"));
        assert!(text.contains("<style>"));
        assert!(text.trim_end().ends_with("</html>"));
        // 完整文档结构：摘要、评分维度、问题三个语义区块，样式内联、无外部引用。
        assert_eq!(text.matches("<section").count(), 3);
        assert!(text.contains("<main>"));
        assert!(text.contains("aria-labelledby=\"summary-heading\""));
        assert!(text.contains("aria-labelledby=\"dimensions-heading\""));
        assert!(text.contains("aria-labelledby=\"issues-heading\""));
        assert!(!text.contains("<script"));
        assert!(!text.contains("src="));
        assert!(!text.contains("href="));
        assert!(!text.contains("@import"));
        assert!(!text.contains("url("));
    }

    #[test]
    fn html_escapes_script_and_style_payloads() {
        let mut scan = snapshot();
        scan.issues[0].title = "</title><script>alert(1)</script>".into();
        scan.issues[0].evidence = "</pre><style>body{display:none}</style>".into();
        scan.issues[0].explanation = "</p><img src=x onerror=alert(2)>".into();
        scan.run.id = "\"onmouseover=alert(3)".into();
        scan.run.project_id = "<iframe src=javascript:alert(4)>".into();
        let text = render(&scan, ReportFormat::Html).unwrap();
        assert!(text.contains("&lt;/title&gt;&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(text.contains("&lt;/pre&gt;&lt;style&gt;body{display:none}&lt;/style&gt;"));
        assert!(text.contains("&lt;/p&gt;&lt;img src=x onerror=alert(2)&gt;"));
        assert!(text.contains("&quot;onmouseover=alert(3)"));
        assert!(text.contains("&lt;iframe src=javascript:alert(4)&gt;"));
        assert!(!text.contains("<script"));
        assert!(!text.contains("<style>body"));
        assert!(!text.contains("<img"));
        assert!(!text.contains("<iframe"));
    }

    #[test]
    fn html_exposes_issue_count_and_all_issue_fields() {
        let text = render(&snapshot(), ReportFormat::Html).unwrap();
        assert!(text.contains("问题 <strong>1</strong>"));
        assert!(text.contains("行列 <code>4:1, 4:6</code>"));
        assert!(text.contains("来源 <code>codedoctor</code>"));
        assert!(text.contains("覆盖 <code>AST</code>"));
        assert!(text.contains("可信度 90%"));
        assert!(text.contains("<pre><code>password = &#39;x&#39;</code></pre>"));
        assert!(text.contains("影响：impact"));
        assert!(text.contains("建议：fix"));
        let empty = ScanSnapshot {
            issues: vec![],
            ..snapshot()
        };
        let text = render(&empty, ReportFormat::Html).unwrap();
        assert!(text.contains("问题 <strong>0</strong>"));
        assert!(text.contains("class=\"empty\""));
    }

    #[test]
    fn html_css_supports_theme_responsive_and_print() {
        let css = include_str!("report.css");
        let text = render(&snapshot(), ReportFormat::Html).unwrap();
        assert!(text.contains(css));
        assert!(css.contains("@media (prefers-color-scheme: dark)"));
        assert!(css.contains("@media (max-width: 640px)"));
        assert!(css.contains("@media print"));
        let print = css.split("@media print").nth(1).unwrap();
        for declaration in [
            "color-scheme: light;",
            "--bg-raised: #ffffff;",
            "--text: #000000;",
            "--code-text: #263d58;",
            "--high: #913b09;",
        ] {
            assert!(
                print.contains(declaration),
                "missing print override: {declaration}"
            );
        }
        assert!(css.contains("color-scheme: light dark"));
        let dark = css
            .split("@media (prefers-color-scheme: dark)")
            .nth(1)
            .unwrap()
            .split("* { box-sizing")
            .next()
            .unwrap();
        for line in dark.lines() {
            if let Some((token, _)) = line.trim().split_once(':')
                && token.starts_with("--")
            {
                assert!(
                    print.contains(&format!("{token}:")),
                    "missing print token: {token}"
                );
            }
        }
        // 独立样式：不依赖应用 CSS，也不引入外部资源。
        assert!(css.contains("--font-mono"));
        assert!(!css.contains("var(--color-"));
        assert!(!css.contains("http"));
    }

    #[test]
    fn markdown_treats_untrusted_fields_and_multiline_evidence_as_literal_text() {
        use pulldown_cmark::{Event, Parser, Tag};

        let mut scan = snapshot();
        let payload = "</h3><script>alert(1)</script> ![remote](https://example.invalid/image) [link](javascript:alert)";
        scan.run.id = "run\n\n# forged heading".into();
        scan.issues[0].title = payload.into();
        scan.issues[0].explanation = format!("{payload}\n\n# forged heading");
        scan.issues[0].impact = payload.into();
        scan.issues[0].recommendation = payload.into();
        let evidence = "first line\n\n```text\n<img src=x onerror=alert(2)>\n```````\n![remote](https://example.invalid/evidence)\n  last `line`  ";
        scan.issues[0].evidence = evidence.into();
        let markdown = render(&scan, ReportFormat::Markdown).unwrap();
        let mut headings = 0;
        let mut blocks = Vec::new();
        let mut in_block = false;
        for event in Parser::new(&markdown) {
            match event {
                Event::Html(_) | Event::InlineHtml(_) => panic!("active HTML in report: {event:?}"),
                Event::Start(Tag::Image { .. } | Tag::Link { .. }) => {
                    panic!("active link in report")
                }
                Event::Start(Tag::Heading { .. }) => headings += 1,
                Event::Start(Tag::CodeBlock(_)) => {
                    blocks.push(String::new());
                    in_block = true;
                }
                Event::End(pulldown_cmark::TagEnd::CodeBlock) => in_block = false,
                Event::Text(text) if in_block => blocks.last_mut().unwrap().push_str(&text),
                _ => {}
            }
        }
        assert_eq!(headings, 4, "untrusted input changed the report outline");
        assert_eq!(blocks, vec![format!("{evidence}\n")]);
        let plain = render(&scan, ReportFormat::Txt).unwrap();
        assert!(plain.contains(payload));
        assert!(plain.contains(evidence));
        assert!(plain.contains("run\n\n# forged heading"));
    }

    #[test]
    fn txt_preserves_source_evidence_characters() {
        let mut scan = snapshot();
        scan.issues[0].evidence = "print(`template`) # source comment".into();
        let text = render(&scan, ReportFormat::Txt).unwrap();
        assert!(text.contains("print(`template`) # source comment"));
        assert!(text.contains("未评估"));
    }
}
