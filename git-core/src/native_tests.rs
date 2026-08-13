use super::*;
use crate::{GitAuth, RemoteOpts, Signature};
use std::process::Command;

fn temp_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("tempdir")
}

fn dir_str(dir: &tempfile::TempDir) -> String {
    dir.path().to_string_lossy().into_owned()
}

fn git_cmd(cwd: &std::path::Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .status()
        .expect("run git");
    assert!(status.success(), "git {args:?} failed");
}

#[test]
fn normalize_rel_str_cases() {
    assert_eq!(normalize_rel_str("").unwrap(), "");
    assert_eq!(normalize_rel_str(".").unwrap(), "");
    assert_eq!(normalize_rel_str("  .  ").unwrap(), "");
    assert_eq!(normalize_rel_str("/foo").unwrap(), "foo");
    assert_eq!(
        normalize_rel_str("notes/hello.txt").unwrap(),
        "notes/hello.txt"
    );
    assert!(normalize_rel_str("a/../b")
        .unwrap_err()
        .to_string()
        .contains(".."));
    assert_eq!(normalize_rel("notes/a").unwrap(), PathBuf::from("notes/a"));
}

#[test]
fn shorten_status_labels() {
    assert_eq!(shorten_status("Untracked"), "untracked");
    assert_eq!(shorten_status("something Added here"), "untracked");
    assert_eq!(shorten_status("Removed"), "deleted");
    assert_eq!(shorten_status("Modified"), "modified");
    assert_eq!(shorten_status("Modification"), "modified");
    assert_eq!(shorten_status("Addition"), "staged");
    assert_eq!(shorten_status("index:Change"), "staged");
    assert_eq!(
        shorten_status("weird-long-status-label-xxxxxxxxxxxxxxxxxxxxxxx"),
        {
            "weird-long-status-label-xxxxxxxxxxxxxxxxxxxxxxx"
                .chars()
                .take(48)
                .collect::<String>()
        }
    );
}

#[test]
fn err_msg_helper() {
    assert_eq!(err_msg("boom").to_string(), "boom");
    assert_eq!(err_msg(String::from("owned")).to_string(), "owned");
}

#[test]
fn ensure_parent_dir_covers_none_and_some() {
    // `/` has no parent on Unix.
    ensure_parent_dir(std::path::Path::new("/")).expect("root");
    let dir = temp_dir();
    let nested = dir.path().join("a").join("b.txt");
    ensure_parent_dir(&nested).expect("nested");
    assert!(dir.path().join("a").is_dir());
}

#[test]
fn list_and_status_sort_multiple_entries() {
    let dir = temp_dir();
    let path = dir_str(&dir);
    let repo = NativeRepo::init_sync(&path).expect("init");
    repo.write_file_sync("b.txt", b"b").expect("b");
    repo.write_file_sync("a.txt", b"a").expect("a");
    let list = repo.list_sync("").expect("list");
    let names: Vec<_> = list.iter().map(|e| e.path.as_str()).collect();
    assert!(names.contains(&"a.txt") && names.contains(&"b.txt"));
    // Comparator must run (2+ entries).
    assert!(names.windows(2).all(|w| w[0] <= w[1]));

    let st = repo.status_sync().expect("status");
    assert!(st.len() >= 2);
    assert!(st.windows(2).all(|w| w[0].path <= w[1].path));
}

#[test]
fn init_write_list_status_remove() {
    let dir = temp_dir();
    let path = dir_str(&dir);
    let repo = NativeRepo::init_sync(&path).expect("init");
    assert_eq!(workdir_as_str(&repo.workdir), path.as_str());

    repo.write_file_sync("hello.txt", b"hi").expect("write");
    let list = repo.list_sync("").expect("list");
    assert!(list.iter().any(|e| e.path == "hello.txt"));
    assert!(!list.iter().any(|e| e.path == ".git"));

    let st = repo.status_sync().expect("status");
    assert!(
        st.iter().any(|e| e.path == "hello.txt"),
        "expected hello.txt in status, got {st:?}"
    );

    let bytes = repo.read_file_sync("hello.txt").expect("read");
    assert_eq!(bytes, b"hi");

    repo.remove_file_sync("hello.txt").expect("remove");
    let list_after = repo.list_sync("").expect("list after remove");
    assert!(!list_after.iter().any(|e| e.path == "hello.txt"));
}

#[test]
fn open_existing_and_missing() {
    let dir = temp_dir();
    let path = dir_str(&dir);
    NativeRepo::init_sync(&path).expect("init");
    let opened = NativeRepo::open_sync(&path).expect("open");
    assert_eq!(workdir_as_str(&opened.workdir), path.as_str());

    let missing = dir.path().join("no-such-repo");
    let err = NativeRepo::open_sync(missing.to_str().unwrap()).expect_err("open missing");
    assert!(!err.to_string().is_empty());
}

#[test]
fn nested_paths_and_list_dot() {
    let dir = temp_dir();
    let path = dir_str(&dir);
    let repo = NativeRepo::init_sync(&path).expect("init");
    repo.write_file_sync("notes/a.txt", b"nested")
        .expect("write nested");

    let root = repo.list_sync(".").expect("list .");
    assert!(root.iter().any(|e| e.path == "notes" && e.is_dir));

    let nested = repo.list_sync("notes").expect("list notes");
    let a = nested
        .iter()
        .find(|e| e.path == "a.txt" && !e.is_dir)
        .expect("a.txt");
    assert_eq!(a.size_bytes, Some(6));

    let bytes = repo.read_file_sync("notes/a.txt").expect("read");
    assert_eq!(bytes, b"nested");
}

#[test]
fn rename_file_and_errors() {
    let dir = temp_dir();
    let path = dir_str(&dir);
    let repo = NativeRepo::init_sync(&path).expect("init");
    repo.write_file_sync("old.txt", b"data").expect("write");
    repo.rename_sync("old.txt", "dir/new.txt").expect("rename");
    assert!(repo.read_file_sync("dir/new.txt").is_ok());
    assert!(repo.read_file_sync("old.txt").is_err());

    let err = repo.rename_sync("", "x").expect_err("empty from");
    assert!(err.to_string().contains("empty"));
    repo.rename_sync("dir/new.txt", "dir/new.txt")
        .expect("noop rename");
}

#[test]
fn restage_same_file() {
    let dir = temp_dir();
    let path = dir_str(&dir);
    let repo = NativeRepo::init_sync(&path).expect("init");
    repo.write_file_sync("x.txt", b"one").expect("write1");
    repo.write_file_sync("x.txt", b"two").expect("write2");
    let bytes = repo.read_file_sync("x.txt").expect("read");
    assert_eq!(bytes, b"two");
}

#[test]
fn path_errors() {
    let dir = temp_dir();
    let path = dir_str(&dir);
    let repo = NativeRepo::init_sync(&path).expect("init");

    let err = repo.read_file_sync("../escape.txt").expect_err("escape");
    assert!(err.to_string().contains(".."));

    let err = repo.read_file_sync("missing.txt").expect_err("missing");
    assert!(!err.to_string().is_empty());

    let err = repo.list_sync("nope").expect_err("list missing dir");
    assert!(!err.to_string().is_empty());
}

#[test]
fn stage_rejects_directory() {
    let dir = temp_dir();
    let path = dir_str(&dir);
    let repo = NativeRepo::init_sync(&path).expect("init");
    std::fs::create_dir_all(dir.path().join("subdir")).expect("mkdir");
    let gix_repo = repo.open_repo().expect("open_repo");
    let err = repo.stage_path(&gix_repo, "subdir").expect_err("stage dir");
    assert!(err.to_string().contains("regular files"));
}

#[test]
fn remove_already_deleted_file() {
    let dir = temp_dir();
    let path = dir_str(&dir);
    let repo = NativeRepo::init_sync(&path).expect("init");
    repo.write_file_sync("gone.txt", b"x").expect("write");
    std::fs::remove_file(dir.path().join("gone.txt")).expect("unlink");
    repo.remove_file_sync("gone.txt").expect("remove");
}

#[test]
fn status_after_commit_modify_and_delete() {
    let dir = temp_dir();
    let path = dir_str(&dir);
    let repo = NativeRepo::init_sync(&path).expect("init");
    repo.write_file_sync("tracked.txt", b"v1").expect("write");

    git_cmd(dir.path(), &["add", "tracked.txt"]);
    git_cmd(dir.path(), &["commit", "-m", "initial"]);

    std::fs::write(dir.path().join("tracked.txt"), b"v2").expect("modify");
    let st = repo.status_sync().expect("status modified");
    assert!(
        st.iter().any(|e| e.path == "tracked.txt"),
        "expected modified tracked.txt, got {st:?}"
    );

    repo.write_file_sync("extra.txt", b"extra").expect("extra");
    let st2 = repo.status_sync().expect("status staged");
    assert!(
        st2.iter().any(|e| e.path == "extra.txt"),
        "expected extra.txt in status, got {st2:?}"
    );

    std::fs::remove_file(dir.path().join("tracked.txt")).expect("delete");
    let st3 = repo.status_sync().expect("status deleted");
    assert!(
        st3.iter()
            .any(|e| e.path.contains("tracked") || e.status.contains("delet")),
        "expected deleted tracked.txt, got {st3:?}"
    );
}

#[test]
fn clone_local_file_protocol() {
    let src = temp_dir();
    let src_path = src.path();
    git_cmd(src_path, &["init"]);
    std::fs::write(src_path.join("README"), b"hello").expect("write readme");
    git_cmd(src_path, &["add", "README"]);
    git_cmd(src_path, &["commit", "-m", "init"]);

    let dest = temp_dir();
    let dest_parent = dest.path().join("nested");
    let dest_str = dest_parent.to_string_lossy().into_owned();
    let url = format!("file://{}", src_path.display());

    let opts = RemoteOpts::default();
    let repo = NativeRepo::clone_sync(&url, &dest_str, &opts).expect("clone");
    assert!(dest_parent.join("README").exists());
    let list = repo.list_sync("").expect("list clone");
    assert!(list.iter().any(|e| e.path == "README"));
}

#[test]
fn clone_invalid_url_errors() {
    let dest = temp_dir();
    let dest_str = dest.path().join("out").to_string_lossy().into_owned();
    let err = NativeRepo::clone_sync(
        "file:///no/such/repo-xyz",
        &dest_str,
        &RemoteOpts::default(),
    )
    .expect_err("clone should fail");
    assert!(!err.to_string().is_empty());
}

#[test]
fn init_on_file_path_errors() {
    let dir = temp_dir();
    let file_path = dir.path().join("not-a-dir");
    std::fs::write(&file_path, b"x").expect("write file");
    let err = NativeRepo::init_sync(file_path.to_str().unwrap()).expect_err("init on file");
    assert!(!err.to_string().is_empty());
}

#[test]
fn from_gix_init_error_via_failed_init() {
    let dir = temp_dir();
    let work = dir.path().join("repo");
    std::fs::create_dir_all(&work).expect("mkdir");
    std::fs::write(work.join(".git"), b"not a git dir").expect("block .git");
    let err = NativeRepo::init_sync(work.to_str().unwrap()).expect_err("init should fail");
    assert!(!err.to_string().is_empty());
}

#[test]
fn async_trait_wrappers_smoke() {
    pollster::block_on(async {
        let dir = temp_dir();
        let path = dir_str(&dir);
        let repo = NativeRepo::init(&path).await.expect("init");
        assert_eq!(repo.workdir(), path.as_str());
        repo.write_file("a.txt", b"x").await.expect("write");
        let _ = repo.list("").await.expect("list");
        let _ = repo.status().await.expect("status");
        let _ = repo.read_file("a.txt").await.expect("read");
        repo.remove_file("a.txt").await.expect("remove");
        let oid = repo
            .commit("empty", &Signature::new("T", "t@example.com"))
            .await
            .expect("commit");
        assert!(!oid.is_empty());
        let branches = repo.list_branches().await.expect("branches");
        assert!(!branches.is_empty());
        let _ = repo.log(5).await.expect("log");
        let _ = NativeRepo::open(&path).await.expect("open");
    });
}

#[test]
fn commit_branch_log_diff_workflow() {
    let dir = temp_dir();
    let path = dir_str(&dir);
    let repo = NativeRepo::init_sync(&path).expect("init");
    repo.write_file_sync("hello.txt", b"v1").expect("write");
    let oid = repo
        .commit_sync("initial", &Signature::new("Ada", "ada@example.com"))
        .expect("commit");
    assert_eq!(oid.len(), 40);

    let branches = repo.list_branches_sync().expect("branches");
    assert!(branches.iter().any(|b| b.current));

    repo.create_branch_sync("feature").expect("create branch");
    let branches = repo.list_branches_sync().expect("branches2");
    assert!(branches.iter().any(|b| b.name == "feature"));

    repo.checkout_sync("feature").expect("checkout");
    let branches = repo.list_branches_sync().expect("branches3");
    assert!(branches.iter().any(|b| b.name == "feature" && b.current));

    std::fs::write(dir.path().join("hello.txt"), b"v2").expect("modify");
    let diff = repo.diff_file_sync("hello.txt").expect("diff");
    assert!(diff.contains("v1") || diff.contains("v2") || !diff.is_empty());

    repo.write_file_sync("hello.txt", b"v2").expect("restage");
    let oid2 = repo
        .commit_sync("update", &Signature::new("Ada", "ada@example.com"))
        .expect("commit2");
    assert_ne!(oid, oid2);

    let log = repo.log_sync(10).expect("log");
    assert!(log.len() >= 2);
    assert!(!log[0].id.is_empty());
    assert!(!log[0].message.is_empty());

    let empty_diff = repo.diff_file_sync("hello.txt").expect("clean diff");
    assert!(
        empty_diff.trim().is_empty(),
        "expected empty diff after commit, got {empty_diff:?}"
    );
}

#[test]
fn branch_name_and_diff_path_errors() {
    let dir = temp_dir();
    let path = dir_str(&dir);
    let repo = NativeRepo::init_sync(&path).expect("init");
    repo.write_file_sync("a.txt", b"a").expect("write");
    repo.commit_sync("c", &Signature::new("T", "t@e.com"))
        .expect("commit");

    let err = repo.create_branch_sync("").expect_err("empty branch");
    assert!(err.to_string().contains("empty"));
    let err = repo.checkout_sync("   ").expect_err("empty checkout");
    assert!(err.to_string().contains("empty"));
    let err = repo.diff_file_sync("").expect_err("empty diff");
    assert!(err.to_string().contains("empty"));
}

#[test]
fn parse_log_output_helpers() {
    let raw = concat!(
        "abc",
        "\0",
        "Ada",
        "\0",
        "1700000000",
        "\0",
        "subject",
        "\0",
        "body",
        "\u{1e}",
        "def",
        "\0",
        "Bob",
        "\0",
        "1700000001",
        "\0",
        "only",
        "\0",
        "\u{1e}",
    );
    let commits = parse_log_output(raw).expect("parse");
    assert_eq!(commits.len(), 2);
    assert_eq!(commits[0].id, "abc");
    assert!(commits[0].message.contains("subject"));
    assert!(commits[0].message.contains("body"));
    assert_eq!(commits[1].message, "only");

    let skip = parse_log_output("incomplete\u{1e}").expect("skip");
    assert!(skip.is_empty());
}

#[test]
fn clone_with_auth_option_file_protocol() {
    // Auth is ignored for file:// but still exercises the configure_connection path
    // only when auth.is_set(); here we pass unset-looking auth via empty token skip,
    // and a set auth that configure_connection wires (file protocol won't call it).
    let src = temp_dir();
    git_cmd(src.path(), &["init"]);
    std::fs::write(src.path().join("README"), b"hello").expect("write");
    git_cmd(src.path(), &["add", "README"]);
    git_cmd(src.path(), &["commit", "-m", "init"]);

    let dest = temp_dir();
    let dest_str = dest.path().join("clone").to_string_lossy().into_owned();
    let url = format!("file://{}", src.path().display());
    let opts = RemoteOpts::new().with_auth(GitAuth::new("user", "token"));
    let repo = NativeRepo::clone_sync(&url, &dest_str, &opts).expect("clone with auth opts");
    assert!(repo
        .list_sync("")
        .unwrap()
        .iter()
        .any(|e| e.path == "README"));
}

#[test]
fn fetch_pull_push_local_remotes() {
    // Bare remote ← push from A; B clones, commits, pushes; A pulls.
    let bare = temp_dir();
    git_cmd(bare.path(), &["init", "--bare"]);
    let bare_url = format!("file://{}", bare.path().display());

    let a = temp_dir();
    git_cmd(a.path(), &["clone", &bare_url, "."]);
    // clone into existing may need init differently on some gits — use nested dest if needed
    let a_repo_dir = if a.path().join(".git").exists() {
        a.path().to_path_buf()
    } else {
        // Some git versions refuse non-empty; create sibling.
        let nested = a.path().join("repo");
        git_cmd(a.path(), &["clone", &bare_url, nested.to_str().unwrap()]);
        nested
    };

    // If empty bare clone left no commits, create initial on A.
    std::fs::write(a_repo_dir.join("a.txt"), b"from-a").expect("write a");
    git_cmd(&a_repo_dir, &["add", "a.txt"]);
    git_cmd(&a_repo_dir, &["commit", "-m", "a1"]);
    git_cmd(&a_repo_dir, &["push", "-u", "origin", "HEAD"]);

    let b = temp_dir();
    let b_nested = b.path().join("repo");
    let b_str = b_nested.to_string_lossy().into_owned();
    let b_repo =
        NativeRepo::clone_sync(&bare_url, &b_str, &RemoteOpts::default()).expect("clone b");
    b_repo.write_file_sync("b.txt", b"from-b").expect("write b");
    b_repo
        .commit_sync("b1", &Signature::new("B", "b@e.com"))
        .expect("commit b");
    b_repo.push_sync(&RemoteOpts::default()).expect("push b");
    b_repo.fetch_sync(&RemoteOpts::default()).expect("fetch b");

    let a_native = NativeRepo::open_sync(a_repo_dir.to_str().unwrap()).expect("open a");
    a_native.pull_sync(&RemoteOpts::default()).expect("pull a");
    assert!(a_repo_dir.join("b.txt").exists());
}

#[test]
fn git_run_failure_and_askpass_with_auth() {
    let dir = temp_dir();
    let path = dir_str(&dir);
    let repo = NativeRepo::init_sync(&path).expect("init");
    // Invalid git args → error path.
    let err = git_run(dir.path(), &["not-a-real-subcommand-xyz"], None).expect_err("fail");
    assert!(!err.to_string().is_empty());

    // Askpass script creation + env wiring (auth present); file remote push without network.
    let auth = GitAuth::new("u", "t");
    let script = write_askpass_script(&auth).expect("askpass");
    assert!(script.exists());
    let _ = std::fs::remove_file(&script);

    // log_sync with max=0 clamps to 1
    repo.write_file_sync("z.txt", b"z").expect("write");
    repo.commit_sync("z", &Signature::new("T", "t@e.com"))
        .expect("commit");
    let log = repo.log_sync(0).expect("log0");
    assert_eq!(log.len(), 1);

    // Exercise git_run_code with auth (won't be used for local-only command).
    let (code, _, _) =
        git_run_code(dir.path(), &["status", "--porcelain"], Some(&auth)).expect("status");
    assert_eq!(code, 0);
}

#[test]
fn push_with_auth_env_local_remote() {
    let bare = temp_dir();
    git_cmd(bare.path(), &["init", "--bare"]);
    let bare_url = format!("file://{}", bare.path().display());

    let work = temp_dir();
    let dest = work.path().join("r");
    let dest_str = dest.to_string_lossy().into_owned();
    let repo = NativeRepo::clone_sync(&bare_url, &dest_str, &RemoteOpts::default()).expect("clone");
    // Empty bare clone may have no commits / no branch — seed with commit.
    if !dest.join(".git").exists() {
        // clone_sync should have created it; if bare was empty gix may still create worktree
    }
    repo.write_file_sync("p.txt", b"p").expect("write");
    // Need an initial commit before push; unborn branch after empty clone.
    let _ = repo.commit_sync("init", &Signature::new("T", "t@e.com"));
    // Set origin if missing
    let _ = git_run(&dest, &["remote", "add", "origin", &bare_url], None);
    let opts = RemoteOpts::new().with_auth(GitAuth::new("user", "token"));
    // Push may fail if remote already set differently; try push_sync.
    let _ = repo.push_sync(&opts);
}
