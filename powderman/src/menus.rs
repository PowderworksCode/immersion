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
/// Undo History: the stack as a list you can jump into, rather than a key you
/// press repeatedly and hope. Blender's is exactly this — the steps, newest
/// first, with the one you are on at the top — and picking a row returns the
/// workbench to just before that command ran.
///
/// Built from the live stack each time it opens, so it is never a stale
/// picture of a history another client has already changed.
pub(crate) fn undo_history_menu(history: &[(usize, String)]) -> String {
    if history.is_empty() {
        return menu_json(&[MenuItem::new("Nothing to undo", "noop", Value::Null)]);
    }
    let mut items = vec![
        MenuItem::new("Undo", "undo", Value::Null).with_chord("Mod+Z"),
        MenuItem::new("Redo", "redo", Value::Null).with_chord("Mod+Shift+Z"),
        MenuItem::sep(),
    ];
    // A cap, because the stack holds 100 and a menu that long is a wall.
    for (depth, label) in history.iter().take(20) {
        items.push(MenuItem::new(
            &format!("{label}  ({depth} back)"),
            "undo_to",
            json!({ "depth": depth }),
        ));
    }
    menu_json(&items)
}

/// Repeat History: the commands that ran, newest first, each re-runnable.
/// Blender's lists recent operators; ours is the Info log filtered the same
/// way Repeat Last filters it — successful, and something that changed the
/// layout, since repeating a tab switch or a command that already failed is
/// not what the menu means.
///
/// The rows carry the original params, so this is the one surface where the
/// exact command an agent ran is a thing a person can run again.
pub(crate) fn repeat_history_menu(log: &[crate::ui::LogEntry]) -> String {
    let commands = crate::workflows::commands();
    let mut seen = Vec::new();
    let mut items = vec![
        MenuItem::new("Repeat last", "repeat_last", Value::Null).with_chord("Shift+R"),
        MenuItem::sep(),
    ];
    for entry in log.iter().rev() {
        if !entry.ok || !commands.records_undo(&entry.name) {
            continue;
        }
        // The same command with the same params twice in a row is one row —
        // a list of fifteen identical splits is not a history anyone reads.
        let key = (entry.name.clone(), entry.params.clone());
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        items.push(MenuItem::new(
            &format!("{}  {}", entry.name, brief(&entry.params)),
            &entry.name,
            entry.params.clone(),
        ));
        if items.len() > 16 {
            break;
        }
    }
    if items.len() == 2 {
        items.push(MenuItem::new("Nothing repeatable yet", "noop", Value::Null));
    }
    menu_json(&items)
}

/// A params object as a short `k=v` line for a menu row. Long values are cut:
/// a whole serialized layout in a menu label is not a label.
fn brief(params: &Value) -> String {
    let Some(map) = params.as_object() else {
        return String::new();
    };
    let mut parts: Vec<String> = map
        .iter()
        .map(|(k, v)| {
            let text = match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            let text: String = if text.chars().count() > 14 {
                text.chars().take(13).chain("…".chars()).collect()
            } else {
                text
            };
            format!("{k}={text}")
        })
        .collect();
    parts.sort();
    parts.join(" ")
}

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

/// The theme picker on its own, for the topbar chip beside the workspace
/// tabs. Same rows as the View menu's middle section — one list, so the two
/// surfaces can never offer different themes or disagree about which is on.
pub(crate) fn theme_menu(settings: &Value) -> String {
    menu_json(&theme_rows(
        settings["theme"].as_str().unwrap_or("Blender Dark"),
    ))
}

/// A row per theme the library ships, with the active one ticked. Adding a
/// preset to `immersion::themes()` is the whole of adding it here.
fn theme_rows(active: &str) -> Vec<MenuItem> {
    immersion::themes()
        .iter()
        .map(|t| {
            let row = MenuItem::new(
                t.name,
                "set_setting",
                json!({ "pointer": "/theme", "value": t.name }),
            );
            if t.name == active {
                row.with_icon("check")
            } else {
                row
            }
        })
        .collect()
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
    items.extend(theme_rows(theme));
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

#[cfg(test)]
mod history_tests {
    use super::*;
    use crate::ui::LogEntry;

    fn rows(menu: &str) -> Vec<serde_json::Value> {
        serde_json::from_str(menu).expect("menu JSON parses")
    }

    fn entry(name: &str, params: Value, ok: bool) -> LogEntry {
        LogEntry {
            name: name.to_string(),
            params,
            at: 0,
            ok,
            source: "ui".into(),
        }
    }

    /// An empty menu renders as an empty box — the shim draws what it is
    /// given. A history with nothing in it has to say so.
    #[test]
    fn an_empty_history_still_says_something() {
        let menu = rows(&undo_history_menu(&[]));
        assert_eq!(menu.len(), 1);
        assert_eq!(menu[0]["label"], "Nothing to undo");
        let menu = rows(&repeat_history_menu(&[]));
        assert!(
            menu.iter().any(|r| r["label"] == "Nothing repeatable yet"),
            "{menu:?}"
        );
    }

    /// Each row carries the depth that reaches it. Off by one here means
    /// picking "split (1 back)" lands somewhere else entirely.
    #[test]
    fn the_undo_rows_carry_the_depth_that_reaches_them() {
        let history = vec![
            (1, "split".to_string()),
            (2, "set_editor".to_string()),
            (3, "join".to_string()),
        ];
        let depths: Vec<u64> = rows(&undo_history_menu(&history))
            .into_iter()
            .filter(|r| r["action"] == "undo_to")
            .map(|r| r["params"]["depth"].as_u64().expect("a depth"))
            .collect();
        assert_eq!(depths, vec![1, 2, 3]);
    }

    /// The rows are re-runnable commands with the params the log recorded. A
    /// row whose params the registry rejects is a menu item that does nothing,
    /// which is the whole failure mode a repeat history has.
    #[test]
    fn the_repeat_rows_run_with_the_params_the_log_kept() {
        let log = vec![
            entry("split", json!({ "id": 1, "dir": "row" }), true),
            entry("set_editor", json!({ "id": 1, "editor": "runs" }), true),
        ];
        let commands = crate::workflows::commands();
        let mut ws = immersion::Workspaces::new("test", Layout::single("runs"));
        for row in rows(&repeat_history_menu(&log)) {
            let Some(action) = row["action"].as_str() else {
                continue;
            };
            if action == "repeat_last" {
                continue;
            }
            commands
                .run(&mut ws, action, &row["params"])
                .unwrap_or_else(|e| panic!("repeat row {action} does nothing: {e}"));
        }
    }

    /// Repeating a command that already failed re-runs a failure; repeating a
    /// tab switch is not what anyone means by repeat. Both are the filter
    /// Repeat Last already uses, applied to the list as well as the key.
    #[test]
    fn failed_and_navigational_commands_are_not_offered() {
        let log = vec![
            entry("split", json!({ "id": 99, "dir": "row" }), false),
            entry("workspace.switch", json!({ "index": 1 }), true),
            entry("join", json!({ "id": 2 }), true),
        ];
        let offered: Vec<String> = rows(&repeat_history_menu(&log))
            .into_iter()
            .filter_map(|r| r["action"].as_str().map(str::to_string))
            .filter(|a| a != "repeat_last")
            .collect();
        assert_eq!(offered, vec!["join"], "offered: {offered:?}");
    }

    /// Fifteen identical splits is not a history anyone reads.
    #[test]
    fn the_same_command_twice_is_one_row() {
        let same = || entry("split", json!({ "id": 1, "dir": "row" }), true);
        let log = vec![same(), same(), same()];
        let n = rows(&repeat_history_menu(&log))
            .into_iter()
            .filter(|r| r["action"] == "split")
            .count();
        assert_eq!(n, 1);
    }
}

#[cfg(test)]
mod topbar_tests {
    use super::*;

    fn labels(menu: &str) -> Vec<String> {
        serde_json::from_str::<Vec<Value>>(menu)
            .expect("menu JSON parses")
            .into_iter()
            .filter_map(|r| r["label"].as_str().map(str::to_string))
            .collect()
    }

    /// The topbar chip and the View menu are two ways to the same setting.
    /// They were about to be two lists of themes; they are one list read
    /// twice, and this is what says so.
    #[test]
    fn the_theme_chip_and_the_view_menu_offer_the_same_themes() {
        let doc = serde_json::json!({ "theme": "Light" });
        let chip = labels(&theme_menu(&doc));
        let names: Vec<String> = immersion::themes()
            .iter()
            .map(|t| t.name.to_string())
            .collect();
        assert_eq!(chip, names, "the chip offers every theme, in order");
        let view = labels(&view_menu(&doc));
        for name in &names {
            assert!(view.contains(name), "the View menu lost {name}");
        }
    }

    /// The tick follows the setting, in both places. A chip that always says
    /// "Blender Dark" is worse than one with no tick at all.
    #[test]
    fn the_active_theme_is_the_ticked_one() {
        for active in immersion::themes().iter().map(|t| t.name) {
            let doc = serde_json::json!({ "theme": active });
            let ticked: Vec<String> = serde_json::from_str::<Vec<Value>>(&theme_menu(&doc))
                .expect("menu JSON parses")
                .into_iter()
                .filter(|r| r.get("icon").is_some())
                .filter_map(|r| r["label"].as_str().map(str::to_string))
                .collect();
            assert_eq!(ticked, vec![active.to_string()]);
        }
    }

    /// The connection chip has no signal of its own: the disconnect script
    /// marks the body and the stylesheet swaps the two labels. That is right —
    /// by the time the chip is wrong there is no server left to re-render it —
    /// but it means a string in daemon.rs and a rule in ui.css have to agree,
    /// with nothing between them that would notice if they stopped.
    #[test]
    fn the_connection_chip_reads_the_class_the_page_actually_sets() {
        let html = crate::daemon::index_html();
        let class = crate::daemon::STALE_CLASS;
        assert!(
            html.contains(&format!("classList.add(\"{class}\")")),
            "the disconnect script no longer sets {class}"
        );
        for rule in [
            format!("body.{class} .conn-on"),
            format!("body.{class} .conn-off"),
            format!("body.{class} .conn-dot"),
        ] {
            assert!(
                crate::ui::CSS.contains(&rule),
                "ui.css has no `{rule}` — the chip would say Connected while offline"
            );
        }
    }
}
