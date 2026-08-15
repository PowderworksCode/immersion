//! Collapsible panels — Blender's property sections inside a region.
//!
//! A [`Panel`] is a titled section with a body that folds away when its header
//! is clicked. Blender's N-panel and toolbars are stacks of these. Collapse is
//! per-client view state (a local signal), not layout: one person folding a
//! panel does not fold it for another, and it costs no server message.

use dioxus::prelude::*;

#[component]
pub fn Panel(
    /// The header label; clicking the header folds the body.
    title: String,
    /// Open by default; pass false for a panel that starts folded.
    #[props(default = true)]
    open: bool,
    children: Element,
) -> Element {
    let mut collapsed = use_signal(move || !open);
    rsx! {
        div { class: "im-panel",
            div {
                class: "im-panel-head",
                onclick: move |_| collapsed.toggle(),
                span { class: "im-panel-arrow", if collapsed() { "▸" } else { "▾" } }
                span { class: "im-panel-title", "{title}" }
            }
            if !collapsed() {
                div { class: "im-panel-body", {children} }
            }
        }
    }
}
