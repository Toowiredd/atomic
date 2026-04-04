//! Import utilities
//!
//! This module provides import functionality for various sources.

pub mod conversations;
pub mod log_ingest;
pub mod obsidian;

pub use conversations::{ImportedConversation, ImportedMessage};
pub use log_ingest::{IngestLogRequest, LogFormat, PreparedLogAtom};

use serde::{Deserialize, Serialize};

/// Result of an import operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub imported: i32,
    pub skipped: i32,
    pub errors: i32,
    pub tags_created: i32,
    pub tags_linked: i32,
}

/// Progress event payload for import operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportProgress {
    pub current: i32,
    pub total: i32,
    pub current_file: String,
    pub status: String,
}
