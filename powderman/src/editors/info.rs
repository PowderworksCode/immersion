//! The info editor.

use dioxus::prelude::*;

use crate::ui::State;
use crate::ui::{hhmmss, short};

/// The Info log: every command that ran, newest first — Blender's Info editor.
/// The workbench's own audit trail, and the more useful now that an agent
/// drives the same command bus: this is where you see what it did.
pub(crate) fn ed_info(s: &State) -> Element {
    rsx! {
        div { class: "info-log",
            if s.log.is_empty() {
                div { class: "note", "no commands yet" }
            }
            for (i, e) in s.log.iter().enumerate() {
                div {
                    class: if e.ok { "log-row" } else { "log-row failed" },
                    key: "{i}-{e.at}",
                    span { class: "when", "{hhmmss(e.at)}" }
                    span { class: "src {e.source}", "{e.source}" }
                    span { class: "k", "{e.name}" }
                    span { class: "note", "{short(&e.params.to_string(), 80)}" }
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
        id: "info",
        label: "Info log",
        icon: "info-circle",
        hints: &[("Type", "Filter")],
        targets: false,
    }
}
