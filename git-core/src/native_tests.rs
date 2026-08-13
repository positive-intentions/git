use super::*;
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
    assert!(nested.iter().any(|e| e.path == "a.txt" && !e.is_dir));

    let bytes = repo.read_file_sync("notes/a.txt").expect("read");
    assert_eq!(bytes, b"nested");
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

    let repo = NativeRepo::clone_sync(&url, &dest_str).expect("clone");
    assert!(dest_parent.join("README").exists());
    let list = repo.list_sync("").expect("list clone");
    assert!(list.iter().any(|e| e.path == "README"));
}

#[test]
fn clone_invalid_url_errors() {
    let dest = temp_dir();
    let dest_str = dest.path().join("out").to_string_lossy().into_owned();
    let err = NativeRepo::clone_sync("file:///no/such/repo-xyz", &dest_str)
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
        let _ = NativeRepo::open(&path).await.expect("open");
    });
}
