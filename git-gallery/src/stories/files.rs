use dioxus::prelude::*;
use git_core::{suggest_workdir, DirEntry, GitRepo, Repo};
use whatsup_ui::components::atoms::BodyText;
use whatsup_ui::gallery::KnobDef;

use crate::chrome::{
    ActionRow, DemoShell, DirList, MonoBlock, PrimaryButton, SecondaryButton, StatusBanner,
    StatusMsg,
};

whatsup_ui::register_gui_story! {
    name: "Files",
    group: "Git",
    docs: include_str!("files.md"),
    knobs: [
        KnobDef::text("path", "Path", "notes/hello.txt"),
        KnobDef::text("content", "Content", "hello from git-gallery"),
    ],
    render: |k| {
        let path = k.get_str("path").to_string();
        let content = k.get_str("content").to_string();
        rsx! { FilesDemo { path, content } }
    },
}

#[component]
fn FilesDemo(path: String, content: String) -> Element {
    let mut status =
        use_signal(|| StatusMsg::info("Init a repo, then Write / Read / Edit / Remove / List."));
    let mut repo_path = use_signal(|| None::<String>);
    let mut listing = use_signal(Vec::<DirEntry>::new);
    let mut read_out = use_signal(|| String::new());
    let mut busy = use_signal(|| false);

    let on_init = move |_| {
        if busy() {
            return;
        }
        busy.set(true);
        spawn(async move {
            let p = suggest_workdir("git-gallery-files");
            match Repo::init(&p).await {
                Ok(repo) => {
                    repo_path.set(Some(repo.workdir().to_string()));
                    listing.set(Vec::new());
                    read_out.set(String::new());
                    status.set(StatusMsg::ok(format!("Ready at {}", repo.workdir())));
                }
                Err(e) => status.set(StatusMsg::err(e.to_string())),
            }
            busy.set(false);
        });
    };

    let on_write = {
        let path = path.clone();
        let content = content.clone();
        move |_| {
            if busy() {
                return;
            }
            let Some(p) = repo_path() else {
                status.set(StatusMsg::err("Init a repository first."));
                return;
            };
            let filepath = path.clone();
            let body = content.clone();
            busy.set(true);
            spawn(async move {
                match Repo::open(&p).await {
                    Ok(repo) => match repo.write_file(&filepath, body.as_bytes()).await {
                        Ok(()) => status.set(StatusMsg::ok(format!("Wrote + staged {filepath}"))),
                        Err(e) => status.set(StatusMsg::err(e.to_string())),
                    },
                    Err(e) => status.set(StatusMsg::err(e.to_string())),
                }
                busy.set(false);
            });
        }
    };

    let on_edit = {
        let path = path.clone();
        let content = content.clone();
        move |_| {
            if busy() {
                return;
            }
            let Some(p) = repo_path() else {
                status.set(StatusMsg::err("Init a repository first."));
                return;
            };
            let filepath = path.clone();
            let body = format!("{content}\n# edited");
            busy.set(true);
            spawn(async move {
                match Repo::open(&p).await {
                    Ok(repo) => match repo.write_file(&filepath, body.as_bytes()).await {
                        Ok(()) => status.set(StatusMsg::ok(format!("Edited + staged {filepath}"))),
                        Err(e) => status.set(StatusMsg::err(e.to_string())),
                    },
                    Err(e) => status.set(StatusMsg::err(e.to_string())),
                }
                busy.set(false);
            });
        }
    };

    let on_read = {
        let path = path.clone();
        move |_| {
            if busy() {
                return;
            }
            let Some(p) = repo_path() else {
                status.set(StatusMsg::err("Init a repository first."));
                return;
            };
            let filepath = path.clone();
            busy.set(true);
            spawn(async move {
                match Repo::open(&p).await {
                    Ok(repo) => match repo.read_file(&filepath).await {
                        Ok(bytes) => {
                            read_out.set(String::from_utf8_lossy(&bytes).into_owned());
                            status.set(StatusMsg::ok(format!("Read {filepath}")));
                        }
                        Err(e) => status.set(StatusMsg::err(e.to_string())),
                    },
                    Err(e) => status.set(StatusMsg::err(e.to_string())),
                }
                busy.set(false);
            });
        }
    };

    let on_remove = {
        let path = path.clone();
        move |_| {
            if busy() {
                return;
            }
            let Some(p) = repo_path() else {
                status.set(StatusMsg::err("Init a repository first."));
                return;
            };
            let filepath = path.clone();
            busy.set(true);
            spawn(async move {
                match Repo::open(&p).await {
                    Ok(repo) => match repo.remove_file(&filepath).await {
                        Ok(()) => {
                            read_out.set(String::new());
                            status.set(StatusMsg::ok(format!("Removed {filepath}")));
                        }
                        Err(e) => status.set(StatusMsg::err(e.to_string())),
                    },
                    Err(e) => status.set(StatusMsg::err(e.to_string())),
                }
                busy.set(false);
            });
        }
    };

    let on_list = {
        let path = path.clone();
        move |_| {
            if busy() {
                return;
            }
            let Some(p) = repo_path() else {
                status.set(StatusMsg::err("Init a repository first."));
                return;
            };
            let filepath = path.clone();
            busy.set(true);
            spawn(async move {
                let rel = filepath.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
                match Repo::open(&p).await {
                    Ok(repo) => match repo.list(rel).await {
                        Ok(entries) => {
                            listing.set(entries);
                            status.set(StatusMsg::ok(format!("Listed '{rel}'")));
                        }
                        Err(e) => status.set(StatusMsg::err(e.to_string())),
                    },
                    Err(e) => status.set(StatusMsg::err(e.to_string())),
                }
                busy.set(false);
            });
        }
    };

    rsx! {
        DemoShell { title: "Files".to_string(),
            StatusBanner { status: status() }
            ActionRow {
                PrimaryButton { label: "Init".to_string(), onclick: on_init }
                SecondaryButton { label: "Write".to_string(), onclick: on_write }
                SecondaryButton { label: "Edit".to_string(), onclick: on_edit }
                SecondaryButton { label: "Read".to_string(), onclick: on_read }
                SecondaryButton { label: "Remove".to_string(), onclick: on_remove }
                SecondaryButton { label: "List".to_string(), onclick: on_list }
            }
            if let Some(p) = repo_path() {
                BodyText { "Workdir" }
                MonoBlock { text: p }
            }
            BodyText { "Listing" }
            DirList { entries: listing() }
            if !read_out().is_empty() {
                BodyText { "Read output" }
                MonoBlock { text: read_out() }
            }
        }
    }
}
