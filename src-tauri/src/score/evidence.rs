use crate::domain::types::{AnalyzerRun, AnalyzerStatus, CoverageKind, FileRecord, Issue};
use crate::rules::config::RuleConfig;
use crate::rules::runner::RuleRunner;
use serde::Serialize;
use std::collections::BTreeSet;

use super::{ScanSnapshot, ratio};

#[derive(Debug, Clone, Default)]
pub struct ScanEvidence {
    /// None for scans that predate persisted parser results.
    pub(crate) files: Option<Vec<FileEvidence>>,
    pub(crate) analyzers: Vec<AnalyzerRun>,
}

#[derive(Debug, Clone)]
pub(crate) struct FileEvidence {
    pub relative_path: String,
    pub language: String,
    pub coverage: CoverageKind,
}

impl FileEvidence {
    pub fn from_record(file: FileRecord) -> Option<Self> {
        // Traversal exclusions have the placeholder language. Discovered but
        // unreadable files still belong in the denominator.
        if file.excluded_reason.is_some() && file.language == "-" {
            return None;
        }
        Some(Self {
            relative_path: file.relative_path,
            language: file.language,
            coverage: if file.excluded_reason.is_some() {
                CoverageKind::NotCovered
            } else {
                file.parse_capability
            },
        })
    }
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FileCoverage {
    pub indexed_files: usize,
    pub ast_files: usize,
    pub structural_files: usize,
    pub lexical_files: usize,
    pub unparsed_files: usize,
}

impl FileCoverage {
    pub fn from_files(files: &[FileEvidence]) -> Self {
        let mut counts = Self {
            indexed_files: files.len(),
            ..Default::default()
        };
        for file in files {
            match file.coverage {
                CoverageKind::Ast => counts.ast_files += 1,
                CoverageKind::Structural => counts.structural_files += 1,
                CoverageKind::Lexical => counts.lexical_files += 1,
                CoverageKind::NotCovered => counts.unparsed_files += 1,
            }
        }
        counts
    }

    pub fn parsed_files(&self) -> usize {
        self.indexed_files - self.unparsed_files
    }
}

pub(crate) fn finding_is_usable(evidence: &ScanEvidence, issue: &Issue) -> bool {
    issue.coverage != CoverageKind::NotCovered
        && evidence
            .analyzers
            .iter()
            .find(|analyzer| analyzer.analyzer == issue.source)
            .is_none_or(|analyzer| analyzer.status == AnalyzerStatus::Executed)
}

pub(crate) fn matching_category(dimension: &str, category: &str) -> bool {
    match dimension {
        "readability" => matches!(category, "quality" | "comments"),
        "structure" => category == "structure",
        "maintainability" => matches!(
            category,
            "quality" | "comments" | "structure" | "complexity"
        ),
        "complexity" => category == "complexity",
        "duplicates" => category == "duplicate",
        "architecture" => category == "architecture",
        "security" => category == "security",
        _ => false,
    }
}

pub(crate) fn coverage_for_dimension(
    dimension: &str,
    snapshot: &ScanSnapshot,
    findings: &[&Issue],
    file_coverage: Option<&FileCoverage>,
    runner: &RuleRunner,
    config: &RuleConfig,
) -> f32 {
    let file_count = snapshot.file_count as usize;
    let mut paths = finding_paths(findings);
    match dimension {
        "architecture" => paths.extend(
            super::dimensions::dependency_edges(&snapshot.edges)
                .map(|edge| edge.source_path.as_str()),
        ),
        "duplicates" => paths.extend(
            snapshot
                .edges
                .iter()
                .filter(|edge| edge.kind == "clone")
                .flat_map(|edge| [edge.source_path.as_str(), edge.target_path.as_str()]),
        ),
        _ => {}
    }
    // Historic scans can confirm individual findings. They cannot establish
    // which additional files were evaluated successfully by each check.
    let observed = 0.25 * ratio(paths.len(), file_count);
    let (Some(files), Some(coverage)) = (&snapshot.evidence.files, file_coverage) else {
        return observed;
    };
    if file_count == 0 {
        return 0.0;
    }

    let scoped = match dimension {
        "duplicates" => 0.5 * ratio(coverage.parsed_files(), file_count),
        // Stored relations include resolved graph edges. Dynamic and unresolved
        // relationships can make the measured static architecture incomplete.
        "architecture" => {
            (0.5 * coverage.ast_files as f32 + 0.25 * coverage.structural_files as f32)
                / file_count as f32
        }
        _ if snapshot.run.rule_version == crate::rules::config::RULE_VERSION => {
            rule_coverage(dimension, files, runner, config) / file_count as f32
        }
        _ => 0.0,
    };
    scoped.max(observed).clamp(0.0, 1.0)
}

fn rule_coverage(
    dimension: &str,
    files: &[FileEvidence],
    runner: &RuleRunner,
    config: &RuleConfig,
) -> f32 {
    let checks: Vec<_> = runner
        .rules()
        .filter(|rule| matching_category(dimension, rule.category()))
        .collect();
    files
        .iter()
        .map(|file| {
            let coverage_factor = match file.coverage {
                CoverageKind::Ast => 1.0,
                CoverageKind::Structural => 0.5,
                CoverageKind::Lexical => 0.25,
                CoverageKind::NotCovered => return 0.0,
            };
            let mut applicable = 0;
            let mut checked = 0;
            for check in &checks {
                let supported = check.supports(&file.language, file.coverage);
                if supported || check.supports(&file.language, CoverageKind::Ast) {
                    applicable += 1;
                }
                if supported && config.is_rule_enabled(check.id()) {
                    checked += 1;
                }
            }
            // Security findings represent heuristic candidates even with AST support.
            let scope_factor = if dimension == "security" { 0.5 } else { 1.0 };
            scope_factor * coverage_factor * ratio(checked, applicable)
        })
        .sum()
}

pub(crate) fn finding_paths<'a>(findings: &[&'a Issue]) -> BTreeSet<&'a str> {
    findings
        .iter()
        .map(|issue| issue.file_path.as_str())
        .filter(|path| !path.is_empty() && *path != "project")
        .collect()
}
