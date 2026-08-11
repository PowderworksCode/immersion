//! The command palette: Blender's Menu Search (F3), over the same actions.
//!
//! A palette is a list of [`PaletteItem`]s — each an `(action, params)` pair,
//! the exact shape the keymap fires and the command bus runs, with a human
//! label to search by and the chord it is bound to (if any) to show. Picking
//! one calls `on_run(action, params)`, so a palette entry, a keyboard chord,
//! and (later) an agent's tool call are the same operation reaching the same
//! router. The palette invents no third write path; it is a searchable face on
//! the ones that exist.
//!
//! The filtering and arrowing are client-side, on the library's liveview
//! budget: the shim ([`palette.js`]) hides non-matching rows and moves the
//! selection without a round trip, and commits exactly one message — the chosen
//! row's index — on Enter or click (or `-1` on Escape / clicking the scrim).

use dioxus::prelude::*;
use serde_json::Value;

const PALETTE_JS: &str = include_str!("palette.js");

/// One searchable entry: an action to run, with what to show for it.
#[derive(Debug, Clone, PartialEq)]
pub struct PaletteItem {
    /// The action name the host router understands — a bus command
    /// (`"workspace.switch"`) or a host action (`"undo"`).
    pub action: String,
    pub label: String,
    pub hint: Option<String>,
    /// The chord bound to this action, in grammar form (`"Mod+Z"`); the shim
    /// prettifies `Mod` to `⌘` on a Mac. `None` if nothing is bound.
    pub chord: Option<String>,
    /// The params passed with the action — baked here so "Next workspace" and
    /// "Previous workspace" are two entries on one command.
    pub params: Value,
}

impl PaletteItem {
    pub fn new(action: &str, label: &str) -> Self {
        PaletteItem {
            action: action.to_string(),
            label: label.to_string(),
            hint: None,
            chord: None,
            params: Value::Null,
        }
    }
    pub fn with_hint(mut self, hint: &str) -> Self {
        self.hint = Some(hint.to_string());
        self
    }
    pub fn with_chord(mut self, chord: &str) -> Self {
        self.chord = Some(chord.to_string());
        self
    }
    pub fn with_params(mut self, params: Value) -> Self {
        self.params = params;
        self
    }
}

#[derive(Props, Clone)]
pub struct PaletteProps {
    pub items: Vec<PaletteItem>,
    /// `(action, params)` for the picked entry. The host routes it exactly as it
    /// routes a keymap action.
    pub on_run: Callback<(String, Value)>,
    /// Fired when the palette should close — after a pick, or on cancel.
    pub on_close: Callback<()>,
}

impl PartialEq for PaletteProps {
    fn eq(&self, other: &Self) -> bool {
        // Only the item list drives a re-render; the callbacks are stable. This
        // is what lets the shim's client-side filter survive the host's poll —
        // the palette subtree does not re-render while the items are unchanged.
        self.items == other.items
    }
}

/// One palette row: the label (and hint) on the left, the chord on the right.
/// `data-text` is what the shim matches against; `data-index` is what it sends
/// back on a pick.
fn palette_row(i: usize, item: &PaletteItem) -> Element {
    rsx! {
        div {
            class: "im-palette-row",
            "data-index": "{i}",
            "data-text": "{item.label.to_lowercase()} {item.action.to_lowercase()}",
            div { class: "im-palette-main",
                span { class: "im-palette-label", "{item.label}" }
                if let Some(h) = item.hint.clone() {
                    span { class: "im-palette-hint", "{h}" }
                }
            }
            if let Some(c) = item.chord.clone() {
                span { class: "im-palette-chord", "{c}" }
            }
        }
    }
}

/// The palette overlay. Renders the scrim, the search input, and a row per
/// item; the shim drives the rest.
#[component]
pub fn Palette(props: PaletteProps) -> Element {
    let items = props.items.clone();
    let on_run = props.on_run;
    let on_close = props.on_close;

    // The shim commits the chosen index (or -1 to cancel); route it and close.
    use_future(move || {
        let items = items.clone();
        async move {
            let mut channel = dioxus::document::eval(PALETTE_JS);
            if let Ok(idx) = channel.recv::<i64>().await
                && idx >= 0
                && let Some(item) = items.get(idx as usize)
            {
                on_run.call((item.action.clone(), item.params.clone()));
            }
            on_close.call(());
        }
    });

    rsx! {
        div { class: "im-palette-scrim",
            div { class: "im-palette",
                input {
                    class: "im-palette-input",
                    r#type: "text",
                    placeholder: "search commands…",
                    spellcheck: "false",
                    autocomplete: "off",
                }
                div { class: "im-palette-list",
                    for (i, item) in props.items.iter().enumerate() {
                        {palette_row(i, item)}
                    }
                }
            }
        }
    }
}
