//! Storage story: clone + native-feeling file explorer with auto two-way sync.

use dioxus::prelude::*;
use git_core::{
    suggest_workdir, DirEntry, GitAuth, GitRepo, RemoteOpts, Repo, Signature,
};
use whatsup_ui::components::atoms::{BodyText, Button, ButtonVariant, SyncState};
use whatsup_ui::components::molecules::{
    BreadcrumbSegment, ConflictChoice, ConflictDialog, ExplorerItem, ExplorerItemKind,
    ExplorerViewMode, OverlayDialog,
};
use whatsup_ui::components::organisms::{DroppedFile, FileExplorer, MoveRequest};

use crate::chrome::{
    ActionRow, DemoShell, FormField, PrimaryButton, StatusBanner, StatusMsg,
};

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
const MONACO_DOM_ID: &str = "git-gallery-storage-monaco";
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
const STORAGE_KEY: &str = "git-gallery:storage";
const KEEP_NAME: &str = ".keep";
const SYNC_MSG: &str = "Sync from storage";
const AUTHOR_NAME: &str = "git-gallery";
const AUTHOR_EMAIL: &str = "gallery@local";
const SYNC_INTERVAL_MS: u32 = 60_000;

whatsup_ui::register_gui_story! {
    name: "Storage",
    group: "Git",
    docs: include_str!("storage.md"),
    knobs: [],
    render: |_k| {
        rsx! { StorageDemo {} }
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OverlayKind {
    None,
    Text,
    Image,
    Binary,
}

#[component]
fn StorageDemo() -> Element {
    let mut url = use_signal(|| "https://github.com/octocat/Hello-World".to_string());
    let mut username = use_signal(String::new);
    let mut token = use_signal(String::new);
    let mut cors_proxy = use_signal(|| "https://cors.isomorphic-git.org".to_string());

    let mut status = use_signal(|| {
        StatusMsg::info(
            "Connect to a remote. Credentials stay in this browser (localStorage).",
        )
    });
    let mut repo_path = use_signal(|| None::<String>);
    let mut cwd = use_signal(String::new);
    let mut items = use_signal(Vec::<ExplorerItem>::new);
    let mut selected_id = use_signal(|| None::<String>);
    let mut view_mode = use_signal(ExplorerViewMode::default);
    let mut sync_state = use_signal(SyncState::default);
    let mut busy = use_signal(|| false);
    let syncing = use_signal(|| false);
    let mut conflict_open = use_signal(|| false);
    let mut remembered_choice = use_signal(|| None::<ConflictChoice>);

    let mut overlay = use_signal(|| OverlayKind::None);
    let mut open_path = use_signal(|| None::<String>);
    let mut file_text = use_signal(String::new);
    let mut image_src = use_signal(String::new);
    let mut new_name = use_signal(String::new);
    let mut naming_mode = use_signal(|| None::<NamingMode>);
    let mut delete_confirm = use_signal(|| None::<ExplorerItem>);

    // Load persisted connection fields (web).
    use_effect(move || {
        #[cfg(target_arch = "wasm32")]
        {
            spawn(async move {
                if let Some(saved) = storage_load(STORAGE_KEY).await {
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
                storage_save(&snapshot, STORAGE_KEY).await;
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

    // Interval auto-sync while a repo is open.
    use_effect(move || {
        if repo_path().is_none() {
            return;
        }
        spawn(async move {
            loop {
                sleep_ms(SYNC_INTERVAL_MS).await;
                kick_sync(
                    repo_path,
                    cwd,
                    items,
                    syncing,
                    sync_state,
                    conflict_open,
                    remembered_choice,
                    cors_proxy,
                    username,
                    token,
                );
            }
        });
    });

    let on_clone = move |_| {
        if busy() {
            return;
        }
        let remote = url();
        let opts = remote_opts();
        busy.set(true);
        status.set(StatusMsg::info(format!("Cloning {remote}…")));
        spawn(async move {
            let p = suggest_workdir("git-gallery-storage");
            match Repo::clone(&remote, &p, &opts).await {
                Ok(repo) => {
                    repo_path.set(Some(repo.workdir().to_string()));
                    cwd.set(String::new());
                    status.set(StatusMsg::ok(format!("Ready at {}", repo.workdir())));
                    match repo.list("").await {
                        Ok(entries) => items.set(to_explorer_items("", entries)),
                        Err(e) => status.set(StatusMsg::err(e.to_string())),
                    }
                    sync_state.set(SyncState::Synced {
                        at: "just now".into(),
                    });
                }
                Err(e) => status.set(StatusMsg::err(format!(
                    "{e} (private remotes need username+token; web may need a CORS proxy that forwards Authorization)"
                ))),
            }
            busy.set(false);
        });
    };

    let path_segments = {
        let path = cwd();
        let mut segments = Vec::new();
        let mut acc = String::new();
        for part in path.split('/').filter(|p| !p.is_empty()) {
            if !acc.is_empty() {
                acc.push('/');
            }
            acc.push_str(part);
            segments.push(BreadcrumbSegment {
                id: acc.clone(),
                label: part.to_string(),
            });
        }
        segments
    };

    rsx! {
        DemoShell { title: "Storage".to_string(),
            StatusBanner { status: status() }

            BodyText { "Remote connection" }
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
                    label: if busy() {
                        "Working…".to_string()
                    } else {
                        "Clone".to_string()
                    },
                    onclick: on_clone,
                }
                if remembered_choice().is_some() {
                    Button {
                        variant: ButtonVariant::Secondary,
                        onclick: move |_| remembered_choice.set(None),
                        "Clear conflict preference"
                    }
                }
            }

            if repo_path().is_some() {
                div { class: "mt-2 min-h-[24rem]",
                    FileExplorer {
                        items: items(),
                        path_segments,
                        view_mode: view_mode(),
                        sync_state: sync_state(),
                        selected_id: selected_id(),
                        on_open: move |id: String| {
                            let Some(p) = repo_path() else { return };
                            let item = items().into_iter().find(|i| i.id == id);
                            let Some(item) = item else { return };
                            if item.kind == ExplorerItemKind::Folder {
                                refresh_listing(repo_path, cwd, items, sync_state, id);
                                return;
                            }
                            selected_id.set(Some(id.clone()));
                            open_path.set(Some(id.clone()));
                            spawn(async move {
                                match Repo::open(&p).await {
                                    Ok(repo) => match repo.read_file(&id).await {
                                        Ok(bytes) => {
                                            if is_image_name(&item.name) {
                                                image_src.set(bytes_to_data_url(&item.name, &bytes));
                                                overlay.set(OverlayKind::Image);
                                            } else if looks_text(&bytes) {
                                                let text = String::from_utf8_lossy(&bytes).into_owned();
                                                file_text.set(text.clone());
                                                overlay.set(OverlayKind::Text);
                                                let lang = guess_lang(&id);
                                                let _ = monaco_mount(MONACO_DOM_ID).await;
                                                let _ = set_editor_value(&text, &lang).await;
                                            } else {
                                                overlay.set(OverlayKind::Binary);
                                            }
                                        }
                                        Err(e) => status.set(StatusMsg::err(e.to_string())),
                                    },
                                    Err(e) => status.set(StatusMsg::err(e.to_string())),
                                }
                            });
                        },
                        on_navigate: move |id: String| {
                            refresh_listing(repo_path, cwd, items, sync_state, id);
                        },
                        on_select: move |id: String| selected_id.set(Some(id)),
                        on_view_mode: move |m| view_mode.set(m),
                        on_create_file: move |_| {
                            naming_mode.set(Some(NamingMode::File));
                            new_name.set("untitled.txt".into());
                        },
                        on_create_folder: move |_| {
                            naming_mode.set(Some(NamingMode::Folder));
                            new_name.set("new-folder".into());
                        },
                        on_delete: move |_| {
                            let Some(id) = selected_id() else { return };
                            if let Some(item) = items().into_iter().find(|i| i.id == id) {
                                delete_confirm.set(Some(item));
                            }
                        },
                        on_drop_files: move |_files: Vec<DroppedFile>| {
                            let Some(p) = repo_path() else { return };
                            let dir = cwd();
                            spawn(async move {
                                let dropped = consume_dropped_files().await;
                                if dropped.is_empty() {
                                    return;
                                }
                                match Repo::open(&p).await {
                                    Ok(repo) => {
                                        for f in dropped {
                                            let rel = join_path(&dir, &f.name);
                                            if let Err(e) = repo.write_file(&rel, &f.data).await {
                                                status.set(StatusMsg::err(e.to_string()));
                                                return;
                                            }
                                        }
                                        refresh_listing(repo_path, cwd, items, sync_state, dir);
                                        kick_sync(
                                            repo_path,
                                            cwd,
                                            items,
                                            syncing,
                                            sync_state,
                                            conflict_open,
                                            remembered_choice,
                                            cors_proxy,
                                            username,
                                            token,
                                        );
                                    }
                                    Err(e) => status.set(StatusMsg::err(e.to_string())),
                                }
                            });
                        },
                        on_move: move |req: MoveRequest| {
                            let Some(p) = repo_path() else { return };
                            let dir = cwd();
                            spawn(async move {
                                let name = req
                                    .item_id
                                    .rsplit('/')
                                    .next()
                                    .unwrap_or(req.item_id.as_str())
                                    .to_string();
                                let dest = join_path(&req.target_dir_id, &name);
                                if dest == req.item_id {
                                    return;
                                }
                                match Repo::open(&p).await {
                                    Ok(repo) => match repo.rename(&req.item_id, &dest).await {
                                        Ok(()) => {
                                            refresh_listing(repo_path, cwd, items, sync_state, dir);
                                            kick_sync(
                                                repo_path,
                                                cwd,
                                                items,
                                                syncing,
                                                sync_state,
                                                conflict_open,
                                                remembered_choice,
                                                cors_proxy,
                                                username,
                                                token,
                                            );
                                        }
                                        Err(e) => status.set(StatusMsg::err(e.to_string())),
                                    },
                                    Err(e) => status.set(StatusMsg::err(e.to_string())),
                                }
                            });
                        },
                    }
                }
            }

            ConflictDialog {
                open: conflict_open(),
                message: "".to_string(),
                on_close: move |_| {},
                on_choice: move |choice: ConflictChoice| {
                    remembered_choice.set(Some(choice));
                    conflict_open.set(false);
                    kick_sync(
                        repo_path,
                        cwd,
                        items,
                        syncing,
                        sync_state,
                        conflict_open,
                        remembered_choice,
                        cors_proxy,
                        username,
                        token,
                    );
                },
            }

            if let Some(mode) = naming_mode() {
                OverlayDialog {
                    title: if mode == NamingMode::Folder {
                        "New folder".to_string()
                    } else {
                        "New file".to_string()
                    },
                    open: true,
                    on_close: move |_| naming_mode.set(None),
                    footer: rsx! {
                        Button {
                            variant: ButtonVariant::Secondary,
                            onclick: move |_| naming_mode.set(None),
                            "Cancel"
                        }
                        Button {
                            variant: ButtonVariant::Primary,
                            onclick: move |_| {
                                let Some(p) = repo_path() else { return };
                                let name = new_name().trim().to_string();
                                if name.is_empty() || name.contains('/') || name.contains('\\') {
                                    status.set(StatusMsg::err("Enter a simple name without slashes."));
                                    return;
                                }
                                let dir = cwd();
                                let mode = naming_mode();
                                naming_mode.set(None);
                                spawn(async move {
                                    match Repo::open(&p).await {
                                        Ok(repo) => {
                                            let rel = join_path(&dir, &name);
                                            let result = match mode {
                                                Some(NamingMode::Folder) => {
                                                    let keep = join_path(&rel, KEEP_NAME);
                                                    repo.write_file(&keep, b"").await
                                                }
                                                _ => repo.write_file(&rel, b"").await,
                                            };
                                            match result {
                                                Ok(()) => {
                                                    refresh_listing(
                                                        repo_path, cwd, items, sync_state, dir,
                                                    );
                                                    kick_sync(
                                                        repo_path,
                                                        cwd,
                                                        items,
                                                        syncing,
                                                        sync_state,
                                                        conflict_open,
                                                        remembered_choice,
                                                        cors_proxy,
                                                        username,
                                                        token,
                                                    );
                                                }
                                                Err(e) => status.set(StatusMsg::err(e.to_string())),
                                            }
                                        }
                                        Err(e) => status.set(StatusMsg::err(e.to_string())),
                                    }
                                });
                            },
                            "Create"
                        }
                    },
                    FormField {
                        label: "Name".to_string(),
                        value: new_name(),
                        input_type: "text".to_string(),
                        oninput: move |v| new_name.set(v),
                    }
                }
            }

            if let Some(item) = delete_confirm() {
                OverlayDialog {
                    title: format!("Delete {}?", item.name),
                    open: true,
                    on_close: move |_| delete_confirm.set(None),
                    footer: rsx! {
                        Button {
                            variant: ButtonVariant::Secondary,
                            onclick: move |_| delete_confirm.set(None),
                            "Cancel"
                        }
                        Button {
                            variant: ButtonVariant::Primary,
                            onclick: move |_| {
                                let Some(p) = repo_path() else { return };
                                let Some(item) = delete_confirm() else { return };
                                let dir = cwd();
                                let is_dir = item.kind == ExplorerItemKind::Folder;
                                let path = item.id.clone();
                                delete_confirm.set(None);
                                spawn(async move {
                                    match Repo::open(&p).await {
                                        Ok(repo) => match delete_path(&repo, &path, is_dir).await {
                                            Ok(()) => {
                                                selected_id.set(None);
                                                refresh_listing(
                                                    repo_path, cwd, items, sync_state, dir,
                                                );
                                                kick_sync(
                                                    repo_path,
                                                    cwd,
                                                    items,
                                                    syncing,
                                                    sync_state,
                                                    conflict_open,
                                                    remembered_choice,
                                                    cors_proxy,
                                                    username,
                                                    token,
                                                );
                                            }
                                            Err(e) => status.set(StatusMsg::err(e.to_string())),
                                        },
                                        Err(e) => status.set(StatusMsg::err(e.to_string())),
                                    }
                                });
                            },
                            "Delete"
                        }
                    },
                    BodyText { "This cannot be undone from the explorer. A sync will commit the removal." }
                }
            }

            OverlayDialog {
                title: open_path().unwrap_or_else(|| "File".into()),
                open: overlay() == OverlayKind::Text,
                on_close: move |_| {
                    overlay.set(OverlayKind::None);
                    open_path.set(None);
                },
                footer: rsx! {
                    Button {
                        variant: ButtonVariant::Secondary,
                        onclick: move |_| {
                            overlay.set(OverlayKind::None);
                            open_path.set(None);
                        },
                        "Close"
                    }
                    Button {
                        variant: ButtonVariant::Primary,
                        onclick: move |_| {
                            let Some(p) = repo_path() else { return };
                            let Some(rel) = open_path() else { return };
                            spawn(async move {
                                let text = read_editor_text(file_text()).await;
                                match Repo::open(&p).await {
                                    Ok(repo) => match repo.write_file(&rel, text.as_bytes()).await {
                                        Ok(()) => {
                                            file_text.set(text);
                                            status.set(StatusMsg::ok(format!("Saved {rel}")));
                                            overlay.set(OverlayKind::None);
                                            kick_sync(
                                                repo_path,
                                                cwd,
                                                items,
                                                syncing,
                                                sync_state,
                                                conflict_open,
                                                remembered_choice,
                                                cors_proxy,
                                                username,
                                                token,
                                            );
                                        }
                                        Err(e) => status.set(StatusMsg::err(e.to_string())),
                                    },
                                    Err(e) => status.set(StatusMsg::err(e.to_string())),
                                }
                            });
                        },
                        "Save"
                    }
                },
                EditorPane {
                    file_text: file_text(),
                    open_path: open_path(),
                    on_text: move |v| file_text.set(v),
                }
            }

            OverlayDialog {
                title: open_path().unwrap_or_else(|| "Image".into()),
                open: overlay() == OverlayKind::Image,
                on_close: move |_| {
                    overlay.set(OverlayKind::None);
                    open_path.set(None);
                },
                img {
                    src: "{image_src}",
                    alt: "Preview",
                    class: "mx-auto max-h-[70vh] max-w-full object-contain",
                }
            }

            OverlayDialog {
                title: open_path().unwrap_or_else(|| "File".into()),
                open: overlay() == OverlayKind::Binary,
                on_close: move |_| {
                    overlay.set(OverlayKind::None);
                    open_path.set(None);
                },
                BodyText { "This file type cannot be previewed in the storage explorer." }
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NamingMode {
    File,
    Folder,
}

#[component]
fn EditorPane(
    file_text: String,
    open_path: Option<String>,
    on_text: EventHandler<String>,
) -> Element {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = on_text;
        let text = file_text.clone();
        let path = open_path.clone().unwrap_or_default();
        rsx! {
            div {
                id: MONACO_DOM_ID,
                style: "width: 100%; height: 288px; min-height: 288px;",
                onmounted: move |_| {
                    let text = text.clone();
                    let path = path.clone();
                    spawn(async move {
                        let _ = monaco_mount(MONACO_DOM_ID).await;
                        let lang = guess_lang(&path);
                        let _ = set_editor_value(&text, &lang).await;
                    });
                },
            }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = open_path;
        rsx! {
            textarea {
                class: "h-72 w-full rounded border border-slate-300 p-2 font-mono text-sm",
                value: "{file_text}",
                oninput: move |e| on_text.call(e.value()),
            }
        }
    }
}

enum SyncOutcome {
    Ok,
    Conflict,
    Err(String),
}

#[allow(clippy::too_many_arguments)]
fn refresh_listing(
    repo_path: Signal<Option<String>>,
    mut cwd: Signal<String>,
    mut items: Signal<Vec<ExplorerItem>>,
    mut sync_state: Signal<SyncState>,
    path: String,
) {
    let Some(p) = repo_path() else {
        return;
    };
    spawn(async move {
        match Repo::open(&p).await {
            Ok(repo) => match repo.list(&path).await {
                Ok(entries) => {
                    items.set(to_explorer_items(&path, entries));
                    cwd.set(path);
                }
                Err(e) => {
                    sync_state.set(SyncState::Error {
                        message: e.to_string(),
                    });
                }
            },
            Err(e) => {
                sync_state.set(SyncState::Error {
                    message: e.to_string(),
                });
            }
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn kick_sync(
    repo_path: Signal<Option<String>>,
    cwd: Signal<String>,
    mut items: Signal<Vec<ExplorerItem>>,
    mut syncing: Signal<bool>,
    mut sync_state: Signal<SyncState>,
    mut conflict_open: Signal<bool>,
    remembered_choice: Signal<Option<ConflictChoice>>,
    cors_proxy: Signal<String>,
    username: Signal<String>,
    token: Signal<String>,
) {
    if syncing() {
        return;
    }
    let Some(p) = repo_path() else {
        return;
    };
    if conflict_open() && remembered_choice().is_none() {
        return;
    }
    let mut opts = RemoteOpts::new().with_cors_proxy(cors_proxy());
    let auth = GitAuth::new(username(), token());
    if auth.is_set() {
        opts = opts.with_auth(auth);
    }
    let path = cwd();
    syncing.set(true);
    sync_state.set(SyncState::Checking);
    spawn(async move {
        sync_state.set(SyncState::Syncing);
        let result = sync_once(&p, &opts, remembered_choice()).await;
        match result {
            SyncOutcome::Ok => {
                sync_state.set(SyncState::Synced {
                    at: "just now".into(),
                });
                conflict_open.set(false);
                if let Ok(repo) = Repo::open(&p).await {
                    if let Ok(entries) = repo.list(&path).await {
                        items.set(to_explorer_items(&path, entries));
                    }
                }
            }
            SyncOutcome::Conflict => {
                sync_state.set(SyncState::Conflict);
                conflict_open.set(true);
            }
            SyncOutcome::Err(msg) => {
                sync_state.set(SyncState::Error { message: msg });
            }
        }
        syncing.set(false);
    });
}

async fn sync_once(
    workdir: &str,
    opts: &RemoteOpts,
    remembered: Option<ConflictChoice>,
) -> SyncOutcome {
    let repo = match Repo::open(workdir).await {
        Ok(r) => r,
        Err(e) => return SyncOutcome::Err(e.to_string()),
    };

    // Commit local dirty state first.
    match repo.status().await {
        Ok(entries) if !entries.is_empty() => {
            let sig = Signature::new(AUTHOR_NAME, AUTHOR_EMAIL);
            if let Err(e) = repo.commit(SYNC_MSG, &sig).await {
                return SyncOutcome::Err(e.to_string());
            }
        }
        Ok(_) => {}
        Err(e) => return SyncOutcome::Err(e.to_string()),
    }

    match repo.pull(opts).await {
        Ok(()) => {}
        Err(e) => {
            let msg = e.to_string();
            if is_divergence_error(&msg) {
                match remembered {
                    Some(ConflictChoice::AcceptRemote) => {
                        if let Err(e) = repo.reset_to_remote(opts).await {
                            return SyncOutcome::Err(e.to_string());
                        }
                    }
                    Some(ConflictChoice::AcceptLocal) => {
                        if let Err(e) = repo.push_force_with_lease(opts).await {
                            return SyncOutcome::Err(e.to_string());
                        }
                        return SyncOutcome::Ok;
                    }
                    None => return SyncOutcome::Conflict,
                }
            } else {
                return SyncOutcome::Err(msg);
            }
        }
    }

    match repo.push(opts).await {
        Ok(()) => SyncOutcome::Ok,
        Err(e) => {
            let msg = e.to_string();
            if is_divergence_error(&msg) {
                match remembered {
                    Some(ConflictChoice::AcceptRemote) => {
                        if let Err(e) = repo.reset_to_remote(opts).await {
                            return SyncOutcome::Err(e.to_string());
                        }
                        SyncOutcome::Ok
                    }
                    Some(ConflictChoice::AcceptLocal) => {
                        if let Err(e) = repo.push_force_with_lease(opts).await {
                            return SyncOutcome::Err(e.to_string());
                        }
                        SyncOutcome::Ok
                    }
                    None => SyncOutcome::Conflict,
                }
            } else {
                SyncOutcome::Err(msg)
            }
        }
    }
}

fn is_divergence_error(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    lower.contains("fast-forward")
        || lower.contains("fast forward")
        || lower.contains("not a simple fast-forward")
        || lower.contains("diverged")
        || lower.contains("non-fast-forward")
        || lower.contains("rejected")
        || lower.contains("cannot merge")
}

fn to_explorer_items(prefix: &str, entries: Vec<DirEntry>) -> Vec<ExplorerItem> {
    let mut out = Vec::new();
    for e in entries {
        if e.path == KEEP_NAME || e.path == ".git" {
            continue;
        }
        let id = join_path(prefix, &e.path);
        let name = e.path.clone();
        if e.is_dir {
            out.push(ExplorerItem::folder(id, name));
        } else if is_image_name(&name) {
            let mut item = ExplorerItem::image(id, name, "");
            if let Some(sz) = e.size_bytes {
                item = item.with_size(sz);
            }
            out.push(item);
        } else {
            let mut item = ExplorerItem::file(id, name);
            if let Some(sz) = e.size_bytes {
                item = item.with_size(sz);
            }
            out.push(item);
        }
    }
    out.sort_by(|a, b| {
        let da = a.kind == ExplorerItemKind::Folder;
        let db = b.kind == ExplorerItemKind::Folder;
        db.cmp(&da).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    out
}

fn join_path(prefix: &str, name: &str) -> String {
    if prefix.is_empty() || prefix == "." {
        name.to_string()
    } else {
        format!("{prefix}/{name}")
    }
}

async fn delete_path(repo: &Repo, path: &str, is_dir: bool) -> git_core::Result<()> {
    if !is_dir {
        return repo.remove_file(path).await;
    }
    let entries = repo.list(path).await?;
    for entry in entries {
        let child = join_path(path, &entry.path);
        if entry.is_dir {
            Box::pin(delete_path(repo, &child, true)).await?;
        } else {
            repo.remove_file(&child).await?;
        }
    }
    Ok(())
}

fn is_image_name(name: &str) -> bool {
    matches!(
        name.rsplit('.').next().unwrap_or("").to_ascii_lowercase().as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "ico"
    )
}

fn looks_text(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return true;
    }
    let sample = &bytes[..bytes.len().min(8_192)];
    !sample.contains(&0) && sample.iter().filter(|b| **b < 9 || (**b > 13 && **b < 32)).count() < 8
}

fn bytes_to_data_url(name: &str, bytes: &[u8]) -> String {
    let mime = match name.rsplit('.').next().unwrap_or("").to_ascii_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    };
    format!("data:{mime};base64,{}", base64_encode(bytes))
}

fn base64_encode(bytes: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let a = chunk[0] as u32;
        let b = chunk.get(1).copied().unwrap_or(0) as u32;
        let c = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (a << 16) | (b << 8) | c;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(T[((n >> 6) & 63) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(T[(n & 63) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn guess_lang(path: &str) -> String {
    match path.rsplit('.').next().unwrap_or("").to_ascii_lowercase().as_str() {
        "rs" => "rust",
        "js" | "mjs" | "cjs" => "javascript",
        "ts" | "tsx" => "typescript",
        "json" => "json",
        "md" | "markdown" => "markdown",
        "html" | "htm" => "html",
        "css" => "css",
        "py" => "python",
        "toml" => "ini",
        "yml" | "yaml" => "yaml",
        "sh" | "bash" => "shell",
        _ => "plaintext",
    }
    .to_string()
}

#[derive(Clone, Default)]
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
struct SavedConnect {
    url: Option<String>,
    username: Option<String>,
    token: Option<String>,
    cors_proxy: Option<String>,
}

async fn sleep_ms(ms: u32) {
    #[cfg(target_arch = "wasm32")]
    {
        gloo_timers::future::TimeoutFuture::new(ms).await;
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        futures_timer::Delay::new(std::time::Duration::from_millis(ms as u64)).await;
    }
}

#[cfg(target_arch = "wasm32")]
async fn storage_load(key: &str) -> Option<SavedConnect> {
    let mut eval = document::eval(&format!(
        r#"
        (async () => {{
          if (!globalThis.MonacoHost || !MonacoHost.storageLoad) {{
            dioxus.send(null);
            return;
          }}
          dioxus.send(MonacoHost.storageLoad({key:?}));
        }})().catch(() => dioxus.send(null));
        "#
    ));
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
async fn storage_save(data: &SavedConnect, key: &str) {
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
          }}, {key:?});
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
async fn consume_dropped_files() -> Vec<DroppedFile> {
    let mut eval = document::eval(
        r#"
        (async () => {
          if (!globalThis.MonacoHost || !MonacoHost.consumeDroppedFiles) {
            dioxus.send([]);
            return;
          }
          const files = await MonacoHost.consumeDroppedFiles();
          dioxus.send(files || []);
        })().catch(() => dioxus.send([]));
        "#,
    );
    match eval.recv::<Vec<DroppedFileJson>>().await {
        Ok(list) => list
            .into_iter()
            .map(|f| DroppedFile {
                name: f.name,
                data: f.data,
            })
            .collect(),
        _ => Vec::new(),
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn consume_dropped_files() -> Vec<DroppedFile> {
    Vec::new()
}

#[cfg(target_arch = "wasm32")]
#[derive(serde::Deserialize)]
struct DroppedFileJson {
    name: String,
    data: Vec<u8>,
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

#[cfg(not(target_arch = "wasm32"))]
async fn monaco_mount(_dom_id: &str) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_and_image_helpers() {
        assert_eq!(join_path("", "a.txt"), "a.txt");
        assert_eq!(join_path("docs", "a.txt"), "docs/a.txt");
        assert!(is_image_name("x.PNG"));
        assert!(!is_image_name("x.txt"));
        assert!(looks_text(b"hello"));
        assert!(!looks_text(&[0, 1, 2]));
        assert!(is_divergence_error("cannot fast-forward"));
    }

    #[test]
    fn explorer_items_hide_keep() {
        let items = to_explorer_items(
            "",
            vec![
                DirEntry {
                    path: KEEP_NAME.into(),
                    is_dir: false,
                    size_bytes: Some(0),
                },
                DirEntry {
                    path: "a.txt".into(),
                    is_dir: false,
                    size_bytes: Some(3),
                },
                DirEntry {
                    path: "docs".into(),
                    is_dir: true,
                    size_bytes: None,
                },
            ],
        );
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].name, "docs");
        assert_eq!(items[1].name, "a.txt");
    }
}
