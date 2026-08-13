//! Gallery application root: routing, GalleryHost, document head.

use dioxus::prelude::*;

use whatsup_ui::call_state::CallController;
use whatsup_ui::components::pages::{CoveragePage, GalleryHome, StoryView};
use whatsup_ui::components::templates::GalleryShell;
use whatsup_ui::data::Theme;
use whatsup_ui::gallery::GalleryHost;
use whatsup_ui::nav::{Nav, NavTarget};
use whatsup_ui::state::AppState;
use whatsup_ui::STYLES;

use crate::stories::{GUI_STORIES, TUI_STORIES};

const COVERAGE_REGENERATE_CMD: &str =
    "cargo llvm-cov -p git-core --html --output-dir git-gallery/assets/coverage-html";

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[route("/")]
    Home {},
    #[route("/coverage")]
    Coverage {},
    #[route("/demo/:kind/:group/:name")]
    Demo { kind: String, group: String, name: String },
    #[route("/:..route")]
    NotFound { route: Vec<String> },
}

#[component]
fn Home() -> Element {
    rsx! {
        GalleryShell { active_slug: String::new(),
            GalleryHome {}
        }
    }
}

#[component]
fn Coverage() -> Element {
    rsx! {
        GalleryShell { active_slug: "coverage".to_string(),
            CoveragePage {}
        }
    }
}

#[component]
fn Demo(kind: String, group: String, name: String) -> Element {
    let active_slug = format!("{kind}/{group}/{name}");
    rsx! {
        GalleryShell { active_slug,
            StoryView { kind, group, name }
        }
    }
}

#[component]
fn NotFound(route: Vec<String>) -> Element {
    let path = route.join("/");
    rsx! {
        GalleryShell { active_slug: String::new(),
            div { class: "flex flex-1 items-center justify-center p-8 text-wa-muted dark:text-wa-muted-dark",
                "Not found: /{path}"
            }
        }
    }
}

#[component]
pub fn App() -> Element {
    use_context_provider(|| {
        GalleryHost::new(
            "git",
            "Git gallery",
            "/",
            Some("/coverage"),
            |kind, group, name| format!("/demo/{kind}/{group}/{name}"),
            GUI_STORIES,
            TUI_STORIES,
        )
        .with_coverage_regenerate_cmd(COVERAGE_REGENERATE_CMD)
    });

    let app_state = use_context_provider(|| Signal::new(AppState::mock()));
    let next_call_id = app_state.read().next_call_history_id();
    use_context_provider(|| Signal::new(CallController::new(next_call_id)));
    let theme = use_context_provider(|| Signal::new(Theme::Light));
    let dark_class = if theme().is_dark() { "dark" } else { "" };

    use_context_provider(|| {
        Nav::new(
            |target: &NavTarget| match target {
                NavTarget::Chats => "/".to_string(),
                NavTarget::Chat(_) => "/".to_string(),
                NavTarget::Calls => "/".to_string(),
                NavTarget::Profile => "/".to_string(),
                NavTarget::ActiveCall { .. } => "/".to_string(),
            },
            |_target| {},
        )
    });

    rsx! {
        document::Meta {
            name: "description",
            content: "Interactive Git demos (git-core: gix on desktop, isomorphic-git on web)",
        }
        document::Link { rel: "stylesheet", href: STYLES }
        // OPFS + isomorphic-git bridge for the wasm backend (no-op on desktop).
        // Must be type=module: git-web.js imports ./git-web-helpers.js.
        document::Script {
            r#type: "module",
            src: asset!("/assets/git-web.js"),
        }
        // Monaco host + localStorage helpers for the Clone story (web debugger).
        // Classic script + IIFE so double-injection (hashed + plain asset) cannot
        // throw on redeclared const; Clone waits for globalThis.MonacoHost.
        document::Script {
            src: asset!("/assets/monaco-host.js"),
        }
        div { class: "{dark_class}",
            Router::<Route> {}
        }
    }
}
