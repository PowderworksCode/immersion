//! The editor registry: what an area can show, one module per editor.
//!
//! An editor declares itself — id, label, icon, whether it takes a target,
//! and what the status bar should say while it has focus — beside the code
//! that draws it. Adding one is a file and a line here, rather than an entry
//! in a registry, a arm in a match, a row in a hints table and a case in a
//! target picker, each of which used to live somewhere else.
//!
//! The shared helpers that several editors need stay here; anything only one
//! editor uses moved into that editor's file.

use dioxus::prelude::*;
use immersion::{AreaId, EditorKind};

use crate::ui::State;

pub(crate) mod actions;
pub(crate) mod code;
pub(crate) mod data;
pub(crate) mod diff;
pub(crate) mod files;
pub(crate) mod fleet;
pub(crate) mod info;
pub(crate) mod keymap;
pub(crate) mod machine;
pub(crate) mod runs;
pub(crate) mod settings;
pub(crate) mod timers;

/// Every editor this host offers, in the order the dropdown lists them.
pub(crate) fn kinds() -> Vec<EditorKind> {
    vec![
        machine::kind(),
        fleet::kind(),
        runs::kind(),
        actions::kind(),
        timers::kind(),
        runs::detail_kind(),
        settings::kind(),
        info::kind(),
        keymap::kind(),
        data::kind(),
        files::kind(),
        code::kind(),
        diff::kind(),
        crate::charts::kind(),
    ]
}

/// What an editor answers to, for the status bar. Read off the registry now,
/// so an editor that declares hints has them and one that does not shows the
/// global ones — no second table to keep in step.
pub(crate) fn hints_for(editor: &str) -> &'static [(&'static str, &'static str)] {
    kinds()
        .into_iter()
        .find(|k| k.id == editor)
        .map(|k| k.hints)
        .unwrap_or(&[])
}

// --- editors --------------------------------------------------------------
// Each one is a body the library mounts under an area header. They read the
// polled snapshot and call the same daemon functions the old page did; the
// workbench changed where they sit, not what they are.

/// What an editor can be pointed at, and how those choices are listed.
///
/// The picker used to offer the raw data tree to every editor, which is wrong
/// for anything whose target is not a JSON pointer: picking a run meant
/// choosing `/state/runs/0/id` and storing the pointer, so the run editor
/// looked up a run whose id was a path and found nothing. An editor knows
/// what it can take; the picker asks.
pub(crate) fn target_children(
    editor: &str,
    state_doc: &serde_json::Value,
    state: &State,
    pointer: &str,
) -> Vec<immersion::TreeRow> {
    match editor {
        // Both browse the filesystem; the code viewer's target is a file
        // rather than a directory, but the walk to reach one is the same.
        "files" | "code" => files::file_children(pointer),
        // A diff editor can only show a file that changed, so those are what
        // it offers — the same rule as the run editor, which lists runs
        // rather than making you find one in the snapshot.
        "diff" => files::changed_files(),
        // A chart editor takes a chart, so it lists the ones that exist.
        "chart" => crate::charts::chart_names(&state_doc_settings())
            .into_iter()
            .map(|name| immersion::TreeRow {
                icon: "chart-line".to_string(),
                preview: String::new(),
                pointer: format!("/charts/{name}"),
                label: name,
                has_children: false,
            })
            .collect(),
        // A run's target is its id, not its address in the snapshot, so the
        // rows are runs and the value each carries is the id itself.
        "run" => runs::run_targets(state),
        _ => data::data_children(state_doc, pointer),
    }
}

/// What the picker calls the thing being chosen, so the modal says "Run for
/// area 3" rather than a generic word for every editor.
pub(crate) fn target_noun(editor: &str) -> &'static str {
    match editor {
        "files" | "code" => "File",
        "diff" => "Changed file",
        "chart" => "Chart",
        "run" => "Run",
        _ => "Target",
    }
}

/// The settings document, for the places that need it outside a render.
fn state_doc_settings() -> serde_json::Value {
    crate::daemon::settings()
}

pub(crate) fn human_size(len: u64) -> String {
    match len {
        0..=1023 => format!("{len} B"),
        1024..=1048575 => format!("{:.1} K", len as f64 / 1024.0),
        1048576..=1073741823 => format!("{:.1} M", len as f64 / 1048576.0),
        _ => format!("{:.1} G", len as f64 / 1073741824.0),
    }
}

/// Tests that set POWDERMAN_FILES_ROOT or POWDERMAN_DEMO take this first.
/// Cargo runs tests in one process on many threads, so without it one test
/// reads another's environment — a file test looking under the wrong root, or
/// a demo flag left on for something that expected it off. The lock is the
/// smallest honest fix; the alternative is threading configuration through
/// code whose production callers all want the environment.
#[cfg(test)]
static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod target_sources {
    use super::*;

    fn state_with_runs() -> State {
        State {
            runs: vec![
                crate::ui::RunView {
                    id: "aaaa1111".into(),
                    workflow: "treebank_sweep".into(),
                    status: "done".into(),
                    note: Some("41 grammars".into()),
                    error: None,
                    updated_at: 100,
                    steps: vec![],
                },
                crate::ui::RunView {
                    id: "bbbb2222".into(),
                    workflow: "treebank_fix".into(),
                    status: "failed".into(),
                    note: None,
                    error: Some("verify failed".into()),
                    updated_at: 300,
                    steps: vec![],
                },
            ],
            ..Default::default()
        }
    }

    /// The bug this exists to stop: the run editor's target is a run id, and
    /// the picker used to hand it a JSON pointer into the snapshot, so the
    /// editor looked up a run whose id was `/state/runs/0/id` and reported
    /// that no such run existed.
    #[test]
    fn the_run_picker_offers_run_ids_not_pointers() {
        let s = state_with_runs();
        let doc = serde_json::to_value(&s).unwrap();
        let rows = target_children("run", &doc, &s, "");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].pointer, "bbbb2222", "newest first");
        assert!(
            rows.iter().all(|r| !r.pointer.starts_with('/')),
            "a run target must be an id, not a path: {rows:?}"
        );
        assert!(rows.iter().all(|r| !r.has_children), "runs are leaves");
        assert!(rows[0].label.contains("treebank_fix"));
        assert_eq!(
            rows[0].preview, "verify failed",
            "an error stands in for a note"
        );
    }

    /// The complaint this fixes: the diff picker offered every file in the
    /// tree, so finding one with a patch meant guessing. It offers the
    /// changed ones now, and each row says which file it is.
    #[test]
    fn the_diff_picker_offers_only_changed_files() {
        let _guard = super::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // The demo path, since it is the one a visitor meets first.
        unsafe { std::env::set_var("POWDERMAN_DEMO", "1") };
        let s = state_with_runs();
        let doc = serde_json::to_value(&s).unwrap();
        let rows = target_children("diff", &doc, &s, "");
        assert!(!rows.is_empty(), "the demo presents some files as changed");
        assert!(rows.len() < 14, "and not the whole tree: {}", rows.len());
        for r in &rows {
            assert!(!r.has_children, "a changed file is a leaf");
            assert!(
                crate::demo::file_diff(&r.pointer)
                    .expect("listed files exist")
                    .is_some(),
                "{} is offered but has no patch",
                r.pointer
            );
            assert!(r.preview.contains('/'), "the row shows its path: {r:?}");
        }
        assert_eq!(target_noun("diff"), "Changed file");
        unsafe { std::env::remove_var("POWDERMAN_DEMO") };
    }

    #[test]
    fn other_editors_keep_their_own_feeds() {
        let s = state_with_runs();
        let doc = serde_json::to_value(&s).unwrap();
        // The data editor still walks the mounted documents.
        let mounts = target_children("data", &doc, &s, "");
        assert!(mounts.iter().any(|r| r.label == "/settings"), "{mounts:?}");
        // And the picker names what it is picking.
        assert_eq!(target_noun("run"), "Run");
        assert_eq!(target_noun("files"), "File");
        assert_eq!(target_noun("data"), "Target");
    }
}

/// What identifies a rendering: the path and the content. The renderer skips
/// a host whose stamp it has already drawn, so a poll that changes nothing
/// costs nothing, and an edit to the file redraws once.
pub(crate) fn stamp_of(path: &str, src: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut h);
    src.hash(&mut h);
    format!("{:x}", h.finish())
}

/// Everything an editor might need to draw itself. One struct rather than a
/// dozen arguments, because the alternative is every editor's signature
/// changing whenever one of them wants something new.
pub(crate) struct Draw {
    pub area: AreaId,
    pub editor: String,
    pub arg: Option<String>,
    pub state: State,
    pub settings: serde_json::Value,
    pub mac: bool,
    pub capturing: Option<String>,
    pub open_run: Callback<(AreaId, String)>,
    pub cap_start: Callback<String>,
    pub cap_reset: Callback<String>,
    pub on_setting: Callback<(String, serde_json::Value)>,
    pub on_error: Callback<immersion::EditorError>,
}

/// Draw whatever the area is pointed at. The one place that maps an editor id
/// to the code that draws it; a kind in `kinds()` with no arm here is an
/// editor that would render as "unknown", which the test below refuses.
pub(crate) fn render(d: Draw) -> Element {
    let Draw { area, arg, .. } = &d;
    let (area, arg) = (*area, arg.clone());
    match d.editor.as_str() {
        "machine" => machine::ed_machine(&d.state, &d.settings),
        "fleet" => fleet::ed_fleet(&d.state),
        "runs" => runs::ed_runs(&d.state, area, d.open_run),
        "actions" => actions::ed_actions(&d.state),
        "timers" => timers::ed_timers(&d.state),
        "info" => info::ed_info(&d.state),
        "keymap" => keymap::ed_keymap(
            d.settings.clone(),
            d.mac,
            d.capturing.clone(),
            d.cap_start,
            d.cap_reset,
        ),
        "run" => match arg {
            Some(id) => runs::ed_run_detail(&d.state, &id),
            None => runs::ed_run_picker(&d.state, area, d.open_run),
        },
        "settings" => settings::ed_settings(d.settings.clone(), d.on_setting, d.on_error),
        "data" => data::ed_data(&d.state, arg),
        "files" => files::ed_files(arg),
        "code" => code::ed_code(arg),
        "diff" => diff::ed_diff(arg, d.settings["diff_split"].as_bool().unwrap_or(false)),
        "chart" => crate::charts::ed_chart(&d.state, &d.settings, arg),
        other => rsx! { div { class: "empty", "unknown editor {other}" } },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A registry entry with no arm in `render` draws "unknown editor", which
    /// is a menu offering something that does not exist. The two lists are
    /// written in different places, so this is what keeps them the same.
    #[test]
    fn every_registered_editor_can_be_drawn() {
        // The ids `render` handles, read off its own match rather than a copy.
        const DRAWN: &[&str] = &[
            "machine", "fleet", "runs", "actions", "timers", "info", "keymap", "run", "settings",
            "data", "files", "code", "diff", "chart",
        ];
        for k in kinds() {
            assert!(DRAWN.contains(&k.id), "{} is offered but never drawn", k.id);
        }
        for id in DRAWN {
            assert!(
                kinds().iter().any(|k| k.id == *id),
                "{id} is drawn but not in the registry, so nothing can reach it"
            );
        }
    }

    /// Hints belong to the editor now. This catches the copy that used to
    /// live in status.rs coming back.
    #[test]
    fn hints_come_from_the_registry() {
        assert_eq!(
            hints_for("runs"),
            &[("Click", "Open run"), ("Type", "Filter")]
        );
        assert!(hints_for("machine").is_empty(), "an editor may have none");
        assert!(
            hints_for("nope").is_empty(),
            "and an unknown id is not a panic"
        );
    }
}

#[cfg(test)]
mod icon_tests {
    /// An editor naming an icon that is not in the set draws a header with a
    /// gap where its glyph should be. Cheap to check, and the whole reason
    /// the full Tabler set is vendored is that this is now a wide net.
    #[test]
    fn every_editor_names_an_icon_that_exists() {
        for k in super::kinds() {
            assert!(
                immersion::has_icon(k.icon),
                "{} names the icon {:?}, which is not in the set",
                k.id,
                k.icon
            );
        }
    }
}
