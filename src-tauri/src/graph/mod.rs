//! Static file dependencies, strongly connected components, and coupling.
pub mod imports;
pub mod metrics;
pub mod scc;

#[cfg(test)]
mod fixture_tests;

use crate::domain::types::CoverageKind;
use crate::parse::ParsedFile;
use crate::rules::IssueDraft;
use imports::{GraphEdge, collect_edges};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub const HIGH_FAN_IN_THRESHOLD: u32 = 12;
pub const HIGH_FAN_OUT_THRESHOLD: u32 = 12;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    pub path: String,
    pub language: String,
    pub fan_in: u32,
    pub fan_out: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cycle {
    pub id: String,
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphSnapshot {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub cycles: Vec<Cycle>,
    pub unresolved_imports: Vec<String>,
    pub incomplete_coverage: bool,
}

pub struct GraphIssue {
    pub draft: IssueDraft,
    pub path: String,
    pub coverage: CoverageKind,
}

impl GraphSnapshot {
    pub fn cycles(&self) -> &[Cycle] {
        &self.cycles
    }

    /// Architecture issues derived from the graph (cycles, coupling).
    pub fn issues(&self) -> Vec<GraphIssue> {
        metrics::snapshot_issues(self)
    }

    fn cycle_id(paths: &[String]) -> String {
        let mut hash: u64 = 0xcbf29ce484222325;
        for path in paths {
            for b in path.bytes().chain([0]) {
                hash ^= u64::from(b);
                hash = hash.wrapping_mul(0x100000001b3);
            }
        }
        format!("cycle-{hash:016x}")
    }
}

pub fn is_dependency(kind: &str) -> bool {
    matches!(kind, "import" | "inheritance" | "component" | "call")
}

/// One directed connection per pair of distinct files, regardless of how
/// many kinds of dependency evidence connect the pair.
pub fn dependency_topology<'a>(
    paths: &[&str],
    edges: impl Iterator<Item = (&'a str, &'a str, &'a str)>,
) -> (Vec<(u32, u32)>, Vec<Cycle>) {
    let indexes: HashMap<&str, usize> = paths
        .iter()
        .enumerate()
        .map(|(index, &path)| (path, index))
        .collect();
    let mut degrees = vec![(0, 0); paths.len()];
    let mut pairs = HashSet::new();
    for (source, target, kind) in edges {
        if is_dependency(kind)
            && let (Some(&from), Some(&to)) = (indexes.get(source), indexes.get(target))
            && from != to
            && pairs.insert((from, to))
        {
            degrees[from].1 += 1;
            degrees[to].0 += 1;
        }
    }

    let index_edges: Vec<_> = pairs.into_iter().collect();
    let mut cycles: Vec<Cycle> = scc::strongly_connected_cycles(paths.len(), &index_edges)
        .into_iter()
        .map(|members| {
            let mut paths: Vec<String> = members.iter().map(|&i| paths[i].to_owned()).collect();
            paths.sort_unstable();
            let id = GraphSnapshot::cycle_id(&paths);
            Cycle { id, paths }
        })
        .collect();
    cycles.sort_unstable_by(|a, b| a.paths.cmp(&b.paths));
    (degrees, cycles)
}

/// Builds the graph snapshot for one scan's parsed files.
pub fn build(parsed_files: &[ParsedFile]) -> GraphSnapshot {
    let collection = collect_edges(parsed_files);
    let paths: Vec<String> = parsed_files
        .iter()
        .map(|f| f.file.relative_path.replace('\\', "/"))
        .collect();
    let (degrees, cycles) = dependency_topology(
        &paths.iter().map(String::as_str).collect::<Vec<_>>(),
        collection.edges.iter().map(|edge| {
            (
                edge.source.as_str(),
                edge.target.as_str(),
                edge.kind.as_str(),
            )
        }),
    );
    let nodes = parsed_files
        .iter()
        .zip(paths)
        .zip(degrees)
        .map(|((f, path), (fan_in, fan_out))| GraphNode {
            path,
            language: f.file.language.clone(),
            fan_in,
            fan_out,
        })
        .collect();

    let incomplete_coverage = !collection.unresolved.is_empty()
        || parsed_files
            .iter()
            .any(|f| f.parse.coverage != CoverageKind::Ast);

    GraphSnapshot {
        nodes,
        edges: collection.edges,
        cycles,
        unresolved_imports: collection.unresolved,
        incomplete_coverage,
    }
}
