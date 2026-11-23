import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AnalyzerRun,
  HistoryDiff,
  HistoryReportFormat,
  ArchitectureGraph,
  IndexSummary,
  IssuePage,
  PromptArtifact,
  PromptMode,
  ProjectSummary,
  ProjectRecord,
  ReportFormat,
  ReportResult,
  ScanEvent,
  ScanRun,
  ScanRunSummary,
} from "./contracts";

/**
 * Typed bridge to the Rust core.
 *
 * Every backend command gets exactly one wrapper here so components never call
 * `invoke` with a raw string, and so command names and payload shapes stay in
 * one place as the contract grows.
 */

/** Returns `"ok"` when the Rust core is reachable and responding. */
export function health(): Promise<string> {
  return invoke<string>("health");
}

export function importProject(path: string): Promise<ProjectSummary> {
  return invoke<ProjectSummary>("import_project", { path });
}

export function listProjects(): Promise<ProjectRecord[]> {
  return invoke<ProjectRecord[]>("list_projects");
}

export function restoreProject(projectId: string): Promise<ProjectSummary> {
  return invoke<ProjectSummary>("restore_project", { projectId });
}

export function indexProject(projectId: string): Promise<IndexSummary> {
  return invoke<IndexSummary>("index_project", { projectId });
}

export function startScan(projectId: string, config?: Record<string, unknown>, analyzers?: string[]): Promise<ScanRunSummary> {
  return invoke<ScanRunSummary>("start_scan", { projectId, config: config ?? null, analyzers: analyzers ?? [] });
}

export function pauseScan(runId: string): Promise<void> {
  return invoke<void>("pause_scan", { runId });
}

export function resumeScan(runId: string): Promise<void> {
  return invoke<void>("resume_scan", { runId });
}

export function cancelScan(runId: string): Promise<void> {
  return invoke<void>("cancel_scan", { runId });
}

export function getScan(runId: string): Promise<ScanRunSummary> {
  return invoke<ScanRunSummary>("get_scan", { runId });
}

export function listAnalyzerRuns(runId: string): Promise<AnalyzerRun[]> {
  return invoke<AnalyzerRun[]>("list_analyzer_runs", { runId });
}

export function listIssues(runId: string, filters: { severity?: string; category?: string; filePath?: string; search?: string; limit?: number; offset?: number } = {}): Promise<IssuePage> {
  return invoke<IssuePage>("list_issues", { runId, ...filters });
}

export function listenToScanProgress(handler: (event: ScanEvent) => void): Promise<UnlistenFn> {
  return listen<ScanEvent>("scan://progress", (event) => handler(event.payload));
}

export function calculateReport(runId: string): Promise<ReportResult> {
  return invoke<ReportResult>("calculate_report", { runId });
}

export function generateGovernancePrompt(
  runId: string,
  mode: PromptMode,
  selectedIssueIds: string[] = [],
  baseRunId?: string,
): Promise<PromptArtifact> {
  return invoke<PromptArtifact>("generate_governance_prompt", {
    runId,
    mode,
    selectedIssueIds,
    baseRunId: baseRunId ?? null,
  });
}

export function exportReport(runId: string, format: ReportFormat, destination: string): Promise<void> {
  return invoke<void>("export_report", { runId, format, destination });
}

export function exportStoredPrompt(runId: string, promptId: string, destination: string): Promise<void> {
  return invoke<void>("export_governance_prompt", { runId, promptId, destination });
}

export function exportIssueList(runId: string, issueIds: string[], destination: string): Promise<void> {
  return invoke<void>("export_issue_list", { runId, issueIds, destination });
}

export function listScanRuns(projectId: string): Promise<ScanRun[]> {
  return invoke<ScanRun[]>("list_scan_runs", { projectId });
}

export function compareScanHistory(baseRunId: string, targetRunId: string): Promise<HistoryDiff> {
  return invoke<HistoryDiff>("compare_scan_history", { baseRunId, targetRunId });
}

export function exportHistoryReport(
  baseRunId: string,
  targetRunId: string,
  format: HistoryReportFormat,
  destination: string,
): Promise<void> {
  return invoke<void>("export_history_report", { baseRunId, targetRunId, format, destination });
}

export function getArchitectureGraph(runId: string, projectId: string): Promise<ArchitectureGraph> {
  return invoke<ArchitectureGraph>("get_architecture_graph", { runId, projectId });
}
