//! Evidence grounded governance prompt generation. No model or network is used.
use crate::domain::types::{
    FileRecord, Issue, ProjectRecord, PromptArtifact, PromptMode, ScoreSnapshot, Severity,
};
use crate::history::{ChangeKind, HistoryDiff, IssueChange};
use crate::report::quality_score;
use crate::score::ScanSnapshot;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write;
use std::time::{SystemTime, UNIX_EPOCH};

pub const TEMPLATE_VERSION: &str = "prompt-3";
pub const GOVERNANCE_PHASES: &[&str] = &[
    "第一阶段：全面理解项目。读取目录、配置、架构、核心业务和已有测试。",
    "第二阶段：逐项审查。对照报告验证问题、识别误报并确认业务影响。",
    "第三阶段：制定修复方案。按严重度、复杂度、重复和架构风险排序。",
    "第四阶段：执行重构。分阶段修改，禁止一次性重写整个项目。",
    "第五阶段：回归验证。执行测试、构建、接口兼容性和业务验证。",
    "第六阶段：修复后二次审查。重新扫描原问题并检查新问题和回归。",
];
pub const CONSTRAINTS: &[&str] = &[
    "不破坏现有功能。",
    "不随意删除业务逻辑。",
    "不随意更换技术栈。",
    "不无必要地引入新依赖。",
    "不直接重写整个项目。",
    "修改数据结构时提供兼容方案。",
    "不随意修改公开 API。",
    "保留必要注释和版权声明。",
    "不为降低行数进行无意义重构。",
    "每次修改说明理由。",
    "重要修改提供对应测试。",
    "保留原项目已有功能。",
    "无法确认的代码先保留。",
    "所有修改经过构建和测试验证。",
    "未验证的功能不得宣称完成。",
];

pub(crate) struct PromptProjectContext {
    project: ProjectRecord,
    languages: BTreeSet<String>,
    modules: Vec<String>,
}

impl PromptProjectContext {
    pub(crate) fn from_index(project: ProjectRecord, files: Vec<FileRecord>) -> Self {
        let mut languages = BTreeSet::new();
        let mut modules = BTreeMap::<String, usize>::new();
        for file in files.into_iter().filter(|file| {
            file.excluded_reason.is_none()
                && !matches!(file.language.as_str(), "" | "-" | "Unknown")
        }) {
            languages.insert(file.language);
            let module = file
                .relative_path
                .split_once('/')
                .map_or("", |(top, _)| top);
            *modules.entry(module.to_owned()).or_default() += 1;
        }
        let mut modules: Vec<_> = modules.into_iter().collect();
        modules.sort_by(|(path_a, count_a), (path_b, count_b)| {
            count_b.cmp(count_a).then(path_a.cmp(path_b))
        });
        let modules = modules
            .into_iter()
            .take(8)
            .map(|(path, count)| {
                let module = if path.is_empty() {
                    "项目根目录"
                } else {
                    &path
                };
                format!("{module}（{count} 个源文件）")
            })
            .collect();
        Self {
            project,
            languages,
            modules,
        }
    }
}

pub(crate) fn generate_prompt(
    snapshot: &ScanSnapshot,
    score: &ScoreSnapshot,
    project_context: &PromptProjectContext,
    mode: PromptMode,
    selected_issue_ids: &[String],
    incremental: Option<&HistoryDiff>,
) -> PromptArtifact {
    let changes = incremental.map(actionable_changes);
    let selected: HashSet<&str> = selected_issue_ids.iter().map(String::as_str).collect();
    let issues: Vec<_> = snapshot
        .issues
        .iter()
        .filter(|issue| mode_includes(mode, &selected, issue))
        .filter(|issue| {
            changes
                .as_ref()
                .is_none_or(|changes| changes.contains_key(issue.id.as_str()))
        })
        .collect();
    let dependency_edges = score.dimensions["metrics"]["dependencyEdgeCount"]
        .as_u64()
        .map_or_else(|| "未评估".to_owned(), |count| count.to_string());
    let mut content = format!(
        "# CodeDoctor 项目治理任务书\n\n模板版本：{TEMPLATE_VERSION}\n治理模式：{}\n目标扫描 ID：{}\n项目 ID：{}\n扫描状态：{:?}\n规则版本：{}\n分析器版本：{}\n文件数：{}（源码 {}）\n依赖边：{dependency_edges}\n质量分：{}\n检测覆盖率：{:.0}%\n评分可信度：{:.0}%\n",
        mode_label(mode),
        snapshot.run.id,
        snapshot.run.project_id,
        snapshot.run.status,
        snapshot.run.rule_version,
        snapshot.run.analyzer_versions,
        snapshot.file_count,
        snapshot.source_file_count,
        quality_score(score),
        score.coverage * 100.0,
        score.confidence * 100.0,
    );
    if let Some(diff) = incremental {
        let _ = writeln!(
            content,
            "治理范围：增量（新增和严重度上升的问题）\n基线扫描 ID：{}\n规则版本变化：{}\n检测覆盖率变化：{:+.1} 个百分点\n范围内问题数：{}",
            diff.base_run_id,
            if diff.rule_version_changed {
                "有"
            } else {
                "无"
            },
            diff.coverage_delta * 100.0,
            issues.len(),
        );
    } else {
        let _ = writeln!(
            content,
            "治理范围：目标扫描\n范围内问题数：{}",
            issues.len()
        );
    }
    write_project_context(&mut content, project_context, snapshot);
    content.push_str("\n## 执行阶段\n\n");
    for phase in GOVERNANCE_PHASES {
        content.push_str(&format!("- {}\n", phase));
    }
    content.push_str("\n## 约束\n\n");
    for item in CONSTRAINTS {
        content.push_str(&format!("- {}\n", item));
    }
    content.push_str("\n## 待处理问题\n\n");
    for issue in &issues {
        let _ = writeln!(content, "### {} [{}]", issue.title, issue.id);
        if let Some(change) = changes
            .as_ref()
            .and_then(|changes| changes.get(issue.id.as_str()))
        {
            if let Some(baseline_severity) = change.baseline_severity {
                let _ = writeln!(
                    content,
                    "- 变化：严重度上升（{:?} → {:?}）",
                    baseline_severity, issue.severity
                );
            } else {
                content.push_str("- 变化：新增\n");
            }
        }
        let _ = writeln!(
            content,
            concat!(
                "- 文件：{}\n- 行号：{}:{}-{}:{}\n- 严重度：{:?}\n",
                "- 来源：{}\n- 覆盖：{:?}\n- 可信度：{:.0}%\n",
                "- 证据：{}\n- 说明：{}\n- 影响：{}\n- 建议：{}\n",
            ),
            issue.file_path,
            issue.start_line,
            issue.start_column,
            issue.end_line,
            issue.end_column,
            issue.severity,
            issue.source,
            issue.coverage,
            issue.confidence * 100.0,
            issue.evidence,
            issue.explanation,
            issue.impact,
            issue.recommendation,
        );
    }
    if issues.is_empty() {
        content.push_str("当前范围内没有待处理问题。请确认扫描覆盖率和未评估项。\n");
    }
    PromptArtifact {
        id: format!("prompt-{:x}", Sha256::digest(content.as_bytes())),
        mode,
        selected_issue_ids: issues.iter().map(|i| i.id.clone()).collect(),
        template_version: TEMPLATE_VERSION.into(),
        content,
        created_at: now(),
    }
}

fn write_project_context(
    content: &mut String,
    context: &PromptProjectContext,
    snapshot: &ScanSnapshot,
) {
    let recorded = &snapshot.run.config["indexedProjectContext"];
    let frameworks = recorded["frameworks"].as_array().map_or_else(
        || "扫描快照未记录".to_owned(),
        |items| {
            describe_items(
                items.iter().filter_map(serde_json::Value::as_str),
                "未识别到框架或构建工具",
            )
        },
    );
    let lines = recorded["sourceLineCount"]
        .as_u64()
        .map_or_else(|| "扫描快照未记录".to_owned(), |count| count.to_string());
    let _ = writeln!(
        content,
        "\n## 项目画像\n\n- 名称：{}\n- 本地路径：{}\n- 语言：{}\n- 框架与构建工具：{frameworks}\n- 已索引源码行数（含注释及空行）：{lines}\n- 主要模块（按顶层源文件数排列，最多八项）：{}",
        context.project.name,
        context.project.normalized_path,
        describe_items(context.languages.iter().map(String::as_str), "未识别到语言"),
        describe_items(
            context.modules.iter().map(String::as_str),
            "未识别到源文件目录"
        ),
    );
}

fn describe_items<'a>(items: impl Iterator<Item = &'a str>, empty: &str) -> String {
    let items = items.collect::<Vec<_>>().join("、");
    if items.is_empty() {
        empty.to_owned()
    } else {
        items
    }
}

fn actionable_changes(diff: &HistoryDiff) -> HashMap<&str, &IssueChange> {
    diff.issue_changes
        .iter()
        .filter(|change| match change.kind {
            ChangeKind::Added => true,
            ChangeKind::SeverityChanged => change
                .baseline_severity
                .zip(change.current_severity)
                .is_some_and(|(base, target)| severity_rank(target) > severity_rank(base)),
            ChangeKind::Resolved | ChangeKind::Persistent | ChangeKind::LocationChanged => false,
        })
        .filter_map(|change| change.current_issue_id.as_deref().map(|id| (id, change)))
        .collect()
}

fn severity_rank(severity: Severity) -> u8 {
    match severity {
        Severity::Info => 0,
        Severity::Low => 1,
        Severity::Medium => 2,
        Severity::High => 3,
        Severity::Critical => 4,
    }
}

fn mode_includes(mode: PromptMode, selected: &HashSet<&str>, issue: &Issue) -> bool {
    match mode {
        PromptMode::Comprehensive => true,
        PromptMode::Focused => {
            matches!(issue.severity, Severity::Critical | Severity::High)
                || issue.category.contains("duplicate")
                || issue.category.contains("architecture")
                || issue.category.contains("security")
                || issue.rule_id.contains("complex")
        }
        PromptMode::Targeted => selected.contains(issue.id.as_str()),
    }
}

fn mode_label(mode: PromptMode) -> &'static str {
    match mode {
        PromptMode::Comprehensive => "全面治理",
        PromptMode::Focused => "重点治理",
        PromptMode::Targeted => "定点修复",
    }
}
fn now() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "0".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::{CoverageKind, ScanRun, ScanStatus};
    use crate::history::compare_scans;
    use crate::score::calculate_score;
    use serde_json::json;

    fn project_context() -> PromptProjectContext {
        PromptProjectContext::from_index(
            ProjectRecord {
                id: "p".into(),
                name: "治理示例".into(),
                normalized_path: "C:/projects/example".into(),
                created_at: "0".into(),
            },
            vec![FileRecord {
                id: "f".into(),
                scan_run_id: "r".into(),
                project_id: "p".into(),
                relative_path: "src/app.py".into(),
                absolute_path: "C:/projects/example/src/app.py".into(),
                encoding: "UTF-8".into(),
                size_bytes: 30,
                content_hash: "a".into(),
                language: "Python".into(),
                parse_capability: CoverageKind::Ast,
                excluded_reason: None,
            }],
        )
    }

    fn snap() -> ScanSnapshot {
        let mk = |id: &str, sev: Severity| Issue {
            id: id.into(),
            rule_id: if id == "high" {
                "security.secret"
            } else {
                "quality.naming"
            }
            .into(),
            category: if id == "high" { "security" } else { "quality" }.into(),
            severity: sev,
            file_path: "src/app.py".into(),
            start_line: 4,
            start_column: 1,
            end_line: 4,
            end_column: 2,
            title: "Secret".into(),
            explanation: "explain".into(),
            evidence: "TOKEN=...".into(),
            impact: "impact".into(),
            recommendation: "rotate".into(),
            source: "codedoctor".into(),
            coverage: CoverageKind::Ast,
            confidence: 0.9,
            fingerprint: id.into(),
        };
        ScanSnapshot {
            run: ScanRun {
                id: "r".into(),
                project_id: "p".into(),
                status: ScanStatus::Completed,
                started_at: None,
                completed_at: None,
                failure_message: None,
                config: json!({}),
                rule_version: "rules-1".into(),
                analyzer_versions: json!({}),
                total_files: 1,
                scanned_files: 1,
                total_issues: 2,
            },
            issues: vec![mk("high", Severity::High), mk("low", Severity::Low)],
            edges: vec![],
            file_count: 1,
            source_file_count: 1,
            evidence: Default::default(),
        }
    }
    #[test]
    fn modes_preserve_real_evidence_and_filter() {
        let s = snap();
        let score = calculate_score(&s);
        let project = project_context();
        let full = generate_prompt(&s, &score, &project, PromptMode::Comprehensive, &[], None);
        assert!(
            full.content.contains("high")
                && full.content.contains("src/app.py")
                && full.content.contains("TOKEN")
        );
        let focused = generate_prompt(&s, &score, &project, PromptMode::Focused, &[], None);
        assert!(focused.content.contains("high") && !focused.content.contains("low"));
        let targeted = generate_prompt(
            &s,
            &score,
            &project,
            PromptMode::Targeted,
            &["low".into()],
            None,
        );
        assert!(targeted.content.contains("low") && !targeted.content.contains("high"));
        assert_eq!(CONSTRAINTS.len(), 15);
        assert_eq!(GOVERNANCE_PHASES.len(), 6);
    }

    #[test]
    fn project_context_uses_indexed_data_and_marks_missing_historical_metadata() {
        let mut s = snap();
        let score = calculate_score(&s);
        let project = project_context();
        let old = generate_prompt(&s, &score, &project, PromptMode::Comprehensive, &[], None);
        assert!(old.content.contains("框架与构建工具：扫描快照未记录"));
        assert!(
            old.content
                .contains("源码行数（含注释及空行）：扫描快照未记录")
        );

        s.run.config["indexedProjectContext"] =
            json!({"sourceLineCount": 5, "frameworks": ["Flask"]});
        let current = generate_prompt(&s, &score, &project, PromptMode::Comprehensive, &[], None);
        assert!(current.content.contains("名称：治理示例"));
        assert!(current.content.contains("本地路径：C:/projects/example"));
        assert!(current.content.contains("语言：Python"));
        assert!(current.content.contains("框架与构建工具：Flask"));
        assert!(current.content.contains("源码行数（含注释及空行）：5"));
        assert!(current.content.contains("src（1 个源文件）"));
    }

    #[test]
    fn incremental_evidence_includes_additions_and_regressions_within_the_mode() {
        let mut target = snap();
        let mut baseline = target.clone();
        baseline.run.id = "baseline".into();
        baseline.issues[0].id = "base-high".into();
        baseline.issues[0].severity = Severity::Medium;
        baseline.issues[1].severity = Severity::Medium;
        let low = target.issues[1].clone();
        let renamed = |id: &str| Issue {
            id: id.into(),
            fingerprint: id.into(),
            evidence: format!("issue evidence for {id}"),
            ..low.clone()
        };
        let old_location = renamed("moved");
        let new_location = Issue {
            id: "moved-target".into(),
            fingerprint: "moved-target".into(),
            start_line: 10,
            end_line: 10,
            ..old_location.clone()
        };
        baseline
            .issues
            .extend([renamed("persistent"), renamed("resolved"), old_location]);
        target
            .issues
            .extend([renamed("persistent"), renamed("new-low"), new_location]);

        let score = calculate_score(&target);
        let diff = compare_scans(&baseline, &target, None, Some(&score));
        let project = project_context();
        let full = generate_prompt(
            &target,
            &score,
            &project,
            PromptMode::Comprehensive,
            &[],
            Some(&diff),
        );
        assert_eq!(full.selected_issue_ids, ["high", "new-low"]);
        assert!(full.content.contains("目标扫描 ID：r"));
        assert!(full.content.contains("基线扫描 ID：baseline"));
        assert!(full.content.contains("严重度上升（Medium → High）"));
        assert!(full.content.contains("变化：新增"));
        assert!(full.content.contains("证据：TOKEN=..."));

        let focused = generate_prompt(
            &target,
            &score,
            &project,
            PromptMode::Focused,
            &[],
            Some(&diff),
        );
        assert_eq!(focused.selected_issue_ids, ["high"]);
        let targeted = generate_prompt(
            &target,
            &score,
            &project,
            PromptMode::Targeted,
            &["low".into(), "new-low".into()],
            Some(&diff),
        );
        assert_eq!(targeted.selected_issue_ids, ["new-low"]);
        assert_ne!(targeted.id, full.id);
    }
}
