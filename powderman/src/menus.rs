//! The menu bar and the menus that hang off it.
//!
//! Every menu is a list of `MenuItem` values serialised to the JSON the
//! context-menu shim draws — the same pipe a right-click uses, so a row in the
//! File menu and a row in an area's own menu are one mechanism. Nothing here
//! holds state: a menu is built from the settings document and the workspaces
//! each time it opens, which is why the ticks beside the themes are always
//! right and why an agent editing a setting changes what the menu shows.

use serde_json::{Value, json};

use immersion::{Layout, MenuItem, menu_json, pretty_chord};

/// The area pie (backquote): the operations worth reaching by muscle memory,
/// laid out radially. "@area" is resolved by the shim to whichever area the
/// The area pie (backquote): the operations worth reaching by muscle memory,
/// laid out radially. "@area" is resolved by the shim to whichever area the
/// pointer is over, so one definition serves every area.
pub(crate) fn pie_menu_json() -> String {
    menu_json(&[
        MenuItem::new("Split H", "split", json!({ "id": "@area", "dir": "row" })),
        MenuItem::new("Split V", "split", json!({ "id": "@area", "dir": "col" })),
        MenuItem::new("Duplicate", "duplicate_area", json!({ "id": "@area" })),
        MenuItem::new("Close", "join", json!({ "id": "@area" })),
        MenuItem::new(
            "Toolbar",
            "toggle_region",
            json!({ "id": "@area", "region": "toolbar" }),
        ),
        MenuItem::new(
            "Sidebar",
            "toggle_region",
            json!({ "id": "@area", "region": "sidebar" }),
        ),
        MenuItem::new("Maximize", "maximize", Value::Null),
        MenuItem::new("Palette", "palette", Value::Null),
    ])
}

/// The Quick Favourites menu (Q), from the settings list. Each entry is the
/// (label, action, params) a menu row carries, so a favourite runs exactly as
/// The Quick Favourites menu (Q), from the settings list. Each entry is the
/// (label, action, params) a menu row carries, so a favourite runs exactly as
/// the menu item it was added from.
pub(crate) fn favorites_menu_json(settings: &serde_json::Value) -> String {
    let items: Vec<MenuItem> = settings["favorites"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|f| {
            let action = f.get("action")?.as_str()?.to_string();
            let label = f
                .get("label")
                .and_then(|l| l.as_str())
                .unwrap_or(&action)
                .to_string();
            Some(MenuItem::new(
                &label,
                &action,
                f.get("params").cloned().unwrap_or(Value::Null),
            ))
        })
        .collect();
    if items.is_empty() {
        return menu_json(&[MenuItem::new(
            "(no favourites — right-click a menu item to add one)",
            "noop",
            Value::Null,
        )]);
    }
    menu_json(&items)
}

/// The menu-bar dropdowns — Blender's Window / Edit / Help, as click-open
/// menus. Each item is an (action, params) the same router handles: host
/// File: the workbench as a thing you make, open and keep. Blender's File menu
/// is new/open/save of a .blend; the layout is our document, so it is the
/// workspaces you add and the JSON you round-trip them through.
pub(crate) fn file_menu() -> String {
    let starter = serde_json::to_value(Layout::single("machine")).unwrap_or(Value::Null);
    menu_json(&[
        MenuItem::new(
            "New workspace",
            "workspace.add",
            json!({ "name": "Workspace", "layout": starter }),
        )
        .with_icon("plus"),
        MenuItem::new("Duplicate workspace", "workspace.duplicate", json!({})).with_icon("copy"),
        MenuItem::sep(),
        MenuItem::new("Save layout…", "export_layout", Value::Null).with_icon("device-floppy"),
        MenuItem::new("Load layout…", "import_layout", Value::Null).with_icon("upload"),
        MenuItem::sep(),
        // No chord: Preferences is not bound, and Mod+, is the browser's own
        // on at least one platform. A menu row showing a chord that does
        // nothing is worse than one showing none.
        MenuItem::new("Preferences", "preferences", Value::Null).with_icon("settings"),
    ])
}

/// View: how the workbench looks, rather than what it holds. Every row is a
/// `set_setting`, so the menu, the preferences window and an agent are all
/// writing the same document — the menu is a shortcut to values, not a second
/// way to hold them.
pub(crate) fn view_menu(settings: &Value) -> String {
    let scale = settings["ui_scale"].as_f64().unwrap_or(1.0);
    let theme = settings["theme"].as_str().unwrap_or("Blender Dark");
    let tooltips = settings["tooltips_on"].as_bool().unwrap_or(true);
    let splash_on = settings["splash_on_start"].as_bool().unwrap_or(true);
    let set = |pointer: &str, value: Value| json!({ "pointer": pointer, "value": value });
    // Rounded to a tenth: repeated float steps otherwise drift into 1.2000001
    // and the readout in Preferences shows it.
    let step = |by: f64| json!(((scale + by) * 10.0).round() / 10.0);
    let mut items = vec![
        MenuItem::new("Zoom in", "set_setting", set("/ui_scale", step(0.1))),
        MenuItem::new("Zoom out", "set_setting", set("/ui_scale", step(-0.1))),
        MenuItem::new("Reset zoom", "set_setting", set("/ui_scale", json!(1.0))),
        MenuItem::sep(),
    ];
    // Themes come from the library's list, so a preset added there appears
    // here without anyone remembering to add a row.
    for t in immersion::themes() {
        let row = MenuItem::new(t.name, "set_setting", set("/theme", json!(t.name)));
        items.push(if t.name == theme {
            row.with_icon("check")
        } else {
            row
        });
    }
    items.push(MenuItem::sep());
    let tips = MenuItem::new(
        "Tooltips",
        "set_setting",
        set("/tooltips_on", json!(!tooltips)),
    );
    items.push(if tooltips {
        tips.with_icon("check")
    } else {
        tips
    });
    let sp = MenuItem::new(
        "Splash on startup",
        "set_setting",
        set("/splash_on_start", json!(!splash_on)),
    );
    items.push(if splash_on { sp.with_icon("check") } else { sp });
    menu_json(&items)
}

pub(crate) fn window_menu(active: usize, mac: bool) -> String {
    menu_json(&[
        MenuItem::new("Duplicate workspace", "workspace.duplicate", json!({})),
        MenuItem::new(
            "Close workspace",
            "workspace.close",
            json!({ "index": active }),
        ),
        MenuItem::sep(),
        MenuItem::new("Next workspace", "workspace.cycle", json!({ "delta": 1 }))
            .with_chord(&pretty_chord("Alt+PageDown", mac)),
        MenuItem::new(
            "Previous workspace",
            "workspace.cycle",
            json!({ "delta": -1 }),
        )
        .with_chord(&pretty_chord("Alt+PageUp", mac)),
        MenuItem::sep(),
        MenuItem::new("Maximize area", "maximize", Value::Null)
            .with_chord(&pretty_chord("Mod+Shift+Space", mac)),
        MenuItem::new("Fullscreen", "fullscreen", Value::Null)
            .with_chord(&pretty_chord("Mod+Shift+F", mac)),
    ])
}

pub(crate) fn edit_menu(mac: bool) -> String {
    menu_json(&[
        MenuItem::new("Undo", "undo", Value::Null).with_chord(&pretty_chord("Mod+Z", mac)),
        MenuItem::new("Redo", "redo", Value::Null).with_chord(&pretty_chord("Mod+Shift+Z", mac)),
        MenuItem::sep(),
        MenuItem::new("Repeat last", "repeat_last", Value::Null)
            .with_chord(&pretty_chord("Shift+R", mac)),
        MenuItem::new("Adjust last operation", "adjust_last", Value::Null).with_chord("F9"),
        MenuItem::sep(),
        MenuItem::new("Preferences", "preferences", Value::Null),
    ])
}

pub(crate) fn help_menu(_mac: bool) -> String {
    menu_json(&[
        MenuItem::new("Command palette", "palette", Value::Null).with_chord("F3"),
        MenuItem::new("Keyboard shortcuts", "cheatsheet", Value::Null).with_chord("F1"),
        MenuItem::sep(),
        MenuItem::new("Splash screen", "splash", Value::Null),
    ])
}
