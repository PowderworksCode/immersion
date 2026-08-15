//! A filter box — type to narrow a list, entirely client-side.
//!
//! Filtering is keystroke-driven, which is exactly what the liveview budget
//! forbids sending: a message per character to re-render a list would feel like
//! typing through syrup. So the rows are already in the DOM, each carrying a
//! `data-filter-text`, and the client bundle hides the ones that do not match as
//! you type. The server is never told; nothing is committed. Clearing the box
//! restores the list.
//!
//! Rows opt in by carrying `data-filter-text` and living inside the same
//! `.im-filter-scope` as the box.

use dioxus::prelude::*;

#[component]
pub fn FilterBox(
    /// Placeholder text, e.g. "filter runs…".
    #[props(default = String::from("filter…"))]
    placeholder: String,
) -> Element {
    rsx! {
        input {
            class: "im-filter",
            r#type: "text",
            placeholder: "{placeholder}",
            spellcheck: "false",
            autocomplete: "off",
            "data-im-filter": "1",
        }
    }
}
