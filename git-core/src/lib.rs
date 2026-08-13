//! Shared Git API for gallery demos.
//!
//! Desktop uses gitoxide (`gix`). Web uses isomorphic-git over OPFS via JS.

#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

mod error;
mod types;

pub use error::{Error, Result};
pub use types::{DirEntry, StatusEntry};

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
    /// On web, `cors_proxy` is required for cross-origin remotes (e.g.
    /// `https://cors.isomorphic-git.org`). Ignored on native.
    async fn clone(url: &str, workdir: &str, cors_proxy: Option<&str>) -> Result<Self>;

    /// List files and directories under `rel` (relative to the worktree root).
    /// Use `""` or `"."` for the root.
    async fn list(&self, rel: &str) -> Result<Vec<DirEntry>>;

    /// Read a file relative to the worktree root.
    async fn read_file(&self, rel: &str) -> Result<Vec<u8>>;

    /// Write (create/overwrite) a file and stage it (`git add`).
    async fn write_file(&self, rel: &str, data: &[u8]) -> Result<()>;

    /// Remove a file from the worktree and index (`git rm`).
    async fn remove_file(&self, rel: &str) -> Result<()>;

    /// Working tree / index status entries.
    async fn status(&self) -> Result<Vec<StatusEntry>>;

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
