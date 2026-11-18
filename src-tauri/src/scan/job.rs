//! Scan job: runs the full pipeline (index → parse → rules → clones → graph →
//! external analyzers → finalize) on a background thread with real progress
//! events, pause/cancel gates, and per-file rule-result caching. Source code
//! is only read, never executed.

use super::events::{EventSink, ScanEvent};
use super::{cache::rule_cache_key, now_millis};
use crate::clones::{CloneConfig, CloneDetector, CloneKind};
use crate::domain::types::{
    AnalyzerRun, AnalyzerStatus, ArchitectureEdge, CoverageKind, FileRecord, Issue, ScanStatus,
    Severity,
};
use crate::error::AppError;
use crate::import::path::{DiscoveredFile, canonical_project_path, collect_files_checked};
use crate::import::zip::{extract_zip_checked, unique_temp_dir};
use crate::index::MAX_SOURCE_FILE_BYTES;
use crate::parse::metrics::FileMetrics;
use crate::parse::{ParseResult, ParsedFile, ParserRegistry};
use crate::rules::IssueDraft;
use crate::rules::config::{RULE_VERSION, RuleConfig};
use crate::rules::runner::RuleRunner;
use crate::storage::{PhaseTransaction, Storage};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::Instant;

/// Pause/cancel gate checked at file and phase boundaries. Pausing is honest:
/// the job stops at the next boundary and reports `paused`, never faking a
/// frozen progress bar.
#[derive(Default)]
pub struct PauseGate {
    state: Mutex<GateState>,
    signal: Condvar,
}

#[derive(Default, Clone, Copy, PartialEq)]
enum GateState {
    #[default]
    Running,
    Paused,
}

impl PauseGate {
    pub fn pause(&self) {
        *self.state.lock().unwrap() = GateState::Paused;
    }

    pub fn resume(&self) {
        *self.state.lock().unwrap() = GateState::Running;
        self.signal.notify_all();
    }

    fn is_paused(&self) -> bool {
        *self.state.lock().unwrap() == GateState::Paused
    }

    pub(crate) fn wait_if_paused(&self) {
        let mut state = self.state.lock().unwrap();
        while *state == GateState::Paused {
            state = self.signal.wait(state).unwrap();
        }
    }
}

/// Shared control handles for one active scan job.
pub struct JobHandle {
    pub cancel: AtomicBool,
    pub gate: PauseGate,
}

impl JobHandle {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            cancel: AtomicBool::new(false),
            gate: PauseGate::default(),
        })
    }
}

pub struct ScanParams {
    pub run_id: String,
    pub project_id: String,
    pub root: PathBuf,
    pub config: RuleConfig,
    pub enabled_analyzers: Vec<String>,
    /// Destination for this run's extracted ZIP, created only by the worker.
    pub workspace: Option<PathBuf>,
}

struct ExtractedRoot(PathBuf);

impl Drop for ExtractedRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct JobContext<'a> {
    params: &'a ScanParams,
    handle: &'a JobHandle,
    sink: &'a EventSink,
    start: Instant,
    processed: u32,
    indexed: u32,
    total: u32,
    issue_count: u32,
}

impl JobContext<'_> {
    fn check_gate(&self, stage: &str) -> Result<(), AppError> {
        self.handle.gate.wait_if_paused();
        if self.handle.cancel.load(Ordering::Relaxed) {
            return Err(AppError::Validation(format!(
                "scan canceled during {stage} phase"
            )));
        }
        Ok(())
    }

    fn emit(&self, stage: &str, status: &str, current: Option<&str>, message: Option<String>) {
        let status = if status == "running" && self.handle.cancel.load(Ordering::Relaxed) {
            "canceling"
        } else if status == "running" && self.handle.gate.is_paused() {
            "paused"
        } else {
            status
        };
        let mut event = ScanEvent::new(&self.params.run_id, stage, status);
        event.current_path = current.map(str::to_owned);
        event.processed_files = self.processed;
        event.total_files = self.total;
        event.issue_count = self.issue_count;
        event.elapsed_ms = self.start.elapsed().as_millis() as u64;
        event.message = message;
        (self.sink)(&event);
    }
}

fn storage_poisoned() -> AppError {
    AppError::Validation("local storage is unavailable".to_owned())
}

fn analyzer_status_label(status: &AnalyzerStatus) -> &'static str {
    match status {
        AnalyzerStatus::Available => "可用",
        AnalyzerStatus::Executed => "已执行",
        AnalyzerStatus::Failed => "执行失败",
        AnalyzerStatus::Skipped => "已跳过",
        AnalyzerStatus::Unavailable => "不可用",
    }
}

/// Runs the whole scan. Every completed phase persists atomically; a failed
/// analyzer never masks findings as "no issues".
pub fn run_scan_job(
    params: &ScanParams,
    handle: &JobHandle,
    sink: &EventSink,
    storage: &Arc<Mutex<Storage>>,
) -> Result<(), AppError> {
    let mut ctx = JobContext {
        params,
        handle,
        sink,
        start: Instant::now(),
        processed: 0,
        indexed: 0,
        total: 0,
        issue_count: 0,
    };

    let result = run_phases(&mut ctx, storage);
    let persisted = (|| {
        let mut store = loop {
            if result.is_ok() {
                handle.gate.wait_if_paused();
            }
            let store = storage.lock().map_err(|_| storage_poisoned())?;
            if result.is_ok() && handle.gate.is_paused() {
                // A pause was accepted after the wait and before the write.
                continue;
            }
            break store;
        };
        let (status, event_status, message) = if handle.cancel.load(Ordering::Relaxed) {
            (ScanStatus::Canceled, "canceled", "扫描已取消".to_owned())
        } else if let Err(error) = &result {
            (ScanStatus::Failed, "failed", format!("扫描失败：{error}"))
        } else {
            (ScanStatus::Completed, "completed", "扫描完成".to_owned())
        };
        ctx.issue_count = store.finish_scan_run(
            &params.run_id,
            status,
            &now_millis().to_string(),
            ctx.total,
            ctx.indexed,
        )?;
        ctx.processed = ctx.indexed;
        Ok::<_, AppError>((event_status, message))
    })();
    match persisted {
        Ok((status, message)) => ctx.emit("done", status, None, Some(message)),
        Err(error) => {
            ctx.emit(
                "done",
                "failed",
                None,
                Some(format!("扫描状态保存失败：{error}")),
            );
            return Err(error);
        }
    }
    result
}

fn persist_issues(
    phase: &mut PhaseTransaction<'_>,
    run_id: &str,
    issues: &[Issue],
) -> Result<(), AppError> {
    for issue in issues {
        let mut issue = issue.clone();
        // Issue fingerprints stay stable across scans. Row IDs belong to a run.
        issue.id = format!("{run_id}:{}", issue.id);
        phase.insert_issue(run_id, &issue)?;
    }
    Ok(())
}

fn run_phases(ctx: &mut JobContext<'_>, storage: &Arc<Mutex<Storage>>) -> Result<(), AppError> {
    let config_json = serde_json::to_string(&ctx.params.config)
        .map_err(|_| AppError::Validation("scan config could not be stored".to_owned()))?;

    ctx.check_gate("index")?;
    let canonical_source = fs::canonicalize(&ctx.params.root)?;
    if canonical_source != ctx.params.root && ctx.params.workspace.is_some() {
        return Err(AppError::Validation(
            "the registered project path has changed".to_owned(),
        ));
    }
    let archive = canonical_source.is_file()
        && canonical_source
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"));
    let extracted = if archive {
        ctx.emit(
            "index",
            "running",
            None,
            Some("正在解压 ZIP 快照".to_owned()),
        );
        Some(ExtractedRoot(extract_zip_checked(
            &canonical_source,
            || match &ctx.params.workspace {
                Some(directory) => {
                    fs::create_dir(directory)?;
                    Ok(directory.clone())
                }
                None => unique_temp_dir(),
            },
            || ctx.check_gate("index"),
        )?))
    } else {
        None
    };
    let root = canonical_project_path(extracted.as_ref().map_or(&canonical_source, |dir| &dir.0))?;

    // ---- Phase 1: index (traversal, language recognition, decoding) ----
    ctx.emit(
        "index",
        "running",
        None,
        Some("正在遍历项目文件".to_owned()),
    );
    let (discovered, excluded) =
        collect_files_checked(&root, &ctx.params.config.excluded_directories, || {
            ctx.check_gate("index")
        })?;
    ctx.total = discovered.len() as u32;
    let mut sources: Vec<(FileRecord, String)> = Vec::with_capacity(discovered.len());
    let mut excluded_records: Vec<FileRecord> = Vec::new();
    let mut source_line_count = 0_u64;
    for file in &discovered {
        ctx.check_gate("index")?;
        ctx.emit("index", "running", Some(&file.relative_path), None);
        let (record, text) = index_file(ctx.params, &root, file, archive);
        if record.excluded_reason.is_none() && record.language != "Unknown" {
            source_line_count += text.lines().count() as u64;
        }
        sources.push((record, text));
        ctx.processed += 1;
    }
    for excluded in &excluded {
        excluded_records.push(excluded_record(
            &ctx.params.run_id,
            &ctx.params.project_id,
            &root,
            excluded,
            archive.then_some(canonical_source.as_path()),
        ));
    }
    let frameworks = crate::index::framework::detect_indexed_frameworks(
        sources
            .iter()
            .filter(|(record, _)| record.excluded_reason.is_none())
            .map(|(record, text)| (record.relative_path.as_str(), text.as_str())),
    );
    {
        let mut store = storage.lock().map_err(|_| storage_poisoned())?;
        store.with_phase(|phase| {
            for (record, _) in &sources {
                phase.upsert_file(record)?;
            }
            for record in &excluded_records {
                phase.upsert_file(record)?;
            }
            phase.set_indexed_project_context(
                &ctx.params.run_id,
                &serde_json::json!({
                    "frameworks": frameworks,
                    "sourceLineCount": source_line_count,
                }),
            )
        })?;
        ctx.indexed = ctx.total;
    }

    // ---- Phase 2: parse ----
    ctx.processed = 0;
    ctx.emit(
        "parse",
        "running",
        None,
        Some("正在解析语法结构".to_owned()),
    );
    let mut parsed: Vec<ParsedFile> = Vec::with_capacity(sources.len());
    for (mut record, text) in sources {
        ctx.check_gate("parse")?;
        ctx.emit("parse", "running", Some(&record.relative_path), None);
        let parse = if text.is_empty() {
            ParseResult {
                parser_name: "skipped".to_owned(),
                parser_version: "0".to_owned(),
                coverage: CoverageKind::NotCovered,
                metrics: FileMetrics {
                    line_count: text.lines().count() as u32,
                    ..Default::default()
                },
                ..Default::default()
            }
        } else {
            ParserRegistry::parse(&record, &text)
        };
        record.parse_capability = parse.coverage;
        ctx.processed += 1;
        parsed.push(ParsedFile {
            file: record,
            text,
            parse,
        });
    }
    {
        let mut store = storage.lock().map_err(|_| storage_poisoned())?;
        store.with_phase(|phase| {
            for parsed_file in &parsed {
                phase.upsert_file(&parsed_file.file)?;
            }
            Ok(())
        })?;
    }

    // ---- Phase 3: rules (quality + security), with per-file cache ----
    ctx.processed = 0;
    ctx.emit(
        "rules",
        "running",
        None,
        Some("正在执行代码质量与安全规则".to_owned()),
    );
    let runner = RuleRunner::with_default_rules();
    let mut all_issues: Vec<Issue> = Vec::new();
    for parsed_file in &parsed {
        ctx.check_gate("rules")?;
        ctx.emit(
            "rules",
            "running",
            Some(&parsed_file.file.relative_path),
            None,
        );
        if !parsed_file.text.is_empty() {
            let content_hash = &parsed_file.file.content_hash;
            // Existing rule cache keys were built from the first 32 hex digits.
            let cache_content_hash = content_hash.get(..32).unwrap_or(content_hash);
            let cache_key = rule_cache_key(
                &ctx.params.project_id,
                &parsed_file.file.relative_path,
                cache_content_hash,
                RULE_VERSION,
                &config_json,
            );
            let cached = storage
                .lock()
                .map_err(|_| storage_poisoned())?
                .get_cached_issues(&cache_key)?;
            let issues = match cached {
                Some(cached) => cached,
                None => {
                    let fresh = runner.run(std::slice::from_ref(parsed_file), &ctx.params.config);
                    storage
                        .lock()
                        .map_err(|_| storage_poisoned())?
                        .put_cached_issues(&cache_key, &ctx.params.project_id, &fresh)?;
                    fresh
                }
            };
            ctx.issue_count += issues.len() as u32;
            all_issues.extend(issues);
        }
        ctx.processed += 1;
    }
    {
        let mut store = storage.lock().map_err(|_| storage_poisoned())?;
        store.with_phase(|phase| persist_issues(phase, &ctx.params.run_id, &all_issues))?;
    }

    // ---- Phase 4: duplicate-code analysis ----
    ctx.emit(
        "clones",
        "running",
        None,
        Some("正在分析重复代码".to_owned()),
    );
    ctx.check_gate("clones")?;
    let clone_config = CloneConfig::from_rule_config(&ctx.params.config);
    let groups = CloneDetector::new(clone_config)
        .find_with_checkpoint(&parsed, || ctx.check_gate("clones"))?;
    let mut clone_issues: Vec<Issue> = Vec::new();
    let mut clone_edges: Vec<ArchitectureEdge> = Vec::new();
    for (group_index, group) in groups.iter().enumerate() {
        ctx.check_gate("clones")?;
        let anchor = &group.members[0];
        let kind_label = match group.kind {
            CloneKind::Exact => "完全重复",
            CloneKind::Similar => "高度相似",
        };
        let others: Vec<String> = group
            .members
            .iter()
            .skip(1)
            .map(|m| format!("{}:{}-{}", m.file_path, m.start_line, m.end_line))
            .collect();
        let draft = IssueDraft::new(
            "clone.duplicate-code",
            "duplicate",
            if group.members.len() > 2 {
                Severity::High
            } else {
                Severity::Medium
            },
            anchor.start_line,
            anchor.end_line,
        )
        .title(format!("重复代码（{kind_label}）"))
        .evidence(format!(
            "{}:{}-{} 与 {} 相似度 {:.0}%，{} 行",
            anchor.file_path,
            anchor.start_line,
            anchor.end_line,
            others.join("；"),
            group.similarity * 100.0,
            group.line_count
        ))
        .explanation(format!(
            "发现 {} 处{}代码片段（各 {} 行）：{}。归一化比较已排除重命名差异；合并前请先确认业务语义一致。",
            group.members.len(),
            kind_label,
            group.line_count,
            others.join("；")
        ))
        .impact("同一逻辑多处实现，修一处漏一处。")
        .recommendation("提取公共函数/模块；确认业务语义一致后再合并。")
        .confidence(0.8);
        clone_issues.push(crate::rules::draft_to_issue(
            &draft,
            &anchor.file_path,
            CoverageKind::Structural,
        ));
        for (i, member) in group.members.iter().enumerate().skip(1) {
            clone_edges.push(ArchitectureEdge {
                id: format!("clone-{}-{i}", group_index),
                source_path: anchor.file_path.clone(),
                target_path: member.file_path.clone(),
                kind: "clone".into(),
                confidence: group.similarity,
                coverage: CoverageKind::Structural,
                metadata: serde_json::json!({
                    "groupKind": match group.kind { CloneKind::Exact => "exact", CloneKind::Similar => "similar" },
                    "similarity": group.similarity,
                    "lineCount": group.line_count,
                    "sourceStart": anchor.start_line,
                    "targetStart": member.start_line,
                    "targetEnd": member.end_line,
                }),
            });
        }
    }
    ctx.issue_count += clone_issues.len() as u32;
    {
        let mut store = storage.lock().map_err(|_| storage_poisoned())?;
        store.with_phase(|phase| {
            persist_issues(phase, &ctx.params.run_id, &clone_issues)?;
            for edge in &clone_edges {
                phase.insert_edge(&ctx.params.run_id, edge)?;
            }
            Ok(())
        })?;
    }
    ctx.emit(
        "clones",
        "running",
        None,
        Some(format!("发现 {} 组重复代码", groups.len())),
    );

    // ---- Phase 5: architecture/dependency analysis ----
    ctx.emit(
        "graph",
        "running",
        None,
        Some("正在分析依赖关系".to_owned()),
    );
    ctx.check_gate("graph")?;
    let snapshot = crate::graph::build(&parsed);
    let graph_issues: Vec<Issue> = snapshot
        .issues()
        .iter()
        .map(|entry| crate::rules::draft_to_issue(&entry.draft, &entry.path, entry.coverage))
        .collect();
    ctx.issue_count += graph_issues.len() as u32;
    {
        let mut store = storage.lock().map_err(|_| storage_poisoned())?;
        store.with_phase(|phase| {
            persist_issues(phase, &ctx.params.run_id, &graph_issues)?;
            for edge in &snapshot.edges {
                phase.insert_edge(
                    &ctx.params.run_id,
                    &ArchitectureEdge {
                        id: serde_json::json!([edge.source, edge.target, edge.kind]).to_string(),
                        source_path: edge.source.clone(),
                        target_path: edge.target.clone(),
                        kind: edge.kind.clone(),
                        confidence: edge.confidence,
                        coverage: edge.coverage,
                        metadata: serde_json::json!({ "dynamic": edge.dynamic, "range": edge.range }),
                    },
                )?;
            }
            Ok(())
        })?;
    }
    ctx.emit(
        "graph",
        "running",
        None,
        Some(format!(
            "{} 条依赖边，{} 个循环依赖",
            snapshot.edges.len(),
            snapshot.cycles.len()
        )),
    );

    // ---- Phase 6: optional external analyzers ----
    ctx.emit(
        "external",
        "running",
        None,
        Some("正在运行可选外部分析器".to_owned()),
    );
    ctx.check_gate("external")?;
    let analyzed_paths: Vec<&str> = parsed
        .iter()
        .filter(|item| item.file.excluded_reason.is_none() && item.file.language == "Python")
        .map(|item| item.file.relative_path.as_str())
        .collect();
    let results = crate::analyzers::run_optional(
        &root,
        &ctx.params.enabled_analyzers,
        &ctx.handle.cancel,
        &analyzed_paths,
    );
    {
        let mut store = storage.lock().map_err(|_| storage_poisoned())?;
        store.with_phase(|phase| {
            let mut versions = serde_json::Map::new();
            for result in &results {
                let run: &AnalyzerRun = &result.run;
                if let Some(version) = &run.version {
                    versions.insert(
                        run.analyzer.clone(),
                        serde_json::Value::String(version.clone()),
                    );
                }
                phase.insert_analyzer_run(&ctx.params.run_id, run)?;
                persist_issues(phase, &ctx.params.run_id, &result.issues)?;
            }
            phase.set_analyzer_versions(&ctx.params.run_id, &serde_json::Value::Object(versions))
        })?;
    }
    ctx.issue_count += results
        .iter()
        .map(|result| result.issues.len() as u32)
        .sum::<u32>();
    let summary = results
        .iter()
        .map(|result| {
            format!(
                "{}: {}",
                result.run.analyzer,
                analyzer_status_label(&result.run.status)
            )
        })
        .collect::<Vec<_>>()
        .join("，");
    ctx.emit("external", "running", None, Some(summary));

    // ---- Phase 7: finalize ----
    ctx.check_gate("finalize")?;
    ctx.emit("finalize", "running", None, Some("正在汇总结果".to_owned()));
    Ok(())
}

/// Reads one discovered file once: content hash, language, decoded text.
fn index_file(
    params: &ScanParams,
    root: &Path,
    file: &DiscoveredFile,
    archive: bool,
) -> (FileRecord, String) {
    let mut record = FileRecord {
        id: format!(
            "{}:{}:{}",
            params.project_id, params.run_id, file.relative_path
        ),
        scan_run_id: params.run_id.clone(),
        project_id: params.project_id.clone(),
        relative_path: file.relative_path.clone(),
        absolute_path: if archive {
            format!("{}::{}", params.root.to_string_lossy(), file.relative_path)
        } else {
            file.absolute_path.to_string_lossy().into_owned()
        },
        encoding: "-".to_owned(),
        size_bytes: file.size_bytes,
        content_hash: String::new(),
        language: crate::index::language_for_path(Path::new(&file.relative_path)),
        parse_capability: CoverageKind::NotCovered,
        excluded_reason: None,
    };
    let text = match read_source(root, file, &mut record) {
        Ok(text) => text,
        Err(error) => {
            record.excluded_reason = Some(error.to_string());
            String::new()
        }
    };
    (record, text)
}

fn read_source(
    root: &Path,
    file: &DiscoveredFile,
    record: &mut FileRecord,
) -> Result<String, AppError> {
    let path = fs::canonicalize(&file.absolute_path)?;
    if path != file.absolute_path || !path.starts_with(root) || !fs::metadata(&path)?.is_file() {
        return Err(AppError::Validation(
            "file changed after discovery".to_owned(),
        ));
    }
    let handle = File::open(path)?;
    record.size_bytes = handle.metadata()?.len();
    if record.size_bytes > MAX_SOURCE_FILE_BYTES {
        return Err(AppError::Validation(
            "file exceeds maximum source size".to_owned(),
        ));
    }
    let mut bytes = Vec::new();
    handle
        .take(MAX_SOURCE_FILE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_SOURCE_FILE_BYTES {
        record.size_bytes = bytes.len() as u64;
        return Err(AppError::Validation(
            "file exceeds maximum source size".to_owned(),
        ));
    }
    record.size_bytes = bytes.len() as u64;
    record.content_hash = format!("{:x}", Sha256::digest(&bytes));
    let decoded = crate::index::encoding::decode_bytes(&bytes)?;
    record.encoding = decoded.encoding;
    if decoded.is_binary {
        return Err(AppError::Validation("binary file".to_owned()));
    }
    Ok(decoded.text)
}

fn excluded_record(
    run_id: &str,
    project_id: &str,
    root: &Path,
    excluded: &crate::import::path::ExcludedPath,
    archive: Option<&Path>,
) -> FileRecord {
    FileRecord {
        id: format!("{project_id}:{run_id}:{}", excluded.relative_path),
        scan_run_id: run_id.to_owned(),
        project_id: project_id.to_owned(),
        relative_path: excluded.relative_path.clone(),
        absolute_path: match archive {
            Some(path) => format!("{}::{}", path.to_string_lossy(), excluded.relative_path),
            None => root
                .join(&excluded.relative_path)
                .to_string_lossy()
                .into_owned(),
        },
        encoding: "-".to_owned(),
        size_bytes: 0,
        content_hash: String::new(),
        language: "-".to_owned(),
        parse_capability: CoverageKind::NotCovered,
        excluded_reason: Some(excluded.reason.clone()),
    }
}
