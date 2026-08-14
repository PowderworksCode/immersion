//! The keymap: chord strings bound to actions, routed like everything else.
//!
//! A binding names an action and its params, exactly the shape a command bus
//! call takes — so a shortcut and a button are the same operation. Some
//! actions are layout commands (fired through the bus); others are host
//! concerns the bus does not own — undo, redo, maximize — which the host
//! dispatches itself. The keymap does not care which; it reports
//! `(action, params)` and lets the host decide.
//!
//! Chords use "Mod" for the platform command key (Ctrl on Linux/Windows, Cmd
//! on macOS); the JS shim resolves it, because only the browser knows the
//! platform. "Ctrl" means the literal Control key on every platform — Blender
//! binds workspace cycling to Ctrl-PageUp on all three, not Cmd.
//!
//! Some of Blender's chords are spoken for by the browser or the OS, and a
//! binding over one is a shortcut that silently does nothing. The defaults here
//! are web-first: Blender's where they are free, remapped where they collide
//! (maximize off Cmd+Space/Spotlight, workspace cycling off the tab-switch
//! chord). docs/keymap-web-safety.md is the record of which and why.

use dioxus::prelude::*;
use serde_json::Value;

const KEYMAP_JS: &str = include_str!("keymap.js");

/// One chord bound to an action.
#[derive(Debug, Clone, PartialEq)]
pub struct Binding {
    /// e.g. `"Ctrl+PageUp"`, `"Mod+Z"`, `"Mod+Space"`.
    pub chord: &'static str,
    /// A command name (dispatched through the bus) or a host action name
    /// (`"undo"`, `"redo"`, `"maximize"`). The host decides.
    pub action: &'static str,
    /// Human label — for the status bar hints, the cheat sheet, and (later) the
    /// keymap editor.
    pub description: &'static str,
    pub params: Value,
}

/// The default bindings — Blender's, where they apply.
pub fn default_keymap() -> Vec<Binding> {
    vec![
        Binding {
            chord: "Alt+PageDown",
            action: "workspace.cycle",
            description: "Next workspace",
            params: serde_json::json!({ "delta": 1 }),
        },
        Binding {
            chord: "Alt+PageUp",
            action: "workspace.cycle",
            description: "Previous workspace",
            params: serde_json::json!({ "delta": -1 }),
        },
        Binding {
            // Repeat Last (Blender's Shift+R). Bare Shift+letter, so the shim's
            // input guard keeps it from firing while typing in a field.
            chord: "Shift+R",
            action: "repeat_last",
            description: "Repeat last command",
            params: Value::Null,
        },
        Binding {
            chord: "Mod+Z",
            action: "undo",
            description: "Undo",
            params: Value::Null,
        },
        Binding {
            chord: "Mod+Shift+Z",
            action: "redo",
            description: "Redo",
            params: Value::Null,
        },
        Binding {
            // Blender maximizes with Ctrl+Space, but that is macOS Input
            // Sources and Cmd+Space is Spotlight — the OS eats both, so the
            // shortcut is simply dead on a Mac. Mod+Shift+Space is free
            // everywhere. See docs/keymap-web-safety.md.
            chord: "Mod+Shift+Space",
            action: "maximize",
            description: "Maximize area",
            params: Value::Null,
        },
        Binding {
            // Blender's Menu Search. F3 is free of any modifier collision; the
            // shim preventDefaults it so the browser's find-again does not also
            // fire.
            chord: "F3",
            action: "palette",
            description: "Command palette",
            params: Value::Null,
        },
        Binding {
            // Blender's context help key; here it opens the shortcut cheat
            // sheet. The shim preventDefaults it so the browser help does not.
            chord: "F1",
            action: "cheatsheet",
            description: "Keyboard shortcuts",
            params: Value::Null,
        },
        Binding {
            // Blender raises its view pie on the backquote; free in browsers.
            chord: "`",
            action: "pie",
            description: "Area pie menu",
            params: Value::Null,
        },
        Binding {
            // Blender's Quick Favourites. A bare letter, so the shim's input
            // guard keeps it from firing while typing in a field.
            chord: "Q",
            action: "favorites",
            description: "Quick favourites",
            params: Value::Null,
        },
        Binding {
            // Blender's Adjust Last Operation. F9 is unclaimed.
            chord: "F9",
            action: "adjust_last",
            description: "Adjust last operation",
            params: Value::Null,
        },
    ]
}

#[derive(Props, Clone)]
pub struct KeymapProps {
    pub bindings: Vec<Binding>,
    /// Fired with `(action, params)` when a bound chord is pressed.
    pub on_action: Callback<(String, Value)>,
}

impl PartialEq for KeymapProps {
    fn eq(&self, other: &Self) -> bool {
        self.bindings == other.bindings
    }
}

/// Installs the capture shim and routes chords to actions. Renders nothing.
#[component]
pub fn Keymap(props: KeymapProps) -> Element {
    let bindings = props.bindings.clone();
    let on_action = props.on_action;

    use_future(move || {
        let bindings = bindings.clone();
        async move {
            let chords: Vec<&str> = bindings.iter().map(|b| b.chord).collect();
            // Seed the shim with the chord set, then install it. The shim reads
            // window.__imChords at load, so the assignment must precede it.
            let setup = format!(
                "window.__imChords = {};\n{}",
                serde_json::to_string(&chords).unwrap_or_else(|_| "[]".into()),
                KEYMAP_JS
            );
            let mut channel = dioxus::document::eval(&setup);
            loop {
                let Ok(chord) = channel.recv::<String>().await else {
                    return; // channel closed; a reload re-installs
                };
                if let Some(b) = bindings.iter().find(|b| b.chord == chord) {
                    on_action.call((b.action.to_string(), b.params.clone()));
                }
            }
        }
    });

    rsx! {}
}

#[derive(Props, Clone)]
pub struct KeymapHelpProps {
    pub bindings: Vec<Binding>,
    /// Fired when the sheet should close (backdrop click).
    pub on_close: Callback<()>,
}

impl PartialEq for KeymapHelpProps {
    fn eq(&self, other: &Self) -> bool {
        self.bindings == other.bindings
    }
}

/// The shortcut cheat sheet: every binding's chord and what it does, over a
/// dimmed backdrop. The chords use the `im-hint-key` class the status-bar shim
/// already prettifies to the platform glyphs, so ⌘ shows on a Mac here too.
#[component]
pub fn KeymapHelp(props: KeymapHelpProps) -> Element {
    let on_close = props.on_close;
    rsx! {
        div {
            class: "im-help-backdrop",
            onclick: move |_| on_close.call(()),
            div {
                class: "im-help",
                onclick: move |e| e.stop_propagation(),
                div { class: "im-help-title", "Keyboard shortcuts" }
                div { class: "im-help-list",
                    for b in props.bindings.iter() {
                        div { class: "im-help-row", key: "{b.chord}",
                            span { class: "im-hint-key", "{b.chord}" }
                            span { class: "im-help-desc", "{b.description}" }
                        }
                    }
                }
            }
        }
    }
}
