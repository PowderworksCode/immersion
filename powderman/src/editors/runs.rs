//! The runs editor.

use dioxus::prelude::*;
use immersion::{AreaId, FilterBox, Panel};

use crate::editors::{Draw, prop};

use crate::ui::{RunView, State, StepView, resume};
use crate::ui::{hhmmss, short};

pub(crate) fn ed_runs(d: &Draw, open_run: Callback<(AreaId, String)>) -> Element {
    let s = &d.state;
    let area = d.area;
    let chosen = d.selection.get("run").cloned().unwrap_or_default();
    rsx! {
        div { class: "im-filter-scope runs-list",
            if s.runs.is_empty() {
                div { class: "empty", "No runs yet — trigger one from an Actions area." }
            } else {
                div { class: "runs-filter", FilterBox { placeholder: "filter runs…" } }
            }
            for r in s.runs.iter().cloned() {
                {run_row(RowCtx {
                    area,
                    open_run,
                    cmd: d.cmd,
                    // Many selected, one active: the whole list marks what is
                    // selected, and the last one picked is drawn as the one
                    // a detail pane is showing.
                    selected: chosen.contains(&r.id),
                    active: chosen.last() == Some(&r.id),
                    all: chosen.to_vec(),
                }, r)}
            }
        }
    }
}

/// One expandable run. Pulled out of `ed_runs` so the view tree stays shallow
/// — a nine-deep rsx block reads no better than a nine-deep function.
/// What a row needs beyond the run itself. One struct because the list is the
/// only caller and its arguments were about to be six.
struct RowCtx {
    area: AreaId,
    open_run: Callback<(AreaId, String)>,
    cmd: Callback<(String, serde_json::Value)>,
    selected: bool,
    active: bool,
    /// Everything selected, so a row action can act across the selection
    /// rather than only on the row it was clicked from.
    all: Vec<String>,
}

fn run_row(ctx: RowCtx, r: RunView) -> Element {
    let RowCtx {
        area,
        open_run,
        cmd,
        selected,
        active,
        all,
    } = ctx;
    let open_id = r.id.clone();
    let pick_id = r.id.clone();
    // Copying one id is the common case; copying the five you just selected
    // is the reason multi-select is worth having, and it needs no new command.
    let copy = if selected && all.len() > 1 {
        all.join("\n")
    } else {
        r.id.clone()
    };
    let copy_label = if selected && all.len() > 1 {
        format!("copy {} selected ids", all.len())
    } else {
        "copy this run's id".to_string()
    };
    rsx! {
        details {
            class: match (selected, active) {
                (true, true) => "run is-sel is-active",
                (true, false) => "run is-sel",
                _ => "run",
            },
            key: "{r.id}",
            "data-filter-text": "{r.workflow} {r.status} {r.note.clone().unwrap_or_default()} {r.error.clone().unwrap_or_default()}",
            // Clicking a row both expands it and selects it. The two do not
            // compete: expanding shows this run's steps here, selecting points
            // any unpinned run area at it. The arrow still means "open a new
            // area", which is the thing a click cannot say.
            // Ctrl or Cmd extends the selection, the way it does in every
            // list anyone has used. A plain click replaces it.
            onclick: move |e: Event<MouseData>| {
                let mods = e.modifiers();
                let mode = if mods.ctrl() || mods.meta() {
                    "extend"
                } else {
                    "replace"
                };
                cmd.call((
                    "select".to_string(),
                    serde_json::json!({ "kind": "run", "value": pick_id.clone(), "mode": mode }),
                ));
            },
            summary {
                class: "im-row",
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
                    span { class: "im-row-actions",
                        // The id is what you paste into a CLI or hand to an
                        // agent, and it is the one thing on the row that is
                        // never fully shown.
                        button {
                            class: "im-row-btn im-copy",
                            title: "{copy_label}",
                            "data-im-copy": "{copy}",
                            onclick: move |e| e.stop_propagation(),
                            dangerous_inner_html: "{immersion::icon(\"copy\")}",
                        }
                        // Pin this run into its own area beside the list.
                        button {
                            class: "im-row-btn",
                            title: "open in a new area",
                            onclick: move |e| { e.stop_propagation(); open_run.call((area, open_id.clone())); },
                            dangerous_inner_html: "{immersion::icon(\"arrow-bar-up\")}",
                        }
                    }
                    // Resume stays visible: a suspended run is waiting for
                    // someone, and an affordance you have to hover to find is
                    // not how you tell them.
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

/// The runs, as pickable rows: newest first, labelled the way the runs list
/// labels them, so recognising one in the picker takes no translation.
pub(crate) fn run_targets(s: &State) -> Vec<immersion::TreeRow> {
    let mut runs = s.runs.clone();
    runs.sort_by_key(|r| std::cmp::Reverse(r.updated_at));
    runs.iter()
        .map(|r| immersion::TreeRow {
            icon: "file-text".to_string(),
            pointer: r.id.clone(),
            label: format!("{} {}", r.status, r.workflow),
            preview: r
                .note
                .clone()
                .or_else(|| r.error.clone())
                .unwrap_or_else(|| short(&r.id, 8)),
            has_children: false,
        })
        .collect()
}

/// This editor's entry in the registry: what it is called, how it is drawn in
/// a header, whether it takes a target, and what the status bar says while it
/// has focus. Declared beside the editor so adding one is one file.
pub(crate) fn kind() -> immersion::EditorKind {
    immersion::EditorKind {
        id: "runs",
        label: "Runs",
        icon: "list-details",
        hints: &[("Click", "Open run"), ("Type", "Filter")],
        targets: false,
    }
}

/// This editor's entry in the registry: what it is called, how it is drawn in
/// a header, whether it takes a target, and what the status bar says while it
/// has focus. Declared beside the editor so adding one is one file.
pub(crate) fn detail_kind() -> immersion::EditorKind {
    immersion::EditorKind {
        id: "run",
        label: "Run detail",
        icon: "file-text",
        hints: &[("Chip", "Pick run")],
        targets: true,
    }
}

/// The run list as a tally, by the same status words the rows are coloured
/// by, so the sidebar and the list never disagree about what "failed" means.
pub(crate) fn sidebar(d: &Draw) -> Element {
    let s = &d.state;
    let n = |status: &str| s.runs.iter().filter(|r| r.status == status).count();
    let mut workflows: Vec<&str> = s.runs.iter().map(|r| r.workflow.as_str()).collect();
    workflows.sort_unstable();
    workflows.dedup();
    let steps: usize = s.runs.iter().map(|r| r.steps.len()).sum();
    rsx! {
        div { class: "area-props",
            Panel { title: "Runs",
                {prop("Total", s.runs.len().to_string())}
                {prop("Running", n("running").to_string())}
                {prop("Suspended", n("suspended").to_string())}
                {prop("Done", n("done").to_string())}
                {prop("Failed", n("failed").to_string())}
            }
            Panel { title: "Work", open: false,
                {prop("Workflows", workflows.len().to_string())}
                {prop("Steps", steps.to_string())}
                {prop("Errors", s.runs.iter().filter(|r| r.error.is_some()).count().to_string())}
            }
        }
    }
}

/// The bottom line: the total, and the two counts worth knowing without
/// reading the list.
pub(crate) fn footer(d: &Draw) -> String {
    let n = |status: &str| d.state.runs.iter().filter(|r| r.status == status).count();
    format!(
        "{} runs · {} running · {} failed",
        d.state.runs.len(),
        n("running"),
        n("failed")
    )
}
