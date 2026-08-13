use super::*;

#[test]
fn suggest_workdir_includes_prefix() {
    let a = suggest_workdir("git-core-demo");
    let b = suggest_workdir("git-core-demo");
    assert!(a.contains("git-core-demo"));
    assert!(b.contains("git-core-demo"));
    // Same millisecond is possible; uniqueness is best-effort for demos.
    assert!(!a.is_empty());
}
