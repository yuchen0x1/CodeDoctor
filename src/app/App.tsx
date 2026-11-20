import { useEffect, useRef, useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import ArchitectureView from "./ArchitectureView";
import IssuesView from "./IssuesView";
import SettingsView from "./SettingsView";
import Updates from "./Updates";
import type {
  HistoryDiff,
  AnalyzerRun,
  Issue,
  PromptArtifact,
  PromptMode,
  ProjectSummary,
  ProjectRecord,
  ReportFormat,
  ScanEvent,
  ScanRun,
  ScanRunSummary,
  ScoreSnapshot,
} from "../lib/contracts";
import {
  calculateReport,
  cancelScan,
  compareScanHistory,
  exportIssueList,
  exportReport,
  exportStoredPrompt,
  generateGovernancePrompt,
  getScan,
  health,
  importProject,
  listIssues,
  listAnalyzerRuns,
  listProjects,
  listScanRuns,
  listenToScanProgress,
  pauseScan,
  resumeScan,
  startScan,
  restoreProject,
} from "../lib/tauri";
import { pickIssueListDestination, pickProjectArchive, pickProjectDirectory, pickPromptDestination, pickReportDestination } from "../lib/nativeDialogs";
import { readSettings, writeSettings, type AppSettings } from "../lib/settings";
import { openIssueSource, readIssueSource } from "../lib/sourceContext";

type PageId = "overview" | "inspection" | "scans" | "issues" | "architecture" | "comments" | "duplicates" | "prompts" | "history" | "settings";
const nav: Array<{ id: PageId; label: string; icon: string }> = [
  { id: "overview", label: "总览仪表盘", icon: "◈" },
  { id: "inspection", label: "项目体检", icon: "⌁" },
  { id: "scans", label: "扫描任务", icon: "◌" },
  { id: "issues", label: "问题分析", icon: "!" },
  { id: "architecture", label: "架构分析", icon: "⌘" },
  { id: "comments", label: "注释治理", icon: "//" },
  { id: "duplicates", label: "重复代码", icon: "≋" },
  { id: "prompts", label: "修复提示词", icon: "✦" },
  { id: "history", label: "历史报告", icon: "◷" },
  { id: "settings", label: "设置", icon: "⚙" },
];
const stageNames: Record<string, string> = { index: "文件索引", parse: "AST 解析", rules: "质量规则", clones: "重复分析", graph: "依赖分析", external: "外部分析器", finalize: "结果汇总", done: "已完成" };
const promptLabels: Record<PromptMode, string> = { comprehensive: "全面治理", focused: "重点治理", targeted: "定点修复" };
const number = (value: number) => new Intl.NumberFormat("zh-CN").format(value);
const safeError = (reason: unknown) => {
  if (reason instanceof Error) return reason.message;
  if (reason && typeof reason === "object") {
    return Object.values(reason).find((value) => typeof value === "string") ?? "本地操作失败";
  }
  return String(reason);
};
type FindingsState = "idle" | "loading" | "error" | "ready";
const findingsStatus = {
  idle: "扫描完成后显示",
  loading: "正在加载问题清单…",
  error: "问题清单暂不可用。",
};
const emptyIssues: Issue[] = [];
const scanActive = (status?: string) => status === "running" || status === "queued" || status === "paused" || status === "canceling";
const scanStatusLabels: Record<ScanRun["status"], string> = {
  queued: "已排队", running: "扫描中", paused: "已暂停", canceling: "正在取消", canceled: "已取消", completed: "已完成", failed: "失败",
};
const completed = (run: ScanRun) => run.status === "completed" && run.ruleVersion !== "index-1";
const displayPath = (path: string) => path.startsWith("\\\\?\\UNC\\")
  ? "\\\\" + path.slice(8)
  : path.startsWith("\\\\?\\") ? path.slice(4) : path;
const scanLabel = (run: ScanRun) => {
  const timestamp = Number(run.startedAt);
  if (!Number.isFinite(timestamp) || timestamp <= 0) return run.id;
  const date = new Date(timestamp < 100_000_000_000 ? timestamp * 1000 : timestamp);
  return `${date.toLocaleString("zh-CN")} · ${run.id.slice(-10)}`;
};

function Stat({ label, value, hint }: { label: string; value: string | number; hint: string }) {
  return <article className="stat-card"><span>{label}</span><strong>{value}</strong><small>{hint}</small></article>;
}
function Empty({ title, text }: { title: string; text: string }) {
  return <div className="empty"><b>⌁</b><h3>{title}</h3><p>{text}</p></div>;
}
async function allIssuesForRun(runId: string): Promise<Issue[]> {
  const limit = 500;
  const first = await listIssues(runId, { limit, offset: 0 });
  const issues = [...first.issues];
  for (let offset = limit; offset < first.total; offset += limit) {
    const next = await listIssues(runId, { limit, offset });
    issues.push(...next.issues);
  }
  return issues;
}

export default function App() {
  const [page, setPage] = useState<PageId>("overview");
  const [project, setProject] = useState<ProjectSummary | null>(null);
  const [savedProjects, setSavedProjects] = useState<ProjectRecord[]>([]);
  const [settings, setSettings] = useState(readSettings);
  const [run, setRun] = useState<ScanRunSummary | null>(null);
  const [event, setEvent] = useState<ScanEvent | null>(null);
  const [loadedIssues, setIssues] = useState<Issue[] | null>(null);
  const [score, setScore] = useState<ScoreSnapshot | null>(null);
  const [issuesLoading, setIssuesLoading] = useState(false);
  const [scoreLoading, setScoreLoading] = useState(false);
  const [issuesError, setIssuesError] = useState("");
  const [scoreError, setScoreError] = useState("");
  const [issuesLoadVersion, setIssuesLoadVersion] = useState(0);
  const [scoreLoadVersion, setScoreLoadVersion] = useState(0);
  const [analyzerRuns, setAnalyzerRuns] = useState<AnalyzerRun[]>([]);
  const [prompt, setPrompt] = useState<PromptArtifact | null>(null);
  const [promptMode, setPromptMode] = useState<PromptMode>("comprehensive");
  const [promptBaseRunId, setPromptBaseRunId] = useState("");
  const [selectedIssueIds, setSelectedIssueIds] = useState<string[]>([]);
  const [historyRuns, setHistoryRuns] = useState<ScanRun[]>([]);
  const [historyDiff, setHistoryDiff] = useState<HistoryDiff | null>(null);
  const [historyIssueDetails, setHistoryIssueDetails] = useState<Record<string, Issue>>({});
  const [historySelection, setHistorySelection] = useState({ baseRunId: "", targetRunId: "", reportRunId: "" });
  const { baseRunId, targetRunId, reportRunId } = historySelection;
  const setBaseRunId = (value: string) => setHistorySelection((selected) => ({ ...selected, baseRunId: value }));
  const setTargetRunId = (value: string) => setHistorySelection((selected) => ({ ...selected, targetRunId: value }));
  const setReportRunId = (value: string) => setHistorySelection((selected) => ({ ...selected, reportRunId: value }));
  const [path, setPath] = useState("");
  const [reportFormat, setReportFormat] = useState<ReportFormat>("markdown");
  const [reportDestination, setReportDestination] = useState("");
  const [reportPathPending, setReportPathPending] = useState(false);
  const reportPathPendingRef = useRef(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [busy, setBusy] = useState(false);
  const [core, setCore] = useState("checking");
  const activeRun = useRef("");
  const activeProject = useRef("");
  const projectRequest = useRef(0);
  const explicitProjectChoice = useRef(false);
  const historyRequest = useRef(0);
  const pageHeading = useRef<HTMLHeadingElement>(null);
  const projectId = project?.projectId;
  const runId = run?.run.id;
  const runStatus = run?.run.status;
  const taskRunning = scanActive(runStatus);
  const issues = loadedIssues ?? emptyIssues;
  const findingsState: FindingsState = runStatus !== "completed" ? "idle" : loadedIssues !== null ? "ready"
    : issuesLoading || !issuesError ? "loading" : "error";
  const canRetryIssues = Boolean(issuesError) && !issuesLoading;
  const canRetryScore = Boolean(scoreError) && !scoreLoading;
  useEffect(() => { health().then((value) => setCore(value === "ok" ? "ok" : "error")).catch(() => setCore("error")); }, []);
  useEffect(() => { document.documentElement.dataset.theme = settings.theme; }, [settings.theme]);

  const showRun = (next: ScanRunSummary | null) => {
    setRun(next);
    // Reopening the current scan leaves its result-loading effects unchanged.
    if (next && activeRun.current === next.run.id) return;
    activeRun.current = next?.run.id ?? "";
    setEvent(null);
    setIssues(null);
    setScore(null);
    setIssuesError(""); setScoreError("");
    setIssuesLoading(false); setScoreLoading(false);
    setAnalyzerRuns([]);
    setPrompt(null);
    setPromptBaseRunId("");
    setSelectedIssueIds([]);
  };

  const showProject = (next: ProjectSummary) => {
    setProject(next);
    setPath(next.canonicalPath);
    if (next.projectId === activeProject.current) return;
    activeProject.current = next.projectId;
    historyRequest.current += 1;
    showRun(null);
    setHistoryRuns([]);
    setHistoryDiff(null);
    setHistoryIssueDetails({});
    setBaseRunId(""); setTargetRunId(""); setReportRunId(""); setReportDestination("");
  };

  const restoreSavedProject = async (projectId: string, automatic = false) => {
    if (!automatic) explicitProjectChoice.current = true;
    const request = ++projectRequest.current;
    const current = () => projectRequest.current === request;
    setBusy(true); setError(""); setNotice("");
    try {
      const nextProject = await restoreProject(projectId);
      if (!current()) return;
      showProject(nextProject);
      const runs = await listScanRuns(projectId);
      if (!current()) return;
      setHistoryRuns(runs);
      const latestCompleted = runs.find(completed);
      const nextRun = latestCompleted ? await getScan(latestCompleted.id) : null;
      if (!current()) return;
      showRun(nextRun);
      setPage("overview");
    } catch (reason) { if (current()) setError(safeError(reason)); }
    finally { if (current()) setBusy(false); }
  };

  useEffect(() => {
    let active = true;
    void listProjects().then(async (items) => {
      if (!active) return;
      if (!explicitProjectChoice.current || !activeProject.current) setSavedProjects(items);
      if (!explicitProjectChoice.current && items.length) await restoreSavedProject(items[0].id, true);
    }).catch((reason) => { if (active && !explicitProjectChoice.current) setError(safeError(reason)); });
    return () => { active = false; projectRequest.current += 1; };
  }, []);

  useEffect(() => {
    if (!isTauri()) return;
    let live = true;
    let cleanup: (() => void) | undefined;
    void listenToScanProgress((next) => {
      if (!live || activeRun.current !== next.runId) return;
      setEvent(next);
      if (next.status === "completed" || next.status === "failed" || next.status === "canceled") {
        void getScan(next.runId).then((latest) => {
          if (live && activeRun.current === next.runId) setRun(latest);
        }).catch((reason) => { if (live) setError(safeError(reason)); });
      }
    }).then((fn) => { if (live) cleanup = fn; else fn(); })
      .catch((reason) => { if (live) setError(safeError(reason)); });
    return () => { live = false; cleanup?.(); };
  }, []);

  useEffect(() => {
    if (!runId || !taskRunning) return;
    let disposed = false;
    let timeout: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try {
        const latest = await getScan(runId);
        if (disposed || activeRun.current !== runId) return;
        setRun(latest);
        if (scanActive(latest.run.status)) timeout = setTimeout(poll, 1200);
      } catch (reason) {
        if (disposed) return;
        setError(safeError(reason));
        timeout = setTimeout(poll, 1200);
      }
    };
    void poll();
    return () => { disposed = true; clearTimeout(timeout); };
  }, [runId, taskRunning]);

  useEffect(() => {
    if (!runId || runStatus !== "completed") return;
    let live = true;
    const current = () => live && activeRun.current === runId;
    setIssuesLoading(true);
    setIssuesError("");
    void allIssuesForRun(runId).then((findings) => {
      if (current()) setIssues(findings);
    }).catch((reason) => { if (current()) setIssuesError(safeError(reason)); })
      .finally(() => { if (current()) setIssuesLoading(false); });
    return () => { live = false; };
  }, [runId, runStatus, issuesLoadVersion]);

  useEffect(() => {
    if (!runId || runStatus !== "completed") return;
    let live = true;
    const current = () => live && activeRun.current === runId;
    setScoreLoading(true);
    setScoreError("");
    void calculateReport(runId).then((report) => {
      if (current()) setScore(report.score);
    }).catch((reason) => { if (current()) setScoreError(safeError(reason)); })
      .finally(() => { if (current()) setScoreLoading(false); });
    return () => { live = false; };
  }, [runId, runStatus, scoreLoadVersion]);

  const retryResults = (button: HTMLButtonElement) => {
    if (!canRetryIssues && !canRetryScore) return;
    if (document.activeElement === button) pageHeading.current?.focus();
    if (canRetryIssues) {
      setIssuesLoading(true);
      setIssuesLoadVersion(issuesLoadVersion + 1);
    }
    if (canRetryScore) {
      setScoreLoading(true);
      setScoreLoadVersion(scoreLoadVersion + 1);
    }
  };

  useEffect(() => {
    if (!runId || taskRunning) return;
    let live = true;
    void listAnalyzerRuns(runId).then((items) => {
      if (live) setAnalyzerRuns(items);
    }).catch((reason) => { if (live) setError(safeError(reason)); });
    return () => { live = false; };
  }, [runId, taskRunning]);

  useEffect(() => {
    if (!projectId) return;
    let live = true;
    void listScanRuns(projectId).then((runs) => {
      if (!live) return;
      setHistoryRuns(runs);
      const snapshots = runs.filter(completed);
      const valid = (id: string) => snapshots.some((item) => item.id === id);
      setHistorySelection((selected) => {
        let base = valid(selected.baseRunId) ? selected.baseRunId : "";
        let target = valid(selected.targetRunId) ? selected.targetRunId : "";
        if (!target) target = snapshots.find((item) => item.id !== base)?.id ?? snapshots[0]?.id ?? "";
        if (!base || base === target) base = snapshots.find((item) => item.id !== target)?.id ?? "";
        return {
          baseRunId: base,
          targetRunId: target,
          reportRunId: valid(selected.reportRunId) ? selected.reportRunId : snapshots[0]?.id ?? "",
        };
      });
    }).catch((reason) => { if (live) setError(safeError(reason)); });
    return () => { live = false; };
  }, [projectId, runStatus, score?.id]);

  const importIt = async (value: string) => {
    if (!value.trim() || busy || taskRunning) return;
    explicitProjectChoice.current = true;
    const request = ++projectRequest.current;
    const current = () => projectRequest.current === request;
    setBusy(true); setError(""); setNotice("");
    try {
      const next = await importProject(value.trim());
      if (!current()) return;
      showProject(next);
      setPage("inspection");
      const items = await listProjects();
      if (current()) setSavedProjects(items);
    } catch (reason) { if (current()) setError(safeError(reason)); }
    finally { if (current()) setBusy(false); }
  };

  const scanIt = async (retry?: ScanRun) => {
    if (!project?.sourceAvailable || busy || taskRunning) return;
    setBusy(true); setError(""); setNotice("");
    try {
      const config = retry?.config ?? {
        ...settings.thresholds,
        excludedDirectories: settings.excludedDirectories,
      };
      const analyzers = retry ? (Array.isArray(retry.config.enabledAnalyzers)
        ? retry.config.enabledAnalyzers.filter((name): name is string => typeof name === "string") : []) : settings.enabledAnalyzers;
      const next = await startScan(project.projectId, config, analyzers);
      showRun(next);
      setPage("scans");
    } catch (reason) { setError(safeError(reason)); } finally { setBusy(false); }
  };

  const openHistoryRun = async (id: string) => {
    setBusy(true); setError(""); setNotice("");
    try {
      showRun(await getScan(id));
      setPage("scans");
    } catch (reason) { setError(safeError(reason)); }
    finally { setBusy(false); }
  };

  const generatePrompt = async () => {
    if (!runId || runStatus !== "completed" || (promptMode === "targeted" && !selectedIssueIds.length)) return;
    setBusy(true); setError("");
    try { setPrompt(await generateGovernancePrompt(runId, promptMode, selectedIssueIds, promptBaseRunId || undefined)); }
    catch (reason) { setError(safeError(reason)); }
    finally { setBusy(false); }
  };

  const savePrompt = async (format: "md" | "txt") => {
    if (!prompt || !runId) return;
    setBusy(true); setError("");
    try {
      const destination = await pickPromptDestination(format);
      if (!destination) return;
      await exportStoredPrompt(runId, prompt.id, destination);
      setNotice("提示词已保存到 " + destination);
    } catch (reason) { setError(safeError(reason)); }
    finally { setBusy(false); }
  };

  const copyPrompt = async () => {
    if (!prompt) return;
    try {
      await navigator.clipboard.writeText(prompt.content);
      setNotice("已复制提示词");
    } catch (reason) { setError(safeError(reason)); }
  };

  const saveIssues = async (visible: Issue[]) => {
    if (!runId || !visible.length) return;
    setError("");
    try {
      const destination = await pickIssueListDestination();
      if (!destination) return;
      await exportIssueList(runId, visible.map((issue) => issue.id), destination);
      setNotice("问题清单已保存到 " + destination);
    } catch (reason) { setError(safeError(reason)); }
  };

  const controlScan = async (action: (id: string) => Promise<void>) => {
    if (!runId) return;
    try {
      await action(runId);
      const latest = await getScan(runId);
      if (activeRun.current === runId) setRun(latest);
    } catch (reason) { setError(safeError(reason)); }
  };

  const exportCurrentReport = async () => {
    if (reportPathPendingRef.current) return;
    const exportRunId = reportRunId || run?.run.id;
    if (!exportRunId || !reportDestination.trim()) { setError("请选择扫描快照并填写报告目标路径。"); return; }
    setBusy(true); setError("");
    try { await exportReport(exportRunId, reportFormat, reportDestination.trim()); setNotice("报告已写入：" + reportDestination.trim()); }
    catch (reason) { setError(safeError(reason)); }
    finally { setBusy(false); }
  };

  const compareHistory = async () => {
    if (!baseRunId || !targetRunId || baseRunId === targetRunId) { setError("请选择两个不同的扫描快照。"); return; }
    const request = ++historyRequest.current;
    setHistoryDiff(null); setHistoryIssueDetails({});
    setBusy(true); setError("");
    try {
      const [diff, baseIssues, targetIssues] = await Promise.all([
        compareScanHistory(baseRunId, targetRunId),
        allIssuesForRun(baseRunId),
        allIssuesForRun(targetRunId),
      ]);
      if (historyRequest.current !== request) return;
      setHistoryDiff(diff);
      setHistoryIssueDetails(Object.fromEntries([...baseIssues, ...targetIssues].map((issue) => [issue.id, issue])));
    }
    catch (reason) { if (historyRequest.current === request) setError(safeError(reason)); }
    finally { setBusy(false); }
  };

  const importSelectedPath = async (selectPath: () => Promise<string | null>) => {
    explicitProjectChoice.current = true;
    try {
      const selectedPath = await selectPath();
      if (selectedPath) {
        setPath(selectedPath);
        await importIt(selectedPath);
      }
    } catch (reason) { setError(safeError(reason)); }
  };

  const chooseReportPath = async () => {
    if (reportPathPendingRef.current) return;
    const request = projectRequest.current;
    const current = () => projectRequest.current === request;
    reportPathPendingRef.current = true;
    setReportPathPending(true);
    try {
      const selectedPath = await pickReportDestination(reportFormat);
      if (current() && selectedPath) setReportDestination(selectedPath);
    } catch (reason) { if (current()) setError(safeError(reason)); }
    finally { reportPathPendingRef.current = false; setReportPathPending(false); }
  };

  useEffect(() => {
    if (!isTauri() || page !== "inspection" || busy || taskRunning) return;
    let unlisten: (() => void) | undefined;
    let disposed = false;
    void getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type !== "drop") return;
      const droppedPath = event.payload.paths[0];
      if (!droppedPath) return;
      setPath(droppedPath);
      void importIt(droppedPath);
    }).then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    }).catch((reason) => setError(safeError(reason)));
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [page, busy, taskRunning]);

  const title = (nav.find((item) => item.id === page) || nav[0]).label;
  const status = runStatus ?? "queued";
  const announcementStatus = event?.status === "completed" || event?.status === "failed" || event?.status === "canceled"
    ? event.status : runStatus;
  const announcementPhase = announcementStatus === "running" ? stageNames[event?.stage ?? ""] : undefined;
  const scanAnnouncement = announcementStatus
    ? `扫描状态：${scanStatusLabels[announcementStatus]}${announcementPhase ? ` · ${announcementPhase}` : ""}` : "";

  return <div className="app-shell">
    <aside className="sidebar">
      <div className="brand"><span className="brand__mark">⌁</span><span><strong>CodeDoctor</strong><small>LOCAL QUALITY LAB</small></span></div>
      <div className={"sidebar__status"}><i className={"status-dot status-dot--" + core} />{core === "ok" ? "本地内核已就绪" : core === "checking" ? "连接本地内核…" : "内核不可用"}</div>
      <nav aria-label="工作台"><ul className="nav__list">{nav.map((item) => <li key={item.id}><button aria-label={item.label} aria-current={page === item.id ? "page" : undefined} className={"nav__item " + (page === item.id ? "nav__item--active" : "")} onClick={() => setPage(item.id)}><span className="nav__icon" aria-hidden="true">{item.icon}</span><span className="nav__label">{item.label}</span></button></li>)}</ul></nav>
      <div className="sidebar__footer">● 离线运行 · 只读分析<br />不会上传源代码</div>
    </aside>
    <main className="main">
      <header className="topbar"><div><p className="eyebrow">CODE QUALITY / {page.toUpperCase()}</p><h1 ref={pageHeading} tabIndex={-1}>{title}</h1><p className="topbar__subtitle">{project ? project.displayName + " · " + displayPath(project.canonicalPath) : "把不确定的代码项目，变成可验证的治理清单"}</p></div><span className="badge">OFFLINE</span></header>
      <div className="visually-hidden" role="status">{scanAnnouncement}</div>
      {error && <div className="alert alert--error" role="alert">{error}<button onClick={() => setError("")}>关闭</button></div>}
      {notice && <div className="alert alert--success" role="status">{notice}<button onClick={() => setNotice("")}>关闭</button></div>}
      {runStatus === "completed" && (issuesError || scoreError) && <div className="alert alert--error" role="alert">
        <div>{issuesError && <p>问题清单加载失败：{issuesError}</p>}{scoreError && <p>评分加载失败：{scoreError}{findingsState === "ready" && "，已读取的问题仍可查看。"}</p>}</div>
        <button disabled={!canRetryIssues && !canRetryScore} onClick={(event) => retryResults(event.currentTarget)}>重新加载结果</button>
      </div>}
      {page === "overview" && <Overview project={project} savedProjects={savedProjects} runs={historyRuns} issues={issues} findingsState={findingsState} score={score} busy={busy || taskRunning} onImport={() => setPage("inspection")} onIssues={() => setPage("issues")} onRestore={restoreSavedProject} onViewRun={openHistoryRun} />}
      {page === "inspection" && <Inspection project={project} path={path} setPath={setPath} busy={busy || taskRunning} onImport={importIt} onPickDirectory={() => importSelectedPath(pickProjectDirectory)} onPickArchive={() => importSelectedPath(pickProjectArchive)} onScan={() => void scanIt()} />}
      {page === "scans" && <Scans run={run} event={event} status={status} analyzers={analyzerRuns} canRetry={project?.sourceAvailable === true} onPause={() => void controlScan(pauseScan)} onResume={() => void controlScan(resumeScan)} onCancel={() => void controlScan(cancelScan)} onIssues={() => setPage("issues")} onRetry={() => void scanIt(run?.run)} />}
      {page === "issues" && <div className="page-stack">
        {(findingsState === "idle" || findingsState === "loading") && <p className="muted" role="status">{findingsStatus[findingsState]}</p>}
        {findingsState === "ready" && <IssuesView key={runId} issues={issues} projectRoot={project?.canonicalPath || ""} loadCodeContext={runId ? (issue) => readIssueSource(runId, issue.id) : undefined}
        onOpenLocation={runId ? (issue) => openIssueSource(runId, issue.id) : undefined} onExport={runStatus === "completed" ? saveIssues : undefined} />}</div>}
      {page === "architecture" && <div className="page-stack"><ArchitectureView projectId={project?.projectId || ""} runId={run?.run.status === "completed" ? run.run.id : null} /></div>}
      {page === "comments" && <Category title="注释治理" text="保留公共 API、版权和复杂算法说明；疑似过时内容需要人工确认。" issues={issues.filter((issue) => issue.category === "comments")} findingsState={findingsState} />}
      {page === "duplicates" && <Category title="重复代码" text="归一化 token 后比较结构片段，结果带有相似度和来源证据。" issues={issues.filter((issue) => issue.category === "duplicate")} findingsState={findingsState} />}
      {page === "prompts" && <Prompts key={runId} run={run} issues={issues} findingsState={findingsState} prompt={prompt} mode={promptMode} setMode={(value) => { setPromptMode(value); setPrompt(null); }}
        baseline={promptBaseRunId} baselines={historyRuns.filter((item) => completed(item) && item.id !== runId)} setBaseline={(value) => { setPromptBaseRunId(value); setPrompt(null); }}
        selectedIssueIds={selectedIssueIds} setSelectedIssueIds={(value) => { setSelectedIssueIds(value); setPrompt(null); }} busy={busy} onGenerate={generatePrompt}
        onCopy={copyPrompt} onExport={savePrompt} />}
      {page === "history" && <History project={project} runs={historyRuns} baseRunId={baseRunId} targetRunId={targetRunId} reportRunId={reportRunId}
        setBaseRunId={(value) => { historyRequest.current += 1; setBaseRunId(value); setHistoryDiff(null); }} setTargetRunId={(value) => { historyRequest.current += 1; setTargetRunId(value); setHistoryDiff(null); }}
        setReportRunId={(value) => { if (!reportPathPendingRef.current) setReportRunId(value); }} diff={historyDiff?.baseRunId === baseRunId && historyDiff.targetRunId === targetRunId ? historyDiff : null} issueDetails={historyIssueDetails} busy={busy} reportPathPending={reportPathPending} onCompare={compareHistory} format={reportFormat}
        setFormat={(value) => { if (reportPathPendingRef.current) return; setReportFormat(value); setReportDestination(""); }} destination={reportDestination} setDestination={(value) => { if (!reportPathPendingRef.current) setReportDestination(value); }} onChooseDestination={chooseReportPath} onExport={exportCurrentReport} />}
      {page === "settings" && <SettingsView settings={settings} onSave={(next: AppSettings) => {
        writeSettings(next);
        setSettings(next);
        setNotice("设置已保存，扫描阈值将在下一次扫描使用。");
      }} />}
      <Updates visible={page === "settings"} scanActive={taskRunning} />
    </main>
  </div>;
}

function Overview({ project, savedProjects, runs, issues, findingsState, score, busy, onImport, onIssues, onRestore, onViewRun }: {
  project: ProjectSummary | null; savedProjects: ProjectRecord[]; runs: ScanRun[]; issues: Issue[];
  findingsState: FindingsState; score: ScoreSnapshot | null; busy: boolean; onImport: () => void; onIssues: () => void; onRestore: (id: string) => void;
  onViewRun: (id: string) => void;
}) {
  const snapshots = runs.filter(completed);
  const trend = snapshots.filter((item) => item.scoreSummary?.coverage).slice(0, 8).reverse();
  const totals = snapshots.reduce((sum, item) => ({
    files: sum.files + item.scannedFiles, issues: sum.issues + item.totalIssues,
  }), { files: 0, issues: 0 });
  const grade = !score?.confidence ? "未评估" : score.overallScore >= 90 ? "优秀" : score.overallScore >= 80 ? "良好"
    : score.overallScore >= 70 ? "一般" : score.overallScore >= 60 ? "较差" : "需要重点治理";
  const buckets = [["critical", "严重"], ["high", "高危"], ["medium", "一般"], ["low", "提示"], ["info", "信息"]];
  return <div className="page-stack">
    <section className="hero">
      <div><h2>{project?.displayName || "开始项目体检"}</h2><p>{project ? displayPath(project.canonicalPath) : "选择项目文件夹或 ZIP，查看本机扫描结果和治理任务书。"}</p></div>
      <button className="button button--primary" onClick={onImport}>{project ? "打开项目体检" : "导入项目"}</button>
    </section>
    <section className="stat-grid">
      <Stat label="项目健康度" value={score?.confidence ? score.overallScore.toFixed(1) : "—"} hint={score ? grade + " · 覆盖 " + Math.round(score.coverage * 100) + "% · 可信度 " + Math.round(score.confidence * 100) + "%" : "完成扫描后计算"} />
      <Stat label="累计扫描文件" value={number(totals.files)} hint={snapshots.length + " 次已完成扫描"} />
      <Stat label="累计发现问题" value={number(totals.issues)} hint="当前项目的全部扫描记录" />
      <Stat label="当前严重 / 高危" value={findingsState === "ready"
        ? issues.filter((issue) => issue.severity === "critical").length + " / " + issues.filter((issue) => issue.severity === "high").length
        : "—"} hint={findingsState === "ready" ? "当前扫描快照" : findingsStatus[findingsState]} />
    </section>
    <div className="two-col">
      <section className="panel">
        <div className="section-heading"><h3>项目与最近扫描</h3></div>
        {savedProjects.length > 0 && <label className="saved-project-select">切换已保存项目<select disabled={busy} value={project?.projectId || ""} onChange={(event) => event.target.value && onRestore(event.target.value)}>
          <option value="">选择项目</option>{savedProjects.map((item) => <option value={item.id} key={item.id} title={displayPath(item.normalizedPath)}>{item.name} · {displayPath(item.normalizedPath)}</option>)}
        </select></label>}
        {runs.length ? <ul className="recent-runs">{runs.slice(0, 5).map((item) =>
          <li key={item.id}><button className="text-button" disabled={busy} onClick={() => onViewRun(item.id)}>{scanLabel(item)}</button><small>{item.config.interrupted === true ? "已中断" : scanStatusLabels[item.status]} · {number(item.totalIssues)} 个问题</small></li>
        )}</ul> : <p className="muted">暂无扫描记录</p>}
      </section>
      <section className="panel">
        <div className="section-heading"><h3>当前问题分布</h3>{findingsState === "ready" && issues.length > 0 && <button className="text-button" onClick={onIssues}>查看全部</button>}</div>
        {findingsState === "ready" ? <div className="bars">{buckets.map(([value, label]) => {
          const count = issues.filter((issue) => issue.severity === value).length;
          return <div className="bar-row" key={value}><span>{label}</span><div><i style={{ width: (issues.length ? count / issues.length * 100 : 0) + "%" }} /></div><b>{count}</b></div>;
        })}</div> : <p className="muted" role="status">{findingsStatus[findingsState]}</p>}
      </section>
    </div>
    {score && <QualityDetails score={score} />}
    {trend.length > 0 && <section className="panel dashboard-detail">
      <div className="section-heading"><h3>质量评分趋势</h3></div>
      <ol className="score-trend">{trend.map((item) => <li key={item.id}>
        <small>{scanLabel(item)}</small><meter min={0} max={100} value={item.scoreSummary?.overallScore ?? 0} aria-label={scanLabel(item)} />
        <span>{item.scoreSummary?.overallScore.toFixed(1)}</span>
      </li>)}</ol>
    </section>}
  </div>;
}

const dimensionLabels: Record<string, string> = {
  readability: "可读性", structure: "代码结构", maintainability: "可维护性", complexity: "代码复杂度",
  duplicates: "重复代码", architecture: "架构合理性", security: "安全与风险",
};

function QualityDetails({ score }: { score: ScoreSnapshot }) {
  const { items, metrics, debt } = score.dimensions;
  const percent = (value: number) => Math.round(value * 100) + "%";
  const displayCount = (value: number | null | undefined) => value == null ? "—" : number(value);
  return <section className="panel dashboard-detail">
    <div className="section-heading"><h3>检测覆盖与评分</h3></div>
    {items && <ul className="recent-runs">{Object.entries(items).map(([name, dimension]) => <li key={name}>
      <span>{dimensionLabels[name] || name}</span>
      <small>{dimension.score == null ? "未评估" : dimension.score.toFixed(1) + " 分 · 证据覆盖 " + percent(dimension.evidenceCoverage)} · 权重 {dimension.weight}%</small>
    </li>)}</ul>}
    {metrics && <div className="stat-grid stat-grid--compact">
      <Stat label="重复文件比例" value={metrics.duplicateFileRatio == null ? "—" : percent(metrics.duplicateFileRatio)} hint={metrics.duplicateEligibleFileCount == null ? "缺少适用文件基数" : `${metrics.duplicateFileCount ?? 0} / ${metrics.duplicateEligibleFileCount} 个可比较文件`} />
      <Stat label="高复杂度文件" value={displayCount(metrics.highComplexityFileCount)} hint="圈／认知复杂度命中" />
      <Stat label="高风险模块" value={displayCount(metrics.riskModuleCount)} hint="静态依赖与循环证据" />
      <Stat label="技术债务" value={debt ? `${debt.minUnits}–${debt.maxUnits}` : "—"} hint={debt ? "估算的治理工作量单位" : "暂无估算依据"} />
    </div>}
    {metrics?.fileCoverage && <p className="muted">
      AST {metrics.fileCoverage.astFiles} · 结构 {metrics.fileCoverage.structuralFiles} · 词法 {metrics.fileCoverage.lexicalFiles} · 未解析 {metrics.fileCoverage.unparsedFiles}
    </p>}
  </section>;
}

function Inspection({ project, path, setPath, busy, onImport, onPickDirectory, onPickArchive, onScan }: {
  project: ProjectSummary | null; path: string; setPath: (value: string) => void; busy: boolean;
  onImport: (value: string) => void; onPickDirectory: () => void; onPickArchive: () => void; onScan: () => void;
}) {
  return <div className="page-stack">
    <section className="dropzone">
      <div className="dropzone__icon" aria-hidden="true">⌁</div>
      <h2>拖入项目文件夹或 ZIP</h2>
      <div className="button-row">
        <button className="button button--primary" type="button" onClick={onPickDirectory} disabled={busy}>选择项目文件夹</button>
        <button className="button button--quiet" type="button" onClick={onPickArchive} disabled={busy}>选择 ZIP 压缩包</button>
      </div>
    </section>
    <form className="path-entry" onSubmit={(event) => { event.preventDefault(); onImport(path); }}>
      <label htmlFor="project-path">粘贴本地路径</label>
      <div><input id="project-path" value={path} onChange={(event) => setPath(event.target.value)} placeholder="项目文件夹或 ZIP 的绝对路径" />
        <button className="button button--quiet" disabled={busy || !path.trim()} type="submit">导入</button></div>
    </form>
    {project && <section className="panel panel--large dashboard-detail">
      <div className="section-heading">
        <div><h3>{project.displayName}</h3><p className="muted">{displayPath(project.canonicalPath)}</p></div>
        {project.sourceAvailable && <button className="button button--primary" onClick={onScan} disabled={busy}>开始全面扫描</button>}
      </div>
      {project.sourceAvailable ? <>
        <div className="stat-grid stat-grid--compact">
          <Stat label="文件" value={project.fileCount} hint={project.sourceFileCount + " 个源码文件"} />
          <Stat label="代码行" value={number(project.lineCount)} hint={"目录深度 " + project.maxDirectoryDepth} />
          <Stat label="语言" value={project.languages.join("、") || "待识别"} hint="扩展名与项目清单" />
          <Stat label="框架" value={project.frameworks.join("、") || "未识别"} hint="配置文件证据" />
        </div>
        {!!project.configFiles?.length && <details className="project-configs"><summary>配置文件（{project.configFiles.length}）</summary>
          <ul>{project.configFiles.map((file) => <li key={file}><code>{file}</code></li>)}</ul>
        </details>}
      </> : <p className="muted">原始路径已失效，仍可查看历史报告。重新扫描前，请导入可用的文件夹或 ZIP。</p>}
    </section>}
  </div>;
}

function Scans({ run, event, status, analyzers, canRetry, onPause, onResume, onCancel, onIssues, onRetry }: {
  canRetry: boolean;
  run: ScanRunSummary | null;
  event: ScanEvent | null;
  status: ScanRun["status"];
  analyzers: AnalyzerRun[];
  onPause: () => void;
  onResume: () => void;
  onCancel: () => void;
  onIssues: () => void;
  onRetry: () => void;
}) {
  const keys = ["index", "parse", "rules", "clones", "graph", "external", "finalize"];
  const total = event?.totalFiles || run?.run.totalFiles || 0;
  const processed = event?.processedFiles ?? run?.run.scannedFiles ?? 0;
  const progress = status === "completed" ? 100 : total ? Math.min(100, Math.round(processed / total * 100)) : 0;
  const canPause = status === "running" || status === "paused";
  const canCancel = scanActive(status) && status !== "canceling";
  const failureMessage = status === "failed"
    ? run?.run.failureMessage || (event?.status === "failed" ? event.message : null)
      || "扫描失败。此任务未记录详细原因，请按原设置重新扫描。"
    : null;
  const analyzerLabels: Record<AnalyzerRun["status"], string> = {
    available: "可用", executed: "已运行", failed: "运行失败", skipped: "已跳过", unavailable: "不可用",
  };
  return <div className="page-stack">
    <section className="panel dashboard-detail">
      <div className="section-heading">
        <div><h2>{run ? "扫描 " + run.run.id.slice(-12) : "等待扫描任务"}</h2>{!failureMessage && event?.message && <p className="muted">{event.message}</p>}</div>
        {run && <span className="pill pill--cyan">{scanStatusLabels[status]}</span>}
      </div>
      {failureMessage && <p className="alert alert--error" role="alert">{failureMessage}</p>}
      {run ? <>
        <div className="progress-meta">
          <strong>{status === "completed" ? "结果已保存" : stageNames[event?.stage || ""] || scanStatusLabels[status]}</strong>
          <span>{processed} / {total || "—"} 文件 · {event?.issueCount ?? run.run.totalIssues} 个问题 · {((event?.elapsedMs ?? run.elapsedMs) / 1000).toFixed(1)} 秒</span>
        </div>
        <div className="progress-track" role="progressbar" aria-label="扫描文件进度" aria-valuemin={0} aria-valuemax={100} aria-valuenow={total || status === "completed" ? progress : undefined}><i style={{ width: progress + "%" }} /></div>
        {event?.currentPath && <div className="current-file">{event.currentPath}</div>}
        <div className="stage-grid">{keys.map((key) => <div className={"stage " + (event?.stage === key ? "stage--active" : "")} key={key}><span aria-hidden="true">{event?.stage === key ? "●" : "○"}</span>{stageNames[key]}</div>)}</div>
        <div className="button-row">
          <button className="button button--quiet" disabled={!canPause} onClick={status === "paused" ? onResume : onPause}>{status === "paused" ? "继续扫描" : "暂停扫描"}</button>
          <button className="button button--danger" disabled={!canCancel} onClick={onCancel}>取消扫描</button>
          {status === "completed" && <button className="button button--primary" onClick={onIssues}>打开问题</button>}
          {(status === "failed" || status === "canceled") && <button className="button button--primary" disabled={!canRetry} onClick={onRetry}>按原设置重新扫描</button>}
        </div>
      </> : <Empty title="还没有扫描任务" text="在项目体检页开始扫描。" />}
    </section>
    {analyzers.length > 0 && <section className="panel dashboard-detail"><h3>分析器状态</h3>
      <ul className="recent-runs">{analyzers.map((item) => <li key={item.id}>
        <span>{item.analyzer} {item.version || ""}</span><small>{analyzerLabels[item.status]}{item.message ? " · " + item.message : ""}</small>
      </li>)}</ul>
    </section>}
  </div>;
}

function Category({ title, text, issues, findingsState }: { title: string; text: string; issues: Issue[]; findingsState: FindingsState }) {
  return <div className="page-stack"><section className="panel panel--large"><p className="eyebrow">GOVERNANCE VIEW</p><h2>{title}</h2><p className="muted">{text}</p>
    {findingsState === "loading" || findingsState === "error" ? <p className="muted" role="status">{findingsStatus[findingsState]}</p>
      : issues.length ? <div className="issue-table">{issues.map((issue) => <article className="issue-row" key={issue.id}><span className={"severity severity--" + issue.severity}>{issue.severity}</span><div className="issue-row__main"><strong>{issue.title}</strong><p>{issue.filePath}:{issue.startLine}</p><small>{issue.evidence}</small></div></article>)}</div>
      : <Empty title="暂无结果" text={findingsState === "ready" ? "当前扫描没有记录此类问题。" : "完成扫描后，这个治理视图会展示真实证据。"} />}
  </section></div>;
}

function Prompts({ run, issues, findingsState, prompt, mode, setMode, baseline, baselines, setBaseline,
  selectedIssueIds, setSelectedIssueIds, busy, onGenerate, onCopy, onExport }: {
  run: ScanRunSummary | null;
  issues: Issue[];
  findingsState: FindingsState;
  prompt: PromptArtifact | null;
  mode: PromptMode;
  setMode: (value: PromptMode) => void;
  baseline: string;
  baselines: ScanRun[];
  setBaseline: (value: string) => void;
  selectedIssueIds: string[];
  setSelectedIssueIds: (value: string[]) => void;
  busy: boolean;
  onGenerate: () => void;
  onCopy: () => void;
  onExport: (format: "md" | "txt") => void;
}) {
  const [query, setQuery] = useState("");
  const [offset, setOffset] = useState(0);
  const selection = new Set(selectedIssueIds);
  const matches = issues.filter((issue) => (issue.filePath + " " + issue.title + " " + issue.ruleId).toLowerCase().includes(query.trim().toLowerCase()));
  const pageSize = 40;
  const canGenerate = run?.run.status === "completed" && (mode !== "targeted" || selection.size > 0) && !busy;
  return <section className="panel panel--large dashboard-detail">
    <div className="prompt-controls">
      <label>治理模式<select value={mode} disabled={busy} onChange={(event) => setMode(event.target.value as PromptMode)}>
        {(Object.keys(promptLabels) as PromptMode[]).map((value) => <option value={value} key={value}>{promptLabels[value]}</option>)}
      </select></label>
      <label>治理范围<select value={baseline} disabled={busy} onChange={(event) => setBaseline(event.target.value)}>
        <option value="">当前扫描</option>
        {baselines.map((item) => <option key={item.id} value={item.id}>增量 · {scanLabel(item)}</option>)}
      </select></label>
    </div>
    {mode === "targeted" && findingsState !== "ready" && <p className="muted" role="status">{findingsStatus[findingsState]}</p>}
    {mode === "targeted" && findingsState === "ready" && <section className="prompt-target">
      <label>筛选待选问题<input value={query} onChange={(event) => { setQuery(event.target.value); setOffset(0); }} placeholder="按文件、规则或名称筛选" /></label>
      <div className="prompt-selection">{matches.slice(offset, offset + pageSize).map((issue) => <label key={issue.id}>
        <input type="checkbox" disabled={busy} checked={selection.has(issue.id)} onChange={(event) => {
          if (event.target.checked) selection.add(issue.id); else selection.delete(issue.id);
          setSelectedIssueIds([...selection]);
        }} />
        <span>{issue.filePath}:{issue.startLine} · {issue.title}</span>
      </label>)}{!matches.length && <p className="muted">没有符合条件的问题</p>}</div>
      <div className="button-row">
        <span className="muted">已选 {selection.size} 项 · 共 {matches.length} 个匹配</span>
        <button className="button button--quiet" type="button" disabled={offset === 0} onClick={() => setOffset(offset - pageSize)}>上一页</button>
        <button className="button button--quiet" type="button" disabled={offset + pageSize >= matches.length} onClick={() => setOffset(offset + pageSize)}>下一页</button>
      </div>
    </section>}
    <textarea aria-label="治理提示词预览" className="prompt-box" value={prompt?.content || ""} placeholder="完成扫描后可生成包含文件位置、证据与覆盖率的治理任务书。" readOnly />
    <div className="button-row">
      <button className="button button--primary" disabled={!canGenerate} onClick={onGenerate}>生成提示词</button>
      <button className="button button--quiet" disabled={!prompt || busy} onClick={onCopy}>复制提示词</button>
      <button className="button button--quiet" disabled={!prompt || busy} onClick={() => onExport("md")}>导出 Markdown</button>
      <button className="button button--quiet" disabled={!prompt || busy} onClick={() => onExport("txt")}>导出 TXT</button>
    </div>
  </section>;
}

function History({ project, runs, baseRunId, targetRunId, reportRunId, setBaseRunId, setTargetRunId, setReportRunId, diff, issueDetails, busy, reportPathPending, onCompare, format, setFormat, destination, setDestination, onChooseDestination, onExport }: {
  project: ProjectSummary | null;
  runs: ScanRun[];
  baseRunId: string;
  targetRunId: string;
  reportRunId: string;
  setBaseRunId: (value: string) => void;
  setTargetRunId: (value: string) => void;
  setReportRunId: (value: string) => void;
  diff: HistoryDiff | null;
  issueDetails: Record<string, Issue>;
  busy: boolean;
  reportPathPending: boolean;
  onCompare: () => void;
  format: ReportFormat;
  setFormat: (value: ReportFormat) => void;
  destination: string;
  setDestination: (value: string) => void;
  onChooseDestination: () => void;
  onExport: () => void;
}) {
  const completedRuns = runs.filter(completed);
  const reportBusy = busy || reportPathPending;
  const changeLabels: Record<HistoryDiff["issueChanges"][number]["kind"], string> = { added: "新增", resolved: "已解决", persistent: "持续存在", severityChanged: "严重度变化", locationChanged: "位置变化" };
  return <section className="panel panel--large dashboard-detail">
    {project ? <>
      <div className="history-controls">
        <select aria-label="基线扫描" value={baseRunId} disabled={busy} onChange={(event) => setBaseRunId(event.target.value)}>
          <option value="">基线扫描</option>{completedRuns.map((item) => <option key={item.id} value={item.id}>{scanLabel(item)}</option>)}
        </select>
        <select aria-label="目标扫描" value={targetRunId} disabled={busy} onChange={(event) => setTargetRunId(event.target.value)}>
          <option value="">目标扫描</option>{completedRuns.map((item) => <option key={item.id} value={item.id}>{scanLabel(item)}</option>)}
        </select>
        <button className="button button--primary" disabled={busy || !baseRunId || !targetRunId || baseRunId === targetRunId} onClick={onCompare}>比较快照</button>
      </div>
      {diff ? <>
        <div className="history-summary">
          <Stat label="问题变更" value={diff.issueChanges.filter((item) => item.kind !== "persistent").length}
            hint={diff.issueChanges.filter((item) => item.kind === "added").length + " 新增 · " + diff.issueChanges.filter((item) => item.kind === "resolved").length + " 已解决"} />
          <Stat label="质量分变化" value={diff.scoreDelta === null ? "未评估" : diff.scoreDelta.toFixed(1)}
            hint={"覆盖变化 " + Math.round(diff.coverageDelta * 100) + " 个百分点"} />
        </div>
        {diff.ruleVersionChanged && <p className="muted">两次扫描使用了不同规则版本。评分与问题变化可能包含检测规则的变化。</p>}
        <div className="issue-table">{diff.issueChanges.map((change) => {
          const old = change.baselineIssueId ? issueDetails[change.baselineIssueId] : undefined;
          const next = change.currentIssueId ? issueDetails[change.currentIssueId] : undefined;
          const issue = next ?? old;
          const location = old && next && old.startLine !== next.startLine ? old.startLine + " → " + next.startLine : issue?.startLine;
          const severity = change.baselineSeverity && change.currentSeverity && change.baselineSeverity !== change.currentSeverity
            ? change.baselineSeverity + " → " + change.currentSeverity : change.currentSeverity ?? change.baselineSeverity ?? "—";
          return <article className="issue-row" key={change.fingerprint}>
            <span className="severity">{changeLabels[change.kind]}</span>
            <div className="issue-row__main"><strong>{issue?.title || change.fingerprint}</strong>
              {issue && <p>{issue.filePath}:{location} · {issue.ruleId}</p>}
              {issue?.evidence && <small>{issue.evidence}</small>}
            </div>
            <span className="pill">{severity}</span>
          </article>;
        })}</div>
      </> : <p className="muted">选择同一项目的两个已完成扫描，查看问题、评分和覆盖率变化。</p>}
      <div className="report-export">
        <h3>导出报告</h3>
        <div className="history-controls">
          <select aria-label="报告快照" value={reportRunId} disabled={reportBusy} onChange={(event) => setReportRunId(event.target.value)}>
            <option value="">选择报告快照</option>{completedRuns.map((item) => <option key={item.id} value={item.id}>{scanLabel(item)}</option>)}
          </select>
          <select aria-label="报告格式" value={format} disabled={reportBusy} onChange={(event) => setFormat(event.target.value as ReportFormat)}>
            <option value="json">JSON</option><option value="markdown">Markdown</option><option value="txt">TXT</option><option value="html">HTML</option><option value="sarif">SARIF</option>
          </select>
          <input aria-label="报告保存路径" value={destination} disabled={reportPathPending} onChange={(event) => setDestination(event.target.value)} placeholder="报告保存路径" />
          <button className="button button--quiet" disabled={reportBusy || !reportRunId} onClick={onChooseDestination}>选择保存位置</button>
          <button className="button button--primary" disabled={!reportRunId || !destination.trim() || reportBusy} onClick={onExport}>导出报告</button>
        </div>
      </div>
    </> : <Empty title="还没有项目历史" text="导入项目并完成扫描后即可比较和导出快照。" />}
  </section>;
}
