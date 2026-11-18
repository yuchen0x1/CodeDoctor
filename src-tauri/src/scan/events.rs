//! Real scan progress events. The frontend only renders what the backend
//! emits — there is no synthetic progress anywhere.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanEvent {
    pub run_id: String,
    /// index | parse | rules | clones | graph | external | finalize | done
    pub stage: String,
    pub current_path: Option<String>,
    pub processed_files: u32,
    pub total_files: u32,
    pub issue_count: u32,
    pub elapsed_ms: u64,
    /// running | paused | canceling | canceled | completed | failed
    pub status: String,
    pub message: Option<String>,
}

impl ScanEvent {
    pub fn new(run_id: &str, stage: &str, status: &str) -> Self {
        Self {
            run_id: run_id.to_owned(),
            stage: stage.to_owned(),
            current_path: None,
            processed_files: 0,
            total_files: 0,
            issue_count: 0,
            elapsed_ms: 0,
            status: status.to_owned(),
            message: None,
        }
    }

    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }
}

pub type EventSink = Box<dyn Fn(&ScanEvent) + Send + Sync>;
