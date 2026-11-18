//! Optional local tool adapters. Project code and configuration are never executed.
use crate::domain::types::{
    AnalyzerRun, AnalyzerStatus, CoverageKind, Issue, IssueFingerprint, Severity,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::atomic::AtomicBool;

pub mod discovery;
pub mod process;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzerMetadata {
    pub id: String,
    pub name: String,
    pub executable: Option<String>,
    pub safe_to_run: bool,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzerResult {
    pub run: AnalyzerRun,
    pub issues: Vec<Issue>,
}

pub fn parse_findings(tool: &str, output: &str, root: &Path) -> Vec<Issue> {
    use serde_json::Value;
    let mut findings = vec![];
    let mut add = |path: &str,
                   code: &str,
                   message: &str,
                   line: u32,
                   column: u32,
                   end_line: u32,
                   end_column: u32,
                   high: bool| {
        let Some(path) = relative_tool_path(root, path) else {
            return;
        };
        let mut issue = Issue {
            id: String::new(),
            rule_id: format!("{tool}:{code}"),
            category: "quality".into(),
            severity: if high {
                Severity::High
            } else {
                Severity::Medium
            },
            file_path: path,
            start_line: line.max(1),
            start_column: column.max(1),
            end_line: end_line.max(line).max(1),
            end_column: end_column.max(column).max(1),
            title: format!("{tool}: {code}"),
            explanation: message.chars().take(2000).collect(),
            evidence: message.chars().take(500).collect(),
            impact: "本机分析器报告的代码质量问题，需结合业务确认。".into(),
            recommendation: format!("核对 {tool} 规则 {code}，修复后执行对应验证。"),
            source: tool.into(),
            coverage: CoverageKind::Structural,
            confidence: 0.9,
            fingerprint: String::new(),
        };
        issue.fingerprint = IssueFingerprint::for_issue(&issue);
        issue.id = format!("external-{}", &issue.fingerprint[..20]);
        findings.push(issue);
    };
    let n = |v: &Value, k: &str| v[k].as_u64().unwrap_or(1) as u32;
    let s = |v: &Value, k: &str| v[k].as_str().unwrap_or("").to_owned();
    match tool {
        "ruff" => {
            if let Ok(Value::Array(items)) = serde_json::from_str(output) {
                for item in items {
                    add(
                        &s(&item, "filename"),
                        &s(&item, "code"),
                        &s(&item, "message"),
                        n(&item["location"], "row"),
                        n(&item["location"], "column"),
                        n(&item["end_location"], "row"),
                        n(&item["end_location"], "column"),
                        false,
                    );
                }
            }
        }
        "eslint" => {
            if let Ok(Value::Array(items)) = serde_json::from_str(output) {
                for item in items {
                    if let Some(messages) = item["messages"].as_array() {
                        for m in messages {
                            add(
                                &s(&item, "filePath"),
                                &s(m, "ruleId"),
                                &s(m, "message"),
                                n(m, "line"),
                                n(m, "column"),
                                n(m, "endLine"),
                                n(m, "endColumn"),
                                m["severity"] == 2,
                            );
                        }
                    }
                }
            }
        }
        "typescript" => {
            for line in output.lines() {
                if let Some((path, rest)) = line.rsplit_once('(') {
                    if let Some((position, message)) = rest.split_once("): ") {
                        if let Some((row, col)) = position.split_once(',') {
                            let code = message
                                .split_whitespace()
                                .find(|w| w.starts_with("TS"))
                                .unwrap_or("TS")
                                .trim_end_matches(':');
                            let row = row.parse().unwrap_or(1);
                            let col = col.parse().unwrap_or(1);
                            add(path, code, message, row, col, row, col, true);
                        }
                    }
                }
            }
        }
        "golangci" => {
            if let Ok(v) = serde_json::from_str::<Value>(output) {
                if let Some(items) = v["Issues"].as_array() {
                    for item in items {
                        let p = &item["Pos"];
                        add(
                            &s(p, "Filename"),
                            &s(item, "FromLinter"),
                            &s(item, "Text"),
                            n(p, "Line"),
                            n(p, "Column"),
                            n(p, "Line"),
                            n(p, "Column"),
                            false,
                        );
                    }
                }
            }
        }
        "clippy" => {
            for line in output.lines() {
                if let Ok(v) = serde_json::from_str::<Value>(line) {
                    let m = &v["message"];
                    if let Some(spans) = m["spans"].as_array() {
                        if let Some(p) = spans.iter().find(|p| p["is_primary"] == true) {
                            add(
                                &s(p, "file_name"),
                                m["code"]["code"].as_str().unwrap_or("rustc"),
                                &s(m, "message"),
                                n(p, "line_start"),
                                n(p, "column_start"),
                                n(p, "line_end"),
                                n(p, "column_end"),
                                m["level"] == "error",
                            );
                        }
                    }
                }
            }
        }
        _ => {}
    }
    let mut seen = std::collections::HashSet::new();
    findings.retain(|i| seen.insert(i.fingerprint.clone()));
    findings
}

fn relative_tool_path(root: &Path, path: &str) -> Option<String> {
    fn normalize(path: &str) -> String {
        let path = path.replace('\\', "/");
        if let Some(unc) = path.strip_prefix("//?/UNC/") {
            format!("//{unc}")
        } else {
            path.strip_prefix("//?/").unwrap_or(&path).to_owned()
        }
    }
    let normalized = normalize(path);
    let prefix = format!(
        "{}/",
        normalize(&root.to_string_lossy()).trim_end_matches('/')
    );
    let value = if normalized
        .get(..prefix.len())
        .is_some_and(|part| part == prefix || (cfg!(windows) && part.eq_ignore_ascii_case(&prefix)))
    {
        &normalized[prefix.len()..]
    } else if normalized.starts_with('/') || normalized.contains(':') {
        return None;
    } else {
        normalized.strip_prefix("./").unwrap_or(&normalized)
    };
    if value.is_empty() || value.starts_with('/') || value.split('/').any(|p| p == "..") {
        None
    } else {
        Some(value.to_owned())
    }
}

pub fn run_optional(
    root: &Path,
    enabled: &[String],
    cancel: &AtomicBool,
    python_files: &[&str],
) -> Vec<AnalyzerResult> {
    discovery::discover()
        .into_iter()
        .map(|meta| {
            let mut run = AnalyzerRun {
                id: format!("analyzer-{}", meta.id),
                analyzer: meta.id.clone(),
                status: AnalyzerStatus::Skipped,
                version: None,
                duration_ms: None,
                message: Some(meta.note.clone()),
            };
            let mut issues = vec![];
            match &meta.executable {
                None => {
                    run.status = AnalyzerStatus::Unavailable;
                    run.message = Some("未发现本机工具。".into());
                }
                Some(_) if !meta.safe_to_run => {}
                Some(_) if !enabled.contains(&meta.id) => {
                    run.message = Some("此分析器未启用。".into());
                }
                Some(exe) if executable_in_project(Path::new(exe), root) => {
                    run.message = Some("分析器位于项目目录内，已跳过。".into());
                }
                Some(_) if python_files.is_empty() => {
                    run.message = Some("没有可分析的 Python 源文件。".into());
                }
                Some(exe) => {
                    issues = run_ruff(Path::new(exe), root, cancel, python_files, &mut run);
                }
            }
            AnalyzerResult { run, issues }
        })
        .collect()
}

fn executable_in_project(executable: &Path, root: &Path) -> bool {
    let mut prefix = root.to_string_lossy().to_lowercase();
    if !prefix.ends_with(std::path::MAIN_SEPARATOR) {
        prefix.push(std::path::MAIN_SEPARATOR);
    }
    executable.starts_with(root)
        || (cfg!(windows)
            && executable
                .to_string_lossy()
                .to_lowercase()
                .starts_with(&prefix))
}

fn run_ruff(
    executable: &Path,
    root: &Path,
    cancel: &AtomicBool,
    python_files: &[&str],
    run: &mut AnalyzerRun,
) -> Vec<Issue> {
    let elapsed = std::time::Instant::now();
    if let Ok(version) = process::run(executable, &["--version".into()], root, cancel, 5)
        && version.success
    {
        run.version = version
            .stdout
            .lines()
            .next()
            .map(|line| line.chars().take(100).collect());
    }
    let result = (|| {
        let mut findings = Vec::new();
        let mut paths = python_files.iter().peekable();
        let mut scanned = 0_usize;
        while paths.peek().is_some() {
            let mut args = [
                "check",
                "--isolated",
                "--no-cache",
                "--output-format",
                "json",
                "--exit-zero",
                "--",
            ]
            .map(str::to_owned)
            .to_vec();
            let mut path_bytes = 0;
            while let Some(path) = paths.peek() {
                if path_bytes != 0 && path_bytes + path.len() > 8_000 {
                    break;
                }
                let path = paths.next().unwrap();
                let relative = path.replace('/', std::path::MAIN_SEPARATOR_STR);
                let expected = root.join(&relative);
                if expected
                    .canonicalize()
                    .is_ok_and(|actual| actual == expected && actual.is_file())
                {
                    // Pass only indexed Python files. Explicit arguments also
                    // prevent Ruff from traversing excluded directories.
                    let argument = format!(".{}{relative}", std::path::MAIN_SEPARATOR);
                    path_bytes += argument.len() + 3;
                    args.push(argument);
                    scanned += 1;
                }
            }
            if path_bytes == 0 {
                continue;
            }
            let remaining = 65_u64.saturating_sub(elapsed.elapsed().as_secs());
            if remaining == 0 {
                return Err("分析器运行超时。".to_owned());
            }
            let output = process::run(executable, &args, root, cancel, remaining)?;
            if !output.success
                || !serde_json::from_str::<serde_json::Value>(&output.stdout)
                    .is_ok_and(|value| value.is_array())
            {
                return Err("分析器退出失败或输出格式不受支持。".to_owned());
            }
            findings.extend(parse_findings("ruff", &output.stdout, root));
        }
        if scanned != python_files.len() {
            return Err("部分源文件在扫描期间发生变化。".to_owned());
        }
        Ok(findings)
    })();
    run.duration_ms = Some(elapsed.elapsed().as_millis() as u64);
    match result {
        Ok(findings) => {
            run.status = AnalyzerStatus::Executed;
            run.message = Some(format!("隔离配置扫描完成，{} 条结果。", findings.len()));
            findings
        }
        Err(error) => {
            run.status = AnalyzerStatus::Failed;
            run.message = Some(error);
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn adapters_keep_tool_codes_paths_and_locations() {
        let root = Path::new("C:/project");
        let cases = [
            (
                "ruff",
                r#"[{"code":"F401","message":"unused import","filename":"C:/project/main.py","location":{"row":3,"column":1},"end_location":{"row":3,"column":10}}]"#,
                "ruff:F401",
                3,
            ),
            (
                "eslint",
                r#"[{"filePath":"C:/project/main.js","messages":[{"ruleId":"no-unused-vars","severity":2,"message":"unused","line":4,"column":2}]}]"#,
                "eslint:no-unused-vars",
                4,
            ),
            (
                "typescript",
                "main.ts(5,2): error TS2322: Type mismatch",
                "typescript:TS2322",
                5,
            ),
            (
                "golangci",
                r#"{"Issues":[{"FromLinter":"unused","Text":"unused function","Pos":{"Filename":"C:/project/main.go","Line":6,"Column":1}}]}"#,
                "golangci:unused",
                6,
            ),
            (
                "clippy",
                r#"{"reason":"compiler-message","message":{"level":"warning","message":"needless return","code":{"code":"clippy::needless_return"},"spans":[{"file_name":"main.rs","line_start":7,"column_start":1,"line_end":7,"column_end":8,"is_primary":true}]}}"#,
                "clippy:clippy::needless_return",
                7,
            ),
        ];
        for (tool, output, code, line) in cases {
            let issues = parse_findings(tool, output, root);
            assert_eq!(issues.len(), 1, "{tool}");
            assert_eq!(issues[0].rule_id, code);
            assert_eq!(issues[0].start_line, line);
            assert!(!issues[0].file_path.contains("project"));
            assert!(!issues[0].fingerprint.is_empty());
        }
    }
    #[test]
    fn malformed_and_outside_paths_are_not_findings() {
        assert!(parse_findings("ruff", "not json", Path::new("C:/project")).is_empty());
        assert!(parse_findings("ruff", r#"[{"code":"X","message":"x","filename":"C:/outside.py","location":{"row":1,"column":1}}]"#, Path::new("C:/project")).is_empty());
    }
    #[test]
    fn disabled_and_missing_tools_remain_visible() {
        let results = run_optional(Path::new("C:/project"), &[], &AtomicBool::new(false), &[]);
        assert_eq!(results.len(), 5);
        assert!(results.iter().all(|r| matches!(
            r.run.status,
            AnalyzerStatus::Skipped | AnalyzerStatus::Unavailable
        )));
        assert!(results.iter().all(|r| r.run.message.is_some()));
    }
}
