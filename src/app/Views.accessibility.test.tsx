// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { ArchitectureGraph, Issue } from "../lib/contracts";
import { DEFAULT_SETTINGS } from "../lib/settings";
import { getArchitectureGraph } from "../lib/tauri";
import ArchitectureView from "./ArchitectureView";
import IssuesView from "./IssuesView";
import SettingsView from "./SettingsView";

vi.mock("../lib/tauri", () => ({ getArchitectureGraph: vi.fn() }));

let container: HTMLDivElement;
let root: Root;
beforeEach(() => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
});
afterEach(async () => {
  await act(async () => root.unmount());
  container.remove();
  vi.unstubAllGlobals();
});

it("exposes graph buttons as a group and keeps keyboard and list selection in sync", async () => {
  const graph: ArchitectureGraph = {
    projectId: "project", runId: "run",
    nodes: [{ path: "src/a.ts", fanIn: 0, fanOut: 1 }, { path: "src/b.ts", fanIn: 1, fanOut: 0 }],
    edges: [], cycles: [],
    coverage: { status: "partial", edgeCount: 0, completenessKnown: false, unresolvedImportsAvailable: false, isolatedFilesAvailable: true, note: "test" },
  };
  vi.mocked(getArchitectureGraph).mockResolvedValue(graph);
  await act(async () => root.render(<ArchitectureView projectId="project" runId="run" />));
  expect(container.querySelector('[role="group"][aria-label="架构关系筛选"]')).not.toBeNull();
  const canvas = container.querySelector('svg[role="group"][aria-label="项目文件依赖关系图"]')!;
  const nodes = [...canvas.querySelectorAll<SVGGElement>('g[role="button"]')];
  expect(canvas.getAttribute("viewBox")).toBe("0 0 940 600");
  expect(Number(nodes[0].querySelector("text")?.getAttribute("y"))).toBeLessThan(Number(nodes[0].querySelector("circle")?.getAttribute("cy")));
  expect(Number(nodes[1].querySelector("text")?.getAttribute("y"))).toBeGreaterThan(Number(nodes[1].querySelector("circle")?.getAttribute("cy")));
  const rows = [...container.querySelectorAll<HTMLButtonElement>(".architecture-view__node-row")];
  const pressed = (items: Element[]) => items.map((item) => item.getAttribute("aria-pressed"));
  expect(pressed(nodes)).toEqual(["true", "false"]);
  expect(pressed(rows)).toEqual(["true", "false"]);
  expect(nodes[1].getAttribute("tabindex")).toBe("0");
  const space = new KeyboardEvent("keydown", { key: " ", bubbles: true, cancelable: true });
  await act(async () => { nodes[1].dispatchEvent(space); });
  expect(space.defaultPrevented).toBe(true);
  expect(pressed(nodes)).toEqual(["false", "true"]);
  expect(pressed(rows)).toEqual(["false", "true"]);
  expect(container.querySelector(".architecture-view__path")?.textContent).toBe("src/b.ts");
  await act(async () => { nodes[0].dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true })); });
  expect(pressed(rows)).toEqual(["true", "false"]);
  await act(async () => rows[1].click());
  expect(pressed(nodes)).toEqual(["false", "true"]);
});

const issue = (index: number): Issue => ({
  id: `test/${index}`, title: `Issue ${index}`, ruleId: "rule", category: "quality", severity: "high", filePath: "main.ts",
  startLine: index + 1, endLine: index + 1, startColumn: 1, endColumn: 2,
  explanation: "explanation", impact: "impact", recommendation: "recommendation", evidence: "evidence",
  source: "builtin", coverage: "ast", confidence: 1, fingerprint: `fingerprint-${index}`,
});

it("links disclosures to stable panels across pagination and exposes source for keyboard scrolling", async () => {
  const loadCodeContext = vi.fn().mockResolvedValue({
    text: "const test = 1;", startLine: 1, endLine: 1, stale: false, redacted: false, truncated: false, canOpenNatively: false,
  });
  await act(async () => root.render(<IssuesView issues={Array.from({ length: 51 }, (_, index) => issue(index))}
    projectRoot="C:/project" loadCodeContext={loadCodeContext} />));
  expect(container.querySelector('[role="group"][aria-label="问题筛选"]')).not.toBeNull();
  const buttons = () => [...container.querySelectorAll<HTMLButtonElement>("button[aria-controls]")];
  const ids = () => buttons().map((button) => button.getAttribute("aria-controls")!);
  const firstPageIds = ids();
  expect(new Set(firstPageIds).size).toBe(50);
  const [first] = buttons();
  const panel = document.getElementById(firstPageIds[0])!;
  expect(panel.hidden).toBe(true);
  expect(first.getAttribute("aria-expanded")).toBe("false");
  expect(loadCodeContext).not.toHaveBeenCalled();
  await act(async () => first.click());
  expect(first.getAttribute("aria-expanded")).toBe("true");
  expect(panel.hidden).toBe(false);
  const source = panel.querySelector<HTMLPreElement>('pre[role="group"][aria-label="源码片段"]')!;
  expect(source.tabIndex).toBe(0);
  source.focus();
  expect(document.activeElement).toBe(source);
  await act(async () => first.click());
  expect(panel.hidden).toBe(true);
  await act(async () => first.click());
  expect(loadCodeContext).toHaveBeenCalledTimes(1);
  const status = container.querySelector('[role="status"]')!;
  expect(status.textContent).toBe("显示第 1–50 项，共 51 项");
  const page = (text: string) => [...container.querySelectorAll("button")].find((button) => button.textContent === text)!;
  await act(async () => page("下一页").click());
  expect(status.textContent).toBe("显示第 51–51 项，共 51 项");
  expect(firstPageIds).not.toContain(ids()[0]);
  await act(async () => page("上一页").click());
  expect(ids()).toEqual(firstPageIds);
});

it("describes invalid directories, focuses them on each failed save, and applies theme only on save", async () => {
  const onSave = vi.fn();
  await act(async () => root.render(<SettingsView settings={DEFAULT_SETTINGS} onSave={onSave} />));
  const theme = container.querySelector("select")!;
  const directory = container.querySelector<HTMLInputElement>('input:not([type])')!;
  const save = container.querySelector<HTMLButtonElement>('button[type="submit"]')!;
  const describe = (node: Element) => node.getAttribute("aria-describedby")!.split(" ").map((id) => document.getElementById(id));
  const enterDirectory = (value: string) => act(async () => {
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(directory, value);
    directory.dispatchEvent(new Event("input", { bubbles: true }));
  });
  expect(describe(theme)[0]?.textContent).toContain("保存设置”后应用主题");
  expect(describe(directory)[0]?.textContent).toContain("使用逗号分隔");
  await act(async () => {
    theme.value = "light";
    theme.dispatchEvent(new Event("change", { bubbles: true }));
  });
  expect(onSave).not.toHaveBeenCalled();
  await enterDirectory("folder/nested");
  for (let attempt = 0; attempt < 2; attempt++) {
    save.focus();
    await act(async () => save.click());
    expect(onSave).not.toHaveBeenCalled();
    expect(document.activeElement).toBe(directory);
    expect(directory.getAttribute("aria-invalid")).toBe("true");
    expect(describe(directory)[1]).toBe(container.querySelector('[role="alert"]'));
  }
  await enterDirectory("generated, vendor");
  expect(directory.getAttribute("aria-invalid")).toBe("false");
  expect(describe(directory)).toHaveLength(1);
  await act(async () => save.click());
  expect(onSave).toHaveBeenCalledWith({ ...DEFAULT_SETTINGS, theme: "light", excludedDirectories: ["generated", "vendor"] });
});

it("keeps a valid page visible when a refreshed findings list becomes shorter", async () => {
  const render = (count: number) => act(async () => root.render(<IssuesView
    issues={Array.from({ length: count }, (_, index) => issue(index))} projectRoot="C:/project" />));
  await render(101);
  const next = () => [...container.querySelectorAll("button")].find((button) => button.textContent === "下一页")!;
  await act(async () => next().click());
  await act(async () => next().click());
  expect(container.querySelector('[role="status"]')?.textContent).toBe("显示第 101–101 项，共 101 项");
  await render(51);
  expect(container.querySelectorAll("button[aria-controls]")).toHaveLength(1);
  expect(container.querySelector('[role="status"]')?.textContent).toBe("显示第 51–51 项，共 51 项");
  expect(next().disabled).toBe(true);
  await render(1);
  expect(container.querySelectorAll("button[aria-controls]")).toHaveLength(1);
  expect(container.textContent).toContain("Issue 0");
});

it("retries transient context failures without leaving the issue", async () => {
  const loadCodeContext = vi.fn().mockRejectedValueOnce(new Error("temporary read failure")).mockResolvedValue({
    text: "const test = 1;", startLine: 1, endLine: 1, stale: false, redacted: false, truncated: false, canOpenNatively: false,
  });
  await act(async () => root.render(<IssuesView issues={[issue(0)]} projectRoot="C:/project" loadCodeContext={loadCodeContext} />));
  await act(async () => container.querySelector<HTMLButtonElement>('button[aria-controls]')!.click());
  expect(container.querySelector('[role="alert"]')?.textContent).toContain("temporary read failure");
  const retry = [...container.querySelectorAll("button")].find((button) => button.textContent === "重试读取上下文")!;
  await act(async () => retry.click());
  expect(loadCodeContext).toHaveBeenCalledTimes(2);
  expect(container.querySelector('[aria-label="源码片段"]')?.textContent).toContain("const test = 1;");
  expect(container.querySelector('[role="alert"]')).toBeNull();
});

it("keeps the retry control mounted, focused and single-flight while the retry runs", async () => {
  let resolveRetry!: (value: unknown) => void;
  const pending = new Promise((resolve) => { resolveRetry = resolve; });
  const loadCodeContext = vi.fn()
    .mockRejectedValueOnce(new Error("temporary read failure"))
    .mockReturnValue(pending);
  await act(async () => root.render(<IssuesView issues={[issue(0)]} projectRoot="C:/project" loadCodeContext={loadCodeContext} />));
  await act(async () => container.querySelector<HTMLButtonElement>('button[aria-controls]')!.click());
  const retry = () => [...container.querySelectorAll<HTMLButtonElement>("button")]
    .find((button) => button.textContent === "重试读取上下文" || button.textContent === "正在重试…")!;
  retry().focus();
  await act(async () => retry().click());
  expect(retry().textContent).toBe("正在重试…");
  expect(retry().getAttribute("aria-disabled")).toBe("true");
  expect(document.activeElement).toBe(retry());
  await act(async () => retry().click());
  expect(loadCodeContext).toHaveBeenCalledTimes(2);
  await act(async () => resolveRetry({
    text: "const test = 1;", startLine: 1, endLine: 1, stale: false, redacted: false, truncated: false, canOpenNatively: false,
  }));
  expect(container.querySelector('[aria-label="源码片段"]')?.textContent).toContain("const test = 1;");
  expect(document.activeElement?.textContent).toBe("代码上下文");
});
