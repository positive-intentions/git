use super::*;

#[test]
fn dir_entry_serde_roundtrip() {
    let entry = DirEntry {
        path: "notes".into(),
        is_dir: true,
    };
    let json = serde_json::to_string(&entry).expect("serialize");
    let back: DirEntry = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, entry);
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
