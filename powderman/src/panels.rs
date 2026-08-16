//! The modal panels: overlays that sit above the deck rather than inside an
//! area. Both are per-client view state — one person adjusting a command or
//! picking a target does not open a modal in someone else's browser — which
//! is why they live beside the App rather than in the layout tree.
//!
//! Split out of `ui.rs` to keep that file about the workbench frame.

use dioxus::prelude::*;
use immersion::{AreaId, FilterBox, PropertyEditor, TreeView};

use crate::ui::{State, fields_from_params};

/// The Adjust Last panel: the last command's params as an editable form, with
/// Cancel and Apply. Extracted so the App view does not nest another four
/// levels deep.
#[component]
pub(crate) fn AdjustPanel(
    name: String,
    doc: serde_json::Value,
    on_edit: Callback<(String, serde_json::Value)>,
    on_error: Callback<immersion::EditorError>,
    on_cancel: Callback<()>,
    on_apply: Callback<()>,
) -> Element {
    let fields = fields_from_params(&doc);
    rsx! {
        div { class: "adjust-backdrop", onclick: move |_| on_cancel.call(()),
            div { class: "adjust-panel", onclick: move |e| e.stop_propagation(),
                div { class: "adjust-title", "Adjust: {name}" }
                PropertyEditor { doc, fields, on_edit, on_error }
                div { class: "adjust-actions",
                    button { class: "adjust-cancel", onclick: move |_| on_cancel.call(()), "Cancel" }
                    button { class: "adjust-apply", onclick: move |_| on_apply.call(()), "Apply" }
                }
            }
        }
    }
}

/// The target picker: the data tree in a modal, so retargeting an area is
/// choosing a node rather than typing a pointer. The host owns this rather
/// than the library because only the host knows what can be pointed at — the
/// library just says which editors have a target and reports the click.
#[component]
pub(crate) fn TargetPicker(
    area: AreaId,
    editor: String,
    current: Option<String>,
    state: State,
    on_pick: Callback<(AreaId, String)>,
    on_cancel: Callback<()>,
) -> Element {
    // A files editor picks paths; everything else picks pointers into the
    // workbench's documents. Same component, different feed.
    let files = editor == "files";
    let state_doc = serde_json::to_value(&state).unwrap_or_default();
    let children_of = Callback::new(move |pointer: String| {
        if files {
            crate::editors::file_children(&pointer)
        } else {
            crate::editors::data_children(&state_doc, &pointer)
        }
    });
    let mut chosen = use_signal(|| current.clone().unwrap_or_default());
    let on_row = Callback::new(move |row: immersion::TreeRow| chosen.set(row.pointer));
    rsx! {
        div { class: "adjust-backdrop", onclick: move |_| on_cancel.call(()),
            div { class: "adjust-panel target-panel", onclick: move |e| e.stop_propagation(),
                div { class: "adjust-title", "Target for area {area}" }
                div { class: "target-current", "{chosen()}" }
                div { class: "target-tree im-filter-scope",
                    div { class: "keymap-head", FilterBox { placeholder: "filter\u{2026}" } }
                    TreeView { children_of, on_pick: on_row }
                }
                div { class: "adjust-actions",
                    button {
                        class: "adjust-cancel",
                        onclick: move |_| on_pick.call((area, String::new())),
                        "Clear"
                    }
                    button { class: "adjust-cancel", onclick: move |_| on_cancel.call(()), "Cancel" }
                    button {
                        class: "adjust-apply",
                        onclick: move |_| on_pick.call((area, chosen())),
                        "Set target"
                    }
                }
            }
        }
    }
}
