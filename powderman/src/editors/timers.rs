//! The timers editor.

use dioxus::prelude::*;

use crate::ui::State;

pub(crate) fn ed_timers(s: &State) -> Element {
    rsx! {
        div { class: "timers",
            if s.timers.is_empty() {
                div { class: "note", "nothing scheduled" }
            }
            for t in s.timers.iter() {
                div { class: "timer", key: "{t.source}-{t.name}",
                    span { class: "src {t.source}", "{t.source}" }
                    span { class: "k", "{t.name}" }
                    span { class: "note", "{t.schedule}" }
                    span { class: "when", "{t.next}" }
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
        id: "timers",
        label: "Timers",
        icon: "clock",
        hints: &[],
        targets: false,
    }
}
