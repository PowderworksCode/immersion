//! The fleet editor.

use dioxus::prelude::*;

use crate::ui::State;
use crate::ui::{gib, short};

pub(crate) fn ed_fleet(s: &State) -> Element {
    rsx! {
        if s.fleet.is_empty() {
            div { class: "empty", "no agents" }
        }
        for a in s.fleet.iter() {
            div { class: "agent", key: "{a.pane}",
                span { class: "status {a.status}", "{a.status}" }
                span { class: "wf", "{a.name}" }
                span { class: "procs",
                    // The two biggest by memory, not every pid. A full process
                    // table is htop's job; this row answers "what is this
                    // agent doing and how big is it".
                    for p in a.procs.iter().take(2) {
                        span { class: "proc", key: "{p.pid}", "{p.name}" }
                    }
                    if a.procs.len() > 2 {
                        span { class: "note", "+{a.procs.len() - 2}" }
                    }
                    span { class: "note", "{gib(a.procs.iter().map(|p| p.rss).sum::<f64>())}" }
                    span { class: "note", "{short(&a.cwd, 46)}" }
                }
            }
        }
    }
}

/// This editor's entry in the registry: what it is called, how it is drawn in
/// a header, whether it takes a target, and what the status bar says while it
/// has focus. Declared beside the editor so adding one is one file.
pub(crate) fn kind() -> immersion::EditorKind {
    immersion::EditorKind {
        id: "fleet",
        label: "Fleet",
        icon: "binary-tree",
        hints: &[],
        targets: false,
    }
}
