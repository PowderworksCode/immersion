//! The editor bodies — what an area shows once the chrome has decided where.
//!
//! Split out of `ui.rs` so that file stays about the workbench itself (the
//! App, its signals, the chrome and the routing) while this one is about
//! rendering host state. Each function takes the polled snapshot and returns a
//! body; none of them own state.

use dioxus::prelude::*;
use immersion::{AreaId, FilterBox, PropertyEditor, pretty_chord};

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
) -> Element {
    rsx! {
        div { class: "settings",
            PropertyEditor {
                doc,
                fields: settings_fields(),
                on_edit: on_setting,
            }
        }
    }
}
