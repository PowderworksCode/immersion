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

const STATUSBAR_JS: &str = include_str!("statusbar.js");

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
}

/// The bottom status strip. Place it after the deck; `.app` is a column, so it
/// settles at the bottom on its own.
#[component]
pub fn StatusBar(props: StatusBarProps) -> Element {
    use_future(|| async {
        dioxus::document::eval(STATUSBAR_JS);
    });
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
            div { class: "im-status-right", "{props.right}" }
        }
    }
}
