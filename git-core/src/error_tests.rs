use super::*;

#[test]
fn new_display_and_from_str() {
    let e = Error::new("boom");
    assert_eq!(e.to_string(), "boom");
    assert_eq!(format!("{e}"), "boom");
    assert_eq!(format!("{e:?}"), r#"Error { message: "boom" }"#);

    let from_str: Error = "from str".into();
    assert_eq!(from_str.to_string(), "from str");

    let from_string: Error = String::from("from string").into();
    assert_eq!(from_string.to_string(), "from string");

    let as_std: &dyn std::error::Error = &e;
    assert_eq!(as_std.to_string(), "boom");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn from_io_error() {
    let io = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
    let e: Error = io.into();
    assert!(e.to_string().contains("missing"));
}
