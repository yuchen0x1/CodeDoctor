use codedoctor_lib::commands::project::import_project_state;
use codedoctor_lib::commands::scan::{get_scan_state, start_scan_state};
use codedoctor_lib::domain::types::{ScanStatus, Severity};
use codedoctor_lib::report::{ReportFormat, render_with_score};
use codedoctor_lib::score::{ScanSnapshot, calculate_score};
use codedoctor_lib::state::AppState;
use serde::de::DeserializeOwned;
use std::error::Error;
use std::io::{Write, stdout};
use std::process::ExitCode;
use std::sync::mpsc;

const USAGE: &str = "CodeDoctor CLI\n\nUsage:\n  codedoctor-cli scan <directory-or-zip> [--format json|sarif|markdown|txt|html] [--fail-on critical|high|medium|low|info]\n  codedoctor-cli --version\n\nReports are written to stdout. Exit codes: 0 success, 1 error, 2 findings at or above the selected severity.";

fn main() -> ExitCode {
    let args = std::env::args_os()
        .skip(1)
        .map(|value| value.into_string().map_err(|_| "arguments must be UTF-8"))
        .collect::<Result<Vec<_>, _>>();
    match args.map_err(Into::into).and_then(run) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("CodeDoctor: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Vec<String>) -> Result<u8, Box<dyn Error>> {
    let mut args = args.into_iter();
    match args.next().as_deref() {
        None | Some("--help" | "-h") => println!("{USAGE}"),
        Some("--version") if args.len() == 0 => {
            println!("CodeDoctor CLI {}", env!("CARGO_PKG_VERSION"))
        }
        Some("scan") => {
            let path = args
                .next()
                .filter(|value| !value.starts_with("--"))
                .ok_or("scan needs a directory or ZIP path. See --help.")?;
            let mut format = None;
            let mut threshold = None;
            while let Some(flag) = args.next() {
                let value = args
                    .next()
                    .ok_or_else(|| format!("missing value for {flag}"))?;
                match flag.as_str() {
                    "--format" if format.is_none() => format = Some(parse_option(&flag, value)?),
                    "--fail-on" if threshold.is_none() => {
                        threshold = Some(parse_option(&flag, value)?)
                    }
                    _ => return Err(format!("unknown or repeated option {flag}").into()),
                }
            }
            return scan(path, format.unwrap_or(ReportFormat::Json), threshold);
        }
        _ => return Err("unknown command. See --help.".into()),
    }
    Ok(0)
}

fn parse_option<T: DeserializeOwned>(flag: &str, value: String) -> Result<T, Box<dyn Error>> {
    serde_json::from_value(serde_json::Value::String(value))
        .map_err(|_| format!("invalid value for {flag}. See --help.").into())
}

fn scan(
    path: String,
    format: ReportFormat,
    threshold: Option<Severity>,
) -> Result<u8, Box<dyn Error>> {
    let state = AppState::in_memory()?;
    let project = import_project_state(&state, path)?;
    let (sender, receiver) = mpsc::channel();
    let initial = start_scan_state(
        &state,
        project.project_id,
        None,
        None,
        Box::new(move |event| {
            if event.stage == "done" {
                let _ = sender.send(event.message.clone());
            }
        }),
    )?;
    let message = receiver
        .recv()?
        .unwrap_or_else(|| "scan did not complete".to_owned());
    let final_run = get_scan_state(&state, &initial.run.id)?.run;
    if final_run.status != ScanStatus::Completed {
        return Err(message.into());
    }

    let snapshot = ScanSnapshot::load(&*state.storage()?, final_run)?;
    let report = render_with_score(&snapshot, &calculate_score(&snapshot), format)?;
    let mut output = stdout().lock();
    output.write_all(report.as_bytes())?;
    if !report.ends_with('\n') {
        output.write_all(b"\n")?;
    }
    output.flush()?;
    let failed = threshold.is_some_and(|minimum| {
        snapshot
            .issues
            .iter()
            .any(|issue| severity_rank(issue.severity) >= severity_rank(minimum))
    });
    Ok(if failed { 2 } else { 0 })
}

fn severity_rank(severity: Severity) -> u8 {
    match severity {
        Severity::Critical => 4,
        Severity::High => 3,
        Severity::Medium => 2,
        Severity::Low => 1,
        Severity::Info => 0,
    }
}
