use dioxus::prelude::*;
use git_core::{suggest_workdir, GitRepo, Repo};
use whatsup_ui::components::atoms::BodyText;

use crate::chrome::{ActionRow, DemoShell, MonoBlock, PrimaryButton, StatusBanner, StatusMsg};

whatsup_ui::register_gui_story! {
    name: "Init",
    group: "Git",
    docs: include_str!("init.md"),
    knobs: [],
    render: |_| {
        rsx! { InitDemo {} }
    },
}

#[component]
fn InitDemo() -> Element {
    let mut status = use_signal(|| StatusMsg::info("Press Init to create a throwaway repository."));
    let mut workdir = use_signal(|| String::new());
    let mut busy = use_signal(|| false);

    rsx! {
        DemoShell { title: "Init".to_string(),
            StatusBanner { status: status() }
            ActionRow {
                PrimaryButton {
                    label: if busy() { "Working…".to_string() } else { "Init".to_string() },
                    onclick: move |_| {
                        if busy() { return; }
                        busy.set(true);
                        spawn(async move {
                            let path = suggest_workdir("git-gallery");
                            match Repo::init(&path).await {
                                Ok(repo) => {
                                    workdir.set(repo.workdir().to_string());
                                    status.set(StatusMsg::ok(format!("Initialized at {}", repo.workdir())));
                                }
                                Err(e) => status.set(StatusMsg::err(e.to_string())),
                            }
                            busy.set(false);
                        });
                    }
                }
            }
            if !workdir().is_empty() {
                BodyText { "Workdir" }
                MonoBlock { text: workdir() }
            }
        }
    }
}
