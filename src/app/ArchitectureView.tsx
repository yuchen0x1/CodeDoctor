import { useEffect, useMemo, useState } from "react";
import type { ArchitectureEdge, ArchitectureGraph } from "../lib/contracts";
import { getArchitectureGraph } from "../lib/tauri";
import "./ArchitectureView.css";

type Props = {
  projectId: string;
  runId: string | null;
};

const NODE_LIMITS = [16, 32, 64, Number.MAX_SAFE_INTEGER] as const;
const DEPENDENCY_KINDS = new Set(["import", "inheritance", "component", "call"]);

function shortPath(path: string): string {
  const parts = path.replace(/\\/g, "/").split("/");
  return parts.slice(-2).join("/");
}

function nodeRadius(node: ArchitectureGraph["nodes"][number]): number {
  return 10 + Math.min(8, Math.log2(1 + node.fanIn + node.fanOut) * 2);
}

function isCycleEdge(edge: ArchitectureEdge, members: Map<string, string>): boolean {
  const group = members.get(edge.sourcePath);
  return edge.sourcePath !== edge.targetPath && DEPENDENCY_KINDS.has(edge.kind)
    && group !== undefined && group === members.get(edge.targetPath);
}

export default function ArchitectureView({ projectId, runId }: Props) {
  const [graph, setGraph] = useState<ArchitectureGraph | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [search, setSearch] = useState("");
  const [kind, setKind] = useState("");
  const [cyclesOnly, setCyclesOnly] = useState(false);
  const [nodeLimit, setNodeLimit] = useState<number>(32);
  const [selectedPath, setSelectedPath] = useState("");

  useEffect(() => {
    if (!runId || !projectId) {
      setGraph(null);
      setLoading(false);
      setError("");
      return;
    }

    let current = true;
    setLoading(true);
    setError("");
    setGraph(null);
    getArchitectureGraph(runId, projectId)
      .then((result) => {
        if (current) {
          setGraph(result);
          setSelectedPath(result.nodes[0]?.path || "");
        }
      })
      .catch((reason: unknown) => {
        if (current) setError(reason instanceof Error ? reason.message : String(reason));
      })
      .finally(() => { if (current) setLoading(false); });
    return () => { current = false; };
  }, [projectId, runId]);

  const graphKinds = useMemo(() => [...new Set(graph?.edges.map((edge) => edge.kind) || [])].sort(), [graph]);
  const cycleMembers = useMemo(() => new Map(graph?.cycles.flatMap((cycle) =>
    cycle.paths.map((path) => [path, cycle.id] as const)
  )), [graph]);
  const dependencyCount = graph?.edges.filter((edge) => DEPENDENCY_KINDS.has(edge.kind)).length ?? 0;
  const cloneCount = graph?.edges.filter((edge) => edge.kind === "clone").length ?? 0;
  const filtered = useMemo(() => {
    if (!graph) return { nodes: [], edges: [] };
    const query = search.trim().toLowerCase();
    const matchingNodes = new Set(graph.nodes
      .filter((node) => !query || node.path.toLowerCase().includes(query))
      .map((node) => node.path));
    const edges = graph.edges.filter((edge) => {
      if (kind && edge.kind !== kind) return false;
      if (cyclesOnly && !isCycleEdge(edge, cycleMembers)) return false;
      return !query || matchingNodes.has(edge.sourcePath) || matchingNodes.has(edge.targetPath);
    });
    const paths = new Set<string>();
    for (const edge of edges) {
      paths.add(edge.sourcePath);
      paths.add(edge.targetPath);
    }
    if (!kind && !cyclesOnly) {
      matchingNodes.forEach((path) => paths.add(path));
    }
    const rankedNodes = graph.nodes
      .filter((node) => paths.has(node.path))
      .sort((left, right) => (right.fanIn + right.fanOut) - (left.fanIn + left.fanOut) || left.path.localeCompare(right.path));
    const visiblePaths = new Set(rankedNodes.slice(0, nodeLimit).map((node) => node.path));
    if (selectedPath && paths.has(selectedPath)) visiblePaths.add(selectedPath);
    const nodes = rankedNodes.filter((node) => visiblePaths.has(node.path));
    const visibleEdges = edges.filter((edge) => visiblePaths.has(edge.sourcePath) && visiblePaths.has(edge.targetPath));
    return { nodes, edges: visibleEdges };
  }, [cycleMembers, cyclesOnly, graph, kind, nodeLimit, search, selectedPath]);

  const layout = useMemo(() => {
    const centerX = 470;
    const centerY = 300;
    const radius = filtered.nodes.length === 1 ? 0 : 190;
    return filtered.nodes.map((node, index) => {
      const angle = filtered.nodes.length === 1
        ? -Math.PI / 2
        : (2 * Math.PI * index / filtered.nodes.length) - Math.PI / 2;
      const x = centerX + radius * Math.cos(angle);
      const y = centerY + radius * Math.sin(angle);
      // 标签沿半径向外排布，避免固定放在节点下方时与相邻节点圆重叠或被画布裁切。
      const offset = nodeRadius(node) + 16;
      const cosine = Math.cos(angle);
      return {
        ...node,
        x,
        y,
        labelX: x + cosine * offset,
        labelY: y + Math.sin(angle) * offset + 4,
        labelAnchor: (cosine > 0.25 ? "start" : cosine < -0.25 ? "end" : "middle") as "start" | "end" | "middle",
      };
    });
  }, [filtered.nodes]);
  const positions = useMemo(() => new Map(layout.map((node) => [node.path, node])), [layout]);
  const selectedNode = graph?.nodes.find((node) => node.path === selectedPath) || null;
  const incoming = graph?.edges.filter((edge) => DEPENDENCY_KINDS.has(edge.kind) && edge.targetPath === selectedPath) || [];
  const outgoing = graph?.edges.filter((edge) => DEPENDENCY_KINDS.has(edge.kind) && edge.sourcePath === selectedPath) || [];
  const clones = graph?.edges.filter((edge) => edge.kind === "clone" && (edge.sourcePath === selectedPath || edge.targetPath === selectedPath)) || [];
  const selectedCycles = graph?.cycles.filter((cycle) => cycle.paths.includes(selectedPath)) || [];

  if (!runId) {
    return <div className="architecture-view"><section className="architecture-view__empty"><h2>架构分析</h2><p>选择一个已完成的扫描快照后，显示该次扫描中保存的项目依赖关系。</p></section></div>;
  }

  return <div className="architecture-view">
    <section className="architecture-view__heading">
      <div>
        <h2>架构关系图</h2>
        <p className="architecture-view__muted">扫描快照 {runId}</p>
      </div>
      {graph && <div className="architecture-view__stats">
        <span><strong>{graph.nodes.length}</strong> 文件节点</span>
        <span><strong>{dependencyCount}</strong> 依赖证据</span>
        {cloneCount > 0 && <span><strong>{cloneCount}</strong> 重复代码关系</span>}
        <span><strong>{graph.cycles.length}</strong> 循环组</span>
      </div>}
    </section>

    {graph && <div className="architecture-view__coverage" role="note">
      <strong>部分覆盖</strong>
      <span>{graph.coverage.note}</span>
    </div>}
    {error && <div className="architecture-view__error" role="alert">无法读取架构快照：{error}</div>}
    {loading && <div className="architecture-view__empty" role="status">正在读取本地依赖快照…</div>}

    {graph && <>
      <div className="architecture-view__filters" role="group" aria-label="架构关系筛选">
        <label>搜索文件
          <input value={search} onChange={(event) => setSearch(event.target.value)} placeholder="输入模块路径" />
        </label>
        <label>关系类型
          <select value={kind} onChange={(event) => setKind(event.target.value)}>
            <option value="">全部关系</option>
            {graphKinds.map((value) => <option key={value} value={value}>{value}</option>)}
          </select>
        </label>
        <label className="architecture-view__check">
          <input type="checkbox" checked={cyclesOnly} onChange={(event) => setCyclesOnly(event.target.checked)} />
          仅看循环关系
        </label>
        <label>图中节点
          <select value={nodeLimit} onChange={(event) => setNodeLimit(Number(event.target.value))}>
            {NODE_LIMITS.map((limit) => <option key={limit} value={limit}>{limit === Number.MAX_SAFE_INTEGER ? "全部" : `前 ${limit} 个`}</option>)}
          </select>
        </label>
      </div>

      {!graph.nodes.length && <div className="architecture-view__empty"><h3>此快照没有可展示的文件节点</h3></div>}
      {graph.nodes.length > 0 && <section className="architecture-view__body">
        <div className="architecture-view__canvas-wrap">
          <div className="architecture-view__legend">
            <span><i className="architecture-view__legend-line" />已保存依赖</span>
            <span><i className="architecture-view__legend-line architecture-view__legend-line--cycle" />循环关系</span>
            {cloneCount > 0 && <span><i className="architecture-view__legend-line architecture-view__legend-line--clone" />重复代码</span>}
          </div>
          {filtered.nodes.length ? <svg className="architecture-view__canvas" viewBox="0 0 940 600" role="group" aria-label="项目文件依赖关系图">
            <defs>
              <marker id="architecture-view-arrow" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" /></marker>
            </defs>
            {filtered.edges.map((edge) => {
              const source = positions.get(edge.sourcePath);
              const target = positions.get(edge.targetPath);
              if (!source || !target) return null;
              const clone = edge.kind === "clone";
              const variant = clone ? "architecture-view__edge--clone" : isCycleEdge(edge, cycleMembers) ? "architecture-view__edge--cycle" : "";
              const dx = target.x - source.x;
              const dy = target.y - source.y;
              const distance = Math.hypot(dx, dy);
              if (distance === 0) return null;
              const inset = Math.min(.5, (nodeRadius(target) + 2) / distance);
              return <line key={edge.id} className={`architecture-view__edge ${variant}`} x1={source.x} y1={source.y} x2={target.x - dx * inset} y2={target.y - dy * inset} markerEnd={clone ? undefined : "url(#architecture-view-arrow)"}>
                <title>{edge.sourcePath} {clone ? "↔" : "→"} {edge.targetPath} · {edge.kind} · {Math.round(edge.confidence * 100)}%</title>
              </line>;
            })}
            {layout.map((node) => <g key={node.path} className={"architecture-view__node " + (selectedPath === node.path ? "architecture-view__node--selected" : "") + (cycleMembers.has(node.path) ? " architecture-view__node--cycle" : "")} role="button" tabIndex={0} aria-pressed={selectedPath === node.path} aria-label={`${node.path}, 入度 ${node.fanIn}, 出度 ${node.fanOut}`}
              onClick={() => setSelectedPath(node.path)} onKeyDown={(event) => {
                if (event.key !== "Enter" && event.key !== " ") return;
                event.preventDefault();
                setSelectedPath(node.path);
              }}>
              <title>{node.path} · 入度 {node.fanIn} · 出度 {node.fanOut}</title>
              <circle cx={node.x} cy={node.y} r={nodeRadius(node)} />
              <text x={node.labelX} y={node.labelY} textAnchor={node.labelAnchor}>{shortPath(node.path).slice(-22)}</text>
            </g>)}
          </svg> : <div className="architecture-view__empty"><h3>当前筛选没有匹配节点</h3><p>调整搜索、关系类型或循环关系筛选。</p></div>}
          <p className="architecture-view__caption">图中默认按连接数展示最相关节点；节点选择和下方文件列表仍可查看未显示节点。</p>
        </div>

        <aside className="architecture-view__inspector">
          <h3>选中文件</h3>
          {selectedNode ? <>
            <code className="architecture-view__path">{selectedNode.path}</code>
            <div className="architecture-view__degree"><span>被引用 <b>{selectedNode.fanIn}</b></span><span>引用其他 <b>{selectedNode.fanOut}</b></span></div>
            {selectedCycles.length > 0 && <div className="architecture-view__cycle-note">位于 {selectedCycles.length} 个循环依赖组</div>}
            <h4>指向此文件</h4><EdgeList edges={incoming} direction="←" selectedPath={selectedPath} onSelect={setSelectedPath} />
            <h4>此文件指向</h4><EdgeList edges={outgoing} direction="→" selectedPath={selectedPath} onSelect={setSelectedPath} />
            {clones.length > 0 && <><h4>重复代码</h4><EdgeList edges={clones} direction="↔" selectedPath={selectedPath} onSelect={setSelectedPath} /></>}
          </> : <p className="architecture-view__muted">选择图中的节点或文件列表。</p>}
          <h3 className="architecture-view__node-heading">文件节点</h3>
          <div className="architecture-view__node-list">{(graph.nodes || []).map((node) => <button className={selectedPath === node.path ? "architecture-view__node-row architecture-view__node-row--selected" : "architecture-view__node-row"} key={node.path} type="button" aria-pressed={selectedPath === node.path} onClick={() => setSelectedPath(node.path)}>
            <span title={node.path}>{node.path}</span><small>{node.fanIn} 入 · {node.fanOut} 出</small>
          </button>)}</div>
        </aside>
      </section>}

      {graph.cycles.length > 0 && <section className="architecture-view__cycles"><h3>循环依赖组</h3>{graph.cycles.map((cycle) => <article key={cycle.id}><strong>{cycle.paths.length} 个文件相互可达</strong><p>{cycle.paths.join("、")}</p></article>)}</section>}
    </>}
  </div>;
}

function EdgeList({ edges, direction, selectedPath, onSelect }: { edges: ArchitectureEdge[]; direction: string; selectedPath: string; onSelect: (path: string) => void }) {
  if (!edges.length) return <p className="architecture-view__muted">没有已保存的关系</p>;
  return <ul className="architecture-view__edge-list">{edges.map((edge) => {
    const path = edge.sourcePath === selectedPath ? edge.targetPath : edge.sourcePath;
    const range = edge.metadata.range;
    const line = range && typeof range === "object" && "startLine" in range ? range.startLine : null;
    const label = edge.kind === "clone"
      ? `${evidenceLocation(edge.sourcePath, edge.metadata.sourceStart)} ↔ ${evidenceLocation(edge.targetPath, edge.metadata.targetStart)}`
      : evidenceLocation(edge.sourcePath, line);
    return <li key={edge.id}><button title={label} onClick={() => onSelect(path)}><span>{direction} {shortPath(path)}</span><small>{edge.kind} · {Math.round(edge.confidence * 100)}%</small></button></li>;
  })}</ul>;
}

function evidenceLocation(path: string, line: unknown): string {
  return typeof line === "number" && line > 0 ? `${path}:${line}` : path;
}
