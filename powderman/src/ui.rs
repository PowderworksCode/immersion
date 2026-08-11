//! The UI, as a Dioxus liveview component.
//!
//! Server-rendered and reactive over a websocket: the browser holds no state
//! and there is no build step. The component polls the database on a timer
//! and re-renders, which at this size is simpler than pushing invalidations
//! and is indistinguishable to look at.
//!
//! What it shows is deliberately narrow — what runs exist, what each is
//! parked on, and what every step recorded. That last one is not a log: it is
//! the replay data, so what you are reading is literally what a resumed run
//! would return instead of re-executing.

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct StepView {
    pub key: String,
    pub result: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RunView {
    pub id: String,
    pub workflow: String,
    pub status: String,
    pub note: Option<String>,
    pub error: Option<String>,
    pub updated_at: i64,
    pub steps: Vec<StepView>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct WorkflowView {
    pub name: String,
    pub description: String,
    pub example: Option<String>,
    pub schedule: Option<String>,
    pub next: Option<String>,
}

/// One thing on this box that fires on a clock, whoever owns it.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TimerRow {
    pub source: String,
    pub name: String,
    pub schedule: String,
    pub next: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Annotation {
    pub id: String,
    pub workflow: String,
    pub from: i64,
    pub to: i64,
    pub status: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub struct State {
    pub herdr: Option<String>,
    pub workflows: Vec<WorkflowView>,
    pub runs: Vec<RunView>,
    pub machine: std::collections::HashMap<String, f64>,
    pub cpu: Vec<(i64, f64)>,
    pub mem: Vec<(i64, f64)>,
    pub annotations: Vec<Annotation>,
    pub fleet: Vec<crate::metrics::FleetAgent>,
    pub timers: Vec<TimerRow>,
    pub window: (i64, i64),
}

pub const CSS: &str = include_str!("ui.css");

fn short(s: &str, n: usize) -> String {
    if s.chars().count() > n {
        format!("{}…", s.chars().take(n).collect::<String>())
    } else {
        s.to_string()
    }
}

fn hhmmss(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|t| {
            t.with_timezone(&chrono::Local)
                .format("%H:%M:%S")
                .to_string()
        })
        .unwrap_or_default()
}

fn gib(bytes: f64) -> String {
    format!("{:.1}G", bytes / 1_073_741_824.0)
}

/// A line chart with the run windows shaded behind it.
///
/// The shading is the point. A CPU line alone says the box was busy, which
/// htop already tells you; the bands say *which run* was alive at the time.
/// Each band carries an SVG <title>, so identity is never colour alone and the
/// tooltip costs no round trip — a hover handler under liveview would be a
/// websocket message per mouse move.
fn chart(
    points: &[(i64, f64)],
    anns: &[Annotation],
    window: (i64, i64),
    max_hint: f64,
    fmt: fn(f64) -> String,
) -> Element {
    let (t0, t1) = window;
    let span = ((t1 - t0) as f64).max(1.0);
    let max = points
        .iter()
        .map(|(_, v)| *v)
        .fold(max_hint, f64::max)
        .max(1.0);
    let x = |t: i64| ((t - t0) as f64 / span * 100.0).clamp(0.0, 100.0);
    let y = |v: f64| 30.0 - (v / max * 28.0).clamp(0.0, 28.0);

    let line: String = points
        .iter()
        .map(|(t, v)| format!("{:.2},{:.2}", x(*t), y(*v)))
        .collect::<Vec<_>>()
        .join(" ");
    // A 10% wash under the line, closed to the baseline.
    let area = if points.len() > 1 {
        format!(
            "{:.2},30 {} {:.2},30",
            x(points[0].0),
            line,
            x(points[points.len() - 1].0)
        )
    } else {
        String::new()
    };
    let last = points.last().copied();

    rsx! {
        svg { class: "chart", view_box: "0 0 100 30", preserve_aspect_ratio: "none",
            // Recessive hairline grid: quarters, solid, one step off surface.
            for i in 1..4 {
                line { key: "{i}", class: "grid",
                    x1: "0", x2: "100",
                    y1: "{30.0 - i as f64 * 7.5}", y2: "{30.0 - i as f64 * 7.5}" }
            }
            for a in anns.iter().filter(|a| a.to > t0) {
                rect {
                    key: "{a.id}",
                    x: "{x(a.from)}", y: "0",
                    width: "{(x(a.to) - x(a.from)).max(0.35)}", height: "30",
                    class: "ann {a.status}",
                    title { "{a.workflow} · {a.status} · {hhmmss(a.from)}–{hhmmss(a.to)}" }
                }
            }
            if !area.is_empty() {
                polygon { points: "{area}", class: "fill" }
            }
            polyline { points: "{line}", class: "line" }
            // No end-dot: preserveAspectRatio="none" scales x and y by
            // different factors, so a circle renders as an ellipse. The
            // endpoint is labelled in the caption instead, which is where a
            // single direct label belongs anyway.
            if let Some((t, v)) = last {
                line { class: "end", x1: "{x(t)}", x2: "{x(t)}",
                    y1: "{y(v) - 1.6}", y2: "{y(v) + 1.6}",
                    title { "{fmt(v)}" } }
            }
        }
    }
}

/// A stat tile. The headline number, big, with its context small beneath —
/// not a run of text in a row.
fn tile(k: &str, v: String, of: Option<String>) -> Element {
    rsx! {
        div { class: "tile",
            div { class: "k", "{k}" }
            div { class: "v", "{v}" }
            if let Some(o) = of { div { class: "of", "{o}" } }
        }
    }
}

use immersion::{AreaId, Areas, Dir, EditorKind};

/// The registry: what an area's dropdown offers. The ids are what the tree
/// stores, so renaming one is a migration, not a refactor.
fn kinds() -> Vec<EditorKind> {
    vec![
        EditorKind {
            id: "machine",
            label: "Machine",
        },
        EditorKind {
            id: "fleet",
            label: "Fleet",
        },
        EditorKind {
            id: "runs",
            label: "Runs",
        },
        EditorKind {
            id: "actions",
            label: "Actions",
        },
        EditorKind {
            id: "timers",
            label: "Timers",
        },
    ]
}

#[component]
pub fn App() -> Element {
    let mut state = use_signal(State::default);
    let mut layout = use_signal(crate::daemon::layout);

    // Poll rather than push. One query per second against a WAL database that
    // one process writes is not a cost worth engineering away yet. The layout
    // rides along so a second browser converges on mutations within a tick.
    use_future(move || async move {
        loop {
            if let Ok(s) = fetch_state().await {
                state.set(s);
            }
            layout.set(crate::daemon::layout());
            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
        }
    });

    let s = state.read().clone();

    // Mutations write through the daemon (which persists) and update the
    // local signal immediately — the poll is for OTHER clients, and waiting a
    // tick for your own split would read as lag.
    let on_switch = use_callback(move |(id, kind): (AreaId, String)| {
        layout.set(crate::daemon::mutate_layout(|l| {
            l.set_editor(id, &kind);
        }));
    });
    let on_split = use_callback(move |(id, dir, frac): (AreaId, Dir, f32)| {
        layout.set(crate::daemon::mutate_layout(|l| {
            l.split(id, dir, frac);
        }));
    });
    let on_join = use_callback(move |id: AreaId| {
        layout.set(crate::daemon::mutate_layout(|l| {
            l.join(id);
        }));
    });
    let on_join_into = use_callback(move |(survivor, victim): (AreaId, AreaId)| {
        // join_into refuses non-siblings, so an over-ambitious drag is a
        // no-op rather than a corrupted tree.
        layout.set(crate::daemon::mutate_layout(|l| {
            l.join_into(survivor, victim);
        }));
    });
    let on_ratio = use_callback(move |(id, ratio): (AreaId, f32)| {
        layout.set(crate::daemon::mutate_layout(|l| {
            l.set_ratio(id, ratio);
        }));
    });

    let render_state = state;
    let render = use_callback(move |(_id, editor): (AreaId, String)| {
        let s = render_state.read().clone();
        match editor.as_str() {
            "machine" => ed_machine(&s),
            "fleet" => ed_fleet(&s),
            "runs" => ed_runs(&s),
            "actions" => ed_actions(&s),
            "timers" => ed_timers(&s),
            other => rsx! { div { class: "empty", "unknown editor {other}" } },
        }
    });

    rsx! {
        style { "{immersion::CSS}" }
        style { "{CSS}" }
        div { class: "app",
            div { class: "topbar",
                span { class: "brand", "powderman" }
                span { class: "sub",
                    {s.herdr.clone().unwrap_or_else(|| "herdr unreachable".into())}
                    " · {s.runs.len()} runs"
                }
            }
            div { class: "deck",
                Areas {
                    layout: layout.read().clone(),
                    kinds: kinds(),
                    render,
                    on_switch,
                    on_split,
                    on_join,
                    on_join_into,
                    on_ratio,
                }
            }
        }
    }
}

// --- editors --------------------------------------------------------------
// Each one is a body the library mounts under an area header. They read the
// polled snapshot and call the same daemon functions the old page did; the
// workbench changed where they sit, not what they are.

fn ed_actions(s: &State) -> Element {
    rsx! {
        div { class: "actions",
            for w in s.workflows.iter().cloned() {
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
                            let name = w.name.clone();
                            // One message on submit, rather than a websocket
                            // round trip per keystroke.
                            let input = match e.get_first("input") {
                                Some(dioxus::events::FormValue::Text(t)) => t,
                                _ => String::new(),
                            };
                            async move { let _ = run_with(name, input).await; }
                        },
                        if let Some(ex) = w.example.clone() {
                            input { name: "input", value: "{ex}", spellcheck: "false",
                                    autocomplete: "off", class: "arg" }
                        }
                        button { r#type: "submit", "run" }
                    }
                }
            }
        }
    }
}

fn ed_machine(s: &State) -> Element {
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

fn ed_fleet(s: &State) -> Element {
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

fn ed_timers(s: &State) -> Element {
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

fn ed_runs(s: &State) -> Element {
    rsx! {
        if s.runs.is_empty() {
            div { class: "empty", "No runs yet — trigger one from an Actions area." }
        }
        for r in s.runs.iter().cloned() {
            details { class: "run", key: "{r.id}",
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
                        if r.status == "suspended" || r.status == "failed" {
                            button {
                                class: "resume",
                                onclick: move |e| {
                                    e.stop_propagation();
                                    let id = r.id.clone();
                                    async move { let _ = resume(id).await; }
                                },
                                "resume"
                            }
                        }
                        "{hhmmss(r.updated_at)}"
                    }
                }
                div { class: "steps",
                    if r.steps.is_empty() {
                        div { class: "step", span {} span { class: "note", "no steps recorded" } }
                    }
                    for st in r.steps.iter().cloned() {
                        div { class: "step", key: "{st.key}",
                            span { class: if st.error.is_some() { "failed" } else { "done" },
                                if st.error.is_some() { "✗" } else { "✓" }
                            }
                            span { class: "k", "{st.key}" }
                            code { class: if st.error.is_some() { "err" } else { "" },
                                {short(st.error.as_deref().or(st.result.as_deref()).unwrap_or(""), 220)}
                            }
                        }
                    }
                }
            }
        }
    }
}

// No server functions: under liveview the component *is* server-side, so it
// reads the database the daemon already owns. The browser holds nothing but a
// websocket and a DOM.
async fn fetch_state() -> anyhow::Result<State> {
    Ok(crate::daemon::snapshot())
}

async fn run_with(name: String, input: String) -> anyhow::Result<()> {
    let value = if input.trim().is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_str(&input)?
    };
    crate::daemon::trigger_with(&name, value)
}

async fn resume(id: String) -> anyhow::Result<()> {
    crate::daemon::resume(&id)
}
