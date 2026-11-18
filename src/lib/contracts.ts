export const SEVERITY_VALUES = ["critical", "high", "medium", "low", "info"] as const;
export type Severity = (typeof SEVERITY_VALUES)[number];

export const SCAN_STATUS_VALUES = [
  "queued",
  "running",
  "paused",
  "canceling",
  "canceled",
  "completed",
  "failed",
] as const;
export type ScanStatus = (typeof SCAN_STATUS_VALUES)[number];

export const COVERAGE_VALUES = ["ast", "lexical", "structural", "notcovered"] as const;
export type CoverageKind = (typeof COVERAGE_VALUES)[number];

export const ANALYZER_STATUS_VALUES = [
  "available",
  "executed",
  "failed",
  "skipped",
  "unavailable",
] as const;
export type AnalyzerStatus = (typeof ANALYZER_STATUS_VALUES)[number];

export const PROMPT_MODE_VALUES = ["comprehensive", "focused", "targeted"] as const;
export type PromptMode = (typeof PROMPT_MODE_VALUES)[number];

export interface ProjectRecord {
  id: string;
  name: string;
  normalizedPath: string;
  createdAt: string;
}

export interface ScanRun {
  id: string;
  projectId: string;
  status: ScanStatus;
  startedAt: string | null;
  completedAt: string | null;
  config: Record<string, unknown>;
  ruleVersion: string;
  analyzerVersions: Record<string, unknown>;
  totalFiles: number;
  scannedFiles: number;
  totalIssues: number;
  scoreSummary?: { overallScore: number; coverage: number } | null;
}

export interface FileRecord {
  id: string;
  scanRunId: string;
  projectId: string;
  relativePath: string;
  absolutePath: string;
  encoding: string;
  sizeBytes: number;
  contentHash: string;
  language: string;
  parseCapability: CoverageKind;
  excludedReason: string | null;
}

export interface Issue {
  id: string;
  ruleId: string;
  category: string;
  severity: Severity;
  filePath: string;
  startLine: number;
  startColumn: number;
  endLine: number;
  endColumn: number;
  title: string;
  explanation: string;
  evidence: string;
  impact: string;
  recommendation: string;
  source: string;
  coverage: CoverageKind;
  confidence: number;
  fingerprint: string;
}

export interface ArchitectureEdge {
  id: string;
  sourcePath: string;
  targetPath: string;
  kind: string;
  confidence: number;
  coverage: CoverageKind;
  metadata: Record<string, unknown>;
}

export interface ArchitectureGraphNode {
  path: string;
  fanIn: number;
  fanOut: number;
}

export interface ArchitectureGraphCycle {
  id: string;
  paths: string[];
}

export interface ArchitectureGraph {
  projectId: string;
  runId: string;
  nodes: ArchitectureGraphNode[];
  edges: ArchitectureEdge[];
  cycles: ArchitectureGraphCycle[];
  coverage: {
    status: "partial";
    edgeCount: number;
    completenessKnown: false;
    unresolvedImportsAvailable: false;
    isolatedFilesAvailable: boolean;
    note: string;
  };
}

export interface AnalyzerRun {
  id: string;
  analyzer: string;
  status: AnalyzerStatus;
  version: string | null;
  durationMs: number | null;
  message: string | null;
}

export interface ScoreSnapshot {
  id: string;
  overallScore: number;
  dimensions: {
    items?: Record<string, { score: number | null; weight: number; evidenceCoverage: number }>;
    metrics?: {
      duplicateFileRatio: number | null;
      duplicateFileCount: number | null;
      duplicateEligibleFileCount: number | null;
      highComplexityFileCount: number | null;
      riskModuleCount: number | null;
      fileCoverage?: { astFiles: number; structuralFiles: number; lexicalFiles: number; unparsedFiles: number } | null;
    };
    debt?: { minUnits: number; maxUnits: number } | null;
  };
  coverage: number;
  confidence: number;
  ruleVersion: string;
  analyzerVersions: Record<string, unknown>;
  config: Record<string, unknown>;
  createdAt: string;
}

export interface PromptArtifact {
  id: string;
  mode: PromptMode;
  selectedIssueIds: string[];
  templateVersion: string;
  content: string;
  createdAt: string;
}

export interface ProjectSummary {
  sourceAvailable: boolean;
  projectId: string;
  canonicalPath: string;
  displayName: string;
  languages: string[];
  frameworks: string[];
  configFiles?: string[];
  fileCount: number;
  sourceFileCount: number;
  lineCount: number;
  maxDirectoryDepth: number;
}

export interface IndexSummary {
  fileCount: number;
  sourceFileCount: number;
  lineCount: number;
  maxDirectoryDepth: number;
  languages: string[];
  frameworks: string[];
  files: Array<{ relativePath: string; absolutePath: string; encoding: string; sizeBytes: number; language: string; excludedReason: string | null }>;
  excludedFiles: Array<{ relativePath: string; reason: string }>;
}

export interface ScanEvent {
  runId: string;
  stage: string;
  currentPath: string | null;
  processedFiles: number;
  totalFiles: number;
  issueCount: number;
  elapsedMs: number;
  status: string;
  message: string | null;
}

export interface ScanRunSummary {
  run: ScanRun;
  elapsedMs: number;
}

export interface IssuePage {
  issues: Issue[];
  total: number;
}

export type ReportFormat = "json" | "markdown" | "txt" | "html";

export interface ReportResult {
  score: ScoreSnapshot;
  content: string;
}

export interface IssueChange {
  fingerprint: string;
  kind: "added" | "resolved" | "persistent" | "severityChanged" | "locationChanged";
  baselineIssueId: string | null;
  currentIssueId: string | null;
  baselineSeverity: Severity | null;
  currentSeverity: Severity | null;
}

export interface HistoryDiff {
  baseRunId: string;
  targetRunId: string;
  issueChanges: IssueChange[];
  scoreDelta: number | null;
  ruleVersionChanged: boolean;
  coverageDelta: number;
}
