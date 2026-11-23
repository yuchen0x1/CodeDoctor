import { describe, expect, it, vi, beforeEach } from "vitest";

const invoke = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (command: string, args?: Record<string, unknown>) =>
    invoke(command, args),
}));

describe("tauri bridge", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("calls the health command and returns its payload", async () => {
    invoke.mockResolvedValue("ok");
    const { health } = await import("./tauri");

    await expect(health()).resolves.toBe("ok");
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenCalledWith("health", undefined);
  });

  it("surfaces backend failures to the caller", async () => {
    invoke.mockRejectedValue(new Error("core unavailable"));
    const { health } = await import("./tauri");

    await expect(health()).rejects.toThrow("core unavailable");
  });

  it("routes report, prompt, export, and history requests with the backend contract", async () => {
    invoke.mockResolvedValue(undefined);
    const {
      calculateReport,
      compareScanHistory,
      exportReport,
      exportHistoryReport,
      generateGovernancePrompt,
      listScanRuns,
      exportStoredPrompt,
      exportIssueList,
    } = await import("./tauri");

    await calculateReport("run-1");
    await generateGovernancePrompt("run-1", "targeted", ["issue-1"]);
    await generateGovernancePrompt("run-1", "focused", [], "run-0");
    await exportReport("run-1", "markdown", "C:/reports/run-1.md");
    await listScanRuns("project-1");
    await compareScanHistory("run-0", "run-1");
    await exportHistoryReport("run-0", "run-1", "markdown", "C:/reports/history.md");
    await exportStoredPrompt("run-1", "prompt-1", "C:/reports/prompt.md");
    await exportIssueList("run-1", ["issue-1"], "C:/reports/issues.json");

    expect(invoke.mock.calls).toEqual([
      ["calculate_report", { runId: "run-1" }],
      ["generate_governance_prompt", { runId: "run-1", mode: "targeted", selectedIssueIds: ["issue-1"], baseRunId: null }],
      ["generate_governance_prompt", { runId: "run-1", mode: "focused", selectedIssueIds: [], baseRunId: "run-0" }],
      ["export_report", { runId: "run-1", format: "markdown", destination: "C:/reports/run-1.md" }],
      ["list_scan_runs", { projectId: "project-1" }],
      ["compare_scan_history", { baseRunId: "run-0", targetRunId: "run-1" }],
      ["export_history_report", { baseRunId: "run-0", targetRunId: "run-1", format: "markdown", destination: "C:/reports/history.md" }],
      ["export_governance_prompt", { runId: "run-1", promptId: "prompt-1", destination: "C:/reports/prompt.md" }],
      ["export_issue_list", { runId: "run-1", issueIds: ["issue-1"], destination: "C:/reports/issues.json" }],
    ]);
  });

  it("requests the selected project's persisted architecture graph", async () => {
    invoke.mockResolvedValue(undefined);
    const { getArchitectureGraph } = await import("./tauri");

    await getArchitectureGraph("run-2", "project-1");

    expect(invoke).toHaveBeenCalledWith("get_architecture_graph", {
      runId: "run-2",
      projectId: "project-1",
    });
  });

  it("loads the selected scan's analyzer outcomes", async () => {
    const outcomes = [{
      id: "ruff",
      analyzer: "ruff",
      status: "unavailable",
      version: null,
      durationMs: null,
      message: "Unavailable",
    }];
    invoke.mockResolvedValue(outcomes);
    const { listAnalyzerRuns } = await import("./tauri");

    await expect(listAnalyzerRuns("run-3")).resolves.toEqual(outcomes);
    expect(invoke).toHaveBeenCalledWith("list_analyzer_runs", { runId: "run-3" });
  });

  it("lists previously imported projects for workspace restoration", async () => {
    invoke.mockResolvedValue([]);
    const { listProjects, restoreProject } = await import("./tauri");

    await listProjects();
    await restoreProject("project-1");

    expect(invoke.mock.calls.slice(-2)).toEqual([
      ["list_projects", undefined],
      ["restore_project", { projectId: "project-1" }],
    ]);
  });
});
