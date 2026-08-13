//! Web backend: isomorphic-git over OPFS, reached through `git-web.js`.

use async_trait::async_trait;
use js_sys::Uint8Array;
use serde::Deserialize;
use wasm_bindgen::prelude::*;

use crate::types::{DirEntry, StatusEntry};
use crate::{Error, GitRepo, Result};

/// JS helpers installed by `assets/git-web.js` on `globalThis.GitWeb`.
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = GitWeb, js_name = init, catch)]
    async fn js_init(workdir: &str) -> std::result::Result<JsValue, JsValue>;

    #[wasm_bindgen(js_namespace = GitWeb, js_name = clone, catch)]
    async fn js_clone(
        url: &str,
        workdir: &str,
        cors_proxy: &str,
    ) -> std::result::Result<JsValue, JsValue>;

    #[wasm_bindgen(js_namespace = GitWeb, js_name = list, catch)]
    async fn js_list(workdir: &str, rel: &str) -> std::result::Result<JsValue, JsValue>;

    #[wasm_bindgen(js_namespace = GitWeb, js_name = readFile, catch)]
    async fn js_read_file(workdir: &str, rel: &str) -> std::result::Result<JsValue, JsValue>;

    #[wasm_bindgen(js_namespace = GitWeb, js_name = writeFile, catch)]
    async fn js_write_file(
        workdir: &str,
        rel: &str,
        data: &[u8],
    ) -> std::result::Result<JsValue, JsValue>;

    #[wasm_bindgen(js_namespace = GitWeb, js_name = removeFile, catch)]
    async fn js_remove_file(workdir: &str, rel: &str) -> std::result::Result<JsValue, JsValue>;

    #[wasm_bindgen(js_namespace = GitWeb, js_name = status, catch)]
    async fn js_status(workdir: &str) -> std::result::Result<JsValue, JsValue>;
}

/// Repository handle for the browser (OPFS path + isomorphic-git).
#[derive(Debug)]
pub struct WebRepo {
    workdir: String,
}

fn js_err(err: JsValue) -> Error {
    let msg = err
        .as_string()
        .or_else(|| {
            js_sys::Reflect::get(&err, &JsValue::from_str("message"))
                .ok()
                .and_then(|v| v.as_string())
        })
        .unwrap_or_else(|| format!("{err:?}"));
    Error::new(msg)
}

fn from_js<T: for<'de> Deserialize<'de>>(value: JsValue) -> Result<T> {
    serde_wasm_bindgen::from_value(value).map_err(|e| Error::new(e.to_string()))
}

#[async_trait(?Send)]
impl GitRepo for WebRepo {
    async fn init(workdir: &str) -> Result<Self> {
        js_init(workdir).await.map_err(js_err)?;
        Ok(Self {
            workdir: workdir.to_string(),
        })
    }

    async fn open(workdir: &str) -> Result<Self> {
        // OPFS path is the identity; isomorphic-git discovers `.git` on demand.
        Ok(Self {
            workdir: workdir.to_string(),
        })
    }

    async fn clone(url: &str, workdir: &str, cors_proxy: Option<&str>) -> Result<Self> {
        js_clone(url, workdir, cors_proxy.unwrap_or(""))
            .await
            .map_err(js_err)?;
        Ok(Self {
            workdir: workdir.to_string(),
        })
    }

    async fn list(&self, rel: &str) -> Result<Vec<DirEntry>> {
        let value = js_list(&self.workdir, rel).await.map_err(js_err)?;
        from_js(value)
    }

    async fn read_file(&self, rel: &str) -> Result<Vec<u8>> {
        let value = js_read_file(&self.workdir, rel).await.map_err(js_err)?;
        if let Some(arr) = value.dyn_ref::<Uint8Array>() {
            let mut buf = vec![0u8; arr.length() as usize];
            arr.copy_to(&mut buf);
            return Ok(buf);
        }
        // Fallback: JS may return a number array
        from_js::<Vec<u8>>(value)
    }

    async fn write_file(&self, rel: &str, data: &[u8]) -> Result<()> {
        js_write_file(&self.workdir, rel, data)
            .await
            .map_err(js_err)?;
        Ok(())
    }

    async fn remove_file(&self, rel: &str) -> Result<()> {
        js_remove_file(&self.workdir, rel).await.map_err(js_err)?;
        Ok(())
    }

    async fn status(&self) -> Result<Vec<StatusEntry>> {
        let value = js_status(&self.workdir).await.map_err(js_err)?;
        from_js(value)
    }

    fn workdir(&self) -> &str {
        &self.workdir
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use wasm_bindgen::JsValue;
    use wasm_bindgen_test::*;

    fn install_ok_mock() {
        js_sys::eval(
            r#"
            globalThis.GitWeb = {
              init: async (_workdir) => null,
              clone: async (_url, _workdir, _cors) => null,
              list: async (_workdir, _rel) => [{ path: "hello.txt", is_dir: false }],
              readFile: async (_workdir, _rel) => new Uint8Array([104, 105]),
              writeFile: async (_workdir, _rel, _data) => null,
              removeFile: async (_workdir, _rel) => null,
              status: async (_workdir) => [{ path: "hello.txt", status: "staged" }],
            };
            "#,
        )
        .expect("install ok mock");
    }

    fn install_err_mock() {
        js_sys::eval(
            r#"
            globalThis.GitWeb = {
              init: async () => { throw "init failed"; },
              clone: async () => { throw { message: "clone failed" }; },
              list: async () => { throw { code: 1 }; },
              readFile: async () => { throw "read failed"; },
              writeFile: async () => { throw "write failed"; },
              removeFile: async () => { throw "remove failed"; },
              status: async () => { throw "status failed"; },
            };
            "#,
        )
        .expect("install err mock");
    }

    fn install_array_read_mock() {
        js_sys::eval(
            r#"
            globalThis.GitWeb = {
              init: async () => null,
              clone: async () => null,
              list: async () => [],
              readFile: async () => [65, 66],
              writeFile: async () => null,
              removeFile: async () => null,
              status: async () => [],
            };
            "#,
        )
        .expect("install array read mock");
    }

    #[wasm_bindgen_test]
    fn js_err_string_object_and_fallback() {
        let from_str = js_err(JsValue::from_str("plain"));
        assert_eq!(from_str.to_string(), "plain");

        let obj = js_sys::Object::new();
        js_sys::Reflect::set(
            &obj,
            &JsValue::from_str("message"),
            &JsValue::from_str("obj-msg"),
        )
        .unwrap();
        assert_eq!(js_err(obj.into()).to_string(), "obj-msg");

        let number = JsValue::from_f64(42.0);
        let fallback = js_err(number);
        assert!(!fallback.to_string().is_empty());
    }

    #[wasm_bindgen_test]
    fn from_js_roundtrip_and_error() {
        let value = js_sys::eval(r#"([{ path: "a", is_dir: true }])"#).unwrap();
        let entries: Vec<DirEntry> = from_js(value).expect("from_js");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "a");
        assert!(entries[0].is_dir);

        let bad = JsValue::from_str("not-an-array");
        assert!(from_js::<Vec<DirEntry>>(bad).is_err());
    }

    #[wasm_bindgen_test]
    async fn init_open_list_status_write_remove_workdir() {
        install_ok_mock();
        let repo = WebRepo::init("/repos/demo").await.expect("init");
        assert_eq!(repo.workdir(), "/repos/demo");

        let opened = WebRepo::open("/repos/demo").await.expect("open");
        assert_eq!(opened.workdir(), "/repos/demo");

        let list = repo.list("").await.expect("list");
        assert!(list.iter().any(|e| e.path == "hello.txt"));

        let st = repo.status().await.expect("status");
        assert!(st
            .iter()
            .any(|e| e.path == "hello.txt" && e.status == "staged"));

        let bytes = repo.read_file("hello.txt").await.expect("read");
        assert_eq!(bytes, b"hi");

        repo.write_file("hello.txt", b"hi").await.expect("write");
        repo.remove_file("hello.txt").await.expect("remove");

        let cloned = WebRepo::clone(
            "https://example.com/r.git",
            "/repos/c",
            Some("https://proxy"),
        )
        .await
        .expect("clone");
        assert_eq!(cloned.workdir(), "/repos/c");

        let cloned2 = WebRepo::clone("https://example.com/r.git", "/repos/c2", None)
            .await
            .expect("clone none cors");
        assert_eq!(cloned2.workdir(), "/repos/c2");
    }

    #[wasm_bindgen_test]
    async fn read_file_number_array_fallback() {
        install_array_read_mock();
        let repo = WebRepo::open("/repos/x").await.expect("open");
        let bytes = repo.read_file("a.bin").await.expect("read");
        assert_eq!(bytes, b"AB");
    }

    #[wasm_bindgen_test]
    async fn js_errors_map_through_js_err() {
        install_err_mock();
        let err = WebRepo::init("/repos/x").await.expect_err("init");
        assert!(err.to_string().contains("init failed"));

        let err = WebRepo::clone("u", "/w", None).await.expect_err("clone");
        assert!(err.to_string().contains("clone failed"));

        let repo = WebRepo::open("/repos/x").await.expect("open");
        let err = repo.list("").await.expect_err("list");
        assert!(!err.to_string().is_empty());

        let err = repo.read_file("a").await.expect_err("read");
        assert!(err.to_string().contains("read failed"));

        let err = repo.write_file("a", b"x").await.expect_err("write");
        assert!(err.to_string().contains("write failed"));

        let err = repo.remove_file("a").await.expect_err("remove");
        assert!(err.to_string().contains("remove failed"));

        let err = repo.status().await.expect_err("status");
        assert!(err.to_string().contains("status failed"));
    }
}
