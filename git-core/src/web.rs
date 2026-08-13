//! Web backend: isomorphic-git over OPFS, reached through `git-web.js`.

use async_trait::async_trait;
use js_sys::Uint8Array;
use serde::Deserialize;
use wasm_bindgen::prelude::*;

use crate::types::{BranchInfo, CommitInfo, DirEntry, RemoteOpts, Signature, StatusEntry};
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
        username: &str,
        token: &str,
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

    #[wasm_bindgen(js_namespace = GitWeb, js_name = rename, catch)]
    async fn js_rename(
        workdir: &str,
        from: &str,
        to: &str,
    ) -> std::result::Result<JsValue, JsValue>;

    #[wasm_bindgen(js_namespace = GitWeb, js_name = status, catch)]
    async fn js_status(workdir: &str) -> std::result::Result<JsValue, JsValue>;

    #[wasm_bindgen(js_namespace = GitWeb, js_name = commit, catch)]
    async fn js_commit(
        workdir: &str,
        message: &str,
        name: &str,
        email: &str,
    ) -> std::result::Result<JsValue, JsValue>;

    #[wasm_bindgen(js_namespace = GitWeb, js_name = fetch, catch)]
    async fn js_fetch(
        workdir: &str,
        cors_proxy: &str,
        username: &str,
        token: &str,
    ) -> std::result::Result<JsValue, JsValue>;

    #[wasm_bindgen(js_namespace = GitWeb, js_name = pull, catch)]
    async fn js_pull(
        workdir: &str,
        cors_proxy: &str,
        username: &str,
        token: &str,
    ) -> std::result::Result<JsValue, JsValue>;

    #[wasm_bindgen(js_namespace = GitWeb, js_name = push, catch)]
    async fn js_push(
        workdir: &str,
        cors_proxy: &str,
        username: &str,
        token: &str,
    ) -> std::result::Result<JsValue, JsValue>;

    #[wasm_bindgen(js_namespace = GitWeb, js_name = resetToRemote, catch)]
    async fn js_reset_to_remote(
        workdir: &str,
        cors_proxy: &str,
        username: &str,
        token: &str,
    ) -> std::result::Result<JsValue, JsValue>;

    #[wasm_bindgen(js_namespace = GitWeb, js_name = pushForceWithLease, catch)]
    async fn js_push_force_with_lease(
        workdir: &str,
        cors_proxy: &str,
        username: &str,
        token: &str,
    ) -> std::result::Result<JsValue, JsValue>;

    #[wasm_bindgen(js_namespace = GitWeb, js_name = listBranches, catch)]
    async fn js_list_branches(workdir: &str) -> std::result::Result<JsValue, JsValue>;

    #[wasm_bindgen(js_namespace = GitWeb, js_name = createBranch, catch)]
    async fn js_create_branch(workdir: &str, name: &str) -> std::result::Result<JsValue, JsValue>;

    #[wasm_bindgen(js_namespace = GitWeb, js_name = checkout, catch)]
    async fn js_checkout(workdir: &str, name: &str) -> std::result::Result<JsValue, JsValue>;

    #[wasm_bindgen(js_namespace = GitWeb, js_name = log, catch)]
    async fn js_log(workdir: &str, max: u32) -> std::result::Result<JsValue, JsValue>;

    #[wasm_bindgen(js_namespace = GitWeb, js_name = diffFile, catch)]
    async fn js_diff_file(workdir: &str, rel: &str) -> std::result::Result<JsValue, JsValue>;
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

fn auth_parts(opts: &RemoteOpts) -> (String, String, String) {
    let cors = opts.cors_proxy.clone().unwrap_or_default();
    let (user, token) = match &opts.auth {
        Some(a) if a.is_set() => (a.username.clone(), a.token.clone()),
        _ => (String::new(), String::new()),
    };
    (cors, user, token)
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

    async fn clone(url: &str, workdir: &str, opts: &RemoteOpts) -> Result<Self> {
        let (cors, user, token) = auth_parts(opts);
        js_clone(url, workdir, &cors, &user, &token)
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

    async fn rename(&self, from: &str, to: &str) -> Result<()> {
        js_rename(&self.workdir, from, to).await.map_err(js_err)?;
        Ok(())
    }

    async fn status(&self) -> Result<Vec<StatusEntry>> {
        let value = js_status(&self.workdir).await.map_err(js_err)?;
        from_js(value)
    }

    async fn commit(&self, message: &str, author: &Signature) -> Result<String> {
        let value = js_commit(&self.workdir, message, &author.name, &author.email)
            .await
            .map_err(js_err)?;
        value
            .as_string()
            .ok_or_else(|| Error::new("commit did not return a sha string"))
    }

    async fn fetch(&self, opts: &RemoteOpts) -> Result<()> {
        let (cors, user, token) = auth_parts(opts);
        js_fetch(&self.workdir, &cors, &user, &token)
            .await
            .map_err(js_err)?;
        Ok(())
    }

    async fn pull(&self, opts: &RemoteOpts) -> Result<()> {
        let (cors, user, token) = auth_parts(opts);
        js_pull(&self.workdir, &cors, &user, &token)
            .await
            .map_err(js_err)?;
        Ok(())
    }

    async fn push(&self, opts: &RemoteOpts) -> Result<()> {
        let (cors, user, token) = auth_parts(opts);
        js_push(&self.workdir, &cors, &user, &token)
            .await
            .map_err(js_err)?;
        Ok(())
    }

    async fn reset_to_remote(&self, opts: &RemoteOpts) -> Result<()> {
        let (cors, user, token) = auth_parts(opts);
        js_reset_to_remote(&self.workdir, &cors, &user, &token)
            .await
            .map_err(js_err)?;
        Ok(())
    }

    async fn push_force_with_lease(&self, opts: &RemoteOpts) -> Result<()> {
        let (cors, user, token) = auth_parts(opts);
        js_push_force_with_lease(&self.workdir, &cors, &user, &token)
            .await
            .map_err(js_err)?;
        Ok(())
    }

    async fn list_branches(&self) -> Result<Vec<BranchInfo>> {
        let value = js_list_branches(&self.workdir).await.map_err(js_err)?;
        from_js(value)
    }

    async fn create_branch(&self, name: &str) -> Result<()> {
        js_create_branch(&self.workdir, name)
            .await
            .map_err(js_err)?;
        Ok(())
    }

    async fn checkout(&self, name: &str) -> Result<()> {
        js_checkout(&self.workdir, name).await.map_err(js_err)?;
        Ok(())
    }

    async fn log(&self, max: usize) -> Result<Vec<CommitInfo>> {
        let n = u32::try_from(max).unwrap_or(u32::MAX);
        let value = js_log(&self.workdir, n).await.map_err(js_err)?;
        from_js(value)
    }

    async fn diff_file(&self, rel: &str) -> Result<String> {
        let value = js_diff_file(&self.workdir, rel).await.map_err(js_err)?;
        value
            .as_string()
            .ok_or_else(|| Error::new("diffFile did not return a string"))
    }

    fn workdir(&self) -> &str {
        &self.workdir
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use crate::GitAuth;
    use wasm_bindgen::JsValue;
    use wasm_bindgen_test::*;

    fn install_ok_mock() {
        js_sys::eval(
            r#"
            globalThis.GitWeb = {
              init: async (_workdir) => null,
              clone: async (_url, _workdir, _cors, _u, _t) => null,
              list: async (_workdir, _rel) => [{ path: "hello.txt", is_dir: false }],
              readFile: async (_workdir, _rel) => new Uint8Array([104, 105]),
              writeFile: async (_workdir, _rel, _data) => null,
              removeFile: async (_workdir, _rel) => null,
              rename: async (_workdir, _from, _to) => null,
              status: async (_workdir) => [{ path: "hello.txt", status: "staged" }],
              commit: async (_workdir, _msg, _name, _email) => "abc123",
              fetch: async (_workdir, _cors, _u, _t) => null,
              pull: async (_workdir, _cors, _u, _t) => null,
              push: async (_workdir, _cors, _u, _t) => null,
              resetToRemote: async (_workdir, _cors, _u, _t) => null,
              pushForceWithLease: async (_workdir, _cors, _u, _t) => null,
              listBranches: async (_workdir) => [{ name: "main", current: true }],
              createBranch: async (_workdir, _name) => null,
              checkout: async (_workdir, _name) => null,
              log: async (_workdir, _max) => [{
                id: "abc123", message: "hi", author: "Ada", time: 1
              }],
              diffFile: async (_workdir, _rel) => "diff --git a/x b/x\n",
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
              rename: async () => { throw "rename failed"; },
              status: async () => { throw "status failed"; },
              commit: async () => { throw "commit failed"; },
              fetch: async () => { throw "fetch failed"; },
              pull: async () => { throw "pull failed"; },
              push: async () => { throw "push failed"; },
              resetToRemote: async () => { throw "reset failed"; },
              pushForceWithLease: async () => { throw "force push failed"; },
              listBranches: async () => { throw "branches failed"; },
              createBranch: async () => { throw "create failed"; },
              checkout: async () => { throw "checkout failed"; },
              log: async () => { throw "log failed"; },
              diffFile: async () => { throw "diff failed"; },
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
              rename: async () => null,
              status: async () => [],
              commit: async () => "x",
              fetch: async () => null,
              pull: async () => null,
              push: async () => null,
              resetToRemote: async () => null,
              pushForceWithLease: async () => null,
              listBranches: async () => [],
              createBranch: async () => null,
              checkout: async () => null,
              log: async () => [],
              diffFile: async () => "",
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
    fn auth_parts_helpers() {
        let empty = auth_parts(&RemoteOpts::default());
        assert_eq!(empty, (String::new(), String::new(), String::new()));

        let opts = RemoteOpts::new()
            .with_cors_proxy("https://proxy")
            .with_auth(GitAuth::new("u", "t"));
        let (cors, user, token) = auth_parts(&opts);
        assert_eq!(cors, "https://proxy");
        assert_eq!(user, "u");
        assert_eq!(token, "t");
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
        repo.rename("hello.txt", "hi.txt").await.expect("rename");
        repo.remove_file("hello.txt").await.expect("remove");

        let oid = repo
            .commit("msg", &Signature::new("Ada", "a@e.com"))
            .await
            .expect("commit");
        assert_eq!(oid, "abc123");

        let branches = repo.list_branches().await.expect("branches");
        assert!(branches.iter().any(|b| b.current));

        repo.create_branch("feat").await.expect("create");
        repo.checkout("feat").await.expect("checkout");

        let log = repo.log(5).await.expect("log");
        assert_eq!(log[0].id, "abc123");

        let diff = repo.diff_file("hello.txt").await.expect("diff");
        assert!(diff.contains("diff"));

        let opts = RemoteOpts::new().with_auth(GitAuth::new("u", "t"));
        repo.fetch(&opts).await.expect("fetch");
        repo.pull(&opts).await.expect("pull");
        repo.push(&opts).await.expect("push");
        repo.reset_to_remote(&opts).await.expect("reset");
        repo.push_force_with_lease(&opts).await.expect("force push");

        let cloned = WebRepo::clone(
            "https://example.com/r.git",
            "/repos/c",
            &RemoteOpts::new().with_cors_proxy("https://proxy"),
        )
        .await
        .expect("clone");
        assert_eq!(cloned.workdir(), "/repos/c");

        let cloned2 = WebRepo::clone(
            "https://example.com/r.git",
            "/repos/c2",
            &RemoteOpts::default(),
        )
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

        let err = WebRepo::clone("u", "/w", &RemoteOpts::default())
            .await
            .expect_err("clone");
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

        let err = repo.rename("a", "b").await.expect_err("rename");
        assert!(err.to_string().contains("rename failed"));

        let err = repo.status().await.expect_err("status");
        assert!(err.to_string().contains("status failed"));

        let err = repo
            .commit("m", &Signature::new("a", "a@e.com"))
            .await
            .expect_err("commit");
        assert!(err.to_string().contains("commit failed"));

        let err = repo.fetch(&RemoteOpts::default()).await.expect_err("fetch");
        assert!(err.to_string().contains("fetch failed"));

        let err = repo.pull(&RemoteOpts::default()).await.expect_err("pull");
        assert!(err.to_string().contains("pull failed"));

        let err = repo.push(&RemoteOpts::default()).await.expect_err("push");
        assert!(err.to_string().contains("push failed"));

        let err = repo
            .reset_to_remote(&RemoteOpts::default())
            .await
            .expect_err("reset");
        assert!(err.to_string().contains("reset failed"));

        let err = repo
            .push_force_with_lease(&RemoteOpts::default())
            .await
            .expect_err("force push");
        assert!(err.to_string().contains("force push failed"));

        let err = repo.list_branches().await.expect_err("branches");
        assert!(err.to_string().contains("branches failed"));

        let err = repo.create_branch("x").await.expect_err("create");
        assert!(err.to_string().contains("create failed"));

        let err = repo.checkout("x").await.expect_err("checkout");
        assert!(err.to_string().contains("checkout failed"));

        let err = repo.log(1).await.expect_err("log");
        assert!(err.to_string().contains("log failed"));

        let err = repo.diff_file("a").await.expect_err("diff");
        assert!(err.to_string().contains("diff failed"));
    }
}
