use dioxus::prelude::*;
use git_core::{suggest_workdir, GitRepo, Repo, StatusEntry};
use whatsup_ui::components::atoms::BodyText;

use crate::chrome::{
    ActionRow, DemoShell, MonoBlock, PrimaryButton, SecondaryButton, StatusBanner, StatusList,
    StatusMsg,
};

whatsup_ui::register_gui_story! {
    name: "Status",
    group: "Git",
    docs: include_str!("status.md"),
    knobs: [],
    render: |_| {
        rsx! { StatusDemo {} }
    },
}

#[component]
fn StatusDemo() -> Element {
    let mut status = use_signal(|| StatusMsg::info("Init, mutate files, then press Status."));
    let mut repo_path = use_signal(|| None::<String>);
    let mut entries = use_signal(Vec::<StatusEntry>::new);
    let mut busy = use_signal(|| false);

    rsx! {
        DemoShell { title: "Status".to_string(),
            StatusBanner { status: status() }
            ActionRow {
                PrimaryButton {
                    label: "Init".to_string(),
                    onclick: move |_| {
                        if busy() { return; }
                        busy.set(true);
                        spawn(async move {
                            let p = suggest_workdir("git-gallery-status");
                            match Repo::init(&p).await {
                                Ok(repo) => {
                                    repo_path.set(Some(repo.workdir().to_string()));
                                    entries.set(Vec::new());
                                    status.set(StatusMsg::ok(format!("Ready at {}", repo.workdir())));
                                }
                                Err(e) => status.set(StatusMsg::err(e.to_string())),
                            }
                            busy.set(false);
                        });
                    }
                }
                SecondaryButton {
                    label: "Write sample".to_string(),
                    onclick: move |_| {
                        if busy() { return; }
                        let Some(p) = repo_path() else {
                            status.set(StatusMsg::err("Init a repository first."));
                            return;
                        };
                        busy.set(true);
                        spawn(async move {
                            match Repo::open(&p).await {
                                Ok(repo) => match repo.write_file("sample.txt", b"status demo").await {
                                    Ok(()) => status.set(StatusMsg::ok("Wrote + staged sample.txt")),
                                    Err(e) => status.set(StatusMsg::err(e.to_string())),
                                },
                                Err(e) => status.set(StatusMsg::err(e.to_string())),
                            }
                            busy.set(false);
                        });
                    }
                }
                SecondaryButton {
                    label: "Edit sample".to_string(),
                    onclick: move |_| {
                        if busy() { return; }
                        let Some(p) = repo_path() else {
                            status.set(StatusMsg::err("Init a repository first."));
                            return;
                        };
                        busy.set(true);
                        spawn(async move {
                            match Repo::open(&p).await {
                                Ok(repo) => {
                                    // Overwrite without going through write_file staging? We stage on write.
                                    // For a dirty worktree demo: write bytes then stage (shows staged).
                                    match repo
                                        .write_file("sample.txt", b"status demo\nedited line")
                                        .await
                                    {
                                        Ok(()) => status.set(StatusMsg::ok("Edited + staged sample.txt")),
                                        Err(e) => status.set(StatusMsg::err(e.to_string())),
                                    }
                                }
                                Err(e) => status.set(StatusMsg::err(e.to_string())),
                            }
                            busy.set(false);
                        });
                    }
                }
                SecondaryButton {
                    label: "Remove sample".to_string(),
                    onclick: move |_| {
                        if busy() { return; }
                        let Some(p) = repo_path() else {
                            status.set(StatusMsg::err("Init a repository first."));
                            return;
                        };
                        busy.set(true);
                        spawn(async move {
                            match Repo::open(&p).await {
                                Ok(repo) => match repo.remove_file("sample.txt").await {
                                    Ok(()) => status.set(StatusMsg::ok("Removed sample.txt")),
                                    Err(e) => status.set(StatusMsg::err(e.to_string())),
                                },
                                Err(e) => status.set(StatusMsg::err(e.to_string())),
                            }
                            busy.set(false);
                        });
                    }
                }
                SecondaryButton {
                    label: "Status".to_string(),
                    onclick: move |_| {
                        if busy() { return; }
                        let Some(p) = repo_path() else {
                            status.set(StatusMsg::err("Init a repository first."));
                            return;
                        };
                        busy.set(true);
                        spawn(async move {
                            match Repo::open(&p).await {
                                Ok(repo) => match repo.status().await {
                                    Ok(list) => {
                                        let n = list.len();
                                        entries.set(list);
                                        status.set(StatusMsg::ok(format!("{n} status entr(y/ies)")));
                                    }
                                    Err(e) => status.set(StatusMsg::err(e.to_string())),
                                },
                                Err(e) => status.set(StatusMsg::err(e.to_string())),
                            }
                            busy.set(false);
                        });
                    }
                }
            }
            if let Some(p) = repo_path() {
                BodyText { "Workdir" }
                MonoBlock { text: p }
            }
            BodyText { "Status" }
            StatusList { entries: entries() }
        }
    }
}
