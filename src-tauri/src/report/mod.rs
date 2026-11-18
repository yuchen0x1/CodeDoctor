//! Deterministic report rendering from one scan snapshot.
pub mod html;
pub mod json;
pub mod markdown;

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
    fn txt_preserves_source_evidence_characters() {
        let mut scan = snapshot();
        scan.issues[0].evidence = "print(`template`) # source comment".into();
        let text = render(&scan, ReportFormat::Txt).unwrap();
        assert!(text.contains("print(`template`) # source comment"));
        assert!(text.contains("未评估"));
    }
}
