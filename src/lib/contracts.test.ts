import { describe, expect, it } from "vitest";
import { SEVERITY_VALUES } from "./contracts";
import type { Issue, ScanRun } from "./contracts";

describe("backend wire contracts", () => {
  it("uses camelCase fields shared with the Rust core", () => {
    const issue: Issue = {
      id: "issue-1",
      ruleId: "rule.unused-import",
      category: "quality",
      severity: "medium",
      filePath: "src/example.py",
      startLine: 7,
      startColumn: 3,
      endLine: 7,
      endColumn: 18,
      title: "Unused import",
      explanation: "The import is never referenced.",
      evidence: "import os",
      impact: "Dependency surface is harder to reason about.",
      recommendation: "Remove the unused import.",
      source: "codedoctor",
      coverage: "ast",
      confidence: 0.94,
      fingerprint: "hash",
    };
    const scanRun: ScanRun = {
      id: "run-1",
      projectId: "project-1",
      status: "running",
      startedAt: "2026-11-17T00:00:00Z",
      completedAt: null,
      config: {},
      ruleVersion: "rules-1",
      analyzerVersions: {},
      totalFiles: 10,
      scannedFiles: 3,
      totalIssues: 1,
    };

    expect(issue).toMatchObject({ ruleId: "rule.unused-import", filePath: "src/example.py" });
    expect(scanRun).toMatchObject({ projectId: "project-1", startedAt: "2026-11-17T00:00:00Z" });
    expect(SEVERITY_VALUES).toEqual(["critical", "high", "medium", "low", "info"]);
  });
});
