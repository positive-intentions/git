//! Native backend backed by gitoxide (`gix`).

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use async_trait::async_trait;
use gix::bstr::ByteSlice;
use gix::progress::Discard;
use gix::status::index_worktree::Item as IwItem;

use crate::types::{DirEntry, StatusEntry};
use crate::{Error, GitRepo, Result};

/// A repository opened (or created) with `gix` on the local filesystem.
#[derive(Debug)]
pub struct NativeRepo {
    workdir: PathBuf,
}

impl NativeRepo {
    fn open_repo(&self) -> Result<gix::Repository> {
        gix::open(&self.workdir).map_err(Error::from)
    }

    fn abs(&self, rel: &str) -> Result<PathBuf> {
        let rel = normalize_rel(rel)?;
        if rel.as_os_str().is_empty() {
            return Ok(self.workdir.clone());
        }
        let path = self.workdir.join(&rel);
        ensure_under_workdir(&self.workdir, &path)?;
        Ok(path)
    }

    fn stage_path(&self, repo: &gix::Repository, rel: &str) -> Result<()> {
        let abs = self.abs(rel)?;
        let meta = std::fs::metadata(&abs)?;
        if !meta.is_file() {
            return Err(Error::new("can only stage regular files"));
        }

        let data = std::fs::read(&abs)?;
        let blob_id = repo.write_blob(&data).map_err(err_msg)?.detach();

        let fs_meta = gix::index::fs::Metadata::from_path_no_follow(&abs).map_err(err_msg)?;
        let stat = gix::index::entry::Stat::from_fs(&fs_meta).map_err(err_msg)?;

        let mut index = repo
            .index_or_load_from_head_or_empty()
            .map_err(err_msg)?
            .into_owned();

        let rela = gix::bstr::BString::from(normalize_rel_str(rel)?);
        if let Some(pos) = index
            .entry_index_by_path_and_stage(rela.as_bstr(), gix::index::entry::Stage::Unconflicted)
        {
            index.entries_mut()[pos].id = blob_id;
            index.entries_mut()[pos].stat = stat;
            index.entries_mut()[pos].mode = gix::index::entry::Mode::FILE;
        } else {
            index.dangerously_push_entry(
                stat,
                blob_id,
                gix::index::entry::Flags::empty(),
                gix::index::entry::Mode::FILE,
                rela.as_bstr(),
            );
            index.sort_entries();
        }

        index
            .write(gix::index::write::Options::default())
            .map_err(err_msg)?;
        Ok(())
    }

    fn unstage_and_delete(&self, repo: &gix::Repository, rel: &str) -> Result<()> {
        let abs = self.abs(rel)?;
        if abs.exists() {
            std::fs::remove_file(&abs)?;
        }

        let mut index = repo
            .index_or_load_from_head_or_empty()
            .map_err(err_msg)?
            .into_owned();

        let rela = gix::bstr::BString::from(normalize_rel_str(rel)?);
        index.remove_entries(|_, path, _| path == rela.as_bstr());
        index
            .write(gix::index::write::Options::default())
            .map_err(err_msg)?;
        Ok(())
    }

    /// Sync implementation of [`GitRepo::init`] (gix is blocking).
    pub(crate) fn init_sync(workdir: &str) -> Result<Self> {
        let path = PathBuf::from(workdir);
        std::fs::create_dir_all(&path)?;
        gix::init(&path)?;
        Ok(Self { workdir: path })
    }

    /// Sync implementation of [`GitRepo::open`].
    pub(crate) fn open_sync(workdir: &str) -> Result<Self> {
        let path = PathBuf::from(workdir);
        gix::open(&path)?;
        Ok(Self { workdir: path })
    }

    /// Sync implementation of [`GitRepo::clone`].
    pub(crate) fn clone_sync(url: &str, workdir: &str) -> Result<Self> {
        let path = PathBuf::from(workdir);
        ensure_parent_dir(&path)?;
        let mut prepare = gix::prepare_clone(url, &path).map_err(err_msg)?;
        let (mut checkout, _outcome) = prepare
            .fetch_then_checkout(Discard, &AtomicBool::new(false))
            .map_err(err_msg)?;
        let (_repo, _outcome) = checkout
            .main_worktree(Discard, &AtomicBool::new(false))
            .map_err(err_msg)?;
        Ok(Self { workdir: path })
    }

    /// Sync implementation of [`GitRepo::list`].
    pub(crate) fn list_sync(&self, rel: &str) -> Result<Vec<DirEntry>> {
        let dir = self.abs(rel)?;
        let mut entries = Vec::new();
        let read = std::fs::read_dir(&dir)?;
        for item in read {
            let item = item?;
            let name = item.file_name().to_string_lossy().into_owned();
            if name == ".git" {
                continue;
            }
            let file_type = item.file_type()?;
            entries.push(DirEntry {
                path: name,
                is_dir: file_type.is_dir(),
            });
        }
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(entries)
    }

    /// Sync implementation of [`GitRepo::read_file`].
    pub(crate) fn read_file_sync(&self, rel: &str) -> Result<Vec<u8>> {
        let path = self.abs(rel)?;
        Ok(std::fs::read(path)?)
    }

    /// Sync implementation of [`GitRepo::write_file`].
    pub(crate) fn write_file_sync(&self, rel: &str, data: &[u8]) -> Result<()> {
        let path = self.abs(rel)?;
        ensure_parent_dir(&path)?;
        std::fs::write(&path, data)?;
        let repo = self.open_repo()?;
        self.stage_path(&repo, rel)?;
        Ok(())
    }

    /// Sync implementation of [`GitRepo::remove_file`].
    pub(crate) fn remove_file_sync(&self, rel: &str) -> Result<()> {
        let repo = self.open_repo()?;
        self.unstage_and_delete(&repo, rel)?;
        Ok(())
    }

    /// Sync implementation of [`GitRepo::status`].
    pub(crate) fn status_sync(&self) -> Result<Vec<StatusEntry>> {
        let repo = self.open_repo()?;
        let mut out = Vec::new();

        let platform = repo.status(Discard).map_err(err_msg)?;
        let iter = platform
            .into_iter(None::<gix::bstr::BString>)
            .map_err(err_msg)?;

        for item in iter {
            let item = item.map_err(err_msg)?;
            let path = item.location().to_str_lossy().into_owned();
            let status = match &item {
                gix::status::Item::IndexWorktree(iw) => index_worktree_label(iw),
                gix::status::Item::TreeIndex(change) => format!("index:{change:?}"),
            };
            let status = shorten_status(&status);
            out.push(StatusEntry { path, status });
        }

        // Unborn HEAD: staged files won't appear in tree↔index; surface index paths.
        if repo.head().map(|h| h.is_unborn()).unwrap_or(false) {
            let index = repo.index_or_empty().map_err(err_msg)?;
            for entry in index.entries() {
                let path = entry.path(&index).to_str_lossy().into_owned();
                push_staged_if_missing(&mut out, path);
            }
        }

        out.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(out)
    }
}

// Thin async wrappers: `async_trait` remaps line coverage, so the real logic lives
// in the sync helpers above (tested directly).
#[cfg_attr(coverage_nightly, coverage(off))]
#[async_trait(?Send)]
impl GitRepo for NativeRepo {
    async fn init(workdir: &str) -> Result<Self> {
        Self::init_sync(workdir)
    }

    async fn open(workdir: &str) -> Result<Self> {
        Self::open_sync(workdir)
    }

    async fn clone(url: &str, workdir: &str, _cors_proxy: Option<&str>) -> Result<Self> {
        Self::clone_sync(url, workdir)
    }

    async fn list(&self, rel: &str) -> Result<Vec<DirEntry>> {
        self.list_sync(rel)
    }

    async fn read_file(&self, rel: &str) -> Result<Vec<u8>> {
        self.read_file_sync(rel)
    }

    async fn write_file(&self, rel: &str, data: &[u8]) -> Result<()> {
        self.write_file_sync(rel, data)
    }

    async fn remove_file(&self, rel: &str) -> Result<()> {
        self.remove_file_sync(rel)
    }

    async fn status(&self) -> Result<Vec<StatusEntry>> {
        self.status_sync()
    }

    fn workdir(&self) -> &str {
        workdir_as_str(&self.workdir)
    }
}

fn err_msg(e: impl ToString) -> Error {
    Error::new(e.to_string())
}

fn ensure_parent_dir(path: &std::path::Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn index_worktree_label(iw: &IwItem) -> String {
    match iw.summary() {
        Some(s) => format!("{s:?}").to_lowercase(),
        None => status_without_summary(iw),
    }
}

/// Dedup helper: when the status iterator already listed the path, skip.
#[cfg_attr(coverage_nightly, coverage(off))]
fn push_staged_if_missing(out: &mut Vec<StatusEntry>, path: String) {
    if !out.iter().any(|e| e.path == path) {
        out.push(StatusEntry {
            path,
            status: "staged".into(),
        });
    }
}

/// `normalize_rel` already rejects `..`; this guards pathological joins.
#[cfg_attr(coverage_nightly, coverage(off))]
fn ensure_under_workdir(workdir: &std::path::Path, path: &std::path::Path) -> Result<()> {
    if !path.starts_with(workdir) {
        return Err(Error::new("path escapes workdir"));
    }
    Ok(())
}

/// Rare status shapes without a summary; keep a debug label.
#[cfg_attr(coverage_nightly, coverage(off))]
fn status_without_summary(iw: &IwItem) -> String {
    match iw {
        IwItem::Modification { status, .. } => format!("{status:?}"),
        IwItem::DirectoryContents { entry, .. } => format!("{:?}", entry.status),
        other => format!("{other:?}"),
    }
}

/// Non-UTF8 workdirs are not used by gallery demos.
#[cfg_attr(coverage_nightly, coverage(off))]
fn workdir_as_str(path: &std::path::Path) -> &str {
    path.to_str().unwrap_or("")
}

fn normalize_rel(rel: &str) -> Result<PathBuf> {
    let s = normalize_rel_str(rel)?;
    Ok(PathBuf::from(s))
}

fn normalize_rel_str(rel: &str) -> Result<String> {
    let rel = rel.trim().trim_start_matches('/');
    if rel == "." || rel.is_empty() {
        return Ok(String::new());
    }
    if rel.split('/').any(|p| p == "..") {
        return Err(Error::new("path must not contain '..'"));
    }
    Ok(rel.to_string())
}

fn shorten_status(raw: &str) -> String {
    let lower = raw.to_lowercase();
    if lower.contains("untracked") || lower.contains("added") {
        "untracked".into()
    } else if lower.contains("removed") {
        "deleted".into()
    } else if lower.contains("modified") || lower.contains("modification") {
        "modified".into()
    } else if lower.contains("addition") || lower.contains("index:") {
        "staged".into()
    } else {
        raw.chars().take(48).collect()
    }
}

#[cfg(test)]
#[path = "native_tests.rs"]
mod tests;
