use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum JobEvent {
    Queued { files: usize },
    Started { path: PathBuf },
    Stage { path: PathBuf, stage: String },
    Finished { path: PathBuf, output: PathBuf, in_bytes: u64, out_bytes: u64, encoder: String, trace: Vec<(String, u64)>, #[serde(default, skip_serializing_if = "Option::is_none")] undo: Option<crate::undo::Undo>,
        /// Something about the result worth telling the user (e.g. why a PNG got no colour palette).
        #[serde(default, skip_serializing_if = "Option::is_none")] note: Option<String> },
    Skipped { path: PathBuf, reason: String },
    Failed { path: PathBuf, error: String },
    Totals { processed: usize, skipped: usize, failed: usize, in_bytes: u64, out_bytes: u64 },
}
