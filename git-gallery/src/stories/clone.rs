//! Clone story: private-repo connect form + full GitRepo debugger UI.
//! Monaco editor is web-only; desktop uses MonoBlock for file contents.

use dioxus::prelude::*;
use git_core::{
    suggest_workdir, BranchInfo, CommitInfo, DirEntry, GitAuth, GitRepo, RemoteOpts, Repo,
    Signature, StatusEntry,
};
use whatsup_ui::components::atoms::BodyText;

use crate::chrome::{
    ActionRow, DemoShell, FormField, MonoBlock, PrimaryButton, SecondaryButton, StatusBanner,
    StatusList, StatusMsg,
};

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
const MONACO_DOM_ID: &str = "git-gallery-monaco";
const STORAGE_PREFIX_NOTE: &str = "Credentials stay in this browser (localStorage).";

whatsup_ui::register_gui_story! {
    name: "Clone",
    group: "Git",
    docs: include_str!("clone.md"),
    knobs: [],
    render: |_k| {
        rsx! { CloneDemo {} }
    },
}

#[derive(Clone, PartialEq)]
struct TreeNode {
    /// Path relative to worktree root.
    path: String,
    is_dir: bool,
    /// Display name (last segment).
    name: String,
}

#[component]
fn CloneDemo() -> Element {
    let mut url = use_signal(|| "https://github.com/octocat/Hello-World".to_string());
    let mut username = use_signal(String::new);
    let mut token = use_signal(String::new);
    let mut cors_proxy = use_signal(|| "https://cors.isomorphic-git.org".to_string());
    let mut author_name = use_signal(|| "git-gallery".to_string());
    let mut author_email = use_signal(|| "gallery@local".to_string());
    let mut commit_message = use_signal(|| "Update from git-gallery".to_string());
    let mut new_branch = use_signal(String::new);

    let mut status = use_signal(|| {
        StatusMsg::info(format!(
            "Connect to a remote (public or private). {STORAGE_PREFIX_NOTE}"
        ))
    });
    let mut repo_path = use_signal(|| None::<String>);
    let mut listing = use_signal(Vec::<TreeNode>::new);
    let mut tree_prefix = use_signal(String::new);
    let mut status_entries = use_signal(Vec::<StatusEntry>::new);
    let mut branches = use_signal(Vec::<BranchInfo>::new);
    let mut commits = use_signal(Vec::<CommitInfo>::new);
    let mut open_path = use_signal(|| None::<String>);
    let mut file_text = use_signal(String::new);
    let mut diff_text = use_signal(String::new);
    let mut busy = use_signal(|| false);
    #[cfg_attr(not(target_arch = "wasm32"), allow(unused_mut))]
    let mut monaco_ready = use_signal(|| false);

    // Load persisted connection fields (web).
    use_effect(move || {
        #[cfg(target_arch = "wasm32")]
        {
            spawn(async move {
                if let Some(saved) = storage_load().await {
                    if let Some(u) = saved.url {
                        url.set(u);
                    }
                    if let Some(u) = saved.username {
                        username.set(u);
                    }
                    if let Some(t) = saved.token {
                        token.set(t);
                    }
                    if let Some(c) = saved.cors_proxy {
                        cors_proxy.set(c);
                    }
                }
            });
        }
    });

    // Persist on change (web).
    use_effect(move || {
        let snapshot = SavedConnect {
            url: Some(url()),
            username: Some(username()),
            token: Some(token()),
            cors_proxy: Some(cors_proxy()),
        };
        #[cfg(target_arch = "wasm32")]
        {
            spawn(async move {
                storage_save(&snapshot).await;
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = snapshot;
        }
    });

    let remote_opts = move || {
        let mut opts = RemoteOpts::new().with_cors_proxy(cors_proxy());
        let auth = GitAuth::new(username(), token());
        if auth.is_set() {
            opts = opts.with_auth(auth);
        }
        opts
    };

    let refresh_tree = move |path: String| {
        if busy() {
            return;
        }
        let Some(p) = repo_path() else {
            status.set(StatusMsg::err("Clone a repository first."));
            return;
        };
        busy.set(true);
        spawn(async move {
            match Repo::open(&p).await {
                Ok(repo) => match repo.list(&path).await {
                    Ok(entries) => {
                        listing.set(to_tree_nodes(&path, entries));
                        tree_prefix.set(path);
                        status.set(StatusMsg::ok("Listed directory".to_string()));
                    }
                    Err(e) => status.set(StatusMsg::err(e.to_string())),
                },
                Err(e) => status.set(StatusMsg::err(e.to_string())),
            }
            busy.set(false);
        });
    };

    let on_clone = move |_| {
        if busy() {
            return;
        }
        let remote = url();
        let opts = remote_opts();
        busy.set(true);
        status.set(StatusMsg::info(format!("Cloning {remote}…")));
        spawn(async move {
            let p = suggest_workdir("git-gallery-clone");
            match Repo::clone(&remote, &p, &opts).await {
                Ok(repo) => {
                    repo_path.set(Some(repo.workdir().to_string()));
                    listing.set(Vec::new());
                    open_path.set(None);
                    file_text.set(String::new());
                    diff_text.set(String::new());
                    status.set(StatusMsg::ok(format!("Cloned into {}", repo.workdir())));
                    match repo.list("").await {
                        Ok(entries) => {
                            let nodes = to_tree_nodes("", entries);
                            listing.set(nodes.clone());
                            tree_prefix.set(String::new());
                            match pick_default_file(&nodes) {
                                Some(rel) => {
                                    match repo.read_file(&rel).await {
                                        Ok(bytes) => {
                                            let text =
                                                String::from_utf8_lossy(&bytes).into_owned();
                                            open_path.set(Some(rel.clone()));
                                            file_text.set(text.clone());
                                            let lang = guess_lang(&rel);
                                            if let Err(e) =
                                                set_editor_value(&text, &lang).await
                                            {
                                                status.set(StatusMsg::err(format!(
                                                    "Cloned, but Monaco failed: {e}"
                                                )));
                                            } else {
                                                match repo.diff_file(&rel).await {
                                                    Ok(d) => diff_text.set(d),
                                                    Err(_) => diff_text.set(String::new()),
                                                }
                                                status.set(StatusMsg::ok(format!(
                                                    "Cloned into {}; opened {rel}",
                                                    repo.workdir()
                                                )));
                                            }
                                        }
                                        Err(e) => status.set(StatusMsg::err(format!(
                                            "Cloned, but could not open {rel}: {e}"
                                        ))),
                                    }
                                }
                                None => {
                                    let _ = set_editor_value(
                                        "// Open a file from the tree to edit.\n",
                                        "plaintext",
                                    )
                                    .await;
                                    status.set(StatusMsg::ok(format!(
                                        "Cloned into {}; open a file from the tree to edit",
                                        repo.workdir()
                                    )));
                                }
                            }
                        }
                        Err(e) => status.set(StatusMsg::err(format!(
                            "Cloned, but list failed: {e}"
                        ))),
                    }
                }
                Err(e) => status.set(StatusMsg::err(format!(
                    "{e} (private remotes need username+token; web may need a CORS proxy that forwards Authorization)"
                ))),
            }
            busy.set(false);
        });
    };

    let on_list_root = {
        let mut refresh_tree = refresh_tree.clone();
        move |_| refresh_tree(String::new())
    };

    let on_status = move |_| {
        if busy() {
            return;
        }
        let Some(p) = repo_path() else {
            status.set(StatusMsg::err("Clone a repository first."));
            return;
        };
        busy.set(true);
        spawn(async move {
            match Repo::open(&p).await {
                Ok(repo) => match repo.status().await {
                    Ok(entries) => {
                        status_entries.set(entries);
                        status.set(StatusMsg::ok("Status refreshed".to_string()));
                    }
                    Err(e) => status.set(StatusMsg::err(e.to_string())),
                },
                Err(e) => status.set(StatusMsg::err(e.to_string())),
            }
            busy.set(false);
        });
    };

    let on_commit = move |_| {
        if busy() {
            return;
        }
        let Some(p) = repo_path() else {
            status.set(StatusMsg::err("Clone a repository first."));
            return;
        };
        let msg = commit_message();
        let sig = Signature::new(author_name(), author_email());
        busy.set(true);
        spawn(async move {
            match Repo::open(&p).await {
                Ok(repo) => match repo.commit(&msg, &sig).await {
                    Ok(oid) => {
                        status.set(StatusMsg::ok(format!("Committed {oid}")));
                        if let Ok(entries) = repo.status().await {
                            status_entries.set(entries);
                        }
                        if let Ok(log) = repo.log(20).await {
                            commits.set(log);
                        }
                    }
                    Err(e) => status.set(StatusMsg::err(e.to_string())),
                },
                Err(e) => status.set(StatusMsg::err(e.to_string())),
            }
            busy.set(false);
        });
    };

    let on_fetch = move |_| {
        run_remote_op(busy, status, repo_path, remote_opts(), RemoteOp::Fetch);
    };
    let on_pull = move |_| {
        run_remote_op(busy, status, repo_path, remote_opts(), RemoteOp::Pull);
    };
    let on_push = move |_| {
        run_remote_op(busy, status, repo_path, remote_opts(), RemoteOp::Push);
    };

    let on_refresh_branches = move |_| {
        if busy() {
            return;
        }
        let Some(p) = repo_path() else {
            status.set(StatusMsg::err("Clone a repository first."));
            return;
        };
        busy.set(true);
        spawn(async move {
            match Repo::open(&p).await {
                Ok(repo) => match repo.list_branches().await {
                    Ok(list) => {
                        branches.set(list);
                        status.set(StatusMsg::ok("Branches refreshed".to_string()));
                    }
                    Err(e) => status.set(StatusMsg::err(e.to_string())),
                },
                Err(e) => status.set(StatusMsg::err(e.to_string())),
            }
            busy.set(false);
        });
    };

    let on_create_branch = move |_| {
        if busy() {
            return;
        }
        let Some(p) = repo_path() else {
            status.set(StatusMsg::err("Clone a repository first."));
            return;
        };
        let name = new_branch();
        busy.set(true);
        spawn(async move {
            match Repo::open(&p).await {
                Ok(repo) => match repo.create_branch(&name).await {
                    Ok(()) => {
                        status.set(StatusMsg::ok(format!("Created branch {name}")));
                        if let Ok(list) = repo.list_branches().await {
                            branches.set(list);
                        }
                    }
                    Err(e) => status.set(StatusMsg::err(e.to_string())),
                },
                Err(e) => status.set(StatusMsg::err(e.to_string())),
            }
            busy.set(false);
        });
    };

    let on_log = move |_| {
        if busy() {
            return;
        }
        let Some(p) = repo_path() else {
            status.set(StatusMsg::err("Clone a repository first."));
            return;
        };
        busy.set(true);
        spawn(async move {
            match Repo::open(&p).await {
                Ok(repo) => match repo.log(20).await {
                    Ok(list) => {
                        commits.set(list);
                        status.set(StatusMsg::ok("Log refreshed".to_string()));
                    }
                    Err(e) => status.set(StatusMsg::err(e.to_string())),
                },
                Err(e) => status.set(StatusMsg::err(e.to_string())),
            }
            busy.set(false);
        });
    };

    let on_save_file = move |_| {
        if busy() {
            return;
        }
        let Some(p) = repo_path() else {
            status.set(StatusMsg::err("Clone a repository first."));
            return;
        };
        let Some(rel) = open_path() else {
            status.set(StatusMsg::err("Open a file first."));
            return;
        };
        busy.set(true);
        spawn(async move {
            let text = read_editor_text(file_text()).await;
            match Repo::open(&p).await {
                Ok(repo) => match repo.write_file(&rel, text.as_bytes()).await {
                    Ok(()) => {
                        file_text.set(text);
                        status.set(StatusMsg::ok(format!("Saved + staged {rel}")));
                        if let Ok(diff) = repo.diff_file(&rel).await {
                            diff_text.set(diff);
                        }
                        if let Ok(entries) = repo.status().await {
                            status_entries.set(entries);
                        }
                    }
                    Err(e) => status.set(StatusMsg::err(e.to_string())),
                },
                Err(e) => status.set(StatusMsg::err(e.to_string())),
            }
            busy.set(false);
        });
    };

    // Fallback mount if onmounted has not run yet (web).
    use_effect(move || {
        #[cfg(target_arch = "wasm32")]
        {
            if !monaco_ready() {
                spawn(async move {
                    match monaco_mount(MONACO_DOM_ID).await {
                        Ok(()) => monaco_ready.set(true),
                        Err(e) => {
                            status.set(StatusMsg::err(format!("Monaco mount failed: {e}")));
                        }
                    }
                });
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = monaco_ready;
        }
    });

    rsx! {
        DemoShell { title: "Clone".to_string(),
            StatusBanner { status: status() }

            BodyText { "Remote connection" }
            // Inline flex: gallery Tailwind does not include sm:grid-cols-2.
            div { style: "display: flex; flex-wrap: wrap; gap: 0.5rem;",
                div { style: "flex: 1 1 220px;",
                    FormField {
                        label: "URL".to_string(),
                        value: url(),
                        input_type: "text".to_string(),
                        oninput: move |v| url.set(v),
                    }
                }
                div { style: "flex: 1 1 160px;",
                    FormField {
                        label: "Username".to_string(),
                        value: username(),
                        input_type: "text".to_string(),
                        oninput: move |v| username.set(v),
                    }
                }
                div { style: "flex: 1 1 160px;",
                    FormField {
                        label: "Access token".to_string(),
                        value: token(),
                        input_type: "password".to_string(),
                        oninput: move |v| token.set(v),
                    }
                }
                div { style: "flex: 1 1 220px;",
                    FormField {
                        label: "CORS proxy (web)".to_string(),
                        value: cors_proxy(),
                        input_type: "text".to_string(),
                        oninput: move |v| cors_proxy.set(v),
                    }
                }
            }

            ActionRow {
                PrimaryButton {
                    label: if busy() { "Working…".to_string() } else { "Clone".to_string() },
                    onclick: on_clone
                }
                SecondaryButton { label: "List root".to_string(), onclick: on_list_root }
                SecondaryButton { label: "Status".to_string(), onclick: on_status }
                SecondaryButton { label: "Fetch".to_string(), onclick: on_fetch }
                SecondaryButton { label: "Pull (FF)".to_string(), onclick: on_pull }
                SecondaryButton { label: "Push".to_string(), onclick: on_push }
            }

            if let Some(p) = repo_path() {
                BodyText { "Workdir" }
                MonoBlock { text: p }
            }

            // IDE workspace: file tree (left) + editor (right).
            // Inline CSS grid — gallery Tailwind does not ship lg:grid-cols-2.
            // Status/Diff/Commit live *below* this box so they cannot overlap Monaco.
            BodyText { "Workspace" }
            if repo_path().is_none() {
                BodyText { "Clone a repository to browse files and edit them here." }
            }
            div {
                style: "display: grid; grid-template-columns: minmax(200px, 260px) minmax(0, 1fr); gap: 0.75rem; align-items: start; min-height: 360px; overflow: visible; border: 1px solid #e2e8f0; border-radius: 0.5rem; padding: 0.75rem; background: #fff; box-sizing: border-box; margin-bottom: 1.25rem;",
                // File tree panel
                div {
                    style: "display: flex; flex-direction: column; gap: 0.5rem; min-width: 0; max-height: 360px; overflow: hidden; border-right: 1px solid #e2e8f0; padding-right: 0.75rem;",
                    BodyText { "Files" }
                    if !tree_prefix().is_empty() {
                        {
                            let parent = parent_path(&tree_prefix());
                            let mut refresh_tree = refresh_tree.clone();
                            rsx! {
                                SecondaryButton {
                                    label: format!("Up ({})", if parent.is_empty() { "/".into() } else { parent.clone() }),
                                    onclick: move |_| refresh_tree(parent.clone())
                                }
                            }
                        }
                    }
                    div {
                        style: "flex: 1 1 auto; min-height: 0; max-height: 320px; overflow: auto;",
                        TreeList {
                            entries: listing(),
                            on_open_dir: {
                                let mut refresh_tree = refresh_tree.clone();
                                move |path: String| refresh_tree(path)
                            },
                            on_open_file: move |path: String| {
                                if busy() { return; }
                                let Some(p) = repo_path() else { return; };
                                busy.set(true);
                                spawn(async move {
                                    match Repo::open(&p).await {
                                        Ok(repo) => {
                                            match repo.read_file(&path).await {
                                                Ok(bytes) => {
                                                    let text = String::from_utf8_lossy(&bytes).into_owned();
                                                    open_path.set(Some(path.clone()));
                                                    file_text.set(text.clone());
                                                    let lang = guess_lang(&path);
                                                    if let Err(e) = set_editor_value(&text, &lang).await {
                                                        status.set(StatusMsg::err(format!(
                                                            "Opened {path} but Monaco failed: {e}"
                                                        )));
                                                    } else {
                                                        match repo.diff_file(&path).await {
                                                            Ok(d) => diff_text.set(d),
                                                            Err(e) => status.set(StatusMsg::err(e.to_string())),
                                                        }
                                                        status.set(StatusMsg::ok(format!("Opened {path}")));
                                                    }
                                                }
                                                Err(e) => status.set(StatusMsg::err(e.to_string())),
                                            }
                                        }
                                        Err(e) => status.set(StatusMsg::err(e.to_string())),
                                    }
                                    busy.set(false);
                                });
                            },
                        }
                    }
                }

                // Editor panel — Monaco uses a hard 288px height (not flex-sized).
                div {
                    style: "display: flex; flex-direction: column; gap: 0.5rem; min-width: 0;",
                    BodyText {
                        {
                            match open_path() {
                                Some(p) => format!("Editor - {p}"),
                                None => "Editor".to_string(),
                            }
                        }
                    }
                    ActionRow {
                        SecondaryButton { label: "Save + stage".to_string(), onclick: on_save_file }
                    }
                    if open_path().is_none() {
                        BodyText { "Click a file in the tree to edit." }
                    }
                    {
                        #[cfg(target_arch = "wasm32")]
                        {
                            rsx! {
                                div {
                                    id: MONACO_DOM_ID,
                                    style: "height: 288px; min-height: 288px; max-height: 288px; width: 100%; display: block; position: relative; overflow: hidden; border: 1px solid #e2e8f0; border-radius: 0.5rem; box-sizing: border-box; flex: none;",
                                    onmounted: move |_| {
                                        spawn(async move {
                                            match monaco_mount(MONACO_DOM_ID).await {
                                                Ok(()) => monaco_ready.set(true),
                                                Err(e) => status.set(StatusMsg::err(format!(
                                                    "Monaco mount failed: {e}"
                                                ))),
                                            }
                                        });
                                    },
                                }
                            }
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            rsx! {
                                textarea {
                                    style: "height: 288px; min-height: 288px; width: 100%; box-sizing: border-box; border: 1px solid #e2e8f0; border-radius: 0.5rem; padding: 0.75rem; font-family: ui-monospace, monospace; font-size: 0.75rem; background: #f8fafc;",
                                    value: file_text(),
                                    oninput: move |e| file_text.set(e.value()),
                                }
                            }
                        }
                    }
                }
            }

            // Below workspace — separate block so nothing overlaps the IDE panes.
            div { style: "display: block; clear: both; margin-top: 0.5rem;",
                BodyText { "Git status" }
                StatusList { entries: status_entries() }
            }
            div { style: "display: block; margin-top: 1rem;",
                BodyText { "Diff (vs HEAD)" }
                MonoBlock { text: if diff_text().is_empty() { "(no diff)".to_string() } else { diff_text() } }
            }

            div { style: "display: block; margin-top: 1.5rem; padding-top: 1rem; border-top: 1px solid #e2e8f0;",
                BodyText { "Commit" }
                div { style: "display: flex; flex-wrap: wrap; gap: 0.5rem;",
                    div { style: "flex: 1 1 200px;",
                        FormField {
                            label: "Author name".to_string(),
                            value: author_name(),
                            input_type: "text".to_string(),
                            oninput: move |v| author_name.set(v),
                        }
                    }
                    div { style: "flex: 1 1 200px;",
                        FormField {
                            label: "Author email".to_string(),
                            value: author_email(),
                            input_type: "text".to_string(),
                            oninput: move |v| author_email.set(v),
                        }
                    }
                }
                FormField {
                    label: "Message".to_string(),
                    value: commit_message(),
                    input_type: "text".to_string(),
                    oninput: move |v| commit_message.set(v),
                }
                ActionRow {
                    PrimaryButton { label: "Commit".to_string(), onclick: on_commit }
                }
            }

            div { style: "display: block; margin-top: 1.5rem; padding-top: 1rem; border-top: 1px solid #e2e8f0;",
                BodyText { "Branches" }
                ActionRow {
                    SecondaryButton { label: "Refresh branches".to_string(), onclick: on_refresh_branches }
                    FormField {
                        label: "New branch".to_string(),
                        value: new_branch(),
                        input_type: "text".to_string(),
                        oninput: move |v| new_branch.set(v),
                    }
                    SecondaryButton { label: "Create".to_string(), onclick: on_create_branch }
                }
                BranchList {
                    entries: branches(),
                    on_checkout: move |name: String| {
                        if busy() { return; }
                        let Some(p) = repo_path() else { return; };
                        busy.set(true);
                        spawn(async move {
                            match Repo::open(&p).await {
                                Ok(repo) => match repo.checkout(&name).await {
                                    Ok(()) => {
                                        status.set(StatusMsg::ok(format!("Checked out {name}")));
                                        if let Ok(list) = repo.list_branches().await {
                                            branches.set(list);
                                        }
                                        if let Ok(entries) = repo.list("").await {
                                            listing.set(to_tree_nodes("", entries));
                                            tree_prefix.set(String::new());
                                        }
                                    }
                                    Err(e) => status.set(StatusMsg::err(e.to_string())),
                                },
                                Err(e) => status.set(StatusMsg::err(e.to_string())),
                            }
                            busy.set(false);
                        });
                    },
                }
            }

            div { style: "display: block; margin-top: 1.5rem; padding-top: 1rem; border-top: 1px solid #e2e8f0;",
                BodyText { "Log" }
                ActionRow {
                    SecondaryButton { label: "Refresh log".to_string(), onclick: on_log }
                }
                CommitList { entries: commits() }
            }
        }
    }
}

#[component]
fn TreeList(
    entries: Vec<TreeNode>,
    on_open_dir: EventHandler<String>,
    on_open_file: EventHandler<String>,
) -> Element {
    if entries.is_empty() {
        return rsx! {
            div { style: "padding: 0.5rem; color: #64748b; font-size: 0.875rem;",
                "No files yet — clone a repository."
            }
        };
    }
    rsx! {
        ul { style: "list-style: none; margin: 0; padding: 0; border: 1px solid #e2e8f0; border-radius: 0.5rem; overflow: hidden;",
            for entry in entries {
                {
                    let path = entry.path.clone();
                    let name = entry.name.clone();
                    let is_dir = entry.is_dir;
                    let label = if is_dir {
                        format!("{name}/")
                    } else {
                        name.clone()
                    };
                    rsx! {
                        li { style: "display: flex; align-items: center; justify-content: space-between; padding: 0.4rem 0.75rem; border-bottom: 1px solid #e2e8f0; font-size: 0.875rem;",
                            button {
                                style: "font-family: ui-monospace, monospace; text-align: left; background: none; border: none; color: #075985; cursor: pointer; padding: 0;",
                                onclick: move |_| {
                                    if is_dir {
                                        on_open_dir.call(path.clone());
                                    } else {
                                        on_open_file.call(path.clone());
                                    }
                                },
                                "{label}"
                            }
                            span { style: "font-size: 0.7rem; color: #64748b; text-transform: uppercase;",
                                if is_dir { "dir" } else { "file" }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn BranchList(entries: Vec<BranchInfo>, on_checkout: EventHandler<String>) -> Element {
    if entries.is_empty() {
        return rsx! { BodyText { "No branches loaded." } };
    }
    rsx! {
        ul { class: "divide-y divide-slate-200 rounded-lg border border-slate-200",
            for b in entries {
                {
                    let name = b.name.clone();
                    let current = b.current;
                    rsx! {
                        li { class: "flex items-center justify-between px-3 py-2 text-sm",
                            span { class: "font-mono",
                                {
                                    if current {
                                        format!("* {name}")
                                    } else {
                                        name.clone()
                                    }
                                }
                            }
                            if !current {
                                button {
                                    class: "text-xs text-sky-700 hover:underline",
                                    onclick: move |_| on_checkout.call(name.clone()),
                                    "Checkout"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn CommitList(entries: Vec<CommitInfo>) -> Element {
    if entries.is_empty() {
        return rsx! { BodyText { "No commits loaded." } };
    }
    rsx! {
        ul { class: "divide-y divide-slate-200 rounded-lg border border-slate-200",
            for c in entries {
                {
                    let id = if c.id.len() > 8 { c.id[..8].to_string() } else { c.id.clone() };
                    let msg = c.message.lines().next().unwrap_or("").to_string();
                    let author = c.author.clone();
                    rsx! {
                        li { class: "flex flex-col gap-0.5 px-3 py-2 text-sm",
                            span { class: "font-mono text-xs text-slate-500", "{id} — {author}" }
                            span { "{msg}" }
                        }
                    }
                }
            }
        }
    }
}

fn to_tree_nodes(prefix: &str, entries: Vec<DirEntry>) -> Vec<TreeNode> {
    let mut nodes: Vec<TreeNode> = entries
        .into_iter()
        .map(|e| {
            let path = if prefix.is_empty() {
                e.path.clone()
            } else {
                format!("{prefix}/{}", e.path)
            };
            TreeNode {
                name: e.path,
                path,
                is_dir: e.is_dir,
            }
        })
        .collect();
    nodes.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.cmp(&b.name),
    });
    nodes
}

/// Prefer README* at repo root; else the first non-directory entry.
fn pick_default_file(nodes: &[TreeNode]) -> Option<String> {
    const PREFERRED: &[&str] = &["README.md", "README", "readme.md", "Readme.md"];
    for name in PREFERRED {
        if let Some(n) = nodes.iter().find(|n| !n.is_dir && n.name == *name) {
            return Some(n.path.clone());
        }
    }
    nodes.iter().find(|n| !n.is_dir).map(|n| n.path.clone())
}

fn parent_path(path: &str) -> String {
    match path.rsplit_once('/') {
        Some((parent, _)) => parent.to_string(),
        None => String::new(),
    }
}

fn guess_lang(path: &str) -> String {
    let ext = path.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "rs" => "rust",
        "js" | "mjs" | "cjs" => "javascript",
        "ts" | "tsx" => "typescript",
        "json" => "json",
        "md" => "markdown",
        "toml" => "ini",
        "yml" | "yaml" => "yaml",
        "css" => "css",
        "html" | "htm" => "html",
        "py" => "python",
        "sh" => "shell",
        _ => "plaintext",
    }
    .to_string()
}

enum RemoteOp {
    Fetch,
    Pull,
    Push,
}

fn run_remote_op(
    mut busy: Signal<bool>,
    mut status: Signal<StatusMsg>,
    repo_path: Signal<Option<String>>,
    opts: RemoteOpts,
    op: RemoteOp,
) {
    if busy() {
        return;
    }
    let Some(p) = repo_path() else {
        status.set(StatusMsg::err("Clone a repository first."));
        return;
    };
    busy.set(true);
    spawn(async move {
        match Repo::open(&p).await {
            Ok(repo) => {
                let result = match op {
                    RemoteOp::Fetch => repo.fetch(&opts).await.map(|_| "Fetched".to_string()),
                    RemoteOp::Pull => repo.pull(&opts).await.map(|_| "Pulled (FF)".to_string()),
                    RemoteOp::Push => repo.push(&opts).await.map(|_| "Pushed".to_string()),
                };
                match result {
                    Ok(msg) => status.set(StatusMsg::ok(msg)),
                    Err(e) => status.set(StatusMsg::err(e.to_string())),
                }
            }
            Err(e) => status.set(StatusMsg::err(e.to_string())),
        }
        busy.set(false);
    });
}

#[derive(Clone, Default)]
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
struct SavedConnect {
    url: Option<String>,
    username: Option<String>,
    token: Option<String>,
    cors_proxy: Option<String>,
}

#[cfg(target_arch = "wasm32")]
async fn storage_load() -> Option<SavedConnect> {
    let mut eval = document::eval(
        r#"
        const raw = (globalThis.MonacoHost && MonacoHost.storageLoad)
          ? MonacoHost.storageLoad()
          : null;
        dioxus.send(raw);
        "#,
    );
    match eval.recv::<Option<SavedConnectJson>>().await {
        Ok(Some(j)) => Some(SavedConnect {
            url: j.url,
            username: j.username,
            token: j.token,
            cors_proxy: j.cors_proxy,
        }),
        _ => None,
    }
}

#[cfg(target_arch = "wasm32")]
async fn storage_save(data: &SavedConnect) {
    let url = data.url.clone().unwrap_or_default();
    let username = data.username.clone().unwrap_or_default();
    let token = data.token.clone().unwrap_or_default();
    let cors = data.cors_proxy.clone().unwrap_or_default();
    let _ = document::eval(&format!(
        r#"
        if (globalThis.MonacoHost && MonacoHost.storageSave) {{
          MonacoHost.storageSave({{
            url: {url:?},
            username: {username:?},
            token: {token:?},
            cors_proxy: {cors:?}
          }});
        }}
        true
        "#
    ));
}

#[cfg(target_arch = "wasm32")]
#[derive(serde::Deserialize, Default)]
struct SavedConnectJson {
    url: Option<String>,
    username: Option<String>,
    token: Option<String>,
    cors_proxy: Option<String>,
}

#[cfg(target_arch = "wasm32")]
async fn monaco_mount(dom_id: &str) -> Result<(), String> {
    let mut eval = document::eval(&format!(
        r#"
        (async () => {{
          const deadline = Date.now() + 15000;
          while (!globalThis.MonacoHost || !MonacoHost.ensureMounted) {{
            if (Date.now() > deadline) throw new Error("MonacoHost missing");
            await new Promise((r) => setTimeout(r, 50));
          }}
          await MonacoHost.ensureMounted({dom_id:?});
          dioxus.send(true);
        }})().catch((e) => dioxus.send(String(e && e.message ? e.message : e)));
        "#
    ));
    match eval.recv::<serde_json::Value>().await {
        Ok(serde_json::Value::Bool(true)) => Ok(()),
        Ok(serde_json::Value::String(e)) => Err(e),
        Ok(_) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(target_arch = "wasm32")]
async fn set_editor_value(text: &str, language: &str) -> Result<(), String> {
    let mut eval = document::eval(&format!(
        r#"
        (async () => {{
          const deadline = Date.now() + 15000;
          while (!globalThis.MonacoHost || !MonacoHost.setValue) {{
            if (Date.now() > deadline) throw new Error("MonacoHost.setValue missing");
            await new Promise((r) => setTimeout(r, 50));
          }}
          await MonacoHost.setValue({text:?}, {language:?}, {MONACO_DOM_ID:?});
          dioxus.send(true);
        }})().catch((e) => dioxus.send(String(e && e.message ? e.message : e)));
        "#
    ));
    match eval.recv::<serde_json::Value>().await {
        Ok(serde_json::Value::Bool(true)) => Ok(()),
        Ok(serde_json::Value::String(e)) => Err(e),
        Ok(_) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(target_arch = "wasm32")]
async fn read_editor_text(fallback: String) -> String {
    let mut eval = document::eval(&format!(
        r#"
        (async () => {{
          if (!globalThis.MonacoHost || !MonacoHost.getValue) {{
            dioxus.send(null);
            return;
          }}
          const v = await MonacoHost.getValue({MONACO_DOM_ID:?});
          dioxus.send(v);
        }})().catch(() => dioxus.send(null));
        "#
    ));
    match eval.recv::<Option<String>>().await {
        Ok(Some(v)) => v,
        _ => fallback,
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn set_editor_value(_text: &str, _language: &str) -> Result<(), String> {
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
async fn read_editor_text(fallback: String) -> String {
    fallback
}

#[cfg(test)]
mod pick_default_file_tests {
    use super::{pick_default_file, TreeNode};

    #[test]
    fn prefers_readme_md() {
        let nodes = vec![
            TreeNode {
                path: "src".into(),
                is_dir: true,
                name: "src".into(),
            },
            TreeNode {
                path: "LICENSE".into(),
                is_dir: false,
                name: "LICENSE".into(),
            },
            TreeNode {
                path: "README.md".into(),
                is_dir: false,
                name: "README.md".into(),
            },
        ];
        assert_eq!(pick_default_file(&nodes).as_deref(), Some("README.md"));
    }

    #[test]
    fn falls_back_to_first_file() {
        let nodes = vec![
            TreeNode {
                path: "docs".into(),
                is_dir: true,
                name: "docs".into(),
            },
            TreeNode {
                path: "hello.txt".into(),
                is_dir: false,
                name: "hello.txt".into(),
            },
        ];
        assert_eq!(pick_default_file(&nodes).as_deref(), Some("hello.txt"));
    }

    #[test]
    fn none_when_only_dirs() {
        let nodes = vec![TreeNode {
            path: "src".into(),
            is_dir: true,
            name: "src".into(),
        }];
        assert!(pick_default_file(&nodes).is_none());
    }
}
