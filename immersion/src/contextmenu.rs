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

#[derive(serde::Deserialize)]
struct Pick {
    action: String,
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

/// The `data-im-menu` JSON for an area leaf — split either way, then close.
/// Kept here so the area view and the menu never drift: both come from the id.
pub fn area_menu_json(id: crate::AreaId) -> String {
    format!(
        r#"[{{"label":"Split horizontal","action":"split","params":{{"id":{id},"dir":"row"}}}},{{"label":"Split vertical","action":"split","params":{{"id":{id},"dir":"col"}}}},{{"sep":true}},{{"label":"Close area","action":"join","params":{{"id":{id}}}}}]"#
    )
}
