import type { Issue } from "../lib/contracts";

export type IssueSort = "severity" | "path" | "line";

export interface IssueViewFilters {
  search: string;
  severity: string;
  category: string;
  language: string;
  file: string;
  module: string;
  sort: IssueSort;
}

const severityOrder: Record<Issue["severity"], number> = {
  critical: 0,
  high: 1,
  medium: 2,
  low: 3,
  info: 4,
};

const languageByExtension: Record<string, string> = {
  c: "C",
  cc: "C++",
  cjs: "JavaScript",
  cpp: "C++",
  cts: "TypeScript",
  cxx: "C++",
  cs: "C#",
  css: "CSS",
  go: "Go",
  h: "C/C++",
  hpp: "C++",
  htm: "HTML",
  html: "HTML",
  java: "Java",
  json: "JSON",
  js: "JavaScript",
  jsx: "JavaScript",
  kt: "Kotlin",
  kts: "Kotlin",
  mjs: "JavaScript",
  mts: "TypeScript",
  php: "PHP",
  ps1: "PowerShell",
  psm1: "PowerShell",
  py: "Python",
  pyi: "Python",
  pyw: "Python",
  rs: "Rust",
  scss: "SCSS",
  bash: "Shell",
  sh: "Shell",
  sql: "SQL",
  svelte: "Svelte",
  swift: "Swift",
  ts: "TypeScript",
  tsx: "TypeScript",
  vue: "Vue",
  xml: "XML",
  yaml: "YAML",
  yml: "YAML",
};

const normalizePath = (value: string) => value.replace(/\\/g, "/").replace(/^\.\//, "");
const lower = (value: string) => value.toLowerCase();

export function languageForIssuePath(filePath: string): string {
  const name = normalizePath(filePath).split("/").pop() || "";
  if (name.toLowerCase() === "dockerfile") return "Dockerfile";
  const extension = name.includes(".") ? name.split(".").pop()?.toLowerCase() : undefined;
  return extension ? languageByExtension[extension] || "Other" : "Other";
}

export function moduleForIssuePath(filePath: string): string {
  const parts = normalizePath(filePath).split("/").filter(Boolean);
  if (parts.length >= 3) return parts.slice(0, 2).join("/");
  return parts[0] || "Root";
}

export function uniqueIssueValues(issues: Issue[], getValue: (issue: Issue) => string): string[] {
  return [...new Set(issues.map(getValue))].sort((a, b) => a.localeCompare(b));
}

export function filterAndSortIssues(issues: Issue[], filters: IssueViewFilters): Issue[] {
  const query = lower(filters.search.trim());
  const fileQuery = lower(normalizePath(filters.file.trim()));
  const filtered = issues.filter((issue) => {
    if (filters.severity && issue.severity !== filters.severity) return false;
    if (filters.category && issue.category !== filters.category) return false;
    if (filters.language && languageForIssuePath(issue.filePath) !== filters.language) return false;
    if (filters.module && moduleForIssuePath(issue.filePath) !== filters.module) return false;
    if (fileQuery && !lower(normalizePath(issue.filePath)).includes(fileQuery)) return false;
    if (!query) return true;
    return [
      issue.title,
      issue.ruleId,
      issue.category,
      issue.filePath,
      issue.evidence,
      issue.explanation,
      issue.impact,
      issue.recommendation,
    ].some((value) => lower(value).includes(query));
  });

  return filtered.sort((left, right) => {
    const pathOrder = normalizePath(left.filePath).localeCompare(normalizePath(right.filePath));
    if (filters.sort === "path") return pathOrder || left.startLine - right.startLine || left.id.localeCompare(right.id);
    if (filters.sort === "line") return left.startLine - right.startLine || pathOrder || left.id.localeCompare(right.id);
    return severityOrder[left.severity] - severityOrder[right.severity]
      || pathOrder
      || left.startLine - right.startLine
      || left.id.localeCompare(right.id);
  });
}
