use crate::domain::types::{ArchitectureEdge, ScanRun, ScanStatus};
use crate::error::AppError;
use crate::graph::{Cycle, dependency_topology};
#[cfg(feature = "desktop")]
use crate::state::AppState;
use serde::Serialize;
use std::collections::BTreeSet;
#[cfg(feature = "desktop")]
use tauri::State;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureNode {
    pub path: String,
    pub fan_in: u32,
    pub fan_out: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureCoverage {
    pub status: &'static str,
    pub edge_count: usize,
    pub completeness_known: bool,
    pub unresolved_imports_available: bool,
    pub isolated_files_available: bool,
    pub note: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureGraphView {
    pub project_id: String,
    pub run_id: String,
    pub nodes: Vec<ArchitectureNode>,
    pub edges: Vec<ArchitectureEdge>,
    pub cycles: Vec<Cycle>,
    pub coverage: ArchitectureCoverage,
}

fn validate_run(run: &ScanRun, expected_project_id: &str) -> Result<(), AppError> {
    if run.project_id != expected_project_id {
        return Err(AppError::Validation(
            "scan run does not belong to the selected project".into(),
        ));
    }
    if run.status != ScanStatus::Completed {
        return Err(AppError::Validation(
            "architecture graph requires a completed scan run".into(),
        ));
    }
    Ok(())
}

fn build_graph_view(
    project_id: &str,
    run_id: &str,
    source_paths: impl Iterator<Item = String>,
    edges: Vec<ArchitectureEdge>,
) -> ArchitectureGraphView {
    let mut paths: BTreeSet<String> = source_paths.map(|path| path.replace('\\', "/")).collect();
    paths.extend(
        edges
            .iter()
            .flat_map(|edge| [edge.source_path.clone(), edge.target_path.clone()]),
    );
    let (degrees, cycles) = dependency_topology(
        &paths.iter().map(String::as_str).collect::<Vec<_>>(),
        edges.iter().map(|edge| {
            (
                edge.source_path.as_str(),
                edge.target_path.as_str(),
                edge.kind.as_str(),
            )
        }),
    );
    let nodes = paths
        .into_iter()
        .zip(degrees)
        .map(|(path, (fan_in, fan_out))| ArchitectureNode {
            path,
            fan_in,
            fan_out,
        })
        .collect();
    ArchitectureGraphView {
        project_id: project_id.to_owned(),
        run_id: run_id.to_owned(),
        coverage: ArchitectureCoverage {
            status: "partial",
            edge_count: edges.len(),
            completeness_known: false,
            unresolved_imports_available: false,
            isolated_files_available: true,
            note: "快照保留可解析的静态导入、部分显式导入绑定支持的调用、继承和 JSX 引用，以及重复代码证据。动态目标、模板、部分包或别名解析和未解析导入明细覆盖有限。",
        },
        nodes,
        edges,
        cycles,
    }
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn get_architecture_graph(
    state: State<AppState>,
    run_id: String,
    project_id: String,
) -> Result<ArchitectureGraphView, AppError> {
    let store = state.storage()?;
    let run = store
        .get_scan_run(&run_id)?
        .ok_or_else(|| AppError::Validation("scan run does not exist".into()))?;
    validate_run(&run, &project_id)?;
    let edges = store.list_edges(&run_id)?;
    let paths = store
        .list_files_for_run(&run_id)?
        .into_iter()
        .filter(|file| file.excluded_reason.is_none())
        .map(|file| file.relative_path);
    Ok(build_graph_view(&project_id, &run_id, paths, edges))
}

#[cfg(test)]
mod tests {
    use super::{build_graph_view, validate_run};
    use crate::domain::types::{ArchitectureEdge, CoverageKind, ScanRun, ScanStatus};
    use serde_json::json;

    fn edge(source: &str, target: &str, kind: &str) -> ArchitectureEdge {
        ArchitectureEdge {
            id: format!("{source}->{target}:{kind}"),
            source_path: source.to_owned(),
            target_path: target.to_owned(),
            kind: kind.to_owned(),
            confidence: 0.9,
            coverage: CoverageKind::Structural,
            metadata: json!({"dynamic": false}),
        }
    }

    fn scan(status: ScanStatus, project_id: &str) -> ScanRun {
        ScanRun {
            id: "run-1".to_owned(),
            project_id: project_id.to_owned(),
            status,
            started_at: Some("2026-11-18T00:00:00Z".to_owned()),
            completed_at: None,
            config: json!({}),
            rule_version: "rules-1".to_owned(),
            analyzer_versions: json!({}),
            total_files: 3,
            scanned_files: 3,
            total_issues: 0,
        }
    }

    #[test]
    fn graph_view_keeps_direction_counts_cycles_and_partial_coverage() {
        let graph = build_graph_view(
            "project-1",
            "run-1",
            ["src/isolated.rs".to_owned()].into_iter(),
            vec![
                edge("src/a.rs", "src/b.rs", "import"),
                edge("src/b.rs", "src/a.rs", "import"),
                edge("src/b.rs", "src/c.rs", "import"),
                edge("src/b.rs", "src/c.rs", "call"),
                edge("src/c.rs", "src/b.rs", "clone"),
            ],
        );

        assert_eq!(graph.nodes.len(), 4);
        let b = graph
            .nodes
            .iter()
            .find(|node| node.path == "src/b.rs")
            .unwrap();
        assert_eq!(b.fan_in, 1);
        assert_eq!(b.fan_out, 2);
        assert_eq!(graph.edges[0].source_path, "src/a.rs");
        assert_eq!(graph.edges[0].target_path, "src/b.rs");
        assert_eq!(graph.cycles.len(), 1);
        assert_eq!(graph.cycles[0].paths, vec!["src/a.rs", "src/b.rs"]);
        assert_eq!(graph.coverage.status, "partial");
        assert_eq!(graph.coverage.edge_count, 5);
        assert!(!graph.coverage.unresolved_imports_available);
        assert!(graph.coverage.isolated_files_available);
    }

    #[test]
    fn graph_view_handles_a_scan_without_edges_without_claiming_full_coverage() {
        let graph = build_graph_view("project-1", "run-empty", std::iter::empty(), Vec::new());

        assert!(graph.nodes.is_empty());
        assert!(graph.edges.is_empty());
        assert!(graph.cycles.is_empty());
        assert_eq!(graph.coverage.status, "partial");
        assert_eq!(graph.coverage.edge_count, 0);
    }

    #[test]
    fn graph_view_rejects_other_projects_and_incomplete_runs() {
        assert!(validate_run(&scan(ScanStatus::Completed, "project-1"), "project-1").is_ok());
        assert!(validate_run(&scan(ScanStatus::Completed, "project-1"), "project-2").is_err());
        assert!(validate_run(&scan(ScanStatus::Running, "project-1"), "project-1").is_err());
    }
}
