use crate::{error::AppError, state::AppState};
use reqwest::redirect::Policy;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, utils::config::BundleType};
use tauri_plugin_updater::{Update, UpdaterExt};

const RELEASE_URL: &str = "https://github.com/yuchen0x1/CodeDoctor/releases";
const MAX_PACKAGE_BYTES: u64 = 128 * 1024 * 1024;

#[derive(Default)]
pub struct UpdateState {
    running: AtomicBool,
    pending: Mutex<Option<(Update, AvailableUpdate)>>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableUpdate {
    version: String,
    notes: String,
    published_at: String,
    size_bytes: u64,
    sha256: String,
    can_install: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    received: u64,
    total: u64,
}

struct UpdateLease<'a>(&'a AtomicBool);

impl Drop for UpdateLease<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl UpdateState {
    fn begin(&self) -> Result<UpdateLease<'_>, AppError> {
        self.running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| UpdateLease(&self.running))
            .map_err(|_| invalid("已有更新操作正在进行。"))
    }
}

fn invalid(message: &str) -> AppError {
    AppError::Validation(message.to_owned())
}

fn trusted_redirects() -> Policy {
    Policy::custom(|attempt| {
        let url = attempt.url();
        if attempt.previous().len() < 5
            && url.scheme() == "https"
            && matches!(
                url.host_str(),
                Some(
                    "github.com"
                        | "release-assets.githubusercontent.com"
                        | "objects.githubusercontent.com"
                )
            )
            && url.port().is_none()
        {
            attempt.follow()
        } else {
            attempt.error(
                "The update service redirected outside the official GitHub release endpoints",
            )
        }
    })
}

fn published_windows_asset(version: &str, msi: bool) -> String {
    if msi {
        format!("CodeDoctor_{version}_x64_en-US.msi")
    } else {
        format!("CodeDoctor_{version}_x64-setup.exe")
    }
}

fn validate_metadata(
    update: &Update,
    msi: bool,
    can_install: bool,
) -> Result<AvailableUpdate, AppError> {
    let expected_url = format!(
        "{RELEASE_URL}/download/v{}/{}",
        update.version,
        published_windows_asset(&update.version, msi)
    );
    if update.download_url.as_str() != expected_url {
        return Err(invalid("更新文件来源校验失败。"));
    }
    let manifest = &update.raw_json;
    let asset = &manifest["platforms"][&update.target];
    let sha256 = asset["sha256"]
        .as_str()
        .filter(|value| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()));
    let size_bytes = asset["size"]
        .as_u64()
        .filter(|size| (1..=MAX_PACKAGE_BYTES).contains(size));
    let published_at = manifest["pub_date"]
        .as_str()
        .filter(|_| update.date.is_some());
    match (sha256, size_bytes, published_at) {
        (Some(sha256), Some(size_bytes), Some(published_at)) => Ok(AvailableUpdate {
            version: update.version.clone(),
            notes: update
                .body
                .as_deref()
                .unwrap_or_default()
                .chars()
                .take(8000)
                .collect(),
            published_at: published_at.to_owned(),
            sha256: sha256.to_ascii_lowercase(),
            size_bytes,
            can_install,
        }),
        _ => Err(invalid("更新信息缺少有效的日期、大小或校验值。")),
    }
}

#[tauri::command]
pub async fn check_for_update(app: AppHandle) -> Result<Option<AvailableUpdate>, AppError> {
    if std::env::consts::ARCH != "x86_64" {
        return Err(invalid("当前架构请通过 GitHub Releases 获取更新。"));
    }
    let session = app.state::<UpdateState>();
    let _lease = session.begin()?;
    let pending = || {
        session
            .pending
            .lock()
            .map_err(|_| invalid("更新状态不可用。"))
    };
    *pending()? = None;
    let bundle = tauri::utils::platform::bundle_type();
    let msi = matches!(bundle, Some(BundleType::Msi));
    let can_install = matches!(bundle, Some(BundleType::Msi | BundleType::Nsis));
    let updater = app
        .updater_builder()
        .target(if msi {
            "windows-x86_64-msi"
        } else {
            "windows-x86_64-nsis"
        })
        .timeout(Duration::from_secs(15))
        .version_comparator(|current, release| {
            release.version.pre.is_empty() && release.version > current
        })
        .configure_client(|builder| builder.redirect(trusted_redirects()))
        .build()
        .map_err(|_| invalid("更新配置不可用。"))?;

    let Some(update) = updater
        .check()
        .await
        .map_err(|_| invalid("暂时无法读取 GitHub 更新信息，请稍后重试。"))?
    else {
        return Ok(None);
    };
    let available = validate_metadata(&update, msi, can_install)?;
    *pending()? = Some((update, available.clone()));
    Ok(Some(available))
}

#[tauri::command]
pub async fn install_update(app: AppHandle, version: String) -> Result<(), AppError> {
    let session = app.state::<UpdateState>();
    let _lease = session.begin()?;
    let (update, available) = session
        .pending
        .lock()
        .map_err(|_| invalid("更新状态不可用。"))?
        .clone()
        .filter(|(_, available)| available.can_install && available.version == version)
        .ok_or_else(|| invalid("请重新检查可安装的更新。"))?;

    let state = app.state::<AppState>();
    drop(ensure_idle(&state)?);
    let bytes = download(&app, &update, &available).await?;
    update
        .verify(&bytes)
        .map_err(|_| invalid("更新签名或版本校验失败。"))?;

    // Keep scan startup serialized with backup and installer handoff.
    let _jobs = ensure_idle(&state)?;
    let backup_dir = app
        .path()
        .app_data_dir()
        .map_err(|_| invalid("本地数据目录不可用。"))?
        .join("update-backups");
    std::fs::create_dir_all(&backup_dir)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    state
        .storage()?
        .backup(&backup_dir.join(format!("codedoctor-before-{stamp}.db")))?;
    update
        .install(bytes)
        .map_err(|_| invalid("安装程序启动失败。当前版本仍可使用。"))
}

fn ensure_idle(
    state: &AppState,
) -> Result<
    std::sync::MutexGuard<
        '_,
        std::collections::HashMap<String, std::sync::Arc<crate::scan::JobHandle>>,
    >,
    AppError,
> {
    let jobs = state.jobs.lock().map_err(|_| invalid("扫描状态不可用。"))?;
    if !jobs.is_empty() {
        return Err(invalid("请先完成或取消扫描，再安装更新。"));
    }
    Ok(jobs)
}

async fn download(
    app: &AppHandle,
    update: &Update,
    available: &AvailableUpdate,
) -> Result<Vec<u8>, AppError> {
    let network = |_| invalid("下载中断或网络不可用。请重试。");
    let client = reqwest::Client::builder()
        .redirect(trusted_redirects())
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(180))
        .user_agent("CodeDoctor updater")
        .build()
        .map_err(network)?;
    let mut response = client
        .get(update.download_url.clone())
        .send()
        .await
        .map_err(network)?
        .error_for_status()
        .map_err(network)?;
    if response
        .content_length()
        .is_some_and(|length| length != available.size_bytes)
    {
        return Err(invalid("更新文件大小校验失败。"));
    }
    let mut bytes = Vec::with_capacity(available.size_bytes as usize);
    while let Some(chunk) = response.chunk().await.map_err(network)? {
        if (bytes.len() + chunk.len()) as u64 > available.size_bytes {
            return Err(invalid("更新文件超过声明大小。"));
        }
        bytes.extend_from_slice(&chunk);
        let _ = app.emit(
            "update://progress",
            Progress {
                received: bytes.len() as u64,
                total: available.size_bytes,
            },
        );
    }
    validate_package(&bytes, available)?;
    Ok(bytes)
}

fn validate_package(bytes: &[u8], available: &AvailableUpdate) -> Result<(), AppError> {
    if bytes.len() as u64 != available.size_bytes
        || format!("{:x}", Sha256::digest(bytes)) != available.sha256
    {
        return Err(invalid("更新文件 SHA-256 或大小校验失败。"));
    }
    Ok(())
}

#[tauri::command]
pub fn open_release_page() -> Result<(), AppError> {
    let root = std::env::var_os("SystemRoot").ok_or_else(|| invalid("Windows 目录不可用。"))?;
    std::process::Command::new(std::path::PathBuf::from(root).join("explorer.exe"))
        .arg(RELEASE_URL)
        .spawn()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_exact_length_and_hash() {
        let data = b"synthetic package";
        let entry = AvailableUpdate {
            version: "0.2.0".into(),
            notes: String::new(),
            published_at: String::new(),
            size_bytes: data.len() as u64,
            sha256: format!("{:x}", Sha256::digest(data)),
            can_install: true,
        };
        assert!(validate_package(data, &entry).is_ok());
        assert!(validate_package(b"synthetic packagE", &entry).is_err());
        assert!(validate_package(b"short", &entry).is_err());
    }
}
