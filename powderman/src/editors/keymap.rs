//! The keymap editor.

use dioxus::prelude::*;
use immersion::{FilterBox, pretty_chord};

use crate::ui::effective_keymap;

/// The Keymap editor: every binding, what it does, and its chord — with a
/// capture button that listens for the next chord you press. Blender's keymap
/// preferences, minus the filtering.
pub(crate) fn ed_keymap(
    settings: serde_json::Value,
    mac: bool,
    capturing: Option<String>,
    on_capture_start: Callback<String>,
    on_reset: Callback<String>,
) -> Element {
    let overrides = settings["keymap"].clone();
    rsx! {
        div { class: "keymap-editor im-filter-scope",
            div { class: "keymap-head", FilterBox { placeholder: "filter shortcuts…" } }
            for b in effective_keymap(&settings) {
                {
                    let custom = overrides.get(&b.action).is_some();
                    let waiting = capturing.as_deref() == Some(b.action.as_str());
                    keymap_row(b, mac, custom, waiting, on_capture_start, on_reset)
                }
            }
        }
    }
}

/// One row of the keymap editor. Its own function so the editor's view does not
/// nest another four levels deep.
fn keymap_row(
    b: immersion::Binding,
    mac: bool,
    custom: bool,
    waiting: bool,
    on_capture_start: Callback<String>,
    on_reset: Callback<String>,
) -> Element {
    let a1 = b.action.clone();
    let a2 = b.action.clone();
    rsx! {
        div {
            class: "keymap-row",
            key: "{b.action}",
            "data-filter-text": "{b.description} {b.action} {b.chord}",
            span { class: "keymap-desc", "{b.description}" }
            span { class: "im-hint-key keymap-chord", "{pretty_chord(&b.chord, mac)}" }
            button {
                class: if waiting { "keymap-set waiting" } else { "keymap-set" },
                onclick: move |_| on_capture_start.call(a1.clone()),
                if waiting { "press a key…" } else { "rebind" }
            }
            if custom {
                button {
                    class: "keymap-reset",
                    title: "restore the default",
                    onclick: move |_| on_reset.call(a2.clone()),
                    "↺"
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
        id: "keymap",
        label: "Keymap",
        icon: "keyboard",
        hints: &[("Set", "Rebind"), ("Type", "Filter")],
        targets: false,
    }
}
