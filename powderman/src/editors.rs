//! The editor bodies — what an area shows once the chrome has decided where.
//!
//! Split out of `ui.rs` so that file stays about the workbench itself (the
//! App, its signals, the chrome and the routing) while this one is about
//! rendering host state. Each function takes the polled snapshot and returns a
//! body; none of them own state.

use dioxus::prelude::*;
use immersion::{AreaId, FilterBox, PropertyEditor, TreeView, pretty_chord};

use crate::ui::{
    RunView, State, StepView, WorkflowView, chart, effective_keymap, gib, hhmmss, settings_fields,
    short, tile,
};
use crate::ui::{resume, run_with};

// --- editors --------------------------------------------------------------
// Each one is a body the library mounts under an area header. They read the
// polled snapshot and call the same daemon functions the old page did; the
// workbench changed where they sit, not what they are.

/// The Keymap editor: every binding, what it does, and its chord — with a
/// capture button that listens for the next chord you press. Blender's keymap
/// preferences, minus the filtering.
pub(crate) fn ed_keymap(
    settings: serde_json::Value,
    mac: bool,
    capturing: Option<String>,
    on_capture_start: Callback<String>,
    on_reset: Callback<String>,
) -> Element {
    let overrides = settings["keymap"].clone();
    rsx! {
        div { class: "keymap-editor im-filter-scope",
            div { class: "keymap-head", FilterBox { placeholder: "filter shortcuts…" } }
            for b in effective_keymap(&settings) {
                {
                    let custom = overrides.get(&b.action).is_some();
                    let waiting = capturing.as_deref() == Some(b.action.as_str());
                    keymap_row(b, mac, custom, waiting, on_capture_start, on_reset)
                }
            }
        }
    }
}

/// One row of the keymap editor. Its own function so the editor's view does not
/// nest another four levels deep.
fn keymap_row(
    b: immersion::Binding,
    mac: bool,
    custom: bool,
    waiting: bool,
    on_capture_start: Callback<String>,
    on_reset: Callback<String>,
) -> Element {
    let a1 = b.action.clone();
    let a2 = b.action.clone();
    rsx! {
        div {
            class: "keymap-row",
            key: "{b.action}",
            "data-filter-text": "{b.description} {b.action} {b.chord}",
            span { class: "keymap-desc", "{b.description}" }
            span { class: "im-hint-key keymap-chord", "{pretty_chord(&b.chord, mac)}" }
            button {
                class: if waiting { "keymap-set waiting" } else { "keymap-set" },
                onclick: move |_| on_capture_start.call(a1.clone()),
                if waiting { "press a key…" } else { "rebind" }
            }
            if custom {
                button {
                    class: "keymap-reset",
                    title: "restore the default",
                    onclick: move |_| on_reset.call(a2.clone()),
                    "↺"
                }
            }
        }
    }
}

pub(crate) fn ed_actions(s: &State) -> Element {
    rsx! {
        div { class: "actions",
            for w in s.workflows.iter().cloned() {
                {action_row(w)}
            }
        }
    }
}

/// One workflow: what it is, and a form to run it. Extracted from `ed_actions`
/// so the loop body stays shallow.
fn action_row(w: WorkflowView) -> Element {
    let example = w.example.clone();
    let name = w.name.clone();
    rsx! {
        div { class: "action", key: "{w.name}",
            div { class: "who",
                span { class: "wf", "{w.name}" }
                if let Some(sched) = w.schedule.clone() {
                    span { class: "sched", "{sched}" }
                }
            }
            div { class: "note", "{w.description}" }
            form {
                class: "go",
                onsubmit: move |e| {
                    let name = name.clone();
                    // One message on submit, not a round trip per keystroke.
                    let input = form_input(&e);
                    async move { let _ = run_with(name, input).await; }
                },
                if let Some(ex) = example.clone() {
                    input { name: "input", value: "{ex}", spellcheck: "false",
                            autocomplete: "off", class: "arg" }
                }
                button { r#type: "submit", "run" }
            }
        }
    }
}

/// The named field's text, or empty. Kept out of the closure so the onsubmit
/// body is one line.
fn form_input(e: &dioxus::prelude::Event<dioxus::events::FormData>) -> String {
    match e.get_first("input") {
        Some(dioxus::events::FormValue::Text(t)) => t,
        _ => String::new(),
    }
}

pub(crate) fn ed_machine(s: &State) -> Element {
    rsx! {
        div { class: "tiles",
            {tile("cpu", format!("{:.0}%", s.machine.get("box.cpu_pct").copied().unwrap_or(0.0)), None)}
            {tile("load", format!("{:.2}", s.machine.get("box.load1").copied().unwrap_or(0.0)), None)}
            {tile("memory", gib(s.machine.get("box.mem_used").copied().unwrap_or(0.0)),
                  Some(format!("of {}", gib(s.machine.get("box.mem_total").copied().unwrap_or(0.0)))))}
            {tile("disk", gib(s.machine.get("box.disk_used").copied().unwrap_or(0.0)),
                  Some(format!("of {}", gib(s.machine.get("box.disk_total").copied().unwrap_or(0.0)))))}
            {tile("agents", format!("{}", s.fleet.len()), None)}
        }

        div { class: "plot",
            div { class: "cap",
                b { "cpu" }
                span { "last hour · now " b { "{s.machine.get(\"box.cpu_pct\").copied().unwrap_or(0.0):.0}%" } }
            }
            {chart(&s.cpu, &s.annotations, s.window, 100.0, |v| format!("{v:.0}%"))}
        }
        div { class: "plot",
            div { class: "cap",
                b { "memory" }
                span { "last hour · now " b { "{gib(s.machine.get(\"box.mem_used\").copied().unwrap_or(0.0))}" } }
            }
            {chart(&s.mem, &s.annotations, s.window,
                   s.machine.get("box.mem_total").copied().unwrap_or(1.0), gib)}
        }
        div { class: "annkey",
            span { "runs shaded:" }
            span { i { class: "done" } "done" }
            span { i { class: "running" } "running" }
            span { i { class: "suspended" } "suspended" }
            span { i { class: "failed" } "failed" }
        }
    }
}

pub(crate) fn ed_fleet(s: &State) -> Element {
    rsx! {
        if s.fleet.is_empty() {
            div { class: "empty", "no agents" }
        }
        for a in s.fleet.iter() {
            div { class: "agent", key: "{a.pane}",
                span { class: "status {a.status}", "{a.status}" }
                span { class: "wf", "{a.name}" }
                span { class: "procs",
                    // The two biggest by memory, not every pid. A full process
                    // table is htop's job; this row answers "what is this
                    // agent doing and how big is it".
                    for p in a.procs.iter().take(2) {
                        span { class: "proc", key: "{p.pid}", "{p.name}" }
                    }
                    if a.procs.len() > 2 {
                        span { class: "note", "+{a.procs.len() - 2}" }
                    }
                    span { class: "note", "{gib(a.procs.iter().map(|p| p.rss).sum::<f64>())}" }
                    span { class: "note", "{short(&a.cwd, 46)}" }
                }
            }
        }
    }
}

/// The Info log: every command that ran, newest first — Blender's Info editor.
/// The workbench's own audit trail, and the more useful now that an agent
/// drives the same command bus: this is where you see what it did.
pub(crate) fn ed_info(s: &State) -> Element {
    rsx! {
        div { class: "info-log",
            if s.log.is_empty() {
                div { class: "note", "no commands yet" }
            }
            for (i, e) in s.log.iter().enumerate() {
                div {
                    class: if e.ok { "log-row" } else { "log-row failed" },
                    key: "{i}-{e.at}",
                    span { class: "when", "{hhmmss(e.at)}" }
                    span { class: "src {e.source}", "{e.source}" }
                    span { class: "k", "{e.name}" }
                    span { class: "note", "{short(&e.params.to_string(), 80)}" }
                }
            }
        }
    }
}

pub(crate) fn ed_timers(s: &State) -> Element {
    rsx! {
        div { class: "timers",
            if s.timers.is_empty() {
                div { class: "note", "nothing scheduled" }
            }
            for t in s.timers.iter() {
                div { class: "timer", key: "{t.source}-{t.name}",
                    span { class: "src {t.source}", "{t.source}" }
                    span { class: "k", "{t.name}" }
                    span { class: "note", "{t.schedule}" }
                    span { class: "when", "{t.next}" }
                }
            }
        }
    }
}

pub(crate) fn ed_runs(s: &State, area: AreaId, open_run: Callback<(AreaId, String)>) -> Element {
    rsx! {
        div { class: "im-filter-scope runs-list",
            if s.runs.is_empty() {
                div { class: "empty", "No runs yet — trigger one from an Actions area." }
            } else {
                div { class: "runs-filter", FilterBox { placeholder: "filter runs…" } }
            }
            for r in s.runs.iter().cloned() {
                {run_row(r, area, open_run)}
            }
        }
    }
}

/// One expandable run. Pulled out of `ed_runs` so the view tree stays shallow
/// — a nine-deep rsx block reads no better than a nine-deep function.
fn run_row(r: RunView, area: AreaId, open_run: Callback<(AreaId, String)>) -> Element {
    let open_id = r.id.clone();
    rsx! {
        details {
            class: "run",
            key: "{r.id}",
            "data-filter-text": "{r.workflow} {r.status} {r.note.clone().unwrap_or_default()} {r.error.clone().unwrap_or_default()}",
            summary {
                span { class: "status {r.status}", "{r.status}" }
                span {
                    span { class: "wf", "{r.workflow}" }
                    if let Some(note) = r.note.clone() {
                        span { class: "note", " — {note}" }
                    }
                    if let Some(err) = r.error.clone() {
                        span { class: "note err", " — {short(&err, 120)}" }
                    }
                }
                span { class: "when",
                    // Pin this run into its own area beside the list.
                    button {
                        class: "open",
                        title: "open in a new area",
                        onclick: move |e| { e.stop_propagation(); open_run.call((area, open_id.clone())); },
                        "⇱"
                    }
                    if r.status == "suspended" || r.status == "failed" {
                        {resume_button(r.id.clone())}
                    }
                    "{hhmmss(r.updated_at)}"
                }
            }
            div { class: "steps",
                if r.steps.is_empty() {
                    div { class: "step", span {} span { class: "note", "no steps recorded" } }
                }
                for st in r.steps.iter().cloned() {
                    {step_row(st)}
                }
            }
        }
    }
}

fn resume_button(id: String) -> Element {
    rsx! {
        button {
            class: "resume",
            onclick: move |e| {
                e.stop_propagation();
                let id = id.clone();
                async move { let _ = resume(id).await; }
            },
            "resume"
        }
    }
}

fn step_row(st: StepView) -> Element {
    let failed = st.error.is_some();
    rsx! {
        div { class: "step", key: "{st.key}",
            span { class: if failed { "failed" } else { "done" },
                if failed { "✗" } else { "✓" }
            }
            span { class: "k", "{st.key}" }
            code { class: if failed { "err" } else { "" },
                {short(st.error.as_deref().or(st.result.as_deref()).unwrap_or(""), 220)}
            }
        }
    }
}

/// A single run, live: header, note/error, every recorded step. This is the
/// area you pin beside the fleet and CPU charts to watch a run work. It reads
/// the same polled snapshot as the list, filtered to one id, so it updates on
/// the same tick with no extra plumbing.
pub(crate) fn ed_run_detail(s: &State, id: &str) -> Element {
    let Some(r) = s.runs.iter().find(|r| r.id == id).cloned() else {
        return rsx! {
            div { class: "empty",
                "run {short(id, 8)} is not in the recent window."
            }
        };
    };
    rsx! {
        div { class: "run-detail",
            div { class: "rd-head",
                span { class: "status {r.status}", "{r.status}" }
                span { class: "wf", "{r.workflow}" }
                span { class: "note", "{short(id, 8)}" }
                if r.status == "suspended" || r.status == "failed" {
                    {resume_button(r.id.clone())}
                }
            }
            if let Some(note) = r.note.clone() {
                div { class: "note", "{note}" }
            }
            if let Some(err) = r.error.clone() {
                div { class: "note err", "{err}" }
            }
            div { class: "steps",
                if r.steps.is_empty() {
                    div { class: "step", span {} span { class: "note", "no steps yet" } }
                }
                for st in r.steps.iter().cloned() {
                    {step_row(st)}
                }
            }
        }
    }
}

/// When a run-detail area has no run yet — picked from the dropdown rather
/// than opened from the list — offer the recent runs to open in place.
pub(crate) fn ed_run_picker(
    s: &State,
    area: AreaId,
    open_run: Callback<(AreaId, String)>,
) -> Element {
    rsx! {
        div { class: "note", "Pick a run to show here:" }
        for r in s.runs.iter().take(12).cloned() {
            div {
                class: "pick",
                onclick: move |_| open_run.call((area, r.id.clone())),
                span { class: "status {r.status}", "{r.status}" }
                span { class: "wf", "{r.workflow}" }
                span { class: "note", "{short(&r.id, 8)} · {hhmmss(r.updated_at)}" }
            }
        }
    }
}

/// The Settings editor: widgets over the settings document. Every control is
/// bound to a pointer by settings_fields(); an edit is (pointer, value) that
/// the host persists. Live: accent recolors the workbench, the interval
/// changes the poll cadence, the splash toggle mirrors the splash's own.
pub(crate) fn ed_settings(
    doc: serde_json::Value,
    on_setting: Callback<(String, serde_json::Value)>,
    on_error: Callback<immersion::EditorError>,
) -> Element {
    rsx! {
        div { class: "settings",
            PropertyEditor {
                doc,
                fields: settings_fields(),
                on_edit: on_setting,
                on_error,
            }
        }
    }
}

/// The data editor — Blender's Outliner in Data API mode, for our world. The
/// workbench's documents mounted under one root, every row addressable,
/// right-click ▸ Copy data path. What `set_setting` edits and what the MCP
/// tools read stops being invisible: this is the address space, on screen.
pub(crate) fn ed_data(s: &State, target: Option<String>) -> Element {
    let state_doc = serde_json::to_value(s).unwrap_or_default();
    // The target roots the view: an area pointed at /settings/favorites shows
    // that subtree and nothing else, which is what makes several data areas
    // useful side by side instead of three copies of the same scroll.
    let root = target.unwrap_or_default();
    let children_of = Callback::new(move |pointer: String| {
        let at = if pointer.is_empty() {
            root.clone()
        } else {
            pointer
        };
        data_children(&state_doc, &at)
    });
    rsx! {
        div { class: "data-editor im-filter-scope",
            div { class: "keymap-head", FilterBox { placeholder: "filter fields\u{2026}" } }
            TreeView { children_of }
        }
    }
}

/// Children under the mounted root. `""` lists the mounts; below, the pointer
/// starts with the mount's name and the rest addresses within that document.
pub(crate) fn data_children(
    state_doc: &serde_json::Value,
    pointer: &str,
) -> Vec<immersion::TreeRow> {
    if pointer.is_empty() {
        let mount = |name: &str, preview: &str| immersion::TreeRow {
            pointer: format!("/{name}"),
            label: format!("/{name}"),
            preview: preview.to_string(),
            has_children: true,
        };
        return vec![
            mount("layout", "the workspace tree"),
            mount("settings", "the settings document"),
            mount("keymap", "chord overrides"),
            mount("favorites", "the Q menu"),
            mount("state", "host snapshot (read-only)"),
        ];
    }
    let (mount, inner) = match pointer[1..].find('/') {
        Some(i) => (&pointer[..i + 1], &pointer[i + 1..]),
        None => (pointer, ""),
    };
    let doc = match mount {
        "/layout" => serde_json::to_value(crate::daemon::workspaces()).unwrap_or_default(),
        "/settings" => crate::daemon::settings(),
        "/keymap" => crate::daemon::settings()["keymap"].clone(),
        "/favorites" => crate::daemon::settings()["favorites"].clone(),
        "/state" => state_doc.clone(),
        _ => return Vec::new(),
    };
    let mut rows = immersion::value_children(&doc, inner);
    for r in &mut rows {
        r.pointer = format!("{mount}{}", r.pointer);
    }
    rows
}

/// The file browser: the tree view over a directory. Lazily loaded — a
/// directory reads only when its branch opens.
pub(crate) fn ed_files(target: Option<String>) -> Element {
    let root = target.unwrap_or_default();
    let children_of = Callback::new(move |pointer: String| {
        let at = if pointer.is_empty() {
            root.clone()
        } else {
            pointer
        };
        file_children(&at)
    });
    rsx! {
        div { class: "files-editor im-filter-scope",
            div { class: "keymap-head", FilterBox { placeholder: "filter files\u{2026}" } }
            TreeView { children_of }
        }
    }
}

/// Where the browser is rooted. `POWDERMAN_FILES_ROOT` confines it; without
/// one it is the daemon's working directory, which is where the things a run
/// touches live. Canonicalized, because containment is checked against it.
fn files_root() -> std::path::PathBuf {
    let raw = std::env::var("POWDERMAN_FILES_ROOT")
        .map(std::path::PathBuf::from)
        .or_else(|_| std::env::current_dir())
        .unwrap_or_else(|_| std::path::PathBuf::from("/"));
    raw.canonicalize().unwrap_or(raw)
}

/// One directory level. The demo serves a fabricated tree instead of the
/// machine's own: a public instance listing its container's filesystem is an
/// invitation, and it only gets worse when the code viewer can read what the
/// browser lists.
pub(crate) fn file_children(pointer: &str) -> Vec<immersion::TreeRow> {
    if crate::demo::enabled() {
        return crate::demo::file_children(pointer);
    }
    const CAP: usize = 500;
    let root = files_root();
    let dir = root.join(pointer.trim_start_matches('/'));
    // Containment, checked rather than assumed: resolve the path and require
    // it to still be under the root. A "../.." in a crafted pointer and a
    // symlink pointing outward both fail here, which is why the check is on
    // the resolved path and not on the text of the pointer.
    let Ok(dir) = dir.canonicalize() else {
        return Vec::new();
    };
    if !dir.starts_with(&root) {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut rows: Vec<(bool, bool, String, u64)> = entries
        .filter_map(|e| {
            let e = e.ok()?;
            let name = e.file_name().to_string_lossy().into_owned();
            // Symlinks are leaves: a link's target is better opened where it
            // really lives, and not following one keeps the walk inside root
            // without a second canonicalize per entry.
            let meta = e.path().symlink_metadata().ok()?;
            Some((meta.is_dir(), name.starts_with('.'), name, meta.len()))
        })
        .collect();
    rows.sort_by(|a, b| (!a.0, a.1, &a.2).cmp(&(!b.0, b.1, &b.2)));
    let extra = rows.len().saturating_sub(CAP);
    let mut out: Vec<immersion::TreeRow> = rows
        .into_iter()
        .take(CAP)
        .map(|(is_dir, _, name, len)| immersion::TreeRow {
            pointer: format!("{pointer}/{name}"),
            label: if is_dir { format!("{name}/") } else { name },
            preview: if is_dir {
                String::new()
            } else {
                human_size(len)
            },
            has_children: is_dir,
        })
        .collect();
    if extra > 0 {
        out.push(immersion::TreeRow {
            pointer: format!("{pointer}/\u{2026}"),
            label: format!("\u{2026} {extra} more"),
            preview: String::new(),
            has_children: false,
        });
    }
    out
}

pub(crate) fn human_size(len: u64) -> String {
    match len {
        0..=1023 => format!("{len} B"),
        1024..=1048575 => format!("{:.1} K", len as f64 / 1024.0),
        1048576..=1073741823 => format!("{:.1} M", len as f64 / 1048576.0),
        _ => format!("{:.1} G", len as f64 / 1073741824.0),
    }
}

#[cfg(test)]
mod file_browser {
    /// Containment is the property that matters: whatever pointer arrives,
    /// the browser must not read outside its root. Checked on the resolved
    /// path, so a crafted "../.." fails even though the text looks harmless
    /// after joining.
    #[test]
    fn a_pointer_cannot_climb_out_of_the_root() {
        let tmp = std::env::temp_dir().join("im-files-root-test");
        let inner = tmp.join("inside");
        std::fs::create_dir_all(&inner).expect("mkdir");
        std::fs::write(inner.join("kept.txt"), b"hi").expect("write");
        // SAFETY: single-threaded test process; the var is read on the next
        // line and nothing else in this test touches the environment.
        unsafe { std::env::set_var("POWDERMAN_FILES_ROOT", &tmp) };

        let inside = super::file_children("/inside");
        assert!(
            inside.iter().any(|r| r.label == "kept.txt"),
            "the root's own files list: {inside:?}"
        );
        for escape in ["/..", "/../..", "/inside/../..", "/../etc"] {
            assert!(
                super::file_children(escape).is_empty(),
                "{escape} escaped the root"
            );
        }
        unsafe { std::env::remove_var("POWDERMAN_FILES_ROOT") };
        std::fs::remove_dir_all(&tmp).ok();
    }
}
