//! Shared value types for listing, status, remotes, and history.

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

/// HTTPS username + access token (or password) for private remotes.
///
/// Passed only at request time — never written into `.git/config` or remote URLs.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GitAuth {
    pub username: String,
    pub token: String,
}

impl GitAuth {
    pub fn new(username: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            token: token.into(),
        }
    }

    /// True when both username and token are non-empty after trim.
    pub fn is_set(&self) -> bool {
        !self.username.trim().is_empty() && !self.token.trim().is_empty()
    }
}

/// Options for network operations (clone / fetch / pull / push).
#[derive(Debug, Clone, Default)]
pub struct RemoteOpts {
    /// Web-only CORS proxy URL. Ignored on native.
    pub cors_proxy: Option<String>,
    /// Optional HTTPS basic auth (username + PAT/token).
    pub auth: Option<GitAuth>,
}

impl RemoteOpts {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_cors_proxy(mut self, proxy: impl Into<String>) -> Self {
        let p = proxy.into();
        self.cors_proxy = if p.trim().is_empty() { None } else { Some(p) };
        self
    }

    pub fn with_auth(mut self, auth: GitAuth) -> Self {
        self.auth = if auth.is_set() { Some(auth) } else { None };
        self
    }
}

/// Author / committer identity for commits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    pub name: String,
    pub email: String,
}

impl Signature {
    pub fn new(name: impl Into<String>, email: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            email: email.into(),
        }
    }
}

/// A local branch listing entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchInfo {
    pub name: String,
    pub current: bool,
}

/// A single commit from `log`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitInfo {
    pub id: String,
    pub message: String,
    pub author: String,
    /// Unix seconds (author time).
    pub time: i64,
}

#[cfg(test)]
#[path = "types_tests.rs"]
mod tests;
