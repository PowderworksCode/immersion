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
    /// e.g. `"Ctrl+PageUp"`, `"Mod+Z"`, `"Mod+Shift+Space"`. Owned, because a
    /// user's rebind replaces it at runtime.
    pub chord: String,
    /// A command name (dispatched through the bus) or a host action name
    /// (`"undo"`, `"redo"`, `"maximize"`). The host decides.
    pub action: String,
    /// Human label — for the status bar hints, the cheat sheet, and the keymap
    /// editor.
    pub description: String,
    pub params: Value,
}

/// The default bindings — Blender's, where they apply.
pub fn default_keymap() -> Vec<Binding> {
    vec![
        Binding {
            chord: "Alt+PageDown".into(),
            action: "workspace.cycle".into(),
            description: "Next workspace".into(),
            params: serde_json::json!({ "delta": 1 }),
        },
        Binding {
            chord: "Alt+PageUp".into(),
            action: "workspace.cycle".into(),
            description: "Previous workspace".into(),
            params: serde_json::json!({ "delta": -1 }),
        },
        Binding {
            // Repeat Last (Blender's Shift+R). Bare Shift+letter, so the shim's
            // input guard keeps it from firing while typing in a field.
            chord: "Shift+R".into(),
            action: "repeat_last".into(),
            description: "Repeat last command".into(),
            params: Value::Null,
        },
        Binding {
            chord: "Mod+Z".into(),
            action: "undo".into(),
            description: "Undo".into(),
            params: Value::Null,
        },
        Binding {
            chord: "Mod+Shift+Z".into(),
            action: "redo".into(),
            description: "Redo".into(),
            params: Value::Null,
        },
        Binding {
            // Distraction-free fullscreen. Blender uses Ctrl+Alt+Space, which
            // is macOS Input Sources; Mod+Shift+F is free everywhere. See
            // docs/keymap-web-safety.md.
            chord: "Mod+Shift+F".into(),
            action: "fullscreen".into(),
            description: "Distraction-free fullscreen".into(),
            params: Value::Null,
        },
        Binding {
            // Blender maximizes with Ctrl+Space, but that is macOS Input
            // Sources and Cmd+Space is Spotlight — the OS eats both, so the
            // shortcut is simply dead on a Mac. Mod+Shift+Space is free
            // everywhere. See docs/keymap-web-safety.md.
            chord: "Mod+Shift+Space".into(),
            action: "maximize".into(),
            description: "Maximize area".into(),
            params: Value::Null,
        },
        Binding {
            // Blender's Menu Search. F3 is free of any modifier collision; the
            // shim preventDefaults it so the browser's find-again does not also
            // fire.
            chord: "F3".into(),
            action: "palette".into(),
            description: "Command palette".into(),
            params: Value::Null,
        },
        Binding {
            // Blender's context help key; here it opens the shortcut cheat
            // sheet. The shim preventDefaults it so the browser help does not.
            chord: "F1".into(),
            action: "cheatsheet".into(),
            description: "Keyboard shortcuts".into(),
            params: Value::Null,
        },
        Binding {
            // Blender raises its view pie on the backquote; free in browsers.
            chord: "`".into(),
            action: "pie".into(),
            description: "Area pie menu".into(),
            params: Value::Null,
        },
        Binding {
            // Blender's Quick Favourites. A bare letter, so the shim's input
            // guard keeps it from firing while typing in a field.
            chord: "Q".into(),
            action: "favorites".into(),
            description: "Quick favourites".into(),
            params: Value::Null,
        },
        Binding {
            // Blender's Adjust Last Operation. F9 is unclaimed.
            chord: "F9".into(),
            action: "adjust_last".into(),
            description: "Adjust last operation".into(),
            params: Value::Null,
        },
    ]
}

#[derive(Props, Clone)]
pub struct KeymapProps {
    pub bindings: Vec<Binding>,
    /// Fired with `(action, params)` when a bound chord is pressed.
    pub on_action: Callback<(String, Value)>,
    /// Fired with the chord the user pressed while capturing a rebind. The
    /// host decides which binding it belongs to.
    #[props(default)]
    pub on_capture: Callback<String>,
}

/// What the shim sends: a pressed chord, or one captured for a rebind.
#[derive(serde::Deserialize)]
#[serde(tag = "t", rename_all = "lowercase")]
enum Msg {
    Chord { chord: String },
    Capture { chord: String },
}

impl PartialEq for KeymapProps {
    fn eq(&self, other: &Self) -> bool {
        self.bindings == other.bindings
    }
}

/// Installs the capture shim and routes chords to actions. Renders nothing.
#[component]
pub fn Keymap(props: KeymapProps) -> Element {
    // The bindings live in a signal, refreshed whenever the props bring new
    // ones: the message loop below outlives a single render, and a rebind must
    // resolve against the current keymap rather than the one at mount.
    let mut bindings_sig = use_signal(|| props.bindings.clone());
    if *bindings_sig.peek() != props.bindings {
        bindings_sig.set(props.bindings.clone());
    }
    let on_action = props.on_action;
    let on_capture = props.on_capture;

    // Re-seed the shim's chord list whenever the bindings change. The
    // component only re-runs when they do (its props compare on bindings), so
    // this is exactly the right cadence — and a rebind takes effect without a
    // reload, because the shim reads this list fresh on every keypress.
    {
        let chords: Vec<String> = props.bindings.iter().map(|b| b.chord.clone()).collect();
        let js = format!(
            "window.__imChords = {};",
            serde_json::to_string(&chords).unwrap_or_else(|_| "[]".into())
        );
        dioxus::document::eval(&js);
    }

    use_future(move || {
        async move {
            // The chord list is (re)seeded above on every render; install the
            // shim, which reads window.__imChords fresh on each keypress.
            let mut channel = dioxus::document::eval(KEYMAP_JS);
            loop {
                let Ok(raw) = channel.recv::<String>().await else {
                    return; // channel closed; a reload re-installs
                };
                // Two kinds of message: a pressed chord, or a chord captured
                // for a rebind. JSON so they cannot be confused.
                match serde_json::from_str::<Msg>(&raw) {
                    Ok(Msg::Chord { chord }) => {
                        // Read the current keymap, not the one captured at mount.
                        let hit = bindings_sig
                            .peek()
                            .iter()
                            .find(|b| b.chord == chord)
                            .map(|b| (b.action.clone(), b.params.clone()));
                        if let Some(pair) = hit {
                            on_action.call(pair);
                        }
                    }
                    Ok(Msg::Capture { chord }) => on_capture.call(chord),
                    Err(_) => {}
                }
            }
        }
    });

    rsx! {}
}

#[derive(Props, Clone)]
pub struct KeymapHelpProps {
    pub bindings: Vec<Binding>,
    /// Write chords the Mac way.
    #[props(default)]
    pub mac: bool,
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
                            span { class: "im-hint-key", "{pretty_chord(&b.chord, props.mac)}" }
                            span { class: "im-help-desc", "{b.description}" }
                        }
                    }
                }
            }
        }
    }
}

/// Render a chord the way the platform writes it: `⌘⇧Z` on a Mac, `Ctrl+Shift+Z`
/// elsewhere. Done on the server, from a platform flag the client reports once
/// on connect — rewriting chord text in the DOM instead would detach the very
/// text nodes the renderer patches, so a chord that later changed would never
/// visibly update.
pub fn pretty_chord(chord: &str, mac: bool) -> String {
    if !mac {
        return chord.replace("Mod", "Ctrl");
    }
    chord
        .split('+')
        .map(|part| match part {
            "Mod" => "⌘",
            "Ctrl" => "⌃",
            "Alt" => "⌥",
            "Shift" => "⇧",
            other => other,
        })
        .collect::<String>()
}

const PLATFORM_JS: &str = r#"
// Report the platform once, so chords can be written server-side.
(() => {
  const isMac =
    /Mac|iPhone|iPad|iPod/.test(navigator.platform || "") ||
    /Mac OS X/.test(navigator.userAgent || "");
  try { dioxus.send(JSON.stringify({ mac: isMac })); } catch (e) { /* channel gone */ }
})();
"#;

#[derive(Props, Clone)]
pub struct PlatformProps {
    /// Fired once with true on macOS. Until then, assume not-Mac.
    pub on_platform: Callback<bool>,
}

impl PartialEq for PlatformProps {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

/// Reports whether the client is a Mac, once, on connect. Renders nothing.
#[component]
pub fn Platform(props: PlatformProps) -> Element {
    let on_platform = props.on_platform;
    use_future(move || async move {
        let mut channel = dioxus::document::eval(PLATFORM_JS);
        if let Ok(raw) = channel.recv::<String>().await
            && let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw)
        {
            on_platform.call(v["mac"].as_bool().unwrap_or(false));
        }
    });
    rsx! {}
}
