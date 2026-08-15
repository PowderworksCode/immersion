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
    let items: Vec<String> = kinds
        .iter()
        .map(|k| {
            let mark = if k.id == current { "• " } else { "" };
            format!(
                r#"{{"label":"{mark}{}","action":"set_editor","params":{{"id":{id},"editor":"{}"}}}}"#,
                k.label, k.id
            )
        })
        .collect();
    format!("[{}]", items.join(","))
}

/// The area header's View dropdown — region toggles and the area operations,
/// Blender's per-editor View menu. Click-opened (data-im-menu-click), so it
/// reads as a menu button rather than a right-click surprise.
pub fn view_menu_json(id: crate::AreaId, toolbar: bool, sidebar: bool, regions: bool) -> String {
    let mut items = Vec::new();
    if regions {
        items.push(format!(
            r#"{{"label":"{} Toolbar","action":"toggle_region","params":{{"id":{id},"region":"toolbar"}},"chord":"T"}}"#,
            if toolbar { "Hide" } else { "Show" }
        ));
        items.push(format!(
            r#"{{"label":"{} Sidebar","action":"toggle_region","params":{{"id":{id},"region":"sidebar"}},"chord":"N"}}"#,
            if sidebar { "Hide" } else { "Show" }
        ));
        items.push(r#"{"sep":true}"#.to_string());
    }
    items.push(format!(
        r#"{{"label":"Split horizontal","action":"split","params":{{"id":{id},"dir":"row"}}}}"#
    ));
    items.push(format!(
        r#"{{"label":"Split vertical","action":"split","params":{{"id":{id},"dir":"col"}}}}"#
    ));
    items.push(format!(
        r#"{{"label":"Duplicate","action":"duplicate_area","params":{{"id":{id}}}}}"#
    ));
    items.push(r#"{"sep":true}"#.to_string());
    items.push(format!(
        r#"{{"label":"Hide header","action":"toggle_region","params":{{"id":{id},"region":"header"}}}}"#
    ));
    items.push(format!(
        r#"{{"label":"Flip header","action":"toggle_region","params":{{"id":{id},"region":"header_flip"}}}}"#
    ));
    items.push(r#"{"sep":true}"#.to_string());
    items.push(format!(
        r#"{{"label":"Close area","action":"join","params":{{"id":{id}}}}}"#
    ));
    format!("[{}]", items.join(","))
}

/// The `data-im-menu` JSON for an area leaf — split either way, then close.
/// Kept here so the area view and the menu never drift: both come from the id.
pub fn area_menu_json(id: crate::AreaId) -> String {
    format!(
        r#"[{{"label":"Split horizontal","action":"split","params":{{"id":{id},"dir":"row"}}}},{{"label":"Split vertical","action":"split","params":{{"id":{id},"dir":"col"}}}},{{"label":"Duplicate","action":"duplicate_area","params":{{"id":{id}}}}},{{"sep":true}},{{"label":"Close area","action":"join","params":{{"id":{id}}}}}]"#
    )
}
