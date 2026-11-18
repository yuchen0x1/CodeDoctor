//! Scan orchestration: job lifecycle, progress events, and result caching.
pub mod cache;
pub mod events;
pub mod job;

pub use events::ScanEvent;
pub use job::{JobHandle, PauseGate, ScanParams};

use crate::domain::types::ScanRun;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub(crate) fn new_run_id(sequence: u64) -> String {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("scan-{stamp}-{}-{sequence}", std::process::id())
}

pub(crate) fn is_managed_scan_id(name: &str) -> bool {
    let mut parts = name.split('-');
    parts.next() == Some("scan")
        && parts
            .next()
            .is_some_and(|part| part.parse::<u128>().is_ok())
        && parts.next().is_some_and(|part| part.parse::<u32>().is_ok())
        && parts.next().is_some_and(|part| part.parse::<u64>().is_ok())
        && parts.next().is_none()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanRunSummary {
    pub run: ScanRun,
    pub elapsed_ms: u64,
}

#[cfg(test)]
mod fixture_tests;
