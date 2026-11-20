import { describe, expect, it } from "vitest";
import type { Issue } from "../lib/contracts";
import {
  filterAndSortIssues,
  languageForIssuePath,
  moduleForIssuePath,
  uniqueIssueValues,
  type IssueViewFilters,
} from "./issuesViewModel";

const issue = (overrides: Partial<Issue>): Issue => ({
  id: "issue-1",
  ruleId: "quality.long-function",
  category: "complexity",
  severity: "medium",
  filePath: "src/features/editor.ts",
  startLine: 40,
  startColumn: 1,
  endLine: 90,
  endColumn: 1,
  title: "Long function",
  explanation: "The function combines too many operations.",
  evidence: "function save() { ... }",
  impact: "Changes are harder to validate.",
  recommendation: "Separate independent operations.",
  source: "codedoctor",
  coverage: "ast",
  confidence: 0.9,
  fingerprint: "fingerprint-1",
  ...overrides,
});

const filters = (overrides: Partial<IssueViewFilters> = {}): IssueViewFilters => ({
  search: "",
  severity: "",
  category: "",
  language: "",
  file: "",
  module: "",
  sort: "severity",
  ...overrides,
});

const backendLanguageMappings = [
  ["src/main.py", "Python"],
  ["src/main.pyw", "Python"],
  ["src/main.pyi", "Python"],
  ["src/main.js", "JavaScript"],
  ["src/main.mjs", "JavaScript"],
  ["src/main.cjs", "JavaScript"],
  ["src/main.ts", "TypeScript"],
  ["src/main.mts", "TypeScript"],
  ["src/main.cts", "TypeScript"],
  ["src/main.CTS", "TypeScript"],
  ["src/main.PYW", "Python"],
  ["src/main.PYI", "Python"],
  // The UI intentionally groups JSX/TSX with JavaScript/TypeScript.
  ["src/view.jsx", "JavaScript"],
  ["src/view.tsx", "TypeScript"],
  ["src/main.java", "Java"],
  ["src/main.c", "C"],
  ["src/main.h", "C/C++"],
  ["src/main.cc", "C++"],
  ["src/main.cpp", "C++"],
  ["src/main.cxx", "C++"],
  ["src/main.hpp", "C++"],
  ["src/main.cs", "C#"],
  ["src/main.go", "Go"],
  ["src/main.rs", "Rust"],
  ["src/main.php", "PHP"],
  ["src/main.kt", "Kotlin"],
  ["src/main.kts", "Kotlin"],
  ["src/main.swift", "Swift"],
  ["src/main.html", "HTML"],
  ["src/main.htm", "HTML"],
  ["src/main.css", "CSS"],
  ["src/main.scss", "SCSS"],
  ["src/main.vue", "Vue"],
  ["src/main.svelte", "Svelte"],
  ["src/main.sql", "SQL"],
  ["src/main.sh", "Shell"],
  ["src/main.bash", "Shell"],
  ["src/main.ps1", "PowerShell"],
  ["src/main.json", "JSON"],
  ["src/main.yaml", "YAML"],
  ["src/main.yml", "YAML"],
  ["src/main.xml", "XML"],
  ["src/Dockerfile", "Dockerfile"],
] as const;

describe("issue view model", () => {
  const issues = [
    issue({ id: "2", filePath: "src/features/editor.ts", startLine: 40 }),
    issue({ id: "3", severity: "critical", category: "security", filePath: "api/auth.py", startLine: 8 }),
    issue({ id: "1", severity: "high", filePath: "src/components/panel.tsx", startLine: 12 }),
  ];

  it("filters by text, severity, category, language, file, and module together", () => {
    const result = filterAndSortIssues(issues, filters({
      search: "long function",
      severity: "medium",
      category: "complexity",
      language: "TypeScript",
      file: "editor.ts",
      module: "src/features",
    }));

    expect(result.map((item) => item.id)).toEqual(["2"]);
  });

  it.each(backendLanguageMappings)("classifies backend language path %s as %s", (filePath, expectedLanguage) => {
    expect(languageForIssuePath(filePath)).toBe(expectedLanguage);
  });

  it("keeps backend-supported paths in the language-filtered export candidates", () => {
    const candidates = backendLanguageMappings.map(([filePath], index) => issue({
      id: `language-candidate-${index}`,
      filePath,
    }));
    const languages = new Set(backendLanguageMappings.map(([, language]) => language));

    for (const language of languages) {
      const expectedPaths = backendLanguageMappings
        .filter(([, mappedLanguage]) => mappedLanguage === language)
        .map(([filePath]) => filePath)
        .sort();
      const exportCandidates = filterAndSortIssues(candidates, filters({ language }));

      expect(exportCandidates.map(({ filePath }) => filePath).sort()).toEqual(expectedPaths);
    }
  });

  it("sorts by severity, path, and source line with deterministic ties", () => {
    expect(filterAndSortIssues(issues, filters({ sort: "severity" })).map((item) => item.id))
      .toEqual(["3", "1", "2"]);
    expect(filterAndSortIssues(issues, filters({ sort: "path" })).map((item) => item.id))
      .toEqual(["3", "1", "2"]);
    expect(filterAndSortIssues([
      issue({ id: "later", startLine: 12 }),
      issue({ id: "earlier", startLine: 3 }),
    ], filters({ sort: "line" })).map((item) => item.id)).toEqual(["earlier", "later"]);
    expect(filterAndSortIssues([
      issue({ id: "line-3", filePath: "z.ts", startLine: 3 }),
      issue({ id: "line-12", filePath: "a.ts", startLine: 12 }),
    ], filters({ sort: "line" })).map((item) => item.id)).toEqual(["line-3", "line-12"]);
  });

  it("normalizes Windows paths and handles files at the project root", () => {
    expect(languageForIssuePath("src\\features\\EDITOR.TS")).toBe("TypeScript");
    expect(moduleForIssuePath("src\\features\\editor.ts")).toBe("src/features");
    expect(moduleForIssuePath("README.md")).toBe("README.md");
  });

  it("returns sorted unique values without changing the source issue list", () => {
    const source = [...issues];
    expect(uniqueIssueValues(issues, (item) => item.category)).toEqual(["complexity", "security"]);
    expect(filterAndSortIssues(issues, filters({ sort: "severity" })).map((item) => item.id))
      .toEqual(["3", "1", "2"]);
    expect(issues).toEqual(source);
  });
});
