# CodeDoctor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a usable offline Windows CodeDoctor desktop application that imports projects, performs evidence-backed multi-language code-quality analysis, stores scan history, exports reports, and generates grounded governance prompts.

**Architecture:** Tauri 2 hosts a React/TypeScript interface and a Rust scanning core. Rust owns file access, parsing, rules, graph analysis, SQLite persistence, external analyzer processes, scoring, reports, and prompt generation; the frontend calls typed commands and listens to scan events. The implementation stays local and read-only, with explicit coverage and confidence for every result.

**Tech Stack:** Tauri 2, React, TypeScript, Vite, pnpm, Rust, Tokio, Serde, SQLite via `rusqlite`, Tree-sitter grammars, `walkdir`, `zip`, `encoding_rs`, SHA-256, Vitest, and Playwright smoke coverage where the environment supports it.

**Spec:** `docs/superpowers/specs/2026-11-17-codedoctor-design.md`

## Global Constraints

- Windows 10 and Windows 11 are the supported desktop targets.
- The application is offline-first and never uploads project source code.
- The scanner never executes scripts from the project being scanned.
- Source files are read-only; no automatic delete or rewrite operation is exposed.
- ZIP import rejects path traversal, symlink escape, excessive entry counts, excessive expanded size, and oversized individual files.
- Default exclusions include `node_modules`, `.git`, `build`, `dist`, caches, and virtual environments, with user overrides.
- Unsupported parser coverage is displayed explicitly; lexical rules are never presented as full AST analysis.
- Dynamic imports, reflection, plugin registration, dependency injection, and external entry points produce “suspected” results when they cannot be proven.
- External analyzers are optional local executables; the app never downloads them automatically.
- Every issue carries an ID, rule, severity, relative path, location, evidence, source, coverage, and confidence.
- Every score records its configuration, rule version, analyzer versions, coverage, and confidence.
- User-facing UI must expose loading, empty, success, error, paused, canceled, and unavailable-analyzer states.
- A build, installer, signature, or update is reported as verified only after the corresponding command or workflow has actually passed.

---

### Task 1: Scaffold the Tauri and React workspace

**Files:**
- Create: `package.json`, `pnpm-lock.yaml`, `index.html`, `src/main.tsx`, `src/vite-env.d.ts`
- Create: `src-tauri/Cargo.toml`, `src-tauri/build.rs`, `src-tauri/tauri.conf.json`, `src-tauri/src/main.rs`, `src-tauri/src/lib.rs`
- Create: `src-tauri/capabilities/default.json`
- Create: `src/styles/app.css`, `src/app/App.tsx`

**Interfaces:**
- Produces the Tauri application entry point and a frontend that can call one health command.

- [ ] **Step 1: Scaffold the official React TypeScript Tauri template**

Run from the repository root:

```powershell
pnpm create tauri-app@latest .
```

Choose React, TypeScript, and pnpm when prompted. Preserve `docs/` and `.git/`.

- [ ] **Step 2: Add the project scripts and baseline dependencies**

Configure scripts for `dev`, `build`, `test`, `test:ui`, `tauri`, and `typecheck`. Add React, Tauri API/core packages, Vitest, and the TypeScript tooling required by the template. Keep the UI dependency set small: CSS variables and SVG are preferred over a large component framework.

- [ ] **Step 3: Add a health command and typed frontend bridge**

Add this Rust command in `src-tauri/src/lib.rs`:

```rust
#[tauri::command]
fn health() -> &'static str {
    "ok"
}
```

Register it with the Tauri builder and call it from `src/lib/tauri.ts` through `invoke<string>('health')`.

- [ ] **Step 4: Create the application shell and theme variables**

Render a sidebar placeholder, a main content region, and the CodeDoctor title. Define dark theme variables for background, panel, border, text, cyan accent, and severity colors in `src/styles/app.css`.

- [ ] **Step 5: Verify the scaffold and commit**

Run:

```powershell
pnpm install
pnpm build
cargo check --manifest-path src-tauri/Cargo.toml
```

Commit with `git add . && git commit -m "chore: scaffold CodeDoctor desktop app"`.

### Task 2: Define shared domain types and SQLite storage

**Files:**
- Create: `src-tauri/src/domain/mod.rs`, `src-tauri/src/domain/types.rs`, `src-tauri/src/error.rs`
- Create: `src-tauri/src/storage/mod.rs`, `src-tauri/src/storage/migrations.rs`, `src-tauri/migrations/0001_initial.sql`
- Create: `src/lib/contracts.ts`
- Test: `src-tauri/src/domain/types.rs` tests and `src-tauri/src/storage/mod.rs` integration tests

**Interfaces:**
- Produces Rust `ProjectRecord`, `ScanRun`, `FileRecord`, `Issue`, `ArchitectureEdge`, `ScoreSnapshot`, and `PromptArtifact`.
- Produces TypeScript mirrors with camelCase JSON names.

Use this minimum issue contract:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub id: String,
    pub rule_id: String,
    pub category: String,
    pub severity: Severity,
    pub file_path: String,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub title: String,
    pub explanation: String,
    pub evidence: String,
    pub impact: String,
    pub recommendation: String,
    pub source: String,
    pub coverage: CoverageKind,
    pub confidence: f32,
    pub fingerprint: String,
}
```

- [ ] **Step 1: Write failing tests for enum serialization and fingerprints**

Assert that severity values serialize as `critical`, `high`, `medium`, `low`, and `info`, and that equal normalized evidence produces the same fingerprint while a changed line range or rule produces a different fingerprint.

- [ ] **Step 2: Implement domain types and error conversion**

Add serde-enabled enums, the scan state enum, coverage enum, analyzer status, and an `AppError` that converts IO, SQLite, ZIP, parser, and process failures into non-sensitive messages.

- [ ] **Step 3: Write the initial SQLite migration**

Create tables for projects, scan runs, files, issues, architecture edges, analyzer runs, score snapshots, prompt artifacts, and schema metadata. Add indexes on project ID, run ID, issue fingerprint, file path, severity, and category.

- [ ] **Step 4: Implement storage repositories**

Expose focused methods such as `insert_project`, `create_scan_run`, `upsert_file`, `insert_issue`, `insert_edge`, `save_score`, `save_prompt`, `list_issues`, and `compare_issue_fingerprints`. Use transactions for each completed scan phase.

- [ ] **Step 5: Run tests and commit**

Run `cargo test --manifest-path src-tauri/Cargo.toml domain storage`. Commit with `git commit -m "feat: add CodeDoctor domain model and storage"`.

### Task 3: Implement safe project import and file indexing

**Files:**
- Create: `src-tauri/src/import/mod.rs`, `src-tauri/src/import/zip.rs`, `src-tauri/src/import/path.rs`
- Create: `src-tauri/src/index/mod.rs`, `src-tauri/src/index/encoding.rs`, `src-tauri/src/index/language.rs`, `src-tauri/src/index/framework.rs`
- Create: `src-tauri/src/commands/project.rs`
- Test: `src-tauri/src/import/*` tests and `src-tauri/src/index/*` tests

**Interfaces:**
- `import_project(path: String) -> Result<ProjectSummary, AppError>`
- `index_project(project_id: String) -> Result<IndexSummary, AppError>`
- `ProjectSummary` includes canonical path, display name, languages, frameworks, file count, source file count, line count, and max directory depth.

- [ ] **Step 1: Write tests for path safety and exclusions**

Use temporary directories to test `../escape.txt`, absolute ZIP entries, symlink escape, default ignored directories, UTF-8/UTF-16 decoding, Chinese paths, binary-file exclusion, and maximum file size.

- [ ] **Step 2: Implement canonical project paths and directory traversal**

Reject non-existent paths and paths that resolve to files when a folder is expected. Walk without following directory symlinks, normalize relative paths, and return explicit exclusion reasons.

- [ ] **Step 3: Implement bounded ZIP extraction**

Extract into a unique temporary directory only after checking normalized entry paths, entry count, compressed/uncompressed limits, and symlink metadata. Never write outside the temporary root.

- [ ] **Step 4: Implement decoding and language/framework identification**

Use BOM detection and UTF-8/UTF-16 decoding, classify binary files conservatively, and combine extensions with `package.json`, `pyproject.toml`, `Cargo.toml`, solution files, lockfiles, and framework markers.

- [ ] **Step 5: Expose project commands and verify**

Register project commands, persist the project and file index, and run `cargo test --manifest-path src-tauri/Cargo.toml import index`. Commit with `git commit -m "feat: add safe project import and indexing"`.

### Task 4: Add parser registry and structural metrics

**Files:**
- Create: `src-tauri/src/parse/mod.rs`, `src-tauri/src/parse/registry.rs`, `src-tauri/src/parse/tree_sitter.rs`, `src-tauri/src/parse/metrics.rs`
- Create: `src-tauri/src/parse/languages/{mod.rs,python.rs,javascript.rs,typescript.rs,common.rs}`
- Create: `docs/language-capability-matrix.md`
- Test: `src-tauri/src/parse/*` tests and parser fixtures under `fixtures/`

**Interfaces:**
- `ParserRegistry::parse(file: &FileRecord, text: &str) -> ParseResult`
- `ParseResult` contains parser name/version, coverage kind, diagnostics, functions, classes, imports, calls, comments, and metrics.

- [ ] **Step 1: Add parser fixtures and failing metric tests**

Cover a Python function, TypeScript class, JavaScript import, malformed source, and a non-AST language. Assert line ranges, function length, nesting depth, branch count, parameter count, and parser coverage.

- [ ] **Step 2: Register Tree-sitter grammars behind a language capability table**

Keep parser construction in one registry. Each language entry declares extensions, grammar, structural features, and fallback rules. Unsupported entries return a lexical-only result with a visible reason.

- [ ] **Step 3: Implement node extraction and source ranges**

Extract functions, classes, imports, calls, inheritance, control-flow nodes, and comments where the grammar supports them. Convert byte offsets to line/column positions using the decoded source.

- [ ] **Step 4: Implement metrics**

Calculate function length, parameter count, branch count, nesting depth, approximate cyclomatic complexity, class method/property counts, and file-level import counts. Preserve missing metrics as `None` rather than zero.

- [ ] **Step 5: Run parser tests and commit**

Run `cargo test --manifest-path src-tauri/Cargo.toml parse`. Update the capability matrix with actual parser and rule coverage. Commit with `git commit -m "feat: add parser registry and code metrics"`.

### Task 5: Implement internal quality and governance rules

**Files:**
- Create: `src-tauri/src/rules/mod.rs`, `src-tauri/src/rules/basic.rs`, `src-tauri/src/rules/complexity.rs`, `src-tauri/src/rules/comments.rs`, `src-tauri/src/rules/security.rs`, `src-tauri/src/rules/config.rs`
- Create: `src-tauri/src/rules/runner.rs`
- Test: `src-tauri/src/rules/*` tests and `fixtures/rules/`

**Interfaces:**
- `RuleContext` contains `FileRecord`, optional `ParseResult`, project configuration, and neighboring module facts.
- `Rule::check(&self, context: &RuleContext) -> Vec<IssueDraft>`.
- `RuleRunner::run(contexts, config) -> Vec<Issue>`.

- [ ] **Step 1: Create failing fixture tests for each rule family**

Add fixtures for long functions, deep nesting, empty exception handling, debug prints, unused imports where parser evidence exists, magic literals, commented-out code, stale comments, hard-coded secrets, dangerous command execution, and SQL concatenation. Assert evidence and line ranges, not only issue counts.

- [ ] **Step 2: Implement rule configuration and threshold loading**

Use a versioned JSON-compatible configuration struct with defaults for line length, function length, nesting depth, parameter count, class size, duplicate minimum length, and severity thresholds.

- [ ] **Step 3: Implement basic, complexity, comment, and security rules**

Rules must emit evidence and confidence policies. Pattern-only security findings use lower confidence than parser-backed findings and are labeled as potential risks.

- [ ] **Step 4: Implement deduplication within one rule run**

Normalize issue messages and merge overlapping findings only when rule ID, file range, and evidence match. Preserve source names when internal and external findings later converge.

- [ ] **Step 5: Run rule tests and commit**

Run `cargo test --manifest-path src-tauri/Cargo.toml rules`. Commit with `git commit -m "feat: add local quality governance rules"`.

### Task 6: Add duplicate-code and architecture analysis

**Files:**
- Create: `src-tauri/src/clones/mod.rs`, `src-tauri/src/clones/token_normalize.rs`, `src-tauri/src/clones/matcher.rs`
- Create: `src-tauri/src/graph/mod.rs`, `src-tauri/src/graph/imports.rs`, `src-tauri/src/graph/scc.rs`, `src-tauri/src/graph/metrics.rs`
- Test: `src-tauri/src/clones/*` and `src-tauri/src/graph/*` tests; `fixtures/architecture/`

**Interfaces:**
- `CloneDetector::find(files: &[ParsedFile], config: &CloneConfig) -> Vec<CloneGroup>`
- `DependencyGraph::build(parsed_files: &[ParsedFile]) -> GraphSnapshot`
- `GraphSnapshot::cycles() -> Vec<Cycle>`

- [ ] **Step 1: Write failing tests for exact and similar clones**

Use two identical functions, two renamed-but-equivalent functions, and a normal repeated UI pattern. Assert that exact and high-similarity groups are distinguished and normal short patterns are ignored.

- [ ] **Step 2: Implement normalized token shingles and candidate matching**

Strip identifiers and literal values according to configuration, preserve control-flow tokens, require a minimum token count, and compare candidates with a structural similarity threshold.

- [ ] **Step 3: Write failing dependency graph tests**

Create a three-file import cycle, a high-fan-out module, and an unresolved dynamic import. Assert strongly connected components, edge evidence, and incomplete-coverage flags.

- [ ] **Step 4: Implement graph construction and metrics**

Build file/module nodes from parser imports and configured path aliases, calculate fan-in/fan-out and coupling, and emit architecture issues for cycles and excessive coupling.

- [ ] **Step 5: Run tests and commit**

Run `cargo test --manifest-path src-tauri/Cargo.toml clones graph`. Commit with `git commit -m "feat: add duplicate and architecture analysis"`.

### Task 7: Add optional external analyzer adapters

**Files:**
- Create: `src-tauri/src/analyzers/mod.rs`, `src-tauri/src/analyzers/process.rs`, `src-tauri/src/analyzers/discovery.rs`
- Create: `src-tauri/src/analyzers/ruff.rs`, `eslint.rs`, `typescript.rs`, `golangci.rs`, `clippy.rs`
- Test: `src-tauri/src/analyzers/*` tests with captured output fixtures

**Interfaces:**
- `AnalyzerAdapter::metadata() -> AnalyzerMetadata`
- `AnalyzerAdapter::run(request: AnalyzerRequest) -> AnalyzerResult`
- `AnalyzerResult` includes status, executable version, duration, raw summary, and normalized issues.

- [ ] **Step 1: Write parser tests for representative analyzer output**

Store small JSON/text outputs for Ruff, ESLint, TypeScript, golangci-lint, and Clippy. Assert mapping to the common issue model and preservation of original tool codes.

- [ ] **Step 2: Implement executable discovery without installation**

Search the project configuration and PATH for known executables, record a missing state when not found, and never invoke a package manager.

- [ ] **Step 3: Implement bounded process execution**

Use a timeout, cancellation token, stdout/stderr size cap, sanitized environment, and project working directory. Do not pass credentials or execute shell command strings.

- [ ] **Step 4: Map, deduplicate, and persist analyzer runs**

Normalize file paths and locations, retain tool versions and raw summaries, and merge matching internal findings without discarding source provenance.

- [ ] **Step 5: Run adapter tests and commit**

Run `cargo test --manifest-path src-tauri/Cargo.toml analyzers`. Commit with `git commit -m "feat: add optional local analyzer adapters"`.

### Task 8: Build the scan orchestrator and Tauri commands

**Files:**
- Create: `src-tauri/src/scan/mod.rs`, `src-tauri/src/scan/job.rs`, `src-tauri/src/scan/cache.rs`, `src-tauri/src/scan/events.rs`
- Create: `src-tauri/src/commands/scan.rs`, `src-tauri/src/state.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/scan/*` integration tests

**Interfaces:**
- `start_scan(request: ScanRequest) -> Result<ScanRunSummary, AppError>`
- `pause_scan(run_id: String) -> Result<(), AppError>`
- `cancel_scan(run_id: String) -> Result<(), AppError>`
- `get_scan(run_id: String) -> Result<ScanRunSummary, AppError>`
- Event name `scan://progress` with `ScanEvent { runId, stage, currentPath, processedFiles, totalFiles, issueCount, elapsedMs, status, message }`.

- [ ] **Step 1: Write lifecycle tests**

Run a scan against a small fixture and assert ordered stage events, persisted counts, pause/resume state, cancellation, and a failed analyzer does not mark the entire scan as “no issues”.

- [ ] **Step 2: Implement job state and cancellation**

Store active jobs in application state keyed by run ID. Use an atomic cancel flag and a pause gate checked between files, parser units, and analyzer invocations.

- [ ] **Step 3: Implement cache lookup and phase orchestration**

Load unchanged file results by content/rule/analyzer/config hash; run only invalidated phases; persist each completed phase before emitting the next completion event.

- [ ] **Step 4: Emit typed progress events and register commands**

Use Tauri commands for request/response operations and events for progress updates. Keep event payloads small and send large result pages through paginated commands.

- [ ] **Step 5: Run scan integration tests and commit**

Run `cargo test --manifest-path src-tauri/Cargo.toml scan`. Commit with `git commit -m "feat: add cancellable scan orchestration"`.

### Task 9: Implement scoring, reports, prompts, and history comparison

**Files:**
- Create: `src-tauri/src/score/mod.rs`, `src-tauri/src/score/dimensions.rs`
- Create: `src-tauri/src/report/mod.rs`, `src-tauri/src/report/json.rs`, `src-tauri/src/report/markdown.rs`, `src-tauri/src/report/html.rs`
- Create: `src-tauri/src/prompt/mod.rs`, `src-tauri/src/history/mod.rs`
- Create: `src-tauri/src/commands/report.rs`, `src-tauri/src/commands/history.rs`
- Test: scoring, report, prompt, and history integration tests

**Interfaces:**
- `calculate_score(snapshot: &ScanSnapshot) -> ScoreSnapshot`
- `generate_prompt(snapshot_id: String, mode: PromptMode, selected_issue_ids: Vec<String>) -> PromptArtifact`
- `export_report(snapshot_id: String, format: ReportFormat, destination: String) -> Result<(), AppError>`
- `compare_scans(base_id: String, target_id: String) -> HistoryDiff`

- [ ] **Step 1: Write failing score tests**

Assert the seven weights, the coverage-adjusted formula, unassessed dimensions, confidence, severity counts, duplicate ratio, high-complexity file count, and no unsupported full score.

- [ ] **Step 2: Implement score dimensions and debt ranges**

Keep every dimension traceable to metric values and issue IDs. Produce a bounded debt range with a list of contributing factors.

- [ ] **Step 3: Write prompt evidence tests**

Generate all three modes from a fixture snapshot and assert that every included issue has its real ID, relative path, line, evidence, and recommendation, while an unrelated issue is absent from focused modes.

- [ ] **Step 4: Implement reports and prompt templates**

Render JSON, Markdown, TXT, and HTML from the same snapshot. Keep the six governance phases and fifteen constraints in one versioned template module.

- [ ] **Step 5: Implement history diff and commands**

Compare issue fingerprints, severity changes, score deltas, rule-version changes, and coverage changes. Run tests and commit with `git commit -m "feat: add scoring reports prompts and history"`.

### Task 10: Build the frontend shell and design system

**Files:**
- Create: `src/app/App.tsx`, `src/app/routes.ts`, `src/app/appStore.ts`
- Create: `src/components/layout/Sidebar.tsx`, `TopBar.tsx`, `PageFrame.tsx`, `StatusPill.tsx`
- Create: `src/components/ui/Card.tsx`, `Badge.tsx`, `Button.tsx`, `EmptyState.tsx`, `ProgressBar.tsx`
- Create: `src/styles/tokens.css`, `src/styles/app.css`
- Test: `src/app/appStore.test.ts`, component tests under `src/components/`

**Interfaces:**
- Store actions: `setActiveProject`, `setActiveScan`, `setScanEvent`, `setTheme`, `resetWorkspace`.
- Navigation IDs match `overview`, `inspection`, `scans`, `issues`, `architecture`, `comments`, `duplicates`, `prompts`, `history`, `settings`.

- [ ] **Step 1: Write frontend tests for navigation and scan state**

Assert that route selection, scan event updates, paused/canceled states, and reset behavior produce the expected store state.

- [ ] **Step 2: Implement the typed Tauri bridge**

Wrap every command in `src/lib/tauri.ts` and expose a typed `listenToScanProgress` helper that unlistens on cleanup.

- [ ] **Step 3: Implement the shell and responsive layout**

Create the sidebar, top bar, page frame, route outlet, and compact layouts for narrow windows. Ensure all buttons have labels and visible keyboard focus.

- [ ] **Step 4: Implement the visual language**

Use CSS variables for dark/light themes, panel borders, severity colors, spacing, typography, and reduced-motion behavior. Use SVG icons with text labels where meaning matters.

- [ ] **Step 5: Run frontend tests and commit**

Run `pnpm test -- --run src/app src/components`. Commit with `git commit -m "feat: add CodeDoctor frontend shell"`.

### Task 11: Implement project import and live scan UI

**Files:**
- Create: `src/features/projects/ProjectInspectionPage.tsx`, `src/features/projects/ProjectDropZone.tsx`, `src/features/projects/projectApi.ts`
- Create: `src/features/scans/ScanPage.tsx`, `src/features/scans/ScanTimeline.tsx`, `src/features/scans/scanApi.ts`
- Modify: `src/app/routes.ts`, `src/app/appStore.ts`
- Test: project and scan feature tests

**Interfaces:**
- UI calls `importProject`, `startScan`, `pauseScan`, `cancelScan`, and `getScan` through the bridge.
- Drop zone accepts folders and `.zip`, displays rejected inputs, and never begins scanning until the user starts it.

- [ ] **Step 1: Write tests for drop behavior and scan state rendering**

Assert folder/ZIP acceptance, invalid input messages, project summary rendering, event-driven progress, pause/resume controls, cancellation, and analyzer-unavailable status.

- [ ] **Step 2: Implement folder picker and drag/drop import**

Use the Tauri dialog for folder selection and the Webview drag/drop payload for files/folders. Call the import command and display the returned summary.

- [ ] **Step 3: Implement scan timeline and counters**

Render file traversal, language recognition, parsing, rule analysis, duplicate analysis, dependency analysis, security analysis, and summary stages from actual events.

- [ ] **Step 4: Implement scan controls and recovery**

Disable conflicting controls, preserve the last event after pause/cancel, and offer resume or a new scan based on the backend status.

- [ ] **Step 5: Run tests and commit**

Run `pnpm test -- --run src/features/projects src/features/scans`. Commit with `git commit -m "feat: add project import and scan workflow"`.

### Task 12: Implement results, architecture, prompt, and history pages

**Files:**
- Create: `src/features/issues/IssuesPage.tsx`, `IssueTable.tsx`, `IssueDetail.tsx`, `issueApi.ts`
- Create: `src/features/architecture/ArchitecturePage.tsx`, `DependencyGraph.tsx`
- Create: `src/features/prompts/PromptPage.tsx`, `promptApi.ts`
- Create: `src/features/history/HistoryPage.tsx`, `historyApi.ts`
- Create: `src/features/dashboard/OverviewPage.tsx`
- Create: `src/features/settings/SettingsPage.tsx`
- Test: feature component and store tests

**Interfaces:**
- Paginated issue query accepts severity, category, language, file, module, search, sort, and page.
- Graph query returns nodes, edges, cycles, coverage, and selected-module details.
- History query returns score deltas and issue diff groups.

- [ ] **Step 1: Write tests for issue filtering and prompt modes**

Assert filters compose, issue details show evidence and code context, selected issue IDs reach the prompt command, and export errors are visible.

- [ ] **Step 2: Implement the issue table and detail drawer**

Add severity/category/file/module filters, search, sorting, pagination, expandable evidence, code context, confidence, and source analyzer labels.

- [ ] **Step 3: Implement dashboard and architecture graph**

Render score dimensions, counts, coverage, confidence, trends, and a zoomable SVG dependency graph with cycle highlighting and incomplete-coverage badges.

- [ ] **Step 4: Implement prompt and history workflows**

Add mode switching, issue selection, preview, copy, Markdown/TXT export, scan comparison, and incremental prompt generation.

- [ ] **Step 5: Implement settings and run tests**

Expose thresholds, exclusion directories, analyzer toggles, editor path, theme, and reduced-motion preference. Run `pnpm test -- --run src/features`. Commit with `git commit -m "feat: add CodeDoctor analysis workflows"`.

### Task 13: Add fixtures, integration tests, documentation, and offline checks

**Files:**
- Create: `fixtures/normal/`, `fixtures/long-function/`, `fixtures/duplicates/`, `fixtures/comments/`, `fixtures/security/`, `fixtures/mixed-project/`
- Create: `tests/integration/scan.test.ts`, `tests/ui/smoke.spec.ts`
- Create: `README.md`, `docs/language-capability-matrix.md`, `docs/analyzers.md`, `docs/privacy.md`
- Modify: `package.json`, `src-tauri/Cargo.toml`

- [ ] **Step 1: Add representative fixture projects**

Include valid and invalid source files, a Chinese path fixture, a ZIP fixture with a traversal entry, and a project containing excluded directories.

- [ ] **Step 2: Add end-to-end scan assertions**

Run a complete fixture scan and assert language detection, line locations, issue counts by category, score coverage, prompt evidence, JSON/Markdown/HTML exports, and history diffs.

- [ ] **Step 3: Add frontend smoke coverage**

Start the Vite/Tauri frontend in test mode and verify the shell, project empty state, scan page, issue filtering, prompt mode selection, and export error state. Mark native-window checks as blocked if WebView2 is unavailable.

- [ ] **Step 4: Write accurate documentation**

Document setup, commands, supported languages and parser levels, external analyzer discovery, privacy boundaries, default exclusions, report formats, and known coverage limits. Do not describe unimplemented behavior as available.

- [ ] **Step 5: Run the full local verification and commit**

Run:

```powershell
pnpm typecheck
pnpm test -- --run
cargo test --manifest-path src-tauri/Cargo.toml
pnpm build
```

Commit with `git commit -m "test: add CodeDoctor fixtures and verification"`.

### Task 14: Package and verify the Windows desktop application

**Files:**
- Modify: `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`
- Create: `src-tauri/icons/icon.svg` and generated Windows icon assets
- Modify: `README.md`

- [ ] **Step 1: Create the application icon source**

Create a simple CodeDoctor mark as SVG, then run the Tauri icon command to generate the configured Windows assets. Keep the source icon in the repository.

- [ ] **Step 2: Configure Windows bundle metadata**

Set the bundle identifier, product name, version, installer targets, icon paths, installation metadata, and application log location. Keep filesystem capabilities limited to the commands that require them.

- [ ] **Step 3: Build the application and installer**

Run:

```powershell
pnpm tauri build
```

Record the exact output paths and exit code. If the build fails because of a missing Windows prerequisite, report it as BLOCKED with the command output.

- [ ] **Step 4: Perform local smoke verification**

Launch the built application, import a fixture project, run a scan, open issues, generate a prompt, and export a report. Verify that the application remains offline and does not modify the fixture source.

- [ ] **Step 5: Document the verified status and commit**

Update README with the actual build result, supported installer format, and any blocked signing or cross-machine checks. Commit with `git commit -m "build: package CodeDoctor for Windows"`.

## Self-Review Checklist

- [ ] The plan covers import, ZIP safety, indexing, language coverage, AST/lexical boundaries, rules, duplicates, architecture, analyzers, cancellation, pause/resume, caching, score coverage, reports, prompts, history, UI, tests, docs, and Windows packaging.
- [ ] Every planned command and interface names its file and return shape.
- [ ] Unsupported parser and missing-analyzer states are represented explicitly.
- [ ] No task permits source modification, project script execution, network installation, or unverified release claims.
- [ ] Each feature task begins with a focused failing test or a concrete scaffold verification and ends with a command and commit.
