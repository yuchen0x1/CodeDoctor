//! Bounded, shell-free process runner.
use std::{
    io::Read,
    path::Path,
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
pub struct Output {
    pub stdout: String,
    pub success: bool,
    pub duration_ms: u64,
}
pub fn run(
    executable: &Path,
    args: &[String],
    cwd: &Path,
    cancel: &AtomicBool,
    timeout_secs: u64,
) -> Result<Output, String> {
    if cancel.load(Ordering::Relaxed) {
        return Err("扫描已取消。".into());
    }
    if !executable.is_absolute() {
        return Err("分析器必须是绝对路径。".into());
    }
    let mut command = Command::new(executable);
    command
        .args(args)
        .current_dir(cwd)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for key in ["SystemRoot", "WINDIR", "TEMP", "TMP"] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let start = Instant::now();
    let mut child = command
        .spawn()
        .map_err(|_| "无法启动本机分析器。".to_owned())?;
    let exceeded = Arc::new(AtomicBool::new(false));
    fn reader(
        mut stream: impl Read + Send + 'static,
        exceeded: Arc<AtomicBool>,
    ) -> thread::JoinHandle<Vec<u8>> {
        thread::spawn(move || {
            let mut bytes = vec![];
            let mut chunk = [0; 8192];
            while let Ok(n) = stream.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                if bytes.len() + n > 4 * 1024 * 1024 {
                    exceeded.store(true, Ordering::Relaxed);
                    break;
                }
                bytes.extend_from_slice(&chunk[..n]);
            }
            bytes
        })
    }
    let stdout = reader(child.stdout.take().unwrap(), exceeded.clone());
    let stderr = reader(child.stderr.take().unwrap(), exceeded.clone());
    let result = loop {
        if cancel.load(Ordering::Relaxed)
            || start.elapsed() > Duration::from_secs(timeout_secs)
            || exceeded.load(Ordering::Relaxed)
        {
            let _ = child.kill();
            let _ = child.wait();
            break Err("分析器已取消、超时或输出超限。".to_owned());
        }
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status.success()),
            Ok(None) => thread::sleep(Duration::from_millis(30)),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err("无法读取分析器状态。".into());
            }
        }
    };
    let bytes = stdout.join().unwrap_or_default();
    let _ = stderr.join();
    Ok(Output {
        stdout: String::from_utf8_lossy(&bytes).into_owned(),
        success: result?,
        duration_ms: start.elapsed().as_millis() as u64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_and_relative_executable_fail_before_spawn() {
        assert!(
            run(
                Path::new("not-a-tool"),
                &[],
                Path::new("."),
                &AtomicBool::new(false),
                1
            )
            .is_err()
        );
        assert!(
            run(
                Path::new("C:/not-a-tool.exe"),
                &[],
                Path::new("."),
                &AtomicBool::new(true),
                1
            )
            .is_err()
        );
    }
}
