// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { HistoryDiff, Issue, ProjectRecord, ProjectSummary, ReportResult, ScanRun } from "../lib/contracts";
import * as bridge from "../lib/tauri";
import { pickProjectDirectory, pickReportDestination } from "../lib/nativeDialogs";
import App from "./App";

vi.mock("@tauri-apps/api/core", () => ({ isTauri: () => false }));
vi.mock("../lib/tauri", () => ({
  health: vi.fn(), listProjects: vi.fn(), restoreProject: vi.fn(), importProject: vi.fn(),
  listScanRuns: vi.fn(), getScan: vi.fn(), listIssues: vi.fn(), calculateReport: vi.fn(),
  listAnalyzerRuns: vi.fn(), compareScanHistory: vi.fn(), exportReport: vi.fn(),
  cancelScan: vi.fn(), exportIssueList: vi.fn(), exportStoredPrompt: vi.fn(),
  generateGovernancePrompt: vi.fn(), listenToScanProgress: vi.fn(), pauseScan: vi.fn(),
  resumeScan: vi.fn(), startScan: vi.fn(),
}));
vi.mock("../lib/nativeDialogs", () => ({
  pickProjectDirectory: vi.fn(), pickProjectArchive: vi.fn(), pickIssueListDestination: vi.fn(),
  pickPromptDestination: vi.fn(), pickReportDestination: vi.fn(),
}));
vi.mock("./Updates", () => ({ default: () => null }));
vi.mock("./IssuesView", () => ({
  default: ({ issues }: { issues: Issue[] }) => <section data-testid="findings">{issues.length ? issues.map((issue) => <p key={issue.id}>{issue.title}</p>) : "没有发现问题"}</section>,
}));

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

const project = (id: string): ProjectSummary => ({
  projectId: id, canonicalPath: `C:/projects/${id}`, displayName: `Project ${id}`,
  sourceAvailable: true, languages: ["Python"], frameworks: [], fileCount: 1,
  sourceFileCount: 1, lineCount: 10, maxDirectoryDepth: 1,
});
const record = (id: string): ProjectRecord => ({ id, name: `Project ${id}`, normalizedPath: `C:/projects/${id}`, createdAt: "1" });
const scan = (id: string): ScanRun => ({
  id, projectId: "A", status: "completed", startedAt: "1", completedAt: "2", config: {},
  ruleVersion: "test", analyzerVersions: {}, totalFiles: 1, scannedFiles: 1, totalIssues: 1,
});
const scans = [scan("scan-3"), scan("scan-2"), scan("scan-1")];
const issue: Issue = {
  id: "issue-1", ruleId: "test-rule", category: "quality", severity: "high", filePath: "main.py",
  startLine: 1, startColumn: 1, endLine: 1, endColumn: 3, title: "Persisted finding",
  explanation: "test", evidence: "test", impact: "test", recommendation: "test", source: "builtin",
  coverage: "ast", confidence: 1, fingerprint: "test-fingerprint",
};
const report: ReportResult = {
  score: {
    id: "score-1", overallScore: 80, dimensions: {}, coverage: 1, confidence: 1,
    ruleVersion: "test", analyzerVersions: {}, config: {}, createdAt: "1",
  },
  content: "{}",
};
const diff: HistoryDiff = {
  baseRunId: "scan-2", targetRunId: "scan-3", issueChanges: [], scoreDelta: 12,
  ruleVersionChanged: false, coverageDelta: 0,
};
let container: HTMLDivElement;
let root: Root;

beforeEach(() => {
  vi.resetAllMocks();
  vi.stubGlobal("localStorage", { getItem: () => null, setItem: vi.fn() });
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  vi.mocked(bridge.health).mockResolvedValue("ok");
  vi.mocked(bridge.listProjects).mockResolvedValue([record("A")]);
  vi.mocked(bridge.restoreProject).mockResolvedValue(project("A"));
  vi.mocked(bridge.importProject).mockResolvedValue(project("B"));
  vi.mocked(bridge.listScanRuns).mockResolvedValue(scans);
  vi.mocked(bridge.getScan).mockImplementation(async (id) => ({ run: scans.find((run) => run.id === id)!, elapsedMs: 0 }));
  vi.mocked(bridge.listIssues).mockResolvedValue({ issues: [issue], total: 1 });
  vi.mocked(bridge.calculateReport).mockResolvedValue(report);
  vi.mocked(bridge.listAnalyzerRuns).mockResolvedValue([]);
  vi.mocked(bridge.compareScanHistory).mockResolvedValue(diff);
  vi.mocked(bridge.exportReport).mockResolvedValue();
  vi.mocked(pickProjectDirectory).mockResolvedValue("C:/projects/B");
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
});
afterEach(async () => {
  await act(async () => root.unmount());
  container.remove();
  vi.unstubAllGlobals();
});
const mount = () => act(async () => { root.render(<App />); });
const button = (text: string) => {
  const result = [...container.querySelectorAll("button")].find((item) => item.textContent === text || item.getAttribute("aria-label") === text);
  if (!result) throw new Error(`Button not found: ${text}`);
  return result;
};
const click = (text: string) => act(async () => { button(text).click(); });
const select = (label: string) => container.querySelector<HTMLSelectElement>(`select[aria-label="${label}"]`)!;
const choose = (label: string, value: string) => act(async () => {
  select(label).value = value;
  select(label).dispatchEvent(new Event("change", { bubbles: true }));
});
const reportDestination = () => container.querySelector<HTMLInputElement>('input[aria-label="报告保存路径"]')!;
const enterReportDestination = (value: string) => act(async () => {
  Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(reportDestination(), value);
  reportDestination().dispatchEvent(new Event("input", { bubbles: true }));
});

describe("project restoration", () => {
  it.each(["before", "after"])("does not restore the startup project when its list arrives %s explicit import completion", async (order) => {
    const startup = deferred<ProjectRecord[]>();
    const imported = deferred<ProjectSummary>();
    vi.mocked(bridge.listProjects).mockReturnValueOnce(startup.promise).mockResolvedValue([record("B")]);
    vi.mocked(bridge.importProject).mockReturnValue(imported.promise);
    await mount();
    await click("项目体检");
    await click("选择项目文件夹");
    expect(bridge.importProject).toHaveBeenCalledWith("C:/projects/B");
    if (order === "before") await act(async () => startup.resolve([record("A")]));
    await act(async () => imported.resolve(project("B")));
    if (order === "after") await act(async () => startup.resolve([record("A")]));
    expect(bridge.restoreProject).not.toHaveBeenCalled();
    expect(container.querySelector(".topbar__subtitle")?.textContent).toContain("Project B");
  });

  it("keeps saved projects selectable if a picker is canceled before startup loading finishes", async () => {
    const startup = deferred<ProjectRecord[]>();
    vi.mocked(bridge.listProjects).mockReturnValue(startup.promise);
    vi.mocked(pickProjectDirectory).mockResolvedValue(null);
    await mount();
    await click("项目体检");
    await click("选择项目文件夹");
    await act(async () => startup.resolve([record("A")]));
    await click("总览仪表盘");
    expect(container.querySelector(".saved-project-select")?.textContent).toContain("Project A");
    expect(bridge.restoreProject).not.toHaveBeenCalled();
    expect(bridge.importProject).not.toHaveBeenCalled();
  });

  it.each(["history", "scan"])("opens the restored project even when its %s cannot load", async (step) => {
    if (step === "history") vi.mocked(bridge.listScanRuns).mockRejectedValue({ Database: "history read failed" });
    else vi.mocked(bridge.getScan).mockRejectedValue({ Database: "scan read failed" });
    await mount();
    expect(container.querySelector(".topbar__subtitle")?.textContent).toContain("Project A");
    expect(container.textContent).toContain(`${step} read failed`);
    await click("项目体检");
    expect(button("开始全面扫描").disabled).toBe(false);
  });

  it("does not commit restoration after unmounting", async () => {
    const restored = deferred<ProjectSummary>();
    vi.mocked(bridge.restoreProject).mockReturnValue(restored.promise);
    await mount();
    await act(async () => root.unmount());
    root = createRoot(container);
    await act(async () => restored.resolve(project("A")));
    expect(bridge.listScanRuns).not.toHaveBeenCalled();
  });
});

describe("saved scan failures", () => {
  const failedScan = (failureMessage?: string): ScanRun => ({
    ...scan("failed-scan"), status: "failed", startedAt: null,
    config: { maxComplexity: 12, enabledAnalyzers: ["ruff"] }, failureMessage,
  });
  const openSaved = async (saved: ScanRun) => {
    vi.mocked(bridge.listScanRuns).mockResolvedValue([saved, ...scans]);
    vi.mocked(bridge.getScan).mockImplementation(async (id) => ({ run: id === saved.id ? saved : scans.find((item) => item.id === id)!, elapsedMs: 0 }));
    await mount();
    await click(saved.id);
  };

  it("shows a persisted failure without a live event and retries with the original configuration", async () => {
    const saved = failedScan("扫描失败：project path does not exist <private>");
    await openSaved(saved);
    expect(container.querySelector('[role="alert"]')?.textContent).toBe(saved.failureMessage);
    expect(container.querySelector("private")).toBeNull();
    expect(button("按原设置重新扫描").disabled).toBe(false);
    vi.mocked(bridge.startScan).mockResolvedValue({ run: scans[0], elapsedMs: 0 });
    await click("按原设置重新扫描");
    expect(bridge.startScan).toHaveBeenCalledWith("A", saved.config, ["ruff"]);
    expect(container.querySelector('[role="alert"]')).toBeNull();
  });

  it("explains when an older failed scan has no recorded diagnostic", async () => {
    await openSaved(failedScan());
    expect(container.querySelector('[role="alert"]')?.textContent).toContain("此任务未记录详细原因");
  });

  it("shows the recorded interruption reason when reopening a recovered task", async () => {
    const saved = failedScan("扫描被应用退出中断，请按原设置重新扫描。");
    saved.config.interrupted = true;
    await openSaved(saved);
    expect(container.querySelector('[role="alert"]')?.textContent).toBe(saved.failureMessage);
    expect(button("按原设置重新扫描").disabled).toBe(false);
  });

  it.each(["completed", "canceled"] as const)("does not present a stale failure diagnostic for a %s task", async (status) => {
    await openSaved({ ...failedScan("outdated diagnostic"), status });
    expect(container.querySelector('[role="alert"]')).toBeNull();
    expect(container.textContent).not.toContain("outdated diagnostic");
  });
});

describe("history selections", () => {
  it("keeps chosen snapshots when delayed score loading refreshes history and exports the chosen snapshot", async () => {
    const score = deferred<ReportResult>();
    vi.mocked(bridge.calculateReport).mockReturnValue(score.promise);
    await mount();
    await click("历史报告");
    await choose("基线扫描", "scan-1");
    await choose("目标扫描", "scan-2");
    await choose("报告快照", "scan-1");
    const refreshCount = vi.mocked(bridge.listScanRuns).mock.calls.length;
    await act(async () => score.resolve(report));
    expect(bridge.listScanRuns).toHaveBeenCalledTimes(refreshCount + 1);
    expect(select("基线扫描").value).toBe("scan-1");
    expect(select("目标扫描").value).toBe("scan-2");
    expect(select("报告快照").value).toBe("scan-1");
    await act(async () => {
      const input = container.querySelector<HTMLInputElement>('input[aria-label="报告保存路径"]')!;
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(input, "C:/report.md");
      input.dispatchEvent(new Event("input", { bubbles: true }));
    });
    await click("导出报告");
    expect(bridge.exportReport).toHaveBeenCalledWith("scan-1", "markdown", "C:/report.md");
  });

  it("hides a delayed diff if background refresh invalidates its selected pair", async () => {
    const score = deferred<ReportResult>();
    const comparison = deferred<HistoryDiff>();
    vi.mocked(bridge.calculateReport).mockReturnValue(score.promise);
    vi.mocked(bridge.compareScanHistory).mockReturnValue(comparison.promise);
    await mount();
    await click("历史报告");
    await click("比较快照");
    vi.mocked(bridge.listScanRuns).mockResolvedValue(scans.slice(1));
    await act(async () => score.resolve(report));
    await act(async () => comparison.resolve(diff));
    expect(select("基线扫描").value).toBe("scan-2");
    expect(select("目标扫描").value).toBe("scan-1");
    expect(button("比较快照").disabled).toBe(false);
    expect(container.querySelector(".history-summary")).toBeNull();
    expect(container.textContent).not.toContain("质量分变化");
  });
});

describe("report save dialog", () => {
  it("locks report controls and synchronously prevents duplicate dialogs and exports", async () => {
    const picker = deferred<string | null>();
    vi.mocked(pickReportDestination).mockReturnValue(picker.promise);
    await mount();
    await click("历史报告");
    await choose("报告快照", "scan-1");
    await enterReportDestination("C:/existing.md");
    await act(async () => {
      button("选择保存位置").click();
      button("选择保存位置").click();
      select("报告格式").value = "sarif";
      select("报告格式").dispatchEvent(new Event("change", { bubbles: true }));
      select("报告快照").value = "scan-2";
      select("报告快照").dispatchEvent(new Event("change", { bubbles: true }));
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(reportDestination(), "C:/changed.sarif");
      reportDestination().dispatchEvent(new Event("input", { bubbles: true }));
      button("导出报告").click();
    });
    expect(pickReportDestination).toHaveBeenCalledTimes(1);
    expect(pickReportDestination).toHaveBeenCalledWith("markdown");
    expect(bridge.exportReport).not.toHaveBeenCalled();
    expect(select("报告格式").disabled).toBe(true);
    expect(select("报告格式").value).toBe("markdown");
    expect(select("报告快照").disabled).toBe(true);
    expect(select("报告快照").value).toBe("scan-1");
    expect(reportDestination().disabled).toBe(true);
    expect(reportDestination().value).toBe("C:/existing.md");
    expect(button("选择保存位置").disabled).toBe(true);
    expect(button("导出报告").disabled).toBe(true);
    expect(select("基线扫描").disabled).toBe(false);
    expect(select("目标扫描").disabled).toBe(false);
    expect(button("比较快照").disabled).toBe(false);
    await act(async () => picker.resolve("C:/chosen.md"));
    expect(select("报告格式").disabled).toBe(false);
    expect(select("报告快照").disabled).toBe(false);
    expect(reportDestination().disabled).toBe(false);
    expect(reportDestination().value).toBe("C:/chosen.md");
    expect(button("选择保存位置").disabled).toBe(false);
    expect(button("导出报告").disabled).toBe(false);
    await click("导出报告");
    expect(bridge.exportReport).toHaveBeenCalledWith("scan-1", "markdown", "C:/chosen.md");
  });

  it.each(["cancel", "failure"])("restores report controls after dialog %s and allows retry", async (result) => {
    const picker = deferred<string | null>();
    vi.mocked(pickReportDestination).mockReturnValueOnce(picker.promise).mockResolvedValue("C:/retry.sarif");
    await mount();
    await click("历史报告");
    await choose("报告快照", "scan-1");
    await enterReportDestination("C:/existing.md");
    await click("选择保存位置");
    expect(select("报告格式").disabled).toBe(true);
    if (result === "cancel") await act(async () => picker.resolve(null));
    else await act(async () => picker.reject(new Error("report picker failed")));
    expect(select("报告格式").disabled).toBe(false);
    expect(select("报告快照").disabled).toBe(false);
    expect(select("报告快照").value).toBe("scan-1");
    expect(reportDestination().disabled).toBe(false);
    expect(reportDestination().value).toBe("C:/existing.md");
    expect(button("选择保存位置").disabled).toBe(false);
    expect(button("导出报告").disabled).toBe(false);
    expect(container.querySelector('[role="alert"]')?.textContent ?? "").toBe(result === "failure" ? "report picker failed关闭" : "");
    expect(bridge.exportReport).not.toHaveBeenCalled();
    await choose("报告格式", "sarif");
    expect(reportDestination().value).toBe("");
    await choose("报告快照", "scan-2");
    await enterReportDestination("C:/manual.sarif");
    expect(reportDestination().value).toBe("C:/manual.sarif");
    await click("选择保存位置");
    expect(pickReportDestination).toHaveBeenCalledTimes(2);
    expect(pickReportDestination).toHaveBeenNthCalledWith(2, "sarif");
    expect(reportDestination().value).toBe("C:/retry.sarif");
    await click("导出报告");
    expect(bridge.exportReport).toHaveBeenCalledWith("scan-2", "sarif", "C:/retry.sarif");
  });

  it.each(["dialog", "comparison"])("keeps dialog pending separate from comparison busy when %s finishes first", async (first) => {
    const picker = deferred<string | null>();
    const comparison = deferred<HistoryDiff>();
    vi.mocked(pickReportDestination).mockReturnValue(picker.promise);
    vi.mocked(bridge.compareScanHistory).mockReturnValue(comparison.promise);
    await mount();
    await click("历史报告");
    await click("选择保存位置");
    expect(button("比较快照").disabled).toBe(false);
    await click("比较快照");
    expect(bridge.compareScanHistory).toHaveBeenCalledTimes(1);
    if (first === "dialog") await act(async () => picker.resolve(null));
    else await act(async () => comparison.resolve(diff));
    expect(button("比较快照").disabled).toBe(first === "dialog");
    expect(select("报告格式").disabled).toBe(true);
    expect(button("选择保存位置").disabled).toBe(true);
    if (first === "dialog") await act(async () => comparison.resolve(diff));
    else await act(async () => picker.resolve(null));
    expect(button("比较快照").disabled).toBe(false);
    expect(select("报告格式").disabled).toBe(false);
    expect(button("选择保存位置").disabled).toBe(false);
  });

  it.each([
    ["path", "before"], ["error", "before"], ["path", "after"], ["error", "after"],
  ])("ignores an old dialog %s returned %s project restoration completes", async (result, order) => {
    const picker = deferred<string | null>();
    const restored = deferred<ProjectSummary>();
    const nextScans = scans.map((run) => ({ ...run, id: `B-${run.id}`, projectId: "B" }));
    vi.mocked(bridge.listProjects).mockResolvedValue([record("A"), record("B")]);
    vi.mocked(bridge.restoreProject).mockImplementation((id) => id === "B" ? restored.promise : Promise.resolve(project("A")));
    vi.mocked(bridge.listScanRuns).mockImplementation(async (id) => id === "B" ? nextScans : scans);
    vi.mocked(bridge.getScan).mockImplementation(async (id) => ({ run: [...scans, ...nextScans].find((run) => run.id === id)!, elapsedMs: 0 }));
    vi.mocked(pickReportDestination).mockReturnValueOnce(picker.promise).mockResolvedValue("C:/B-report.md");
    await mount();
    await click("历史报告");
    await enterReportDestination("C:/A-report.md");
    await click("选择保存位置");
    await click("总览仪表盘");
    const projects = container.querySelector<HTMLSelectElement>(".saved-project-select select")!;
    expect(projects.disabled).toBe(false);
    await act(async () => {
      projects.value = "B";
      projects.dispatchEvent(new Event("change", { bubbles: true }));
    });
    expect(bridge.restoreProject).toHaveBeenLastCalledWith("B");
    if (order === "after") await act(async () => restored.resolve(project("B")));
    await click("历史报告");
    expect(select("报告格式").disabled).toBe(true);
    expect(button("选择保存位置").disabled).toBe(true);
    await click("选择保存位置");
    expect(pickReportDestination).toHaveBeenCalledTimes(1);
    if (result === "path") await act(async () => picker.resolve("C:/stale-A.md"));
    else await act(async () => picker.reject(new Error("stale A picker error")));
    expect(reportDestination().value).toBe(order === "before" ? "C:/A-report.md" : "");
    expect(container.querySelector('[role="alert"]')).toBeNull();
    expect(bridge.exportReport).not.toHaveBeenCalled();
    if (order === "before") {
      expect(button("比较快照").disabled).toBe(true);
      expect(select("报告格式").disabled).toBe(true);
      await act(async () => restored.resolve(project("B")));
      await click("历史报告");
    }
    expect(container.querySelector(".topbar__subtitle")?.textContent).toContain("Project B");
    expect(select("报告快照").value).toBe("B-scan-3");
    expect(reportDestination().value).toBe("");
    expect(select("报告格式").disabled).toBe(false);
    expect(button("选择保存位置").disabled).toBe(false);
    await click("选择保存位置");
    expect(pickReportDestination).toHaveBeenCalledTimes(2);
    expect(reportDestination().value).toBe("C:/B-report.md");
    await click("导出报告");
    expect(bridge.exportReport).toHaveBeenCalledWith("B-scan-3", "markdown", "C:/B-report.md");
    expect(container.textContent).not.toContain("stale A picker error");
    expect(container.textContent).not.toContain("C:/stale-A.md");
  });
});

describe("independent result loading", () => {
  it("keeps persisted findings visible when score saving fails, and retries without reopening the run", async () => {
    vi.mocked(bridge.calculateReport).mockRejectedValueOnce({ Database: "score write failed" }).mockResolvedValue(report);
    await mount();
    await click("问题分析");
    expect(container.querySelector('[data-testid="findings"]')?.textContent).toContain(issue.title);
    expect(container.textContent).toContain("评分加载失败：score write failed");
    await click("重新加载结果");
    expect(bridge.calculateReport).toHaveBeenCalledTimes(2);
    expect(container.textContent).not.toContain("评分加载失败");
    expect(container.textContent).toContain(issue.title);
  });

  it("does not claim zero findings when loading fails, and allows retry", async () => {
    vi.mocked(bridge.listIssues).mockRejectedValueOnce({ Database: "read failed" }).mockResolvedValue({ issues: [issue], total: 1 });
    await mount();
    await click("问题分析");
    expect(container.textContent).toContain("问题清单加载失败：read failed");
    expect(container.textContent).not.toContain("没有发现问题");
    await click("重新加载结果");
    expect(container.textContent).toContain(issue.title);
    expect(container.textContent).not.toContain("问题清单加载失败");
  });

  it("displays findings before a pending score finishes", async () => {
    const score = deferred<ReportResult>();
    vi.mocked(bridge.calculateReport).mockReturnValue(score.promise);
    await mount();
    await click("问题分析");
    expect(container.textContent).toContain(issue.title);
    expect(container.textContent).not.toContain("没有发现问题");
    await act(async () => score.resolve(report));
  });

  it("keeps the dashboard free of zero counts when the findings read failed", async () => {
    vi.mocked(bridge.listIssues).mockRejectedValue({ Database: "read failed" });
    await mount();
    const cards = () => [...container.querySelectorAll(".stat-card")];
    const severityCard = () => cards().find((card) => card.textContent?.includes("当前严重 / 高危"))!;
    expect(severityCard().querySelector("strong")?.textContent).toBe("—");
    expect(severityCard().querySelector("small")?.textContent).toBe("问题清单暂不可用。");
    expect(container.querySelector(".bars")).toBeNull();
    expect(container.textContent).not.toContain("0 / 0");
  });

  it("keeps the dashboard counts once findings load even if the score failed", async () => {
    vi.mocked(bridge.calculateReport).mockRejectedValue({ Database: "score write failed" });
    await mount();
    const severityCard = () => [...container.querySelectorAll(".stat-card")]
      .find((card) => card.textContent?.includes("当前严重 / 高危"))!;
    expect(severityCard().querySelector("strong")?.textContent).toBe("0 / 1");
    expect(severityCard().querySelector("small")?.textContent).toBe("当前扫描快照");
    expect(container.querySelectorAll(".bars .bar-row")).toHaveLength(5);
  });

  it("moves focus to the page heading when the retry control is removed", async () => {
    vi.mocked(bridge.listIssues).mockRejectedValueOnce({ Database: "read failed" }).mockResolvedValue({ issues: [issue], total: 1 });
    await mount();
    const retry = button("重新加载结果");
    retry.focus();
    expect(document.activeElement).toBe(retry);
    await act(async () => { retry.click(); });
    expect(document.activeElement).toBe(container.querySelector("h1"));
    expect(container.textContent).not.toContain("问题清单加载失败");
  });
});
