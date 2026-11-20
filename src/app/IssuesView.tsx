import { useId, useMemo, useRef, useState } from "react";
import type { Issue, Severity } from "../lib/contracts";
import type { IssueSourceContext } from "../lib/sourceContext";
import {
  filterAndSortIssues,
  languageForIssuePath,
  moduleForIssuePath,
  uniqueIssueValues,
  type IssueSort,
  type IssueViewFilters,
} from "./issuesViewModel";

export type IssueContextLoader = (issue: Issue, projectRoot: string) => Promise<IssueSourceContext | null>;
export type IssueLocationOpener = (issue: Issue, projectRoot: string) => Promise<void> | void;

export interface IssuesViewProps {
  issues: Issue[];
  projectRoot: string;
  onOpenLocation?: IssueLocationOpener;
  loadCodeContext?: IssueContextLoader;
  onExport?: (issues: Issue[]) => Promise<void> | void;
}

const initialFilters: IssueViewFilters = {
  search: "",
  severity: "",
  category: "",
  language: "",
  file: "",
  module: "",
  sort: "severity",
};

const severityLabels: Record<Severity, string> = {
  critical: "严重",
  high: "高危",
  medium: "一般",
  low: "提示",
  info: "信息",
};

export default function IssuesView({ issues, projectRoot, onOpenLocation, loadCodeContext, onExport }: IssuesViewProps) {
  const [filters, setFilters] = useState<IssueViewFilters>(initialFilters);
  const [offset, setOffset] = useState(0);
  const detailsPrefix = useId();
  const pageSize = 50;
  const visibleIssues = useMemo(() => filterAndSortIssues(issues, filters), [issues, filters]);
  const pageOffset = Math.min(offset, Math.max(0, Math.ceil(visibleIssues.length / pageSize) - 1) * pageSize);
  const categories = useMemo(() => uniqueIssueValues(issues, (issue) => issue.category), [issues]);
  const languages = useMemo(() => uniqueIssueValues(issues, (issue) => languageForIssuePath(issue.filePath)), [issues]);
  const modules = useMemo(() => uniqueIssueValues(issues, (issue) => moduleForIssuePath(issue.filePath)), [issues]);

  const update = <K extends keyof IssueViewFilters>(key: K, value: IssueViewFilters[K]) => {
    setFilters((current) => ({ ...current, [key]: value }));
    setOffset(0);
  };

  return <section className="issues-view" aria-label="问题分析">
    <div className="issues-view__heading">
      <div>
        <h2>问题清单 <span>{visibleIssues.length} / {issues.length}</span></h2>
        <p>按证据筛选问题；展开条目可检查影响、建议和可用的源码上下文。</p>
      </div>
      {onExport && <button className="button button--quiet" type="button" disabled={!visibleIssues.length} onClick={() => void onExport(visibleIssues)}>导出当前筛选</button>}
    </div>

    <div className="issues-view__filters" role="group" aria-label="问题筛选">
      <label className="issues-view__search">
        <span>搜索问题</span>
        <input value={filters.search} onChange={(event) => update("search", event.target.value)} placeholder="名称、规则、文件或证据" />
      </label>
      <label>
        <span>严重程度</span>
        <select value={filters.severity} onChange={(event) => update("severity", event.target.value)}>
          <option value="">全部</option>
          {Object.entries(severityLabels).map(([value, label]) => <option value={value} key={value}>{label}</option>)}
        </select>
      </label>
      <label>
        <span>问题类型</span>
        <select value={filters.category} onChange={(event) => update("category", event.target.value)}>
          <option value="">全部</option>
          {categories.map((value) => <option value={value} key={value}>{value}</option>)}
        </select>
      </label>
      <label>
        <span>语言</span>
        <select value={filters.language} onChange={(event) => update("language", event.target.value)}>
          <option value="">全部</option>
          {languages.map((value) => <option value={value} key={value}>{value}</option>)}
        </select>
      </label>
      <label>
        <span>模块</span>
        <select value={filters.module} onChange={(event) => update("module", event.target.value)}>
          <option value="">全部</option>
          {modules.map((value) => <option value={value} key={value}>{value}</option>)}
        </select>
      </label>
      <label>
        <span>文件路径</span>
        <input value={filters.file} onChange={(event) => update("file", event.target.value)} placeholder="包含此路径文本" />
      </label>
      <label>
        <span>排序</span>
        <select value={filters.sort} onChange={(event) => update("sort", event.target.value as IssueSort)}>
          <option value="severity">严重程度</option>
          <option value="path">文件路径</option>
          <option value="line">代码行号</option>
        </select>
      </label>
      <button className="text-button issues-view__reset" type="button" onClick={() => { setFilters(initialFilters); setOffset(0); }}>清除筛选</button>
    </div>

    {visibleIssues.length ? <div className="issues-view__list">
      {visibleIssues.slice(pageOffset, pageOffset + pageSize).map((issue) => <IssueItem
        key={issue.id}
        issue={issue}
        detailsId={`${detailsPrefix}-details-${encodeURIComponent(issue.id)}`}
        projectRoot={projectRoot}
        onOpenLocation={onOpenLocation}
        loadCodeContext={loadCodeContext}
      />)}
    </div> : <div className="issues-view__empty" role="status">{issues.length ? "没有符合当前筛选条件的问题。" : "当前扫描没有记录问题。"}</div>}
    {visibleIssues.length > pageSize && <div className="button-row">
      <span className="muted" role="status">显示第 {pageOffset + 1}–{Math.min(pageOffset + pageSize, visibleIssues.length)} 项，共 {visibleIssues.length} 项</span>
      <button className="button button--quiet" disabled={pageOffset === 0} onClick={() => setOffset(pageOffset - pageSize)}>上一页</button>
      <button className="button button--quiet" disabled={pageOffset + pageSize >= visibleIssues.length} onClick={() => setOffset(pageOffset + pageSize)}>下一页</button>
    </div>}
  </section>;
}

function IssueItem({ issue, detailsId, projectRoot, onOpenLocation, loadCodeContext }: {
  issue: Issue;
  detailsId: string;
  projectRoot: string;
  onOpenLocation?: IssueLocationOpener;
  loadCodeContext?: IssueContextLoader;
}) {
  const [expanded, setExpanded] = useState(false);
  const [context, setContext] = useState<IssueSourceContext | null>(null);
  const [contextState, setContextState] = useState<"idle" | "loading" | "loaded" | "unavailable" | "error">("idle");
  const [contextError, setContextError] = useState("");
  const [locationError, setLocationError] = useState("");
  const [contextRequested, setContextRequested] = useState(false);
  const [retrying, setRetrying] = useState(false);
  const contextHeading = useRef<HTMLHeadingElement>(null);
  const retryButton = useRef<HTMLButtonElement>(null);

  const requestContext = (fromRetry = false) => {
    if (retrying) return;
    setContextRequested(true);
    if (!loadCodeContext) {
      setContextState("unavailable");
      return;
    }
    if (fromRetry) setRetrying(true);
    else setContextState("loading");
    void loadCodeContext(issue, projectRoot).then((result) => {
      if (fromRetry && document.activeElement === retryButton.current) contextHeading.current?.focus();
      setContext(result);
      setContextState(result ? "loaded" : "unavailable");
    }).catch((reason: unknown) => {
      setContextError(reason instanceof Error ? reason.message : String(reason));
      setContextState("error");
    }).finally(() => { if (fromRetry) setRetrying(false); });
  };

  const toggleExpanded = () => {
    const next = !expanded;
    setExpanded(next);
    if (next && !contextRequested) requestContext();
  };

  const openLocation = async () => {
    if (!onOpenLocation) return;
    setLocationError("");
    try { await onOpenLocation(issue, projectRoot); }
    catch (reason) { setLocationError(reason instanceof Error ? reason.message : String(reason)); }
  };

  return <article className={"issues-view__item " + (expanded ? "issues-view__item--expanded" : "")}>
    <div className="issues-view__item-head">
      <span className={"severity severity--" + issue.severity}>{severityLabels[issue.severity]}</span>
      <button className="issues-view__summary" type="button" aria-expanded={expanded} aria-controls={detailsId} onClick={toggleExpanded}>
        <strong>{issue.title}</strong>
        <span>{issue.filePath}:{issue.startLine}:{issue.startColumn} · {issue.ruleId}</span>
      </button>
      <span className="issues-view__confidence">{issue.coverage} · {Math.round(issue.confidence * 100)}%</span>
    </div>

    <div id={detailsId} className={expanded ? "issues-view__details" : undefined} hidden={!expanded}>{expanded && <>
      <dl>
        <div><dt>问题说明</dt><dd>{issue.explanation}</dd></div>
        <div><dt>潜在影响</dt><dd>{issue.impact}</dd></div>
        <div><dt>修复建议</dt><dd>{issue.recommendation}</dd></div>
        <div><dt>检测依据</dt><dd><pre>{issue.ruleId === "security.hardcoded-secret" ? "疑似凭据已隐藏" : issue.evidence}</pre></dd></div>
        <div><dt>问题编号</dt><dd>{issue.id}</dd></div>
        <div><dt>来源</dt><dd>{issue.source} · {issue.category} · {issue.fingerprint}</dd></div>
      </dl>

      <section className="issues-view__context" aria-label="代码上下文">
        <h3 ref={contextHeading} tabIndex={-1}>代码上下文</h3>
        {contextState === "idle" && <p>展开问题后读取上下文状态。</p>}
        {contextState === "loading" && <p role="status">正在读取本地代码上下文…</p>}
        {contextState === "unavailable" && <p>该问题暂无可读取的源码上下文。</p>}
        {contextState === "error" && <>
          <p role="alert">无法读取源码上下文：{contextError}</p>
          <button ref={retryButton} className="button button--quiet" type="button" aria-disabled={retrying} onClick={() => requestContext(true)}>{retrying ? "正在重试…" : "重试读取上下文"}</button>
        </>}
        {context?.stale && <p role="status">源码已变化。重新扫描后可查看对应代码。</p>}
        {context && !context.stale && <pre className="issues-view__code" tabIndex={0} role="group" aria-label="源码片段"><code>{context.text.split(/\r?\n/).map((line, index) => {
          const lineNumber = context.startLine + index;
          const matched = lineNumber >= issue.startLine && lineNumber <= issue.endLine;
          return <span className={matched ? "code-line code-line--matched" : "code-line"} key={lineNumber}>{lineNumber}: {line}</span>;
        })}</code></pre>}
        {context && !context.stale && <p className="issues-view__line-range">{issue.filePath} · 第 {issue.startLine}–{issue.endLine} 行{context.redacted ? " · 已遮蔽敏感内容" : ""}{context.truncated ? " · 上下文已截断" : ""}</p>}
      </section>

      {onOpenLocation && context?.canOpenNatively && <div className="issues-view__actions"><button className="button button--quiet" type="button" onClick={() => void openLocation()}>用记事本打开源文件</button>{locationError && <p role="alert">无法打开源文件：{locationError}</p>}</div>}
    </>}</div>
  </article>;
}
