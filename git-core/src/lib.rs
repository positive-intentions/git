//! Shared Git API for gallery demos.
//!
//! Desktop uses gitoxide (`gix`) plus system `git` for push / some mutations.
//! Web uses isomorphic-git over OPFS via JS.

#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

mod error;
mod types;

pub use error::{Error, Result};
pub use types::{BranchInfo, CommitInfo, DirEntry, GitAuth, RemoteOpts, Signature, StatusEntry};

use async_trait::async_trait;

/// Platform-agnostic Git operations used by gallery stories.
#[async_trait(?Send)]
pub trait GitRepo: Sized {
    /// Initialize a new repository at `workdir`.
    async fn init(workdir: &str) -> Result<Self>;

    /// Open an existing repository at `workdir` (no network).
    async fn open(workdir: &str) -> Result<Self>;

    /// Clone `url` into `workdir`.
    ///
    /// On web, `opts.cors_proxy` is required for cross-origin remotes (e.g.
    /// `https://cors.isomorphic-git.org`). Ignored on native.
    /// Credentials in `opts.auth` are used only for this request.
    async fn clone(url: &str, workdir: &str, opts: &RemoteOpts) -> Result<Self>;

    /// List files and directories under `rel` (relative to the worktree root).
    /// Use `""` or `"."` for the root.
    async fn list(&self, rel: &str) -> Result<Vec<DirEntry>>;

    /// Read a file relative to the worktree root.
    async fn read_file(&self, rel: &str) -> Result<Vec<u8>>;

    /// Write (create/overwrite) a file and stage it (`git add`).
    async fn write_file(&self, rel: &str, data: &[u8]) -> Result<()>;

    /// Remove a file from the worktree and index (`git rm`).
    async fn remove_file(&self, rel: &str) -> Result<()>;

    /// Rename / move a file or directory in the worktree and stage the change.
    async fn rename(&self, from: &str, to: &str) -> Result<()>;

    /// Working tree / index status entries.
    async fn status(&self) -> Result<Vec<StatusEntry>>;

    /// Commit the index with `message` and `author` (also used as committer).
    /// Returns the new commit object id (hex).
    async fn commit(&self, message: &str, author: &Signature) -> Result<String>;

    /// Fetch from `origin` (fast network update of remotes).
    async fn fetch(&self, opts: &RemoteOpts) -> Result<()>;

    /// Fast-forward pull from `origin` into the current branch.
    /// Fails if a merge would be required.
    async fn pull(&self, opts: &RemoteOpts) -> Result<()>;

    /// Push the current branch to `origin`.
    async fn push(&self, opts: &RemoteOpts) -> Result<()>;

    /// Fetch `origin` and hard-reset the current branch to `origin/<branch>`.
    /// Used when resolving sync conflicts by accepting the remote version.
    async fn reset_to_remote(&self, opts: &RemoteOpts) -> Result<()>;

    /// Force-push the current branch with lease (`--force-with-lease`).
    /// Used when resolving sync conflicts by keeping the local version.
    async fn push_force_with_lease(&self, opts: &RemoteOpts) -> Result<()>;

    /// List local branches.
    async fn list_branches(&self) -> Result<Vec<BranchInfo>>;

    /// Create a new local branch at HEAD (does not switch).
    async fn create_branch(&self, name: &str) -> Result<()>;

    /// Check out an existing local branch (updates worktree).
    async fn checkout(&self, name: &str) -> Result<()>;

    /// Recent commits (newest first), up to `max` entries.
    async fn log(&self, max: usize) -> Result<Vec<CommitInfo>>;

    /// Unified diff of worktree file `rel` vs HEAD (empty string if unchanged / new).
    async fn diff_file(&self, rel: &str) -> Result<String>;

    /// Absolute (or OPFS) workdir path for this repository.
    fn workdir(&self) -> &str;
}

/// Suggest a fresh workdir path for demos.
pub fn suggest_workdir(prefix: &str) -> String {
    let id = unique_id();
    #[cfg(not(target_arch = "wasm32"))]
    {
        let base = std::env::temp_dir();
        base.join(format!("{prefix}-{id}"))
            .to_string_lossy()
            .into_owned()
    }
    #[cfg(target_arch = "wasm32")]
    {
        format!("/repos/{prefix}-{id}")
    }
}

fn unique_id() -> String {
    // `SystemTime::now` panics on wasm32-unknown-unknown.
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        format!("{millis:x}")
    }
    #[cfg(target_arch = "wasm32")]
    {
        let millis = js_sys::Date::now() as u64;
        format!("{millis:x}")
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub mod native;

#[cfg(target_arch = "wasm32")]
pub mod web;

/// Concrete repo type for the current platform.
#[cfg(not(target_arch = "wasm32"))]
pub type Repo = native::NativeRepo;

#[cfg(target_arch = "wasm32")]
pub type Repo = web::WebRepo;

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
