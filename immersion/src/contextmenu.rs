//! The context menu: right-click an area for its operations.
//!
//! The menu itself lives in the DOM — each right-clickable element carries its
//! items as a `data-im-menu` attribute — and the shim ([`contextmenu.js`]) draws
//! and drives it entirely client-side. This component only opens the eval
//! channel the shim commits a pick over, and hands each pick to the host as
//! `(action, params)` — the same shape a button or a chord produces, so a
//! right-click Split and a header-button Split are one operation on one bus.
//!
//! Renders nothing; place it once, like [`crate::Keymap`].

use dioxus::prelude::*;
use serde_json::Value;

const CONTEXTMENU_JS: &str = include_str!("contextmenu.js");

/// One row of a menu. Menus were assembled as JSON text, which nothing on this
/// side checked and which broke the moment a label carried a quote — a run id
/// or a workspace name would have been enough. They are values now, serialised
/// by serde, and the shim's `MenuItem` is generated from this rather than
/// written to match it.
#[derive(Debug, Clone, Default, serde::Serialize, ts_rs::TS)]
#[ts(export, export_to = "../ts/generated/")]
pub struct MenuItem {
    // `optional` so the generated type says `label?: string` — matching what
    // serde actually emits, since these are skipped when absent. Without it the
    // TypeScript would promise a key that is not there.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "unknown")]
    pub params: Option<Value>,
    /// The chord to show right-aligned, already written the platform's way.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub chord: Option<String>,
    /// A divider rather than a row. Always emitted, so the type can promise it.
    pub sep: bool,
}

impl MenuItem {
    /// A row that runs `action` with `params`.
    pub fn new(label: &str, action: &str, params: Value) -> Self {
        MenuItem {
            label: Some(label.to_string()),
            action: Some(action.to_string()),
            params: Some(params),
            ..Default::default()
        }
    }

    /// Show a chord beside the label.
    pub fn with_chord(mut self, chord: &str) -> Self {
        self.chord = Some(chord.to_string());
        self
    }

    /// A divider.
    pub fn sep() -> Self {
        MenuItem {
            sep: true,
            ..Default::default()
        }
    }
}

/// Serialise a menu for a `data-im-menu` attribute. Escaping is serde's problem
/// now, not the caller's.
pub fn menu_json(items: &[MenuItem]) -> String {
    serde_json::to_string(items).unwrap_or_else(|_| "[]".into())
}

#[derive(serde::Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../ts/generated/", rename = "MenuPick")]
pub struct Pick {
    action: String,
    /// Whatever the menu item carried. `unknown` on the TypeScript side, which
    /// is honest: a pick's params are the command's business, not the shim's.
    #[ts(type = "unknown")]
    params: Value,
}

#[derive(Props, Clone)]
pub struct ContextMenuProps {
    /// `(action, params)` for a picked item — routed like any command.
    pub on_command: Callback<(String, Value)>,
}

impl PartialEq for ContextMenuProps {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

/// Installs the context-menu shim and routes picks to the host.
#[component]
pub fn ContextMenu(props: ContextMenuProps) -> Element {
    let on_command = props.on_command;
    use_future(move || async move {
        let mut channel = dioxus::document::eval(CONTEXTMENU_JS);
        loop {
            let Ok(raw) = channel.recv::<String>().await else {
                return; // channel closed; a reload re-installs
            };
            if let Ok(pick) = serde_json::from_str::<Pick>(&raw) {
                on_command.call((pick.action, pick.params));
            }
        }
    });
    rsx! {}
}

/// The editor switcher's menu — every registered editor kind, as a click-open
/// list. Blender's leftmost header button is a menu of editor types, not a
/// dropdown control, and a menu can show labels the way the registry names them.
pub fn editor_menu_json(id: crate::AreaId, kinds: &[crate::EditorKind], current: &str) -> String {
    let items: Vec<MenuItem> = kinds
        .iter()
        .map(|k| {
            let mark = if k.id == current { "• " } else { "" };
            MenuItem::new(
                &format!("{mark}{}", k.label),
                "set_editor",
                serde_json::json!({ "id": id, "editor": k.id }),
            )
        })
        .collect();
    menu_json(&items)
}

/// The area header's View dropdown — region toggles and the area operations,
/// Blender's per-editor View menu. Click-opened (data-im-menu-click), so it
/// reads as a menu button rather than a right-click surprise.
pub fn view_menu_json(id: crate::AreaId, toolbar: bool, sidebar: bool, regions: bool) -> String {
    let mut items = Vec::new();
    if regions {
        let word = |on: bool| if on { "Hide" } else { "Show" };
        items.push(
            MenuItem::new(
                &format!("{} Toolbar", word(toolbar)),
                "toggle_region",
                serde_json::json!({ "id": id, "region": crate::area::Region::Toolbar.as_str() }),
            )
            .with_chord("T"),
        );
        items.push(
            MenuItem::new(
                &format!("{} Sidebar", word(sidebar)),
                "toggle_region",
                serde_json::json!({ "id": id, "region": crate::area::Region::Sidebar.as_str() }),
            )
            .with_chord("N"),
        );
        items.push(MenuItem::sep());
    }
    items.extend([
        MenuItem::new(
            "Split horizontal",
            "split",
            serde_json::json!({ "id": id, "dir": "row" }),
        ),
        MenuItem::new(
            "Split vertical",
            "split",
            serde_json::json!({ "id": id, "dir": "col" }),
        ),
        MenuItem::new(
            "Duplicate",
            "duplicate_area",
            serde_json::json!({ "id": id }),
        ),
        MenuItem::sep(),
        MenuItem::new(
            "Hide header",
            "toggle_region",
            serde_json::json!({ "id": id, "region": crate::area::Region::Header.as_str() }),
        ),
        MenuItem::new(
            "Flip header",
            "toggle_region",
            serde_json::json!({ "id": id, "region": crate::area::Region::HeaderFlip.as_str() }),
        ),
        MenuItem::sep(),
        MenuItem::new("Close area", "join", serde_json::json!({ "id": id })),
    ]);
    menu_json(&items)
}

/// The `data-im-menu` JSON for an area leaf — split either way, then close.
/// Kept here so the area view and the menu never drift: both come from the id.
pub fn area_menu_json(id: crate::AreaId) -> String {
    menu_json(&[
        MenuItem::new(
            "Split horizontal",
            "split",
            serde_json::json!({ "id": id, "dir": "row" }),
        ),
        MenuItem::new(
            "Split vertical",
            "split",
            serde_json::json!({ "id": id, "dir": "col" }),
        ),
        MenuItem::new(
            "Duplicate",
            "duplicate_area",
            serde_json::json!({ "id": id }),
        ),
        MenuItem::sep(),
        MenuItem::new("Close area", "join", serde_json::json!({ "id": id })),
    ])
}
