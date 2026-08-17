//! The data editor.

use dioxus::prelude::*;
use immersion::{FilterBox, TreeView};

use crate::ui::State;

/// The data editor — Blender's Outliner in Data API mode, for our world. The
/// workbench's documents mounted under one root, every row addressable,
/// right-click ▸ Copy data path. What `set_setting` edits and what the MCP
/// tools read stops being invisible: this is the address space, on screen.
pub(crate) fn ed_data(s: &State, target: Option<String>) -> Element {
    let state_doc = serde_json::to_value(s).unwrap_or_default();
    // The target roots the view: an area pointed at /settings/favorites shows
    // that subtree and nothing else, which is what makes several data areas
    // useful side by side instead of three copies of the same scroll.
    let root = target.unwrap_or_default();
    let children_of = Callback::new(move |pointer: String| {
        let at = if pointer.is_empty() {
            root.clone()
        } else {
            pointer
        };
        data_children(&state_doc, &at)
    });
    rsx! {
        div { class: "data-editor im-filter-scope",
            div { class: "keymap-head", FilterBox { placeholder: "filter fields\u{2026}" } }
            TreeView { children_of }
        }
    }
}

/// Children under the mounted root. `""` lists the mounts; below, the pointer
/// starts with the mount's name and the rest addresses within that document.
pub(crate) fn data_children(
    state_doc: &serde_json::Value,
    pointer: &str,
) -> Vec<immersion::TreeRow> {
    if pointer.is_empty() {
        let mount = |name: &str, preview: &str| immersion::TreeRow {
            pointer: format!("/{name}"),
            label: format!("/{name}"),
            preview: preview.to_string(),
            has_children: true,
            icon: "braces".to_string(),
        };
        return vec![
            mount("layout", "the workspace tree"),
            mount("settings", "the settings document"),
            mount("keymap", "chord overrides"),
            mount("favorites", "the Q menu"),
            mount("state", "host snapshot (read-only)"),
        ];
    }
    let (mount, inner) = match pointer[1..].find('/') {
        Some(i) => (&pointer[..i + 1], &pointer[i + 1..]),
        None => (pointer, ""),
    };
    let doc = match mount {
        "/layout" => serde_json::to_value(crate::daemon::workspaces()).unwrap_or_default(),
        "/settings" => crate::daemon::settings(),
        "/keymap" => crate::daemon::settings()["keymap"].clone(),
        "/favorites" => crate::daemon::settings()["favorites"].clone(),
        "/state" => state_doc.clone(),
        _ => return Vec::new(),
    };
    let mut rows = immersion::value_children(&doc, inner);
    for r in &mut rows {
        r.pointer = format!("{mount}{}", r.pointer);
    }
    rows
}

/// This editor's entry in the registry: what it is called, how it is drawn in
/// a header, whether it takes a target, and what the status bar says while it
/// has focus. Declared beside the editor so adding one is one file.
pub(crate) fn kind() -> immersion::EditorKind {
    immersion::EditorKind {
        id: "data",
        label: "Data",
        icon: "binary-tree",
        hints: &[("Click", "Expand"), ("Right-click", "Copy data path")],
        targets: true,
    }
}
