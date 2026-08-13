use super::*;

#[test]
fn dir_entry_serde_roundtrip() {
    let entry = DirEntry {
        path: "notes".into(),
        is_dir: true,
        size_bytes: None,
    };
    let json = serde_json::to_string(&entry).expect("serialize");
    let back: DirEntry = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, entry);

    // Older payloads without size_bytes still deserialize.
    let legacy: DirEntry =
        serde_json::from_str(r#"{"path":"a.txt","is_dir":false}"#).expect("legacy");
    assert_eq!(legacy.path, "a.txt");
    assert_eq!(legacy.size_bytes, None);
}

#[test]
fn status_entry_serde_roundtrip() {
    let entry = StatusEntry {
        path: "a.txt".into(),
        status: "staged".into(),
    };
    let json = serde_json::to_string(&entry).expect("serialize");
    let back: StatusEntry = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, entry);
}

#[test]
fn git_auth_is_set_and_new() {
    let empty = GitAuth::default();
    assert!(!empty.is_set());
    let partial = GitAuth::new("u", "");
    assert!(!partial.is_set());
    let ok = GitAuth::new("user", "token");
    assert!(ok.is_set());
    assert_eq!(ok.username, "user");
    assert_eq!(ok.token, "token");
}

#[test]
fn remote_opts_builders() {
    let opts = RemoteOpts::new()
        .with_cors_proxy("https://proxy.example")
        .with_auth(GitAuth::new("u", "t"));
    assert_eq!(opts.cors_proxy.as_deref(), Some("https://proxy.example"));
    assert!(opts.auth.as_ref().map(|a| a.is_set()).unwrap_or(false));

    let cleared = RemoteOpts::new().with_cors_proxy("   ");
    assert!(cleared.cors_proxy.is_none());

    let no_auth = RemoteOpts::new().with_auth(GitAuth::new("", "t"));
    assert!(no_auth.auth.is_none());
}

#[test]
fn signature_branch_commit_types() {
    let sig = Signature::new("Ada", "ada@example.com");
    assert_eq!(sig.name, "Ada");
    assert_eq!(sig.email, "ada@example.com");

    let branch = BranchInfo {
        name: "main".into(),
        current: true,
    };
    let json = serde_json::to_string(&branch).expect("serialize");
    let back: BranchInfo = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, branch);

    let commit = CommitInfo {
        id: "abc".into(),
        message: "msg".into(),
        author: "Ada".into(),
        time: 1,
    };
    let json = serde_json::to_string(&commit).expect("serialize");
    let back: CommitInfo = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, commit);
}
