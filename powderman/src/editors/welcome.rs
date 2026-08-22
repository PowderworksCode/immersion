//! The welcome editor: what this is, and how to start using it.
//!
//! The old Immersion had a pane like this and it was the only place that
//! answered "what am I looking at" from inside the workbench. The splash says
//! some of it, but a splash is dismissed before you have a reason to read it,
//! and it cannot hand you an editor.
//!
//! Everything on it is read from somewhere that already knows: the editor
//! grid is the registry, the chords are the status bar's list, the agent line
//! is the address the server is reachable at. Nothing here is a second copy
//! of anything, so nothing here goes stale.

use dioxus::prelude::*;

use crate::editors::Draw;

pub(crate) fn ed_welcome(d: &Draw) -> Element {
    rsx! {
        div { class: "welcome",
            div { class: "welcome-eyebrow", "durable workflows for a herd of agents" }
            h1 { class: "welcome-brand", "powderman" }
            p { class: "welcome-lede",
                "The layout is the workspace. Split and join areas by dragging a corner, "
                "turn any area into any editor from its header, and drive all of it by "
                "hand — or hand the keys to an agent over MCP."
            }
            {jump_grid(d)}
            div { class: "welcome-cols",
                {keys_col(d.mac)}
                {agent_col()}
            }
        }
    }
}

/// Every editor this build has, as something to click. Generated from the
/// registry, so an editor added tomorrow is on this page tomorrow — which is
/// the whole reason it is a grid and not a paragraph naming a few.
fn jump_grid(d: &Draw) -> Element {
    let (area, cmd) = (d.area, d.cmd);
    rsx! {
        div { class: "welcome-h", "Jump into an editor" }
        div { class: "welcome-note",
            "This area becomes what you pick — the same in-place switch as the header's dropdown."
        }
        div { class: "welcome-grid",
            for kind in crate::editors::kinds() {
                button {
                    key: "{kind.id}",
                    class: "welcome-jump",
                    title: "{kind.id}",
                    onclick: move |_| {
                        cmd.call((
                            "set_editor".to_string(),
                            serde_json::json!({ "id": area, "editor": kind.id }),
                        ));
                    },
                    span {
                        class: "welcome-jump-icon",
                        dangerous_inner_html: "{immersion::icon(kind.icon)}",
                    }
                    "{kind.label}"
                }
            }
        }
    }
}

/// The chords worth knowing — the status bar's own list, so the two cannot
/// name different keys for the same thing.
fn keys_col(mac: bool) -> Element {
    rsx! {
        div { class: "welcome-col",
            div { class: "welcome-h", "Keyboard basics" }
            for (chord, label) in crate::status::status_hints(mac, None) {
                div { class: "welcome-key", key: "{label}",
                    span { class: "im-hint-key", "{chord}" }
                    span { class: "welcome-note", "{label}" }
                }
            }
            div { class: "welcome-key",
                span { class: "im-hint-key", "Right-click" }
                span { class: "welcome-note", "Context menus" }
            }
        }
    }
}

/// The handoff, at the address this instance is actually reachable at.
fn agent_col() -> Element {
    let line = format!(
        "claude mcp add --transport http powderman {}/mcp",
        crate::daemon::public_url()
    );
    rsx! {
        div { class: "welcome-col",
            div { class: "welcome-h", "Hand it to an agent" }
            div { class: "welcome-note",
                "Everything on this page is a command an agent can run too, over the same bus."
            }
            div { class: "im-splash-copy",
                code { "{line}" }
                button {
                    class: "im-copy",
                    title: "copy",
                    "data-im-copy": "{line}",
                    dangerous_inner_html: "{immersion::icon(\"copy\")}",
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
        id: "welcome",
        label: "Welcome",
        icon: "home",
        hints: &[("Click", "Become that editor")],
        targets: false,
    }
}

pub(crate) fn footer(_d: &Draw) -> String {
    format!(
        "{} editors · v{}",
        crate::editors::kinds().len(),
        env!("CARGO_PKG_VERSION")
    )
}

#[cfg(test)]
mod tests {
    /// The grid is the registry, so every button on it emits `set_editor`
    /// with an id the registry supplies. A dead button here would be an
    /// editor you can see and cannot open — and the parity test would be
    /// satisfied, because the action name is fine and only the id is wrong.
    #[test]
    fn every_editor_on_the_grid_can_actually_be_opened() {
        let commands = crate::workflows::commands();
        let kinds = crate::editors::kinds();
        assert!(kinds.len() > 10, "the grid looks empty: {}", kinds.len());
        for kind in kinds {
            let mut ws = immersion::Workspaces::new("test", immersion::Layout::single("runs"));
            commands
                .run(
                    &mut ws,
                    "set_editor",
                    &serde_json::json!({ "id": 1, "editor": kind.id }),
                )
                .unwrap_or_else(|e| panic!("the {} button does nothing: {e}", kind.label));
            assert!(
                matches!(
                    ws.current().layout.root.find(1),
                    Some(immersion::Area::Leaf { editor, .. }) if editor == kind.id
                ),
                "{} did not take",
                kind.label
            );
        }
    }

    /// Welcome is the page that explains the workbench, so it is the first
    /// thing a first visit lands on. Both halves: the tab, and the editor in
    /// it — a Start workspace showing something else would be the same bug as
    /// no Start workspace at all.
    #[test]
    fn a_first_visit_lands_on_it() {
        let ws = crate::daemon::default_workspaces();
        let first = &ws.tabs[0];
        assert_eq!(first.name, "Start");
        assert!(
            matches!(
                first.layout.root.find(1),
                Some(immersion::Area::Leaf { editor, .. }) if editor == "welcome"
            ),
            "the Start workspace does not open on the welcome pane"
        );
    }
}
