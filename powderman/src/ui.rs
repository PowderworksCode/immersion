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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct LogEntry {
    pub name: String,
    pub params: serde_json::Value,
    pub at: i64,
    pub ok: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub struct State {
    pub herdr: Option<String>,
    pub log: Vec<LogEntry>,
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

use immersion::{
    AreaId, Areas, ContextMenu, Dir, EditorKind, Field, FieldKind, Keymap, Layout, Palette,
    PaletteItem, PropertyEditor, Splash, SplashRecent, StatusBar, Template, Tooltips,
    WorkspaceTabs, default_keymap,
};

/// The registry: what an area's dropdown offers. The ids are what the tree
/// stores, so renaming one is a migration, not a refactor.
/// The preset layouts offered on the splash. Each is a small tree the "New
/// workspace" column builds from.
fn templates() -> Vec<Template> {
    let overview = {
        let mut l = Layout::single("machine");
        if let Some(b) = l.split(1, Dir::Col, 0.45) {
            l.set_editor(b, "runs");
            if let Some(r) = l.split(b, Dir::Row, 0.6) {
                l.set_editor(r, "fleet");
            }
        }
        l
    };
    let runs_focus = {
        let mut l = Layout::single("runs");
        if let Some(r) = l.split(1, Dir::Row, 0.5) {
            l.set_editor(r, "run");
        }
        l
    };
    let monitoring = {
        let mut l = Layout::single("machine");
        if let Some(r) = l.split(1, Dir::Row, 0.62) {
            l.set_editor(r, "fleet");
        }
        l
    };
    vec![
        Template {
            name: "Overview".into(),
            hint: "machine, runs and fleet at a glance".into(),
            layout: overview,
        },
        Template {
            name: "Runs".into(),
            hint: "the run list beside a detail pane".into(),
            layout: runs_focus,
        },
        Template {
            name: "Monitoring".into(),
            hint: "machine graphs and the live fleet".into(),
            layout: monitoring,
        },
        Template {
            name: "Single".into(),
            hint: "one area to split as you like".into(),
            layout: Layout::single("machine"),
        },
    ]
}

/// Recent runs to jump back into — the "Recent files" column, run-shaped.
fn recents(s: &State) -> Vec<SplashRecent> {
    s.runs
        .iter()
        .take(8)
        .map(|r| SplashRecent {
            label: r.workflow.clone(),
            sub: format!("{} · {}", short(&r.id, 8), hhmmss(r.updated_at)),
            status: r.status.clone(),
            key: r.id.clone(),
        })
        .collect()
}

/// The palette's entries: the actions searchable by F3. They are the same
/// `(action, params)` pairs the keymap fires — host actions (undo/redo/
/// maximize) and navigational bus commands — with a label to search by and the
/// chord they are bound to, so the palette, a chord, and an agent all reach one
/// router. Workspace switches are generated per tab, so the palette lists the
/// rooms you can jump to by name.
fn palette_items(ws: &immersion::Workspaces) -> Vec<PaletteItem> {
    let mut items = vec![
        PaletteItem::new("undo", "Undo")
            .with_hint("revert the last layout change")
            .with_chord("Mod+Z"),
        PaletteItem::new("redo", "Redo")
            .with_hint("reapply an undone change")
            .with_chord("Mod+Shift+Z"),
        PaletteItem::new("maximize", "Maximize area")
            .with_hint("toggle the focused area full-deck")
            .with_chord("Mod+Shift+Space"),
        PaletteItem::new("workspace.cycle", "Next workspace")
            .with_chord("Alt+PageDown")
            .with_params(serde_json::json!({ "delta": 1 })),
        PaletteItem::new("workspace.cycle", "Previous workspace")
            .with_chord("Alt+PageUp")
            .with_params(serde_json::json!({ "delta": -1 })),
    ];
    for (i, tab) in ws.tabs.iter().enumerate() {
        items.push(
            PaletteItem::new("workspace.switch", &format!("Switch to {}", tab.name))
                .with_hint("go to this workspace")
                .with_params(serde_json::json!({ "index": i })),
        );
    }
    items
}

/// The Settings editor's schema: which widget edits which pointer in the
/// settings document. This is the whole binding — a Field per knob, the doc is
/// serde, edits are (pointer, value).
fn settings_fields() -> Vec<Field> {
    vec![
        Field::new("/accent", "Accent color", FieldKind::Color)
            .with_hint("the widget-blue used across the workbench"),
        Field::new(
            "/poll_ms",
            "Refresh interval",
            FieldKind::Slider {
                min: 250.0,
                max: 5000.0,
                step: 250.0,
            },
        )
        .with_hint("how often the page repolls, in ms"),
        Field::new("/splash_on_start", "Splash on startup", FieldKind::Bool),
        Field::new("/tooltips_on", "Tooltips", FieldKind::Bool)
            .with_hint("hover help on the workbench controls"),
        Field::new(
            "/sweep_limit",
            "Default sweep limit",
            FieldKind::Number {
                min: Some(1.0),
                max: Some(1000.0),
                step: Some(50.0),
            },
        )
        .with_hint("packages per ecosystem the daily sweep fetches"),
        Field::new(
            "/density",
            "Density",
            FieldKind::Select(vec![
                ("cozy".into(), "Cozy".into()),
                ("compact".into(), "Compact".into()),
            ]),
        ),
    ]
}

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
        EditorKind {
            id: "run",
            label: "Run detail",
        },
        EditorKind {
            id: "settings",
            label: "Settings",
        },
        EditorKind {
            id: "info",
            label: "Info log",
        },
    ]
}

#[component]
pub fn App() -> Element {
    let mut state = use_signal(State::default);
    let mut ws = use_signal(crate::daemon::workspaces);
    // Settings are a serde document the widget editor edits by pointer. Not on
    // the layout bus — a preference is not a layout mutation — but every edit
    // still round-trips through serde and the database like everything else.
    let mut settings = use_signal(crate::daemon::settings);

    // Poll rather than push. One query per second against a WAL database that
    // one process writes is not a cost worth engineering away yet. The
    // workbench rides along so a second browser converges within a tick.
    use_future(move || async move {
        loop {
            if let Ok(s) = fetch_state().await {
                state.set(s);
            }
            ws.set(crate::daemon::workspaces());
            settings.set(crate::daemon::settings());
            let ms = settings.read()["poll_ms"]
                .as_u64()
                .unwrap_or(1000)
                .clamp(200, 10000);
            tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
        }
    });

    let s = state.read().clone();

    // The splash opens on load unless suppressed, and can be reopened from the
    // brand. A signal, because dismissing is transient UI state that need not
    // touch the database — only the "don't show again" preference persists.
    let mut splash_open = use_signal(move || {
        crate::daemon::settings()["splash_on_start"]
            .as_bool()
            .unwrap_or(true)
    });

    // THE write path. Every button, dropdown, gesture and tab emits a named
    // command; this fires it through the daemon (which applies + persists) and
    // updates the local signal immediately, since the poll is for OTHER
    // clients and waiting a tick for your own edit would read as lag.
    let cmd = use_callback(move |(name, params): (String, serde_json::Value)| {
        ws.set(crate::daemon::dispatch(&name, params));
    });

    let on_setting = use_callback(move |(pointer, value): (String, serde_json::Value)| {
        settings.set(crate::daemon::set_setting(&pointer, value));
    });

    // The command palette is per-client view state, like maximize — one client
    // searching commands does not open the palette in another.
    let mut palette_open = use_signal(|| false);

    // Maximize is per-client view state (two browsers may maximize different
    // areas), so it is a local signal, not a command on the shared tree.
    let mut maximized = use_signal(|| None::<AreaId>);

    // Keymap actions. Layout commands go to the bus; undo/redo/maximize are
    // host concerns the bus does not own.
    let on_action = use_callback(move |(action, params): (String, serde_json::Value)| {
        match action.as_str() {
            "undo" => ws.set(crate::daemon::undo()),
            "redo" => ws.set(crate::daemon::redo()),
            "repeat_last" => ws.set(crate::daemon::repeat_last()),
            "maximize" => {
                // Toggle: maximize the first area if none is, else restore.
                let cur = maximized();
                let first = ws.read().current().layout.root.leaves().first().copied();
                maximized.set(if cur.is_some() { None } else { first });
            }
            "palette" => palette_open.set(true),
            _ => ws.set(crate::daemon::dispatch(&action, params)),
        }
    });

    let on_template = use_callback(move |i: usize| {
        if let Some(t) = templates().into_iter().nth(i) {
            cmd.call((
                "workspace.add".to_string(),
                serde_json::json!({ "name": t.name, "layout": t.layout }),
            ));
        }
    });
    let on_recent = use_callback(move |run_id: String| {
        // A recent run opens into a fresh workspace showing just its detail.
        let mut l = Layout::single("run");
        l.set_editor_arg(1, "run", &run_id);
        let name = format!("run {}", &run_id[..run_id.len().min(8)]);
        cmd.call((
            "workspace.add".to_string(),
            serde_json::json!({ "name": name, "layout": l }),
        ));
    });
    let on_dismiss = use_callback(move |()| splash_open.set(false));
    // The splash's "don't show" checkbox edits the same setting the Settings
    // editor does — one value, two surfaces.
    let on_dont_show = use_callback(move |off: bool| {
        settings.set(crate::daemon::set_setting(
            "/splash_on_start",
            serde_json::json!(!off),
        ));
    });

    // Every mutation writes through the daemon (which persists) and updates
    // the local signal immediately — the poll is for OTHER clients, and
    // waiting a tick for your own split would read as lag. Layout mutations
    // land on the ACTIVE workspace, the only one the gestures can reach.
    // The pin-beside-the-charts payoff, now a command like everything else.
    let open_run = use_callback(move |(area, run_id): (AreaId, String)| {
        cmd.call((
            "open_run".to_string(),
            serde_json::json!({ "area": area, "run": run_id }),
        ));
    });

    // Add duplicates the current tree — you branch off what you are looking
    // at, not a blank — so it needs the live layout and composes the
    // workspace.add command rather than being a bare command.
    let ws_add = use_callback(move |()| {
        let dup = ws.read().current().layout.clone();
        cmd.call((
            "workspace.add".to_string(),
            serde_json::json!({ "name": "New", "layout": dup }),
        ));
    });

    let render_state = state;
    let render = use_callback(
        move |(area, editor, arg): (AreaId, String, Option<String>)| {
            let s = render_state.read().clone();
            let settings_doc = settings.read().clone();
            match editor.as_str() {
                "machine" => ed_machine(&s),
                "fleet" => ed_fleet(&s),
                "runs" => ed_runs(&s, area, open_run),
                "actions" => ed_actions(&s),
                "timers" => ed_timers(&s),
                "info" => ed_info(&s),
                "run" => match arg {
                    Some(id) => ed_run_detail(&s, &id),
                    None => ed_run_picker(&s, area, open_run),
                },
                "settings" => ed_settings(settings_doc.clone(), on_setting),
                other => rsx! { div { class: "empty", "unknown editor {other}" } },
            }
        },
    );

    rsx! {
        style { "{immersion::CSS}" }
        style { "{CSS}" }
        div {
            class: "app",
            style: "--im-accent: {settings()[\"accent\"].as_str().unwrap_or(\"#5680c2\")}; --accent-live: {settings()[\"accent\"].as_str().unwrap_or(\"#5680c2\")}",
            if splash_open() {
                Splash {
                    brand: "powderman",
                    subtitle: "durable workflows · a herdr workbench",
                    templates: templates(),
                    recents: recents(&s),
                    on_template,
                    on_recent,
                    on_dismiss,
                    dont_show: !settings()["splash_on_start"].as_bool().unwrap_or(true),
                    on_dont_show,
                }
            }
            div { class: "topbar",
                span {
                    class: "brand",
                    title: "open the splash",
                    onclick: move |_| splash_open.set(true),
                    "powderman"
                }
                WorkspaceTabs {
                    names: ws.read().tabs.iter().map(|t| t.name.clone()).collect::<Vec<_>>(),
                    active: ws.read().active,
                    on_command: cmd,
                    on_add: ws_add,
                }
                span { class: "sub",
                    {s.herdr.clone().unwrap_or_else(|| "herdr unreachable".into())}
                    " · {s.runs.len()} runs"
                }
            }
            Keymap { bindings: default_keymap(), on_action }
            ContextMenu { on_command: cmd }
            Tooltips { enabled: settings()["tooltips_on"].as_bool().unwrap_or(true) }
            if palette_open() {
                Palette {
                    items: palette_items(&ws.read()),
                    on_run: on_action,
                    on_close: move |()| palette_open.set(false),
                }
            }
            div { class: "deck",
                Areas {
                    layout: ws.read().current().layout.clone(),
                    kinds: kinds(),
                    render,
                    on_command: cmd,
                    maximized: maximized(),
                }
            }
            StatusBar {
                hints: status_hints(),
                right: format!(
                    "{} · {} runs",
                    s.herdr.clone().unwrap_or_else(|| "herdr unreachable".into()),
                    s.runs.len()
                ),
            }
        }
    }
}

/// The key hints the status bar keeps in view — the chords worth knowing, in
/// grammar form (the bar's shim renders `Mod` as the platform glyph). Global
/// only for now; area-scoped hints arrive with regions.
fn status_hints() -> Vec<(String, String)> {
    vec![
        ("Mod+Z".into(), "Undo".into()),
        ("Mod+Shift+Z".into(), "Redo".into()),
        ("F3".into(), "Commands".into()),
        ("Mod+Shift+Space".into(), "Maximize".into()),
        ("Alt+PageDown".into(), "Next workspace".into()),
    ]
}

// --- editors --------------------------------------------------------------
// Each one is a body the library mounts under an area header. They read the
// polled snapshot and call the same daemon functions the old page did; the
// workbench changed where they sit, not what they are.

fn ed_actions(s: &State) -> Element {
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

/// The Info log: every command that ran, newest first — Blender's Info editor.
/// The workbench's own audit trail, and the more useful now that an agent
/// drives the same command bus: this is where you see what it did.
fn ed_info(s: &State) -> Element {
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

fn ed_runs(s: &State, area: AreaId, open_run: Callback<(AreaId, String)>) -> Element {
    rsx! {
        if s.runs.is_empty() {
            div { class: "empty", "No runs yet — trigger one from an Actions area." }
        }
        for r in s.runs.iter().cloned() {
            {run_row(r, area, open_run)}
        }
    }
}

/// One expandable run. Pulled out of `ed_runs` so the view tree stays shallow
/// — a nine-deep rsx block reads no better than a nine-deep function.
fn run_row(r: RunView, area: AreaId, open_run: Callback<(AreaId, String)>) -> Element {
    let open_id = r.id.clone();
    rsx! {
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

/// A single run, live: header, note/error, every recorded step. This is the
/// area you pin beside the fleet and CPU charts to watch a run work. It reads
/// the same polled snapshot as the list, filtered to one id, so it updates on
/// the same tick with no extra plumbing.
fn ed_run_detail(s: &State, id: &str) -> Element {
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
fn ed_run_picker(s: &State, area: AreaId, open_run: Callback<(AreaId, String)>) -> Element {
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
fn ed_settings(
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
