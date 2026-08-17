//! The status bar: Blender's bottom strip.
//!
//! Left are the keymap hints — the chords in play, the contextual key line that
//! makes the workbench feel discoverable. Right is a host segment (a version,
//! counts — whatever the host wants to keep in view). The center is left for a
//! report toast a later slice fills.
//!
//! The hints are handed in as `(chord, label)` pairs, so the bar stays
//! domain-agnostic and the host decides which few to surface. The chords arrive
//! in grammar form (`Mod+Z`); a tiny shim ([`statusbar.js`]) rewrites `Mod` to
//! the platform glyph (`⌘` on a Mac, `Ctrl` elsewhere) client-side, because
//! only the browser knows the platform. Nothing here is continuous input, so
//! the bar itself is plain server-rendered chrome.

use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct StatusBarProps {
    /// `(chord, label)` pairs for the left slot, e.g. `("Mod+Z", "Undo")`.
    pub hints: Vec<(String, String)>,
    /// A transient report shown in the centre — the last operation, or a
    /// warning — that the host clears after a moment. `None` shows nothing.
    #[props(default)]
    pub message: Option<String>,
    /// The right-hand segment: a version, counts, whatever the host keeps in
    /// view.
    #[props(default)]
    pub right: String,
    /// A running task: `(label, fraction)`. Blender's status bar grows a
    /// progress bar while something long is happening and loses it again when
    /// nothing is. The fraction is 0..=1; `None` for work whose size is not
    /// known, which shows the label and a bar that does not claim a position.
    #[props(default)]
    pub task: Option<(String, Option<f32>)>,
    /// A standing label pinned to the far right — what this instance *is*,
    /// not what it is doing. A demo says DEMO here so its numbers are never
    /// read as a live box. `None` shows nothing.
    #[props(default)]
    pub badge: Option<String>,
}

/// The bottom status strip. Place it after the deck; `.app` is a column, so it
/// settles at the bottom on its own.
#[component]
pub fn StatusBar(props: StatusBarProps) -> Element {
    rsx! {
        div { class: "im-statusbar",
            div { class: "im-status-hints",
                for (chord, label) in props.hints.iter().cloned() {
                    span { class: "im-hint",
                        span { class: "im-hint-key", "{chord}" }
                        span { class: "im-hint-label", "{label}" }
                    }
                }
            }
            if let Some(msg) = props.message.clone() {
                div { class: "im-status-report", "{msg}" }
            }
            if let Some((label, done)) = props.task.clone() {
                div { class: "im-status-task",
                    span { class: "im-status-task-label", "{label}" }
                    div {
                        class: if done.is_some() { "im-progress" } else { "im-progress im-progress-idle" },
                        // The width is the whole mechanism: a bar with no
                        // known fraction gets a stripe that says "working"
                        // rather than a position it would be inventing.
                        div {
                            class: "im-progress-fill",
                            style: match done {
                                Some(f) => format!("width:{:.0}%", (f.clamp(0.0, 1.0)) * 100.0),
                                None => String::new(),
                            },
                        }
                    }
                }
            }
            div { class: "im-status-right", "{props.right}" }
            if let Some(badge) = props.badge.clone() {
                div { class: "im-status-badge", title: "fabricated data — no live fleet", "{badge}" }
            }
        }
    }
}
