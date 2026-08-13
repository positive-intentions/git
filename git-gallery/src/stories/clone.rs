use dioxus::prelude::*;
use git_core::{suggest_workdir, DirEntry, GitRepo, Repo};
use whatsup_ui::components::atoms::BodyText;
use whatsup_ui::gallery::KnobDef;

use crate::chrome::{
    ActionRow, DemoShell, DirList, MonoBlock, PrimaryButton, SecondaryButton, StatusBanner,
    StatusMsg,
};

whatsup_ui::register_gui_story! {
    name: "Clone",
    group: "Git",
    docs: include_str!("clone.md"),
    knobs: [
        KnobDef::text(
            "url",
            "URL",
            "https://github.com/octocat/Hello-World",
        ),
        KnobDef::text(
            "cors_proxy",
            "CORS proxy (web)",
            "https://cors.isomorphic-git.org",
        ),
    ],
    render: |k| {
        let url = k.get_str("url").to_string();
        let cors_proxy = k.get_str("cors_proxy").to_string();
        rsx! { CloneDemo { url, cors_proxy } }
    },
}

#[component]
fn CloneDemo(url: String, cors_proxy: String) -> Element {
    let mut status = use_signal(|| {
        StatusMsg::info("Clone is optional. On web, a CORS proxy is required for GitHub.")
    });
    let mut repo_path = use_signal(|| None::<String>);
    let mut listing = use_signal(Vec::<DirEntry>::new);
    let mut busy = use_signal(|| false);

    let on_clone = {
        let url = url.clone();
        let cors_proxy = cors_proxy.clone();
        move |_| {
            if busy() {
                return;
            }
            let remote = url.clone();
            let cors = cors_proxy.clone();
            busy.set(true);
            spawn(async move {
                let p = suggest_workdir("git-gallery-clone");
                let proxy = if cors.trim().is_empty() {
                    None
                } else {
                    Some(cors.as_str())
                };
                match Repo::clone(&remote, &p, proxy).await {
                    Ok(repo) => {
                        repo_path.set(Some(repo.workdir().to_string()));
                        listing.set(Vec::new());
                        status.set(StatusMsg::ok(format!("Cloned into {}", repo.workdir())));
                    }
                    Err(e) => status.set(StatusMsg::err(e.to_string())),
                }
                busy.set(false);
            });
        }
    };

    let on_list = move |_| {
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
                Ok(repo) => match repo.list("").await {
                    Ok(entries) => {
                        listing.set(entries);
                        status.set(StatusMsg::ok("Listed worktree root".to_string()));
                    }
                    Err(e) => status.set(StatusMsg::err(e.to_string())),
                },
                Err(e) => status.set(StatusMsg::err(e.to_string())),
            }
            busy.set(false);
        });
    };

    rsx! {
        DemoShell { title: "Clone".to_string(),
            StatusBanner { status: status() }
            ActionRow {
                PrimaryButton {
                    label: if busy() { "Cloning…".to_string() } else { "Clone".to_string() },
                    onclick: on_clone
                }
                SecondaryButton { label: "List root".to_string(), onclick: on_list }
            }
            if let Some(p) = repo_path() {
                BodyText { "Workdir" }
                MonoBlock { text: p }
            }
            BodyText { "Listing" }
            DirList { entries: listing() }
        }
    }
}
