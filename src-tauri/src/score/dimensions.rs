use super::evidence::{FileCoverage, finding_paths};
use super::{ScanSnapshot, ratio};
use crate::domain::types::{ArchitectureEdge, CoverageKind, Issue, Severity};
use crate::graph::{
    HIGH_FAN_IN_THRESHOLD, HIGH_FAN_OUT_THRESHOLD, dependency_topology, is_dependency,
};
use crate::rules::config::{RULE_VERSION, RuleConfig};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeSet;

const CYCLOMATIC_RULE: &str = "complexity.high-cyclomatic";
const COGNITIVE_RULE: &str = "complexity.high-cognitive";

#[derive(Debug, Default, Serialize)]
struct SeverityCounts {
    critical: usize,
    high: usize,
    medium: usize,
    low: usize,
    info: usize,
}

fn severity_counts<'a>(issues: impl IntoIterator<Item = &'a Issue>) -> SeverityCounts {
    let mut counts = SeverityCounts::default();
    for issue in issues {
        match issue.severity {
            Severity::Critical => counts.critical += 1,
            Severity::High => counts.high += 1,
            Severity::Medium => counts.medium += 1,
            Severity::Low => counts.low += 1,
            Severity::Info => counts.info += 1,
        }
    }
    counts
}

pub(super) fn debt_range(findings: &[&Issue]) -> Option<Value> {
    let actionable: Vec<_> = findings
        .iter()
        .copied()
        .filter(|issue| issue.severity != Severity::Info)
        .collect();
    if actionable.is_empty() {
        return None;
    }
    let locations: BTreeSet<_> = actionable
        .iter()
        .map(|issue| {
            let project_finding = (issue.file_path == "project").then_some(&issue.id);
            (
                &issue.file_path,
                issue.start_line,
                issue.end_line,
                project_finding,
            )
        })
        .collect();
    Some(json!({
        "minUnits": locations.len(),
        "maxUnits": actionable.len(),
        "factors": severity_counts(actionable),
        "basis": "findingLocationsToIndividualFindings",
    }))
}

pub(super) fn scan_metrics(
    snapshot: &ScanSnapshot,
    findings: &[&Issue],
    coverage: Option<&FileCoverage>,
    config: &RuleConfig,
) -> Value {
    let duplicates: Vec<_> = findings
        .iter()
        .copied()
        .filter(|issue| issue.rule_id == "clone.duplicate-code")
        .collect();
    let clone_edges = snapshot.edges.iter().filter(|edge| edge.kind == "clone");
    let mut duplicate_paths = finding_paths(&duplicates);
    duplicate_paths.extend(
        clone_edges.flat_map(|edge| [edge.source_path.as_str(), edge.target_path.as_str()]),
    );
    let parsed_files = coverage
        .map(FileCoverage::parsed_files)
        .filter(|count| *count > 0);
    let duplicate_file_count =
        (!duplicate_paths.is_empty() || parsed_files.is_some()).then_some(duplicate_paths.len());
    let duplicate_file_ratio = parsed_files.and_then(|count| {
        let parsed_paths: BTreeSet<_> = snapshot
            .evidence
            .files
            .as_ref()?
            .iter()
            .filter(|file| file.coverage != CoverageKind::NotCovered)
            .map(|file| file.relative_path.as_str())
            .collect();
        duplicate_paths
            .is_subset(&parsed_paths)
            .then(|| ratio(duplicate_paths.len(), count))
    });

    let cyclomatic = (snapshot.run.rule_version == RULE_VERSION
        && config.is_rule_enabled(CYCLOMATIC_RULE))
    .then_some(config.max_cyclomatic);
    let cognitive = (snapshot.run.rule_version == RULE_VERSION
        && config.is_rule_enabled(COGNITIVE_RULE))
    .then_some(config.max_cognitive);
    let has_structural_scope =
        coverage.is_some_and(|counts| counts.ast_files + counts.structural_files > 0);
    let high_complexity_findings: Vec<_> = findings
        .iter()
        .copied()
        .filter(|issue| matches!(issue.rule_id.as_str(), CYCLOMATIC_RULE | COGNITIVE_RULE))
        .collect();
    let high_complexity_files = finding_paths(&high_complexity_findings).len();
    let complexity_covered = has_structural_scope && (cyclomatic.is_some() || cognitive.is_some());

    let (dependency_count, risk_modules) = architecture_metrics(&snapshot.edges);
    let graph_covered = has_structural_scope || dependency_count > 0;
    json!({
        "severityCounts": severity_counts(&snapshot.issues),
        "fileCoverage": coverage,
        "duplicateGroupCount": (!duplicates.is_empty() || parsed_files.is_some()).then_some(duplicates.len()),
        "duplicateFileCount": duplicate_file_count,
        "duplicateEligibleFileCount": parsed_files,
        "duplicateFileRatio": duplicate_file_ratio,
        "highComplexityFileCount": (high_complexity_files > 0 || complexity_covered).then_some(high_complexity_files),
        "complexityThresholds": { "cyclomatic": cyclomatic, "cognitive": cognitive },
        "riskModuleCount": graph_covered.then_some(risk_modules),
        "dependencyEdgeCount": graph_covered.then_some(dependency_count),
    })
}

pub(super) fn dependency_edges(
    edges: &[ArchitectureEdge],
) -> impl Iterator<Item = &ArchitectureEdge> + Clone {
    edges.iter().filter(|edge| {
        is_dependency(&edge.kind)
            && edge.coverage != CoverageKind::NotCovered
            && edge.source_path != edge.target_path
    })
}

fn architecture_metrics(edges: &[ArchitectureEdge]) -> (usize, usize) {
    let dependencies = dependency_edges(edges);
    let paths: BTreeSet<_> = dependencies
        .clone()
        .flat_map(|edge| [edge.source_path.as_str(), edge.target_path.as_str()])
        .collect();
    let paths: Vec<_> = paths.into_iter().collect();
    let (degrees, cycles) = dependency_topology(
        &paths,
        dependencies.map(|edge| {
            (
                edge.source_path.as_str(),
                edge.target_path.as_str(),
                edge.kind.as_str(),
            )
        }),
    );
    let mut risk_nodes: BTreeSet<_> = cycles
        .iter()
        .flat_map(|cycle| cycle.paths.iter().map(String::as_str))
        .collect();
    let mut dependency_count = 0;
    for (path, (input, output)) in paths.into_iter().zip(degrees) {
        dependency_count += output as usize;
        if input > HIGH_FAN_IN_THRESHOLD || output > HIGH_FAN_OUT_THRESHOLD {
            risk_nodes.insert(path);
        }
    }
    (dependency_count, risk_nodes.len())
}
