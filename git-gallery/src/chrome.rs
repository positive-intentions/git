//! Shared demo chrome (status banner + read-only lists).

use dioxus::prelude::*;
use whatsup_ui::components::atoms::{BodyText, Button, ButtonVariant, Heading};

use git_core::{DirEntry, StatusEntry};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    Info,
    Ok,
    Err,
}

#[derive(Clone, PartialEq)]
pub struct StatusMsg {
    pub kind: StatusKind,
    pub text: String,
}

impl StatusMsg {
    pub fn info(text: impl Into<String>) -> Self {
        Self {
            kind: StatusKind::Info,
            text: text.into(),
        }
    }
    pub fn ok(text: impl Into<String>) -> Self {
        Self {
            kind: StatusKind::Ok,
            text: text.into(),
        }
    }
    pub fn err(text: impl Into<String>) -> Self {
        Self {
            kind: StatusKind::Err,
            text: text.into(),
        }
    }
}

#[component]
pub fn DemoShell(title: String, children: Element) -> Element {
    rsx! {
        div { class: "flex h-full flex-col gap-4 overflow-auto p-4",
            Heading { "{title}" }
            {children}
        }
    }
}

#[component]
pub fn StatusBanner(status: StatusMsg) -> Element {
    let cls = match status.kind {
        StatusKind::Info => "border-slate-300 bg-slate-50 text-slate-700",
        StatusKind::Ok => "border-emerald-500/40 bg-emerald-500/10 text-emerald-800",
        StatusKind::Err => "border-red-500/40 bg-red-500/10 text-red-800",
    };
    rsx! {
        div { class: "rounded-lg border px-3 py-2 text-sm {cls}",
            "{status.text}"
        }
    }
}

#[component]
pub fn ActionRow(children: Element) -> Element {
    rsx! {
        div { class: "flex flex-wrap gap-2",
            {children}
        }
    }
}

#[component]
pub fn PrimaryButton(label: String, onclick: EventHandler<MouseEvent>) -> Element {
    rsx! {
        Button {
            variant: ButtonVariant::Primary,
            onclick: move |e| onclick.call(e),
            "{label}"
        }
    }
}

#[component]
pub fn SecondaryButton(label: String, onclick: EventHandler<MouseEvent>) -> Element {
    rsx! {
        Button {
            variant: ButtonVariant::Secondary,
            onclick: move |e| onclick.call(e),
            "{label}"
        }
    }
}

#[component]
pub fn DirList(entries: Vec<DirEntry>) -> Element {
    if entries.is_empty() {
        return rsx! { BodyText { "No entries." } };
    }
    rsx! {
        ul { class: "divide-y divide-slate-200 rounded-lg border border-slate-200",
            for entry in entries {
                {
                    let kind = if entry.is_dir { "dir" } else { "file" };
                    let path = entry.path.clone();
                    rsx! {
                        li { class: "flex items-center justify-between px-3 py-2 text-sm",
                            span { class: "font-mono", "{path}" }
                            span { class: "text-xs text-slate-500", "{kind}" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn StatusList(entries: Vec<StatusEntry>) -> Element {
    if entries.is_empty() {
        return rsx! { BodyText { "Clean working tree." } };
    }
    rsx! {
        ul { class: "divide-y divide-slate-200 rounded-lg border border-slate-200",
            for entry in entries {
                {
                    let path = entry.path.clone();
                    let status = entry.status.clone();
                    rsx! {
                        li { class: "flex items-center justify-between px-3 py-2 text-sm",
                            span { class: "font-mono", "{path}" }
                            span { class: "text-xs font-semibold uppercase tracking-wide text-sky-700", "{status}" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn MonoBlock(text: String) -> Element {
    rsx! {
        pre { class: "overflow-auto rounded-lg border border-slate-200 bg-slate-50 p-3 text-xs",
            "{text}"
        }
    }
}

/// Labeled text / password input used by Clone and Storage connect forms.
#[component]
pub fn FormField(
    label: String,
    value: String,
    input_type: String,
    oninput: EventHandler<String>,
) -> Element {
    rsx! {
        label { class: "flex flex-col gap-1 text-sm",
            span { class: "text-slate-600", "{label}" }
            input {
                r#type: "{input_type}",
                class: "rounded-md border border-slate-300 px-2 py-1.5 font-mono text-sm",
                value: "{value}",
                oninput: move |e| oninput.call(e.value()),
            }
        }
    }
}
