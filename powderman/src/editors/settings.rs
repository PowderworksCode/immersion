//! The settings editor.

use dioxus::prelude::*;
use immersion::PropertyEditor;

/// The Settings editor: widgets over the settings document. Every control is
/// bound to a pointer by settings_fields(); an edit is (pointer, value) that
/// the host persists. Live: accent recolors the workbench, the interval
/// changes the poll cadence, the splash toggle mirrors the splash's own.
pub(crate) fn ed_settings(
    doc: serde_json::Value,
    on_setting: Callback<(String, serde_json::Value)>,
    on_error: Callback<immersion::EditorError>,
) -> Element {
    rsx! {
        div { class: "settings",
            PropertyEditor {
                doc,
                fields: crate::settings::settings_fields(),
                on_edit: on_setting,
                on_error,
            }
        }
    }
}

/// This editor's entry in the registry: what it is called, how it is drawn in
/// a header, whether it takes a target, and what the status bar says while it
/// has focus. Declared beside the editor so adding one is one file.
pub(crate) fn kind() -> immersion::EditorKind {
    immersion::EditorKind {
        id: "settings",
        label: "Settings",
        icon: "settings",
        hints: &[("Drag", "Scrub number"), ("Type", "3*2 works")],
        targets: false,
    }
}
