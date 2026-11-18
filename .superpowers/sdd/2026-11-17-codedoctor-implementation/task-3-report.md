# Task 3 report

- Status: DONE (local implementation and verification complete)
- Scope: safe project path canonicalization and traversal, default exclusions, symlink exclusion, bounded ZIP extraction, BOM-aware UTF-8/UTF-16 decoding, conservative binary exclusion, source size limits, language/framework detection, project/index summaries, and registered Tauri project commands.
- Persistence: `import_project` stores an imported project in the in-memory `Storage`; `index_project` creates an index scan run and persists `FileRecord` rows.
- Tests added: Chinese paths, nonexistent/file path rejection, traversal normalization, default ignored directories, directory symlink escape (Unix), ZIP traversal and entry-count limits, extraction cleanup after failure, UTF-8/UTF-16 BOM decoding, binary exclusion, language detection, framework markers including false-positive rejection, and oversized files.

## Verification

- PASS — `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
- PASS — `cargo check --offline --manifest-path src-tauri/Cargo.toml`

## Final review residual fixes

- File record IDs now include the scan ID (`project:scan:relativePath`), so repeated indexing preserves both historical scan rows without primary-key collisions. Added a duplicate-index regression test and a storage count assertion.
- The ZIP-suffix directory regression fixture now uses a real directory name ending in `.zip`.
- Pyproject framework matching now uses identifier boundaries, rejecting prefixes such as `djangoish` and `pytest-extra`; added explicit false-positive tests.
- PASS — `cargo test --offline --manifest-path src-tauri/Cargo.toml commands::project::tests` (2 tests)
- PASS — `cargo test --offline --manifest-path src-tauri/Cargo.toml index::framework` (3 tests)
- PASS — `cargo test --offline --manifest-path src-tauri/Cargo.toml` (32 tests)
- PASS — `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
- PASS — `cargo check --offline --manifest-path src-tauri/Cargo.toml`
- PASS — `cargo test --offline --manifest-path src-tauri/Cargo.toml import` (7 tests)
- PASS — `cargo test --offline --manifest-path src-tauri/Cargo.toml index` (6 tests)
- PASS — `cargo test --offline --manifest-path src-tauri/Cargo.toml` (29 tests)
- Note — Cargo accepts one test filter, so the requested `import index` filter was run as separate `import` and `index` commands.

## Concerns

- The repository had no cached ZIP/temporary-file crates and the environment could not reach crates.io. To keep the task offline and avoid installing dependencies, the ZIP reader is dependency-free and currently accepts stored entries only; deflated ZIP entries return a safe “unsupported” error. Path traversal, absolute paths, entry count, compressed archive size, expanded size, per-file size, duplicate destination, and symlink metadata checks remain enforced.
- The Tauri command registry uses process-local in-memory state and SQLite `Storage::in_memory()` until a later task provides application state/database path wiring.

## Review fixes

- ZIP imports now derive identity from the canonical archive path, clean extraction directories on both success and failure, and re-extract only for indexing. Local ZIP headers are checked against the central directory for names, flags, compression, sizes, entry count, and symlink metadata.
- Index persistence now uses `Storage::with_phase`, so scan creation and file rows roll back together. Scan IDs include a process-local atomic sequence to avoid same-second collisions.
- Project paths are inserted into the registry only after project persistence succeeds.
- Index reads are bounded to `MAX_SOURCE_FILE_BYTES + 1` to detect growth beyond the limit without unbounded reads.
- Framework detection reads dependency keys from `package.json` and constrained manifest lines instead of matching arbitrary raw substrings.

## Follow-up review fix

- ZIP extraction during indexing now requires both `path.is_file()` and a `.zip` extension. Ordinary directories whose names end in `.zip` remain directory projects and are indexed normally. Added a regression test for this case.
- PASS — `cargo test --offline --manifest-path src-tauri/Cargo.toml commands::project::tests::directory_named_with_zip_suffix_is_indexed_as_directory` (1 test)
- PASS — `cargo test --offline --manifest-path src-tauri/Cargo.toml` (30 tests)
- PASS — `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
- PASS — `cargo check --offline --manifest-path src-tauri/Cargo.toml`
