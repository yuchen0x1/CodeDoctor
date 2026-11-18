//! Evidence weighted project scoring.
pub mod dimensions;
mod evidence;

use self::evidence::{
    FileCoverage, FileEvidence, coverage_for_dimension, finding_is_usable, finding_paths,
    matching_category,
};
use crate::domain::types::{ArchitectureEdge, Issue, ScanRun, ScanStatus, ScoreSnapshot, Severity};
use crate::error::AppError;
use crate::rules::config::RuleConfig;
use crate::rules::runner::RuleRunner;
use crate::storage::Storage;
pub use evidence::ScanEvidence;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanSnapshot {
    pub run: ScanRun,
    pub issues: Vec<Issue>,
    pub edges: Vec<ArchitectureEdge>,
    pub file_count: u32,
    pub source_file_count: u32,
    #[serde(skip)]
    pub evidence: ScanEvidence,
}

impl ScanSnapshot {
    pub fn load(store: &Storage, run: ScanRun) -> Result<Self, AppError> {
        let issues = store.list_issues(&run.id)?;
        let edges = store.list_edges(&run.id)?;
        let analyzers = store.list_analyzer_runs_for_run(&run.id)?;
        let files = if run.status == ScanStatus::Completed && run.config["scanEvidenceVersion"] == 1
        {
            Some(
                store
                    .list_files_for_run(&run.id)?
                    .into_iter()
                    .filter_map(FileEvidence::from_record)
                    .collect::<Vec<_>>(),
            )
        } else {
            None
        };
        let (file_count, source_file_count) = match &files {
            Some(files) => (
                files.len() as u32,
                FileCoverage::from_files(files).parsed_files() as u32,
            ),
            None => (
                store.count_files_for_run(&run.id)?,
                store.count_source_files_for_run(&run.id)?,
            ),
        };
        Ok(Self {
            run,
            issues,
            edges,
            file_count,
            source_file_count,
            evidence: ScanEvidence { files, analyzers },
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DimensionResult {
    pub score: Option<f32>,
    pub weight: u32,
    pub evidence_coverage: f32,
    pub issue_ids: Vec<String>,
    pub evidence: Vec<String>,
}

pub const WEIGHTS: [(&str, u32); 7] = [
    ("readability", 15),
    ("structure", 15),
    ("maintainability", 20),
    ("complexity", 15),
    ("duplicates", 10),
    ("architecture", 15),
    ("security", 10),
];

pub const SCORING_VERSION: &str = "evidence-1";

pub fn calculate_score(snapshot: &ScanSnapshot) -> ScoreSnapshot {
    let findings: Vec<_> = snapshot
        .issues
        .iter()
        .filter(|issue| finding_is_usable(&snapshot.evidence, issue))
        .collect();
    let file_coverage = snapshot
        .evidence
        .files
        .as_deref()
        .map(FileCoverage::from_files);
    let coverage = ratio(
        file_coverage.as_ref().map_or_else(
            || finding_paths(&findings).len(),
            FileCoverage::parsed_files,
        ),
        snapshot.file_count as usize,
    );
    let runner = RuleRunner::with_default_rules();
    let config = RuleConfig::from_json(&snapshot.run.config);
    let mut dimensions = BTreeMap::new();
    let mut weighted = 0.0;
    let mut confidence = 0.0;
    let mut assessed_sum = 0.0;
    let mut assessed_count = 0;
    let mut unassessed = Vec::new();
    for (name, weight) in WEIGHTS {
        let matched: Vec<_> = findings
            .iter()
            .copied()
            .filter(|issue| matching_category(name, &issue.category))
            .collect();
        let evidence_coverage = coverage_for_dimension(
            name,
            snapshot,
            &matched,
            file_coverage.as_ref(),
            &runner,
            &config,
        );
        let evidence = matched
            .iter()
            .map(|issue| format!("{}:{} {}", issue.file_path, issue.start_line, issue.title))
            .take(12)
            .collect();
        let score = (evidence_coverage > 0.0).then(|| dimension_score(&matched));
        if let Some(value) = score {
            let evidence_weight = weight as f32 * evidence_coverage;
            weighted += value * evidence_weight;
            confidence += evidence_weight;
            assessed_sum += value;
            assessed_count += 1;
        } else {
            unassessed.push(name);
        }
        dimensions.insert(
            name,
            DimensionResult {
                score,
                weight,
                evidence_coverage,
                issue_ids: matched.iter().map(|issue| issue.id.clone()).collect(),
                evidence,
            },
        );
    }
    let total_weight = WEIGHTS.iter().map(|(_, weight)| *weight).sum::<u32>() as f32;
    let dimensions_json = json!({
        "scoringVersion": SCORING_VERSION,
        "items": dimensions,
        "assessedAverage": (assessed_count > 0).then(|| assessed_sum / assessed_count as f32),
        "unassessed": unassessed,
        "debt": dimensions::debt_range(&findings),
        "metrics": dimensions::scan_metrics(snapshot, &findings, file_coverage.as_ref(), &config),
        "coverageBasis": if file_coverage.is_some() { "parserResults" } else { "confirmedFindings" },
        "analyzerRuns": snapshot.evidence.analyzers.iter().map(|analyzer| json!({
            "analyzer": analyzer.analyzer, "status": analyzer.status, "version": analyzer.version
        })).collect::<Vec<_>>(),
        "weights": WEIGHTS.iter().map(|(name, weight)| json!({"name":name,"weight":weight})).collect::<Vec<_>>(),
    });
    ScoreSnapshot {
        id: format!("score-{}", snapshot.run.id),
        overall_score: (weighted / total_weight).clamp(0.0, 100.0),
        dimensions: dimensions_json,
        coverage,
        confidence: (confidence / total_weight).clamp(0.0, 1.0),
        rule_version: snapshot.run.rule_version.clone(),
        analyzer_versions: snapshot.run.analyzer_versions.clone(),
        config: snapshot.run.config.clone(),
        created_at: snapshot
            .run
            .completed_at
            .clone()
            .or_else(|| snapshot.run.started_at.clone())
            .unwrap_or_default(),
    }
}

fn ratio(numerator: usize, denominator: usize) -> f32 {
    if denominator == 0 {
        0.0
    } else {
        (numerator as f32 / denominator as f32).clamp(0.0, 1.0)
    }
}

fn dimension_score(findings: &[&Issue]) -> f32 {
    let penalty: f32 = findings
        .iter()
        .map(|issue| match issue.severity {
            Severity::Critical => 22.0,
            Severity::High => 12.0,
            Severity::Medium => 6.0,
            Severity::Low => 2.0,
            Severity::Info => 0.5,
        })
        .sum();
    (100.0 - penalty).clamp(0.0, 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::{
        AnalyzerRun, AnalyzerStatus, CoverageKind, FileRecord, ProjectRecord,
    };
    fn issue(id: &str, category: &str, severity: Severity) -> Issue {
        Issue {
            id: id.into(),
            rule_id: format!("{category}.rule"),
            category: category.into(),
            severity,
            file_path: "src/a.py".into(),
            start_line: 3,
            start_column: 1,
            end_line: 3,
            end_column: 10,
            title: "Finding".into(),
            explanation: "evidence".into(),
            evidence: "evidence".into(),
            impact: "impact".into(),
            recommendation: "fix".into(),
            source: "codedoctor".into(),
            coverage: CoverageKind::Ast,
            confidence: 0.9,
            fingerprint: id.into(),
        }
    }
    fn snapshot(issues: Vec<Issue>, files: u32, source: u32) -> ScanSnapshot {
        ScanSnapshot {
            run: ScanRun {
                id: "run".into(),
                project_id: "p".into(),
                status: crate::domain::types::ScanStatus::Completed,
                started_at: Some("1".into()),
                completed_at: Some("2".into()),
                config: json!({}),
                rule_version: crate::rules::config::RULE_VERSION.into(),
                analyzer_versions: json!({}),
                total_files: files,
                scanned_files: source,
                total_issues: issues.len() as u32,
            },
            issues,
            edges: vec![],
            file_count: files,
            source_file_count: source,
            evidence: ScanEvidence::default(),
        }
    }

    fn file(path: &str, coverage: CoverageKind) -> FileEvidence {
        FileEvidence {
            relative_path: path.into(),
            language: "Python".into(),
            coverage,
        }
    }

    fn edge(source: &str, target: &str, kind: &str) -> ArchitectureEdge {
        ArchitectureEdge {
            id: format!("{source}:{target}:{kind}"),
            source_path: source.into(),
            target_path: target.into(),
            kind: kind.into(),
            confidence: 0.9,
            coverage: CoverageKind::Structural,
            metadata: json!({}),
        }
    }

    #[test]
    fn score_uses_fixed_weights_and_coverage_adjustment() {
        let mut scan = snapshot(vec![issue("s", "security", Severity::High)], 2, 1);
        scan.evidence.files = Some(vec![
            file("src/a.py", CoverageKind::Ast),
            file("src/skipped.py", CoverageKind::NotCovered),
        ]);
        let score = calculate_score(&scan);
        let items = &score.dimensions["items"];
        let expected_overall: f64 = WEIGHTS
            .iter()
            .map(|(name, weight)| {
                items[name]["score"].as_f64().unwrap_or(0.0)
                    * items[name]["evidenceCoverage"].as_f64().unwrap()
                    * f64::from(*weight)
                    / 100.0
            })
            .sum();
        let expected_confidence: f64 = WEIGHTS
            .iter()
            .map(|(name, weight)| {
                items[name]["evidenceCoverage"].as_f64().unwrap() * f64::from(*weight) / 100.0
            })
            .sum();
        assert_eq!(WEIGHTS.iter().map(|(_, w)| *w).sum::<u32>(), 100);
        assert!((f64::from(score.overall_score) - expected_overall).abs() < 0.00001);
        assert!((f64::from(score.confidence) - expected_confidence).abs() < 0.00001);
        assert_eq!(score.coverage, 0.5);
        assert_eq!(items["security"]["score"], 88.0);
        assert_eq!(items["security"]["issueIds"], json!(["s"]));
        assert_eq!(items["security"]["evidenceCoverage"], 0.25);
        assert_eq!(items["complexity"]["evidenceCoverage"], 0.5);
    }

    #[test]
    fn no_files_are_unassessed_and_do_not_claim_full_score() {
        let score = calculate_score(&snapshot(vec![], 0, 0));
        assert_eq!(score.overall_score, 0.0);
        assert_eq!(score.confidence, 0.0);
        assert!(score.dimensions["assessedAverage"].is_null());
        assert!(score.dimensions["debt"].is_null());
        assert_eq!(score.dimensions["unassessed"].as_array().unwrap().len(), 7);
    }

    #[test]
    fn lexical_sources_do_not_assess_complexity_or_architecture() {
        let mut scan = snapshot(Vec::new(), 1, 1);
        scan.evidence.files = Some(vec![file("src/a.py", CoverageKind::Lexical)]);
        let score = calculate_score(&scan);
        assert_eq!(score.coverage, 1.0);
        assert!(score.confidence < 0.5);
        assert!(score.dimensions["items"]["complexity"]["score"].is_null());
        assert!(score.dimensions["items"]["architecture"]["score"].is_null());
        assert!(score.dimensions["metrics"]["highComplexityFileCount"].is_null());
    }

    #[test]
    fn disabled_rules_and_failed_analyzers_do_not_add_coverage() {
        let mut external = issue("external", "quality", Severity::High);
        external.source = "ruff".into();
        let mut scan = snapshot(vec![external], 1, 1);
        scan.evidence.files = Some(vec![file("src/a.py", CoverageKind::Ast)]);
        scan.evidence.analyzers.push(AnalyzerRun {
            id: "ruff".into(),
            analyzer: "ruff".into(),
            status: AnalyzerStatus::Failed,
            version: Some("1".into()),
            duration_ms: None,
            message: None,
        });
        scan.run.config = json!({"enabledRules": []});
        let score = calculate_score(&scan);
        assert!(score.dimensions["items"]["readability"]["score"].is_null());
        assert!(score.dimensions["items"]["security"]["score"].is_null());
        assert_eq!(score.dimensions["analyzerRuns"][0]["status"], "failed");
    }

    #[test]
    fn summary_metrics_use_distinct_paths_and_report_observed_debt_units() {
        let mut cycling = issue("cp", "complexity", Severity::High);
        cycling.rule_id = "complexity.high-cyclomatic".into();
        let mut cognitive = issue("cg", "complexity", Severity::Medium);
        cognitive.rule_id = "complexity.high-cognitive".into();
        let mut duplicate = issue("d", "duplicate", Severity::Medium);
        duplicate.rule_id = "clone.duplicate-code".into();
        let mut scan = snapshot(vec![cycling, cognitive, duplicate], 3, 3);
        scan.run.config = json!({"maxCyclomatic": 9});
        scan.evidence.files = Some(vec![
            file("src/a.py", CoverageKind::Ast),
            file("src/b.py", CoverageKind::Ast),
            file("src/c.py", CoverageKind::Ast),
        ]);
        scan.edges = vec![
            edge("src/a.py", "src/b.py", "clone"),
            edge("src/a.py", "src/b.py", "import"),
            edge("src/a.py", "src/b.py", "call"),
            edge("src/a.py", "src/a.py", "call"),
            edge("src/b.py", "src/a.py", "import"),
        ];
        let score = calculate_score(&scan);
        let metrics = &score.dimensions["metrics"];
        assert_eq!(metrics["duplicateFileCount"], 2);
        assert_eq!(metrics["duplicateEligibleFileCount"], 3);
        assert!((metrics["duplicateFileRatio"].as_f64().unwrap() - 2.0 / 3.0).abs() < 0.00001);
        assert_eq!(metrics["highComplexityFileCount"], 1);
        assert_eq!(metrics["complexityThresholds"]["cyclomatic"], 9);
        assert_eq!(metrics["riskModuleCount"], 2);
        assert_eq!(metrics["dependencyEdgeCount"], 2);
        assert_eq!(metrics["severityCounts"]["medium"], 2);
        assert_eq!(score.dimensions["debt"]["minUnits"], 1);
        assert_eq!(score.dimensions["debt"]["maxUnits"], 3);
    }

    #[test]
    fn legacy_snapshot_never_infers_scan_wide_scope_from_index_counts() {
        let mut scan = snapshot(vec![issue("s", "security", Severity::High)], 2, 2);
        scan.edges.push(edge("src/a.py", "src/b.py", "clone"));
        let score = calculate_score(&scan);
        assert_eq!(score.dimensions["coverageBasis"], "confirmedFindings");
        assert!(score.dimensions["items"]["structure"]["score"].is_null());
        assert_eq!(score.dimensions["metrics"]["duplicateFileCount"], 2);
        assert!(score.dimensions["metrics"]["duplicateFileRatio"].is_null());
    }

    #[test]
    fn storage_loader_keeps_unreadable_files_in_scope_without_exporting_paths() {
        let mut store = Storage::in_memory().unwrap();
        store
            .insert_project(&ProjectRecord {
                id: "p".into(),
                name: "fixture".into(),
                normalized_path: "C:/private/project".into(),
                created_at: "1".into(),
            })
            .unwrap();
        let mut run = snapshot(Vec::new(), 2, 1).run;
        run.config = json!({"scanEvidenceVersion": 1});
        store.create_scan_run(&run).unwrap();
        let record = |path: &str, language: &str, coverage, exclusion: Option<&str>| FileRecord {
            id: path.into(),
            scan_run_id: run.id.clone(),
            project_id: run.project_id.clone(),
            relative_path: path.into(),
            absolute_path: format!("C:/private/project/{path}"),
            encoding: "utf-8".into(),
            size_bytes: 6,
            content_hash: String::new(),
            language: language.into(),
            parse_capability: coverage,
            excluded_reason: exclusion.map(str::to_owned),
        };
        for file in [
            record("a.py", "Python", CoverageKind::Ast, None),
            record(
                "unreadable.py",
                "Python",
                CoverageKind::NotCovered,
                Some("file too large"),
            ),
            record(
                "ignored",
                "-",
                CoverageKind::NotCovered,
                Some("directory excluded"),
            ),
        ] {
            store.upsert_file(&file).unwrap();
        }
        let snap = ScanSnapshot::load(&store, run).unwrap();
        assert_eq!(snap.file_count, 2);
        assert_eq!(snap.source_file_count, 1);
        assert_eq!(calculate_score(&snap).coverage, 0.5);
        assert!(!serde_json::to_string(&snap).unwrap().contains("C:/private"));
    }
}
