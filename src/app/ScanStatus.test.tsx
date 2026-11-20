// @vitest-environment jsdom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import type { ScanEvent, ScanRunSummary } from "../lib/contracts";
import * as bridge from "../lib/tauri";
import { pickProjectDirectory } from "../lib/nativeDialogs";
import App from "./App";

vi.mock("@tauri-apps/api/core", () => ({ isTauri: () => true }));
vi.mock("@tauri-apps/api/webview", () => ({ getCurrentWebview: () => ({ onDragDropEvent: async () => () => {} }) }));
vi.mock("../lib/tauri", async (importOriginal) => ({
  ...await importOriginal<typeof import("../lib/tauri")>(),
  health: vi.fn(), listProjects: vi.fn(), importProject: vi.fn(), listScanRuns: vi.fn(),
  startScan: vi.fn(), getScan: vi.fn(), listenToScanProgress: vi.fn(), pauseScan: vi.fn(), listAnalyzerRuns: vi.fn(),
}));
vi.mock("../lib/nativeDialogs", () => ({ pickProjectDirectory: vi.fn() }));
vi.mock("./Updates", () => ({ default: () => null }));

it("announces phase and status changes without mutating the live region for file updates", async () => {
  const summary: ScanRunSummary = { run: {
    id: "run", projectId: "project", status: "running", startedAt: "1", completedAt: null, config: {},
    ruleVersion: "test", analyzerVersions: {}, totalFiles: 5, scannedFiles: 0, totalIssues: 0,
  }, elapsedMs: 0 };
  let current = summary;
  let receive!: (event: ScanEvent) => void;
  vi.mocked(bridge.health).mockResolvedValue("ok");
  vi.mocked(bridge.listProjects).mockResolvedValue([]);
  vi.mocked(pickProjectDirectory).mockResolvedValue("C:/project");
  vi.mocked(bridge.importProject).mockResolvedValue({
    projectId: "project", displayName: "Project", canonicalPath: "C:/project", sourceAvailable: true,
    languages: [], frameworks: [], fileCount: 5, sourceFileCount: 5, lineCount: 10, maxDirectoryDepth: 1,
  });
  vi.mocked(bridge.listScanRuns).mockResolvedValue([]);
  vi.mocked(bridge.listAnalyzerRuns).mockResolvedValue([]);
  vi.mocked(bridge.startScan).mockResolvedValue(summary);
  vi.mocked(bridge.getScan).mockImplementation(async () => current);
  vi.mocked(bridge.pauseScan).mockImplementation(async () => { current = { ...current, run: { ...current.run, status: "paused" } }; });
  vi.mocked(bridge.listenToScanProgress).mockImplementation(async (handler) => { receive = handler; return () => {}; });
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const container = document.createElement("div");
  document.body.append(container);
  const root = createRoot(container);
  const click = (label: string) => act(async () => {
    [...container.querySelectorAll("button")].find((button) => (button.getAttribute("aria-label") ?? button.textContent) === label)!.click();
  });
  const emit = (overrides: Partial<ScanEvent>) => act(async () => receive({
    runId: "run", status: "running", stage: "parse", currentPath: "src/a.ts", message: "progress details",
    processedFiles: 1, totalFiles: 5, issueCount: 0, elapsedMs: 100, ...overrides,
  }));

  try {
    await act(async () => root.render(<App />));
    const status = container.querySelector('[role="status"]')!;
    expect(status.textContent).toBe("");
    await click("项目体检");
    await click("选择项目文件夹");
    await click("开始全面扫描");
    expect(status.textContent).toBe("扫描状态：扫描中");
    await emit({});
    expect(status.textContent).toBe("扫描状态：扫描中 · AST 解析");
    const mutations = vi.fn();
    const observer = new MutationObserver(mutations);
    observer.observe(status, { subtree: true, childList: true, characterData: true });
    try {
      await emit({ currentPath: "src/b.ts", processedFiles: 2, issueCount: 1, elapsedMs: 200, message: "updated details" });
      expect(mutations).not.toHaveBeenCalled();
      expect(container.querySelector(".current-file")?.textContent).toBe("src/b.ts");
      expect(container.querySelector('[role="progressbar"]')?.getAttribute("aria-valuenow")).toBe("40");
    } finally {
      observer.disconnect();
    }
    await emit({ stage: "rules" });
    expect(status.textContent).toBe("扫描状态：扫描中 · 质量规则");
    await click("暂停扫描");
    expect(status.textContent).toBe("扫描状态：已暂停");
    current = { ...current, run: { ...current.run, status: "canceled" } };
    await emit({ stage: "done", status: "canceled", currentPath: null });
    expect(status.textContent).toBe("扫描状态：已取消");
  } finally {
    await act(async () => root.unmount());
    container.remove();
    vi.unstubAllGlobals();
  }
});
