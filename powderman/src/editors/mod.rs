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
pub(crate) mod help;
pub(crate) mod info;
pub(crate) mod keymap;
pub(crate) mod machine;
pub(crate) mod runs;
pub(crate) mod settings;
pub(crate) mod timers;
pub(crate) mod welcome;

/// An editor: its registry entry, and the function that draws it. One record
/// rather than a list and a matching set of match arms — the shape `Command`
/// already uses, and for the same reason.
pub(crate) struct Editor {
    pub kind: EditorKind,
    pub draw: fn(&Draw) -> Element,
    /// The N panel beside this editor — Blender's sidebar region. An editor
    /// that has something to say about what it is showing says it here, in
    /// its own file, the way it already declares its hints. `None` leaves the
    /// host's generic area properties in place.
    pub sidebar: Option<fn(&Draw) -> Element>,
    /// One line along the area's bottom edge — what this editor is showing,
    /// stated. Empty, or `None`, draws no strip.
    pub footer: Option<fn(&Draw) -> String>,
    /// The T region's own tools — Blender's toolbar. The host appends the
    /// area operations (split, duplicate) after these, so an editor declares
    /// only what is particular to it.
    pub toolbar: Option<fn(&Draw) -> Element>,
    /// The editor's own controls in the area header — Blender's header
    /// carries what the editor it holds needs. For the few things that are
    /// worth a click without opening a region first.
    pub header: Option<fn(&Draw) -> Element>,
}

impl Editor {
    /// The parts beyond a body are optional and there are three of them, so
    /// they are named at the call site rather than counted into a
    /// five-argument constructor.
    fn with_sidebar(mut self, f: fn(&Draw) -> Element) -> Self {
        self.sidebar = Some(f);
        self
    }
    fn with_footer(mut self, f: fn(&Draw) -> String) -> Self {
        self.footer = Some(f);
        self
    }
    fn with_toolbar(mut self, f: fn(&Draw) -> Element) -> Self {
        self.toolbar = Some(f);
        self
    }
    fn with_header(mut self, f: fn(&Draw) -> Element) -> Self {
        self.header = Some(f);
        self
    }
}

/// Every editor this host offers, in the order the dropdown lists them.
pub(crate) fn editors() -> Vec<Editor> {
    fn e(kind: EditorKind, draw: fn(&Draw) -> Element) -> Editor {
        Editor {
            kind,
            draw,
            sidebar: None,
            footer: None,
            toolbar: None,
            header: None,
        }
    }
    vec![
        // First, so it leads the header dropdown the way it leads the splash.
        e(welcome::kind(), welcome::ed_welcome).with_footer(welcome::footer),
        e(machine::kind(), |d| {
            machine::ed_machine(&d.state, &d.settings)
        })
        .with_sidebar(machine::sidebar)
        .with_footer(machine::footer)
        .with_header(machine::header),
        e(fleet::kind(), |d| fleet::ed_fleet(&d.state))
            .with_sidebar(fleet::sidebar)
            .with_footer(fleet::footer),
        e(runs::kind(), |d| runs::ed_runs(d, d.open_run))
            .with_sidebar(runs::sidebar)
            .with_footer(runs::footer),
        e(actions::kind(), |d| actions::ed_actions(&d.state)),
        e(timers::kind(), |d| timers::ed_timers(&d.state)),
        e(runs::detail_kind(), |d| match &d.arg {
            Some(id) => runs::ed_run_detail(&d.state, id),
            None => runs::ed_run_picker(&d.state, d.area, d.open_run),
        }),
        e(settings::kind(), |d| {
            settings::ed_settings(d.settings.clone(), d.on_setting, d.on_error)
        }),
        e(info::kind(), |d| info::ed_info(&d.state)),
        e(help::kind(), help::ed_help)
            .with_sidebar(help::sidebar)
            .with_footer(help::footer),
        e(keymap::kind(), |d| {
            keymap::ed_keymap(
                d.settings.clone(),
                d.mac,
                d.capturing.clone(),
                d.cap_start,
                d.cap_reset,
            )
        }),
        e(data::kind(), |d| data::ed_data(&d.state, d.arg.clone())),
        e(files::kind(), files::ed_files)
            .with_sidebar(files::sidebar)
            .with_footer(files::footer)
            .with_toolbar(files::toolbar)
            .with_header(files::header),
        e(code::kind(), |d| code::ed_code(d.arg.clone()))
            .with_sidebar(code::sidebar)
            .with_footer(code::footer)
            .with_toolbar(code::toolbar),
        e(diff::kind(), |d| {
            diff::ed_diff(
                d.arg.clone(),
                d.settings["diff_split"].as_bool().unwrap_or(false),
            )
        })
        .with_sidebar(diff::sidebar)
        .with_footer(diff::footer)
        .with_toolbar(diff::toolbar),
        e(crate::charts::kind(), |d| {
            crate::charts::ed_chart(&d.state, &d.settings, d.arg.clone())
        })
        // A chart's sidebar is the document that makes it — the spec editor,
        // not a read-out. It used to be a special case in the host's
        // render_sidebar; it is a registration now, like the rest.
        .with_sidebar(|d| {
            crate::charts::chart_sidebar(&d.settings, d.arg.clone(), d.on_setting, d.on_error)
        })
        .with_footer(crate::charts::footer),
    ]
}

/// The sidebar this editor brings, if it brings one. The host falls back to
/// its generic area properties when this is `None`.
pub(crate) fn sidebar(d: &Draw) -> Option<Element> {
    editors()
        .into_iter()
        .find(|e| e.kind.id == d.editor)
        .and_then(|e| e.sidebar)
        .map(|f| f(d))
}

/// The editor's own controls for its area header, if it has any.
pub(crate) fn header(d: &Draw) -> Option<Element> {
    editors()
        .into_iter()
        .find(|e| e.kind.id == d.editor)
        .and_then(|e| e.header)
        .map(|f| f(d))
}

/// The T region's own tools for this editor, if it has any.
pub(crate) fn toolbar(d: &Draw) -> Option<Element> {
    editors()
        .into_iter()
        .find(|e| e.kind.id == d.editor)
        .and_then(|e| e.toolbar)
        .map(|f| f(d))
}

/// Switching an area between two views of the same thing — the file and what
/// changed in it. `open_editor` rather than `set_editor` because the target is
/// the point: `set_editor` clears the arg, which would land you on a picker
/// having asked to look at the file you were already reading.
pub(crate) fn swap_viewer(area: AreaId, editor: &str, path: &str) -> serde_json::Value {
    serde_json::json!({ "id": area, "editor": editor, "arg": path })
}

/// The footer line this editor states, if it states one.
pub(crate) fn footer(d: &Draw) -> String {
    editors()
        .into_iter()
        .find(|e| e.kind.id == d.editor)
        .and_then(|e| e.footer)
        .map(|f| f(d))
        .unwrap_or_default()
}

/// One `name — value` row in a sidebar panel. Every sidebar is made of these,
/// so they line up across editors rather than each inventing a layout.
pub(crate) fn prop(k: &str, v: String) -> Element {
    rsx! {
        div { class: "area-props-row",
            span { class: "k", "{k}" }
            span { "{v}" }
        }
    }
}

/// The registry entries alone, for the chrome.
pub(crate) fn kinds() -> Vec<EditorKind> {
    editors().into_iter().map(|e| e.kind).collect()
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

/// The kind of thing an editor points at — the key a selection is stored
/// under, and the reason one click can drive two panes.
///
/// Deliberately not [`target_noun`], which is the word shown to a person. The
/// code viewer and the diff viewer both point at a path, so they share a kind
/// and picking a file drives both. The file browser points at a *folder*, so
/// it must not share one with them: a browser that re-rooted itself onto the
/// file you just clicked would collapse the tree you clicked it in.
///
/// `None` is an editor that follows nothing — either it takes no target, or
/// its target is not the sort of thing anything else selects.
pub(crate) fn target_kind(editor: &str) -> Option<&'static str> {
    match editor {
        "code" | "diff" => Some("file"),
        "files" => Some("folder"),
        "run" => Some("run"),
        "chart" => Some("chart"),
        "data" => Some("data"),
        _ => None,
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
    /// What is selected in this workspace, by kind. A list marks the rows in
    /// it and treats the last as active — many selected, one active, which is
    /// what makes an operation across several of them expressible.
    pub selection: std::collections::BTreeMap<String, Vec<String>>,
    /// The write path, for an editor that acts on its own area — a toolbar
    /// button that turns this pane from a file into its diff. The same bus
    /// every header button and every chord goes through.
    pub cmd: Callback<(String, serde_json::Value)>,
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
    match editors().into_iter().find(|e| e.kind.id == d.editor) {
        Some(e) => (e.draw)(&d),
        // An id the registry does not know: a stale layout naming an editor
        // this host no longer has.
        None => {
            let editor = d.editor.clone();
            rsx! { div { class: "empty", "unknown editor {editor}" } }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The registry is one list now, so an editor cannot be offered without
    /// something to draw it — that is a type error rather than a test. What
    /// is still worth checking is that no two editors claim the same id, and
    /// that the order the dropdown shows is the order written here.
    #[test]
    fn the_registry_is_unambiguous_and_ordered() {
        let ids: Vec<&str> = editors().iter().map(|e| e.kind.id).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "two editors share an id: {ids:?}");
        assert_eq!(
            ids.first(),
            Some(&"welcome"),
            "the dropdown's order is the list's order, and welcome leads it"
        );
    }

    /// The chart editor's sidebar *is* its spec editor — the thing that makes
    /// a chart editable at all. It used to be an `if editor == "chart"` in the
    /// host, which a later refactor of the host could drop without a compile
    /// error. It is a registration now, and this is what notices if it goes.
    #[test]
    fn the_editors_that_declare_a_sidebar_have_one() {
        let with: Vec<&str> = editors()
            .iter()
            .filter(|e| e.sidebar.is_some())
            .map(|e| e.kind.id)
            .collect();
        for id in ["chart", "machine", "runs", "fleet", "code", "diff", "files"] {
            assert!(with.contains(&id), "{id} lost its sidebar: {with:?}");
        }
        // And an editor with nothing to say still gets the generic panel.
        assert!(!with.contains(&"info"));
    }

    /// The toolbar buttons that switch a pane between a file and its diff
    /// emit a command with params. `every_ui_action_resolves` would see the
    /// name and be satisfied; what breaks silently is the params — and
    /// `set_editor` instead of `open_editor` is exactly the mistake, because
    /// it clears the arg and lands you on a picker having asked to look at
    /// the file you were already reading.
    #[test]
    fn swapping_the_viewer_keeps_the_file_it_was_looking_at() {
        let commands = crate::workflows::commands();
        let mut ws = immersion::Workspaces::new("test", immersion::Layout::single("code"));
        let params = swap_viewer(1, "diff", "src/main.rs");
        commands
            .run(&mut ws, "open_editor", &params)
            .expect("the toolbar's command runs");
        let layout = &ws.current().layout;
        assert_eq!(
            layout.target_of(1).as_deref(),
            Some("src/main.rs"),
            "the swap dropped the file"
        );
        assert!(matches!(
            layout.root.find(1),
            Some(immersion::Area::Leaf { editor, .. }) if editor == "diff"
        ));
    }

    /// Which editors bring tools of their own. The area operations are
    /// appended by the host to every strip, so an editor listed here has
    /// something particular to it and one that is not has nothing.
    #[test]
    fn the_editors_that_declare_a_toolbar_have_one() {
        let with: Vec<&str> = editors()
            .iter()
            .filter(|e| e.toolbar.is_some())
            .map(|e| e.kind.id)
            .collect();
        assert_eq!(with, vec!["files", "code", "diff"]);
    }

    /// The footer is the line the old Immersion put in an area's bottom-right
    /// corner — "JavaScript · 16 lines". An editor states one or it does not;
    /// the strip is only drawn for a non-empty line, so silence is free.
    #[test]
    fn the_editors_that_state_a_footer_have_one() {
        let with: Vec<&str> = editors()
            .iter()
            .filter(|e| e.footer.is_some())
            .map(|e| e.kind.id)
            .collect();
        for id in ["machine", "fleet", "runs", "code", "diff", "files", "chart"] {
            assert!(with.contains(&id), "{id} lost its footer: {with:?}");
        }
        assert!(!with.contains(&"info"), "and one that has nothing to say");
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
