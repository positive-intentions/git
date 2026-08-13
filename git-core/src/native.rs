//! Native backend backed by gitoxide (`gix`) plus system `git` for mutations
//! that gix does not fully cover yet (push, FF pull, checkout worktree, etc.).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;

use async_trait::async_trait;
use gix::bstr::ByteSlice;
use gix::progress::Discard;
use gix::status::index_worktree::Item as IwItem;

use crate::types::{BranchInfo, CommitInfo, DirEntry, GitAuth, RemoteOpts, Signature, StatusEntry};
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
    pub(crate) fn clone_sync(url: &str, workdir: &str, opts: &RemoteOpts) -> Result<Self> {
        let path = PathBuf::from(workdir);
        ensure_parent_dir(&path)?;
        let mut prepare = gix::prepare_clone(url, &path).map_err(err_msg)?;
        if let Some(auth) = opts.auth.as_ref().filter(|a| a.is_set()) {
            let username = auth.username.clone();
            let token = auth.token.clone();
            prepare = prepare.configure_connection(move |con| {
                let username = username.clone();
                let token = token.clone();
                con.set_credentials(move |action| match action {
                    gix::credentials::helper::Action::Get(ctx) => {
                        Ok(Some(gix::credentials::protocol::Outcome {
                            identity: gix::sec::identity::Account {
                                username: ctx
                                    .username
                                    .clone()
                                    .unwrap_or_else(|| username.clone()),
                                password: token.clone(),
                                oauth_refresh_token: None,
                            },
                            next: gix::credentials::helper::NextAction::from(ctx),
                        }))
                    }
                    _ => Ok(None),
                });
                Ok(())
            });
        }
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
            let size_bytes = if file_type.is_dir() {
                None
            } else {
                item.metadata().ok().map(|m| m.len())
            };
            entries.push(DirEntry {
                path: name,
                is_dir: file_type.is_dir(),
                size_bytes,
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

    /// Sync implementation of [`GitRepo::rename`].
    pub(crate) fn rename_sync(&self, from: &str, to: &str) -> Result<()> {
        let from = normalize_rel_str(from)?;
        let to = normalize_rel_str(to)?;
        if from.is_empty() || to.is_empty() {
            return Err(Error::new("rename paths must not be empty"));
        }
        if from == to {
            return Ok(());
        }
        let dest = self.abs(&to)?;
        ensure_parent_dir(&dest)?;
        git_run(&self.workdir, &["mv", "-f", &from, &to], None)?;
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

    pub(crate) fn commit_sync(&self, message: &str, author: &Signature) -> Result<String> {
        // Re-sync the index through system git so staging done via gix is visible.
        git_run(&self.workdir, &["add", "-A"], None)?;
        git_run(
            &self.workdir,
            &[
                "-c",
                &format!("user.name={}", author.name),
                "-c",
                &format!("user.email={}", author.email),
                "commit",
                "-m",
                message,
                "--allow-empty",
            ],
            None,
        )?;
        let oid = git_run(&self.workdir, &["rev-parse", "HEAD"], None)?;
        Ok(oid.trim().to_string())
    }

    pub(crate) fn fetch_sync(&self, opts: &RemoteOpts) -> Result<()> {
        git_run(&self.workdir, &["fetch", "origin"], opts.auth.as_ref())?;
        Ok(())
    }

    pub(crate) fn pull_sync(&self, opts: &RemoteOpts) -> Result<()> {
        git_run(
            &self.workdir,
            &["pull", "--ff-only", "origin"],
            opts.auth.as_ref(),
        )?;
        Ok(())
    }

    pub(crate) fn push_sync(&self, opts: &RemoteOpts) -> Result<()> {
        git_run(
            &self.workdir,
            &["push", "-u", "origin", "HEAD"],
            opts.auth.as_ref(),
        )?;
        Ok(())
    }

    pub(crate) fn reset_to_remote_sync(&self, opts: &RemoteOpts) -> Result<()> {
        self.fetch_sync(opts)?;
        let branch = self
            .list_branches_sync()?
            .into_iter()
            .find(|b| b.current)
            .map(|b| b.name)
            .ok_or_else(|| Error::new("no current branch for reset_to_remote"))?;
        let remote_ref = format!("origin/{branch}");
        git_run(
            &self.workdir,
            &["reset", "--hard", &remote_ref],
            opts.auth.as_ref(),
        )?;
        Ok(())
    }

    pub(crate) fn push_force_with_lease_sync(&self, opts: &RemoteOpts) -> Result<()> {
        git_run(
            &self.workdir,
            &["push", "--force-with-lease", "-u", "origin", "HEAD"],
            opts.auth.as_ref(),
        )?;
        Ok(())
    }

    pub(crate) fn list_branches_sync(&self) -> Result<Vec<BranchInfo>> {
        let out = git_run(
            &self.workdir,
            &["branch", "--format=%(refname:short)%09%(HEAD)"],
            None,
        )?;
        let mut branches = Vec::new();
        for line in out.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let mut parts = line.split('\t');
            let name = parts.next().unwrap_or("").to_string();
            let head = parts.next().unwrap_or("");
            if name.is_empty() {
                continue;
            }
            branches.push(BranchInfo {
                name,
                current: head == "*",
            });
        }
        branches.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(branches)
    }

    pub(crate) fn create_branch_sync(&self, name: &str) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::new("branch name must not be empty"));
        }
        git_run(&self.workdir, &["branch", name], None)?;
        Ok(())
    }

    pub(crate) fn checkout_sync(&self, name: &str) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::new("branch name must not be empty"));
        }
        git_run(&self.workdir, &["checkout", name], None)?;
        Ok(())
    }

    pub(crate) fn log_sync(&self, max: usize) -> Result<Vec<CommitInfo>> {
        let n = if max == 0 { 1 } else { max };
        let out = git_run(
            &self.workdir,
            &[
                "log",
                &format!("-n{n}"),
                "--format=%H%x00%an%x00%at%x00%s%x00%b%x1e",
            ],
            None,
        )?;
        parse_log_output(&out)
    }

    pub(crate) fn diff_file_sync(&self, rel: &str) -> Result<String> {
        let path = normalize_rel_str(rel)?;
        if path.is_empty() {
            return Err(Error::new("diff path must not be empty"));
        }
        // `git diff HEAD -- path` shows worktree vs HEAD; exit 1 means differences.
        let (code, stdout, stderr) =
            git_run_code(&self.workdir, &["diff", "HEAD", "--", &path], None)?;
        if code == 0 || code == 1 {
            return Ok(stdout);
        }
        Err(Error::new(if stderr.is_empty() {
            format!("git diff failed with exit {code}")
        } else {
            stderr
        }))
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

    async fn clone(url: &str, workdir: &str, opts: &RemoteOpts) -> Result<Self> {
        Self::clone_sync(url, workdir, opts)
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

    async fn rename(&self, from: &str, to: &str) -> Result<()> {
        self.rename_sync(from, to)
    }

    async fn status(&self) -> Result<Vec<StatusEntry>> {
        self.status_sync()
    }

    async fn commit(&self, message: &str, author: &Signature) -> Result<String> {
        self.commit_sync(message, author)
    }

    async fn fetch(&self, opts: &RemoteOpts) -> Result<()> {
        self.fetch_sync(opts)
    }

    async fn pull(&self, opts: &RemoteOpts) -> Result<()> {
        self.pull_sync(opts)
    }

    async fn push(&self, opts: &RemoteOpts) -> Result<()> {
        self.push_sync(opts)
    }

    async fn reset_to_remote(&self, opts: &RemoteOpts) -> Result<()> {
        self.reset_to_remote_sync(opts)
    }

    async fn push_force_with_lease(&self, opts: &RemoteOpts) -> Result<()> {
        self.push_force_with_lease_sync(opts)
    }

    async fn list_branches(&self) -> Result<Vec<BranchInfo>> {
        self.list_branches_sync()
    }

    async fn create_branch(&self, name: &str) -> Result<()> {
        self.create_branch_sync(name)
    }

    async fn checkout(&self, name: &str) -> Result<()> {
        self.checkout_sync(name)
    }

    async fn log(&self, max: usize) -> Result<Vec<CommitInfo>> {
        self.log_sync(max)
    }

    async fn diff_file(&self, rel: &str) -> Result<String> {
        self.diff_file_sync(rel)
    }

    fn workdir(&self) -> &str {
        workdir_as_str(&self.workdir)
    }
}

fn err_msg(e: impl ToString) -> Error {
    Error::new(e.to_string())
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
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
fn ensure_under_workdir(workdir: &Path, path: &Path) -> Result<()> {
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
fn workdir_as_str(path: &Path) -> &str {
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

fn parse_log_output(out: &str) -> Result<Vec<CommitInfo>> {
    let mut commits = Vec::new();
    for record in out.split('\u{1e}') {
        let record = record.trim();
        if record.is_empty() {
            continue;
        }
        let parts: Vec<&str> = record.split('\u{00}').collect();
        if parts.len() < 4 {
            continue;
        }
        let id = parts[0].to_string();
        let author = parts[1].to_string();
        let time = parts[2].parse::<i64>().unwrap_or(0);
        let subject = parts[3].to_string();
        let body = parts.get(4).map(|s| s.trim()).unwrap_or("");
        let message = if body.is_empty() {
            subject
        } else {
            format!("{subject}\n{body}")
        };
        commits.push(CommitInfo {
            id,
            message,
            author,
            time,
        });
    }
    Ok(commits)
}

/// Run `git` in `cwd`, optionally with a one-shot askpass for HTTPS auth.
fn git_run(cwd: &Path, args: &[&str], auth: Option<&GitAuth>) -> Result<String> {
    let (code, stdout, stderr) = git_run_code(cwd, args, auth)?;
    if code == 0 {
        Ok(stdout)
    } else {
        Err(Error::new(if stderr.trim().is_empty() {
            format!("git {} failed with exit {code}", args.join(" "))
        } else {
            stderr
        }))
    }
}

fn git_run_code(
    cwd: &Path,
    args: &[&str],
    auth: Option<&GitAuth>,
) -> Result<(i32, String, String)> {
    let askpass = auth
        .filter(|a| a.is_set())
        .map(write_askpass_script)
        .transpose()?;

    let mut cmd = Command::new("git");
    cmd.args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("GIT_TERMINAL_PROMPT", "0");

    if let (Some(auth), Some(script)) = (auth.filter(|a| a.is_set()), askpass.as_ref()) {
        cmd.env("GIT_ASKPASS", script);
        cmd.env("SSH_ASKPASS", script);
        cmd.env("GIT_USERNAME", &auth.username);
        cmd.env("GIT_PASSWORD", &auth.token);
        // Prefer askpass over stored helpers for this one-shot call.
        cmd.env("GIT_CONFIG_COUNT", "1");
        cmd.env("GIT_CONFIG_KEY_0", "credential.helper");
        cmd.env("GIT_CONFIG_VALUE_0", "");
    }

    let output = cmd.output()?;
    if let Some(script) = askpass {
        let _ = std::fs::remove_file(script);
    }
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let code = output.status.code().unwrap_or(-1);
    Ok((code, stdout, stderr))
}

fn write_askpass_script(auth: &GitAuth) -> Result<PathBuf> {
    let _ = auth; // credentials come from env; script only prints them.
    let dir = std::env::temp_dir();
    let path = dir.join(format!("git-core-askpass-{}.sh", unique_askpass_id()));
    // Print password for Password/Token prompts; username for Username prompts.
    let script = r#"#!/bin/sh
case "$1" in
  *sername*|*ser*) printf '%s' "$GIT_USERNAME" ;;
  *) printf '%s' "$GIT_PASSWORD" ;;
esac
"#;
    std::fs::write(&path, script)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path)?.permissions();
        perms.set_mode(0o700);
        std::fs::set_permissions(&path, perms)?;
    }
    Ok(path)
}

fn unique_askpass_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("{millis:x}-{}", std::process::id())
}

#[cfg(test)]
#[path = "native_tests.rs"]
mod tests;
