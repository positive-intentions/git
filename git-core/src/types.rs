//! Shared value types for listing and status.

use serde::{Deserialize, Serialize};

/// A single directory listing entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirEntry {
    pub path: String,
    pub is_dir: bool,
}

/// A single status line (path + short status label).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusEntry {
    pub path: String,
    pub status: String,
}

#[cfg(test)]
#[path = "types_tests.rs"]
mod tests;
