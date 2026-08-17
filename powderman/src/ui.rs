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
use serde_json::{Value, json};

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
    /// Who ran it — "ui" for a click or a chord, "agent" for the MCP server.
    /// The bus is deliberately one path, so the log is the only place the two
    /// are distinguishable, and it is worth being able to tell.
    #[serde(default)]
    pub source: String,
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

pub(crate) fn short(s: &str, n: usize) -> String {
    if s.chars().count() > n {
        format!("{}…", s.chars().take(n).collect::<String>())
    } else {
        s.to_string()
    }
}

pub(crate) fn hhmmss(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|t| {
            t.with_timezone(&chrono::Local)
                .format("%H:%M:%S")
                .to_string()
        })
        .unwrap_or_default()
}

pub(crate) fn gib(bytes: f64) -> String {
    format!("{:.1}G", bytes / 1_073_741_824.0)
}

/// A line chart with the run windows shaded behind it.
///
/// The shading is the point. A CPU line alone says the box was busy, which
/// htop already tells you; the bands say *which run* was alive at the time.
/// Each band carries an SVG <title>, so identity is never colour alone and the
/// tooltip costs no round trip — a hover handler under liveview would be a
/// websocket message per mouse move.
pub(crate) fn chart(
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
pub(crate) fn tile(k: &str, v: String, of: Option<String>) -> Element {
    rsx! {
        div { class: "tile",
            div { class: "k", "{k}" }
            div { class: "v", "{v}" }
            if let Some(o) = of { div { class: "of", "{o}" } }
        }
    }
}

use immersion::{
    AreaId, Areas, Chrome, ContextMenu, Dir, EditorKind, Field, FieldKind, Keymap, KeymapHelp,
    Layout, LayoutFile, MenuItem, Palette, PaletteItem, Panel, Platform, Splash, SplashRecent,
    StatusBar, Template, WorkspaceTabs, default_keymap, menu_json, pretty_chord,
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
/// A host mutation: changes server truth, but needs state the bus signature
/// cannot reach (the undo stacks, the settings document). The parity contract
/// for these is that each is attributed in the Info log and reachable by an
/// agent — `mcp::parity` counts the tools, so a new variant here fails CI
/// until it has one.
///
/// An enum rather than a list of strings so the router's match is exhaustive:
/// adding a variant does not compile until every surface handles it. That is
/// the difference between a convention and an invariant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HostAction {
    Undo,
    Redo,
    RepeatLast,
    SetSetting,
    FavoriteAdd,
    LoadLayout,
}

impl HostAction {
    const ALL: &'static [HostAction] = &[
        HostAction::Undo,
        HostAction::Redo,
        HostAction::RepeatLast,
        HostAction::SetSetting,
        HostAction::FavoriteAdd,
        HostAction::LoadLayout,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            HostAction::Undo => "undo",
            HostAction::Redo => "redo",
            HostAction::RepeatLast => "repeat_last",
            HostAction::SetSetting => "set_setting",
            HostAction::FavoriteAdd => "favorite_add",
            HostAction::LoadLayout => "load_layout",
        }
    }
}

/// Per-client view state — deliberately NOT server commands. Two browsers may
/// maximize different areas; opening the palette in one must not open it in
/// another. An agent has no viewport, so the absence of these from MCP is by
/// design, and `mcp::parity` asserts they stay absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClientAction {
    Maximize,
    Fullscreen,
    Palette,
    Cheatsheet,
    AdjustLast,
    Pie,
    Favorites,
    Noop,
}

impl ClientAction {
    const ALL: &'static [ClientAction] = &[
        ClientAction::Maximize,
        ClientAction::Fullscreen,
        ClientAction::Palette,
        ClientAction::Cheatsheet,
        ClientAction::AdjustLast,
        ClientAction::Pie,
        ClientAction::Favorites,
        ClientAction::Noop,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            ClientAction::Maximize => "maximize",
            ClientAction::Fullscreen => "fullscreen",
            ClientAction::Palette => "palette",
            ClientAction::Cheatsheet => "cheatsheet",
            ClientAction::AdjustLast => "adjust_last",
            ClientAction::Pie => "pie",
            ClientAction::Favorites => "favorites",
            ClientAction::Noop => "noop",
        }
    }
}

/// Where an action goes. One resolution function, used by the router that
/// executes actions AND by the test that proves every surface emits something
/// resolvable — so the two cannot disagree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Route {
    /// A named command on the bus.
    Bus,
    Host(HostAction),
    ClientView(ClientAction),
}

pub(crate) fn route(action: &str) -> Route {
    if let Some(h) = HostAction::ALL.iter().find(|h| h.name() == action) {
        return Route::Host(*h);
    }
    if let Some(c) = ClientAction::ALL.iter().find(|c| c.name() == action) {
        return Route::ClientView(*c);
    }
    Route::Bus
}

/// The names, derived from the enums rather than repeated beside them: a list
/// written twice is a list that drifts. Only the parity tests enumerate them —
/// the running code matches on the enums.
#[cfg(test)]
pub(crate) fn host_actions() -> Vec<&'static str> {
    HostAction::ALL.iter().map(|h| h.name()).collect()
}

#[cfg(test)]
pub(crate) fn client_view_actions() -> Vec<&'static str> {
    ClientAction::ALL.iter().map(|c| c.name()).collect()
}

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
pub(crate) fn settings_fields() -> Vec<Field> {
    vec![
        Field::new("/accent", "Accent color", FieldKind::Color)
            .with_hint("the widget-blue used across the workbench")
            .with_default(serde_json::json!("#5680c2")),
        Field::new(
            "/poll_ms",
            "Refresh interval",
            FieldKind::Slider {
                min: 250.0,
                max: 5000.0,
                step: 250.0,
            },
        )
        .with_hint("how often the page repolls, in ms")
        .with_default(serde_json::json!(1000)),
        Field::new("/splash_on_start", "Splash on startup", FieldKind::Bool)
            .with_default(serde_json::json!(true)),
        Field::new("/tooltips_on", "Tooltips", FieldKind::Toggle)
            .with_hint("hover help on the workbench controls")
            .with_default(serde_json::json!(true)),
        Field::new(
            "/sweep_limit",
            "Default sweep limit",
            FieldKind::Number {
                min: Some(1.0),
                max: Some(1000.0),
                step: Some(50.0),
            },
        )
        .with_hint("packages per ecosystem the daily sweep fetches")
        .with_default(serde_json::json!(100)),
        Field::new(
            "/theme",
            "Theme",
            FieldKind::Select(
                immersion::themes()
                    .iter()
                    .map(|t| (t.name.to_string(), t.name.to_string()))
                    .collect(),
            ),
        )
        .with_hint("the workbench palette; accent stays your own"),
        Field::new(
            "/chart_window",
            "Chart window",
            FieldKind::Vector {
                labels: vec!["H".into(), "N".into(), "S".into()],
                step: Some(1.0),
            },
        )
        .with_hint("hours shown, samples, smoothing")
        .with_default(serde_json::json!([1, 60, 3])),
        Field::new("/diff_split", "Split diffs", FieldKind::Toggle)
            .with_hint("show diffs side by side rather than stacked")
            .with_default(serde_json::json!(false)),
        Field::new(
            "/ui_scale",
            "Resolution scale",
            FieldKind::Slider {
                min: 0.8,
                max: 1.6,
                step: 0.05,
            },
        )
        .with_hint("size of the whole interface")
        .with_default(serde_json::json!(1.0)),
        Field::new(
            "/density",
            "Density",
            FieldKind::Radio(vec![
                ("cozy".into(), "Cozy".into()),
                ("compact".into(), "Compact".into()),
            ]),
        )
        .with_default(serde_json::json!("cozy")),
    ]
}

fn kinds() -> Vec<EditorKind> {
    vec![
        EditorKind {
            id: "machine",
            label: "Machine",
            targets: false,
        },
        EditorKind {
            id: "fleet",
            label: "Fleet",
            targets: false,
        },
        EditorKind {
            id: "runs",
            label: "Runs",
            targets: false,
        },
        EditorKind {
            id: "actions",
            label: "Actions",
            targets: false,
        },
        EditorKind {
            id: "timers",
            label: "Timers",
            targets: false,
        },
        EditorKind {
            id: "run",
            label: "Run detail",
            targets: true,
        },
        EditorKind {
            id: "settings",
            label: "Settings",
            targets: false,
        },
        EditorKind {
            id: "info",
            label: "Info log",
            targets: false,
        },
        EditorKind {
            id: "keymap",
            label: "Keymap",
            targets: false,
        },
        EditorKind {
            id: "data",
            label: "Data",
            targets: true,
        },
        EditorKind {
            id: "files",
            label: "Files",
            targets: true,
        },
        EditorKind {
            id: "code",
            label: "Code",
            targets: true,
        },
        EditorKind {
            id: "diff",
            label: "Diff",
            targets: true,
        },
        EditorKind {
            id: "chart",
            label: "Chart",
            targets: true,
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
    // A transient status report (Blender names the operation you just ran in
    // the status bar). Cleared after a few seconds by a token: each report
    // bumps a generation, and the timer only clears if its generation is still
    // current, so a newer report is never wiped by an older timer.
    // The client reports its platform once; chords are written server-side
    // from it, so a rebind updates everywhere without touching the DOM.
    let mut mac = use_signal(|| false);
    let mut help_open = use_signal(|| false);
    // Which binding is waiting for a chord, while the keymap editor captures.
    let mut capturing = use_signal(|| None::<String>);
    // Adjust Last (F9): re-run the last command with edited params.
    let mut adjust_open = use_signal(|| false);
    let mut adjust_name = use_signal(String::new);
    let mut adjust_doc = use_signal(|| serde_json::Value::Null);
    let adj_edit = use_callback(move |(ptr, val): (String, serde_json::Value)| {
        let mut d = adjust_doc();
        immersion::apply_edit(&mut d, &ptr, val);
        adjust_doc.set(d);
    });
    let adj_cancel = use_callback(move |()| adjust_open.set(false));
    let adj_apply = use_callback(move |()| {
        // Replace the last operation with the edited one: revert it, then re-run.
        ws.set(crate::daemon::undo("ui"));
        ws.set(crate::daemon::dispatch(&adjust_name(), adjust_doc()));
        adjust_open.set(false);
    });
    let mut report = use_signal(|| None::<String>);
    let mut report_gen = use_signal(|| 0u64);
    let do_report = use_callback(move |msg: String| {
        report.set(Some(msg));
        let g = report_gen() + 1;
        report_gen.set(g);
        spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            if report_gen() == g {
                report.set(None);
            }
        });
    });

    // The second error surface: a field flags itself locally, and the same
    // message lands here, Blender's Info-line echo.
    let on_editor_error = use_callback(move |e: immersion::EditorError| {
        do_report.call(format!("\u{26a0} {e}"));
    });

    let cmd = use_callback(move |(name, params): (String, serde_json::Value)| {
        let label = report_label(&name, &params);
        // The checked path: a command that fails must say so, not report the
        // success label over a workbench that did not change.
        match crate::daemon::dispatch_checked(&name, params) {
            Ok(w) => {
                ws.set(w);
                do_report.call(label);
            }
            Err(e) => do_report.call(format!("\u{26a0} {label}: {e}")),
        }
    });

    let on_setting = use_callback(move |(pointer, value): (String, serde_json::Value)| {
        settings.set(crate::daemon::set_setting("ui", &pointer, value));
    });

    // The command palette is per-client view state, like maximize — one client
    // searching commands does not open the palette in another.
    let mut palette_open = use_signal(|| false);
    // Which area is being retargeted, if any. Per-client like the palette: two
    // people picking targets should not fight over one modal.
    let mut picking_for = use_signal(|| None::<AreaId>);

    // Maximize is per-client view state (two browsers may maximize different
    // areas), so it is a local signal, not a command on the shared tree.
    let mut maximized = use_signal(|| None::<AreaId>);
    // Fullscreen is distraction-free mode: hide all chrome (topbar, headers,
    // status bar) and show a single area. Per-client, like maximize.
    let mut fullscreen = use_signal(|| false);

    // Keymap actions. Layout commands go to the bus; undo/redo/maximize are
    // host concerns the bus does not own.
    let on_action = use_callback(move |(action, params): (String, serde_json::Value)| {
        // Exhaustive over the enums: a new host or client action does not
        // compile until it is handled here.
        match route(&action) {
            Route::Host(HostAction::Undo) => ws.set(crate::daemon::undo("ui")),
            Route::Host(HostAction::Redo) => ws.set(crate::daemon::redo("ui")),
            Route::Host(HostAction::RepeatLast) => ws.set(crate::daemon::repeat_last("ui")),
            Route::Host(HostAction::SetSetting) => {
                let pointer = params.get("pointer").and_then(|p| p.as_str()).unwrap_or("");
                let value = params
                    .get("value")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                settings.set(crate::daemon::set_setting("ui", pointer, value));
            }
            Route::Host(HostAction::FavoriteAdd) => {
                // The dedup/cap logic lives in the daemon so an agent can
                // curate favourites too; the UI keeps only the report.
                let label = params.get("label").and_then(|l| l.as_str()).unwrap_or("");
                let (doc, added) = crate::daemon::favorite_add("ui", params.clone());
                settings.set(doc);
                if added {
                    do_report.call(format!("Added to favourites: {label}"));
                }
            }
            // The layout-file shim owns this one: it arrives as file text on
            // that component's own channel, not as an action with params.
            Route::Host(HostAction::LoadLayout) => {}
            Route::ClientView(ClientAction::Noop) => {}
            Route::ClientView(ClientAction::Cheatsheet) => help_open.toggle(),
            Route::ClientView(ClientAction::Pie) => {
                let js = format!(
                    "window.__imOpenPie && window.__imOpenPie({});",
                    serde_json::to_string(&pie_menu_json()).unwrap_or_default()
                );
                dioxus::document::eval(&js);
            }
            Route::ClientView(ClientAction::Favorites) => {
                // Hand the list to the menu shim, which raises it at the
                // pointer and sends the pick back over its own channel.
                let items = favorites_menu_json(&settings());
                let js = format!(
                    "window.__imOpenMenu && window.__imOpenMenu({});",
                    serde_json::to_string(&items).unwrap_or_default()
                );
                dioxus::document::eval(&js);
            }
            Route::ClientView(ClientAction::AdjustLast) => {
                if let Some((name, params)) = crate::daemon::last_command() {
                    adjust_name.set(name);
                    adjust_doc.set(params);
                    adjust_open.set(true);
                }
            }
            Route::ClientView(ClientAction::Fullscreen) => fullscreen.toggle(),
            Route::ClientView(ClientAction::Maximize) => {
                // Toggle: maximize the first area if none is, else restore.
                let cur = maximized();
                let first = ws.read().current().layout.root.leaves().first().copied();
                maximized.set(if cur.is_some() { None } else { first });
            }
            Route::ClientView(ClientAction::Palette) => palette_open.set(true),
            Route::Bus => cmd.call((action, params)),
        }
    });

    // A menu pick is just an action: a settings edit, a host mutation, a
    // client-view toggle, or a bus command. One resolution decides which, so
    // a menu item can be any of them without a new arm here.
    let on_menu = use_callback(move |(action, params): (String, serde_json::Value)| {
        on_action.call((action, params));
    });

    // Start capturing a chord for a binding: arm the shim, remember which
    // action is waiting. The Keymap component reports the chord back.
    let cap_start = use_callback(move |action: String| {
        capturing.set(Some(action));
        dioxus::document::eval("window.__imCaptureChord && window.__imCaptureChord();");
    });
    let cap_reset = use_callback(move |action: String| {
        let mut km = settings()["keymap"]
            .as_object()
            .cloned()
            .unwrap_or_default();
        km.remove(&action);
        settings.set(crate::daemon::set_setting(
            "ui",
            "/keymap",
            serde_json::Value::Object(km),
        ));
    });

    let on_pick_target = use_callback(move |id: AreaId| picking_for.set(Some(id)));
    let pick_target = use_callback(move |(id, target): (AreaId, String)| {
        // Through the bus like everything else, so an agent retargets an area
        // the same way the picker does.
        cmd.call((
            "set_target".to_string(),
            serde_json::json!({ "id": id, "target": target }),
        ));
        picking_for.set(None);
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
            "ui",
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

    // Region content (Blender's T toolbar / N sidebar). The toolbar is a
    // vertical strip of area actions; the sidebar is a small properties panel
    // for the focused area. Both route through the same command bus.
    let render_toolbar = use_callback(move |(id, _editor): (AreaId, String)| -> Element {
        rsx! {
            div { class: "area-tools",
                button {
                    class: "area-tool",
                    title: "split horizontal",
                    onclick: move |_| cmd.call(("split".to_string(), serde_json::json!({ "id": id, "dir": "row" }))),
                    "⬒"
                }
                button {
                    class: "area-tool",
                    title: "split vertical",
                    onclick: move |_| cmd.call(("split".to_string(), serde_json::json!({ "id": id, "dir": "col" }))),
                    "◧"
                }
                button {
                    class: "area-tool",
                    title: "duplicate",
                    onclick: move |_| cmd.call(("duplicate_area".to_string(), serde_json::json!({ "id": id }))),
                    "⧉"
                }
            }
        }
    });
    let render_sidebar = use_callback(move |(id, editor): (AreaId, String)| -> Element {
        // The chart editor's sidebar is its spec editor, not the generic
        // properties panel: what you want beside a chart is the document that
        // makes it.
        if editor == "chart" {
            let target = ws.read().current().layout.target_of(id);
            return crate::editors::chart_sidebar(
                &settings.read().clone(),
                target,
                on_setting,
                on_editor_error,
            );
        }
        rsx! {
            div { class: "area-props",
                Panel { title: "Properties",
                    div { class: "area-props-row",
                        span { class: "k", "Editor" }
                        span { "{editor}" }
                    }
                    div { class: "area-props-row",
                        span { class: "k", "Area" }
                        span { "{id}" }
                    }
                }
                Panel { title: "Layout", open: false,
                    div { class: "area-props-row",
                        span { class: "k", "Regions" }
                        span { "T · N" }
                    }
                }
            }
        }
    });

    let render_state = state;
    let render = use_callback(
        move |(area, editor, arg): (AreaId, String, Option<String>)| {
            let s = render_state.read().clone();
            let settings_doc = settings.read().clone();
            match editor.as_str() {
                "machine" => crate::editors::ed_machine(&s),
                "fleet" => crate::editors::ed_fleet(&s),
                "runs" => crate::editors::ed_runs(&s, area, open_run),
                "actions" => crate::editors::ed_actions(&s),
                "timers" => crate::editors::ed_timers(&s),
                "info" => crate::editors::ed_info(&s),
                "keymap" => crate::editors::ed_keymap(
                    settings_doc.clone(),
                    mac(),
                    capturing(),
                    cap_start,
                    cap_reset,
                ),
                "run" => match arg {
                    Some(id) => crate::editors::ed_run_detail(&s, &id),
                    None => crate::editors::ed_run_picker(&s, area, open_run),
                },
                "settings" => {
                    crate::editors::ed_settings(settings_doc.clone(), on_setting, on_editor_error)
                }
                "data" => crate::editors::ed_data(&s, arg.clone()),
                "files" => crate::editors::ed_files(arg.clone()),
                "code" => crate::editors::ed_code(arg.clone()),
                "chart" => crate::editors::ed_chart(&s, &settings_doc, arg.clone()),
                "diff" => crate::editors::ed_diff(
                    arg.clone(),
                    settings_doc["diff_split"].as_bool().unwrap_or(false),
                ),
                other => rsx! { div { class: "empty", "unknown editor {other}" } },
            }
        },
    );

    rsx! {
        // Blender's Resolution Scale: the library sizes everything in rem, so
        // the whole interface scales from the root font size.
        style { "html {{ font-size: {(settings()[\"ui_scale\"].as_f64().unwrap_or(1.0)).clamp(0.8, 1.6) * 100.0}%; }}" }
        style { "{immersion::CSS}" }
        style { "{immersion::theme_css(settings()[\"theme\"].as_str().unwrap_or(\"Blender Dark\"))}" }
        style { "{CSS}" }
        div {
            class: if fullscreen() { "app im-fullscreen" } else { "app" },
            style: "--im-accent: {settings()[\"accent\"].as_str().unwrap_or(\"#5680c2\")}; --accent-live: {settings()[\"accent\"].as_str().unwrap_or(\"#5680c2\")}; font-size: {(settings()[\"ui_scale\"].as_f64().unwrap_or(1.0)).clamp(0.8, 1.6) * 100.0}%",
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
                span { class: "menubar",
                    button { class: "im-menubtn", "data-im-menu-click": "{window_menu(ws.read().active, mac())}", "Window" }
                    button { class: "im-menubtn", "data-im-menu-click": "{edit_menu(mac())}", "Edit" }
                    button { class: "im-menubtn", "data-im-menu-click": "{help_menu(mac())}", "Help" }
                }
                WorkspaceTabs {
                    names: ws.read().tabs.iter().map(|t| t.name.clone()).collect::<Vec<_>>(),
                    active: ws.read().active,
                    on_command: cmd,
                    on_add: ws_add,
                }
                LayoutFile {
                    layout_json: serde_json::to_string(&ws.read().clone()).unwrap_or_default(),
                    on_import: move |json: String| {
                        ws.set(crate::daemon::set_workspaces_from_json("ui", &json));
                    },
                }
                span { class: "sub",
                    {s.herdr.clone().unwrap_or_else(|| "herdr unreachable".into())}
                    " · {s.runs.len()} runs"
                }
            }
            Platform { on_platform: move |m: bool| mac.set(m) }
            Keymap {
                bindings: effective_keymap(&settings()),
                on_action,
                on_capture: move |chord: String| {
                    if let Some(action) = capturing() {
                        settings.set(crate::daemon::set_setting("ui",
                            &format!("/keymap/{action}"),
                            serde_json::json!(chord),
                        ));
                        capturing.set(None);
                    }
                },
            }
            ContextMenu { on_command: on_menu }
            if help_open() {
                KeymapHelp {
                    bindings: effective_keymap(&settings()),
                    mac: mac(),
                    on_close: move |()| help_open.set(false),
                }
            }
            if let Some(id) = picking_for() {
                crate::panels::TargetPicker {
                    area: id,
                    current: ws.read().current().layout.target_of(id),
                    editor: ws
                        .read()
                        .current()
                        .layout
                        .root
                        .find(id)
                        .and_then(|a| match a {
                            immersion::Area::Leaf { editor, .. } => Some(editor.clone()),
                            _ => None,
                        })
                        .unwrap_or_default(),
                    state: s.clone(),
                    on_pick: pick_target,
                    on_cancel: use_callback(move |()| picking_for.set(None)),
                }
            }
            if adjust_open() {
                crate::panels::AdjustPanel {
                    name: adjust_name(),
                    doc: adjust_doc(),
                    on_edit: adj_edit,
                    on_error: on_editor_error,
                    on_cancel: adj_cancel,
                    on_apply: adj_apply,
                }
            }
            Chrome { tooltips_enabled: settings()["tooltips_on"].as_bool().unwrap_or(true) }
            if fullscreen() {
                button {
                    class: "fullscreen-exit",
                    title: "exit fullscreen",
                    onclick: move |_| fullscreen.set(false),
                    "⤢ exit"
                }
            }
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
                    render_toolbar: Some(render_toolbar),
                    render_sidebar: Some(render_sidebar),
                    on_command: cmd,
                    on_pick_target,
                    maximized: maximized().or_else(|| {
                        if fullscreen() {
                            ws.read().current().layout.root.leaves().first().copied()
                        } else {
                            None
                        }
                    }),
                    revision: settings_revision(&settings()),
                    chords: chord_map(&settings(), mac()),
                }
            }
            StatusBar {
                hints: status_hints(mac()),
                message: report(),
                right: format!(
                    "{} · {} runs",
                    s.herdr.clone().unwrap_or_else(|| "herdr unreachable".into()),
                    s.runs.len()
                ),
                badge: crate::demo::enabled().then(|| "demo".to_string()),
            }
        }
    }
}

/// Turn a command'"'"'s params into an editable form: a field per key, its widget
/// chosen by the value's type. Adjust Last renders this over the last command's
/// params so you can re-run it with tweaks.
pub(crate) fn fields_from_params(params: &serde_json::Value) -> Vec<Field> {
    let mut fields = Vec::new();
    if let Some(obj) = params.as_object() {
        for (k, v) in obj {
            let kind = if v.is_number() {
                FieldKind::Number {
                    min: None,
                    max: None,
                    step: None,
                }
            } else if v.is_boolean() {
                FieldKind::Bool
            } else {
                FieldKind::Text
            };
            fields.push(Field::new(&format!("/{k}"), k, kind));
        }
    }
    fields
}

/// A short human label for a command, for the status-bar report — Blender's
/// "Split Area" line. Falls back to the raw name for anything unmapped.
fn report_label(name: &str, params: &serde_json::Value) -> String {
    match name {
        "split" => {
            let dir = params.get("dir").and_then(|d| d.as_str()).unwrap_or("");
            format!(
                "Split area {}",
                if dir == "col" {
                    "vertical"
                } else {
                    "horizontal"
                }
            )
        }
        "join" | "join_into" => "Close area".to_string(),
        "swap" => "Swap areas".to_string(),
        "ratio" => "Resize".to_string(),
        "set_editor" | "open_editor" => "Change editor".to_string(),
        "open_run" => "Open run".to_string(),
        n if n.starts_with("workspace.") => "Workspace".to_string(),
        other => other.to_string(),
    }
}

/// `action -> chord`, written the platform's way, for the chrome to show in
/// tooltips. Built from the effective keymap so a rebind moves the hint too.
fn chord_map(settings: &serde_json::Value, mac: bool) -> std::collections::HashMap<String, String> {
    effective_keymap(settings)
        .into_iter()
        .map(|b| (b.action, pretty_chord(&b.chord, mac)))
        .collect()
}

/// A cheap version of the settings document, so the deck re-renders when a
/// setting an editor shows has changed (the layout alone does not encode it).
fn settings_revision(settings: &serde_json::Value) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    settings.to_string().hash(&mut h);
    h.finish()
}

/// The keymap as it actually stands: the shipped defaults with the user's
/// rebinds applied. An override is stored per action, so a rebind survives a
/// change to the default and the editor can show what is customised.
pub(crate) fn effective_keymap(settings: &serde_json::Value) -> Vec<immersion::Binding> {
    let overrides = &settings["keymap"];
    default_keymap()
        .into_iter()
        .map(|mut b| {
            if let Some(chord) = overrides.get(&b.action).and_then(|c| c.as_str()) {
                b.chord = chord.to_string();
            }
            b
        })
        .collect()
}

/// The area pie (backquote): the operations worth reaching by muscle memory,
/// laid out radially. "@area" is resolved by the shim to whichever area the
/// The area pie (backquote): the operations worth reaching by muscle memory,
/// laid out radially. "@area" is resolved by the shim to whichever area the
/// pointer is over, so one definition serves every area.
fn pie_menu_json() -> String {
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
fn favorites_menu_json(settings: &serde_json::Value) -> String {
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
fn window_menu(active: usize, mac: bool) -> String {
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

fn edit_menu(mac: bool) -> String {
    menu_json(&[
        MenuItem::new("Undo", "undo", Value::Null).with_chord(&pretty_chord("Mod+Z", mac)),
        MenuItem::new("Redo", "redo", Value::Null).with_chord(&pretty_chord("Mod+Shift+Z", mac)),
        MenuItem::sep(),
        MenuItem::new("Repeat last", "repeat_last", Value::Null)
            .with_chord(&pretty_chord("Shift+R", mac)),
        MenuItem::new("Adjust last operation", "adjust_last", Value::Null).with_chord("F9"),
    ])
}

fn help_menu(_mac: bool) -> String {
    menu_json(&[
        MenuItem::new("Command palette", "palette", Value::Null).with_chord("F3"),
        MenuItem::new("Keyboard shortcuts", "cheatsheet", Value::Null).with_chord("F1"),
    ])
}

/// The key hints the status bar keeps in view — the chords worth knowing, in
/// grammar form (the bar's shim renders `Mod` as the platform glyph). Global
/// only for now; area-scoped hints arrive with regions.
fn status_hints(mac: bool) -> Vec<(String, String)> {
    [
        ("Mod+Z", "Undo"),
        ("Mod+Shift+Z", "Redo"),
        ("F3", "Commands"),
        ("Mod+Shift+Space", "Maximize"),
        ("Alt+PageDown", "Next workspace"),
    ]
    .into_iter()
    .map(|(c, l)| (pretty_chord(c, mac), l.to_string()))
    .collect()
}

// --- editors --------------------------------------------------------------
// Each one is a body the library mounts under an area header. They read the
// polled snapshot and call the same daemon functions the old page did; the
// workbench changed where they sit, not what they are.

// No server functions: under liveview the component *is* server-side, so it
// reads the database the daemon already owns. The browser holds nothing but a
// websocket and a DOM.
async fn fetch_state() -> anyhow::Result<State> {
    Ok(crate::daemon::snapshot())
}

pub(crate) async fn run_with(name: String, input: String) -> anyhow::Result<()> {
    let value = if input.trim().is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_str(&input)?
    };
    crate::daemon::trigger_with(&name, value)
}

pub(crate) async fn resume(id: String) -> anyhow::Result<()> {
    crate::daemon::resume(&id)
}

#[cfg(test)]
mod keymap_tests {
    use super::*;

    #[test]
    fn an_override_replaces_the_default_chord() {
        let settings = serde_json::json!({ "keymap": { "favorites": "Mod+Shift+K" } });
        let km = effective_keymap(&settings);
        let fav = km
            .iter()
            .find(|b| b.action == "favorites")
            .expect("favorites binding");
        assert_eq!(fav.chord, "Mod+Shift+K");
        // untouched bindings keep their defaults
        let undo = km
            .iter()
            .find(|b| b.action == "undo")
            .expect("undo binding");
        assert_eq!(undo.chord, "Mod+Z");
    }
}

#[cfg(test)]
mod parity_tests {
    use super::*;

    /// Pull every "action" out of a menu-JSON string. Menus are the JSON the
    /// shim receives, so parsing them exercises exactly what a click emits.
    fn menu_actions(json: &str) -> Vec<String> {
        serde_json::from_str::<Vec<serde_json::Value>>(json)
            .expect("menu JSON parses")
            .into_iter()
            .filter_map(|i| i.get("action")?.as_str().map(str::to_string))
            .collect()
    }

    /// The parity invariant: every action any UI surface can emit — a default
    /// binding, a palette row, a menu item, a pie slice — resolves to a bus
    /// command, a host action, or a declared client-view action. A name in
    /// none of the three is a control that silently does nothing; this is the
    /// test that turns "one write path" from a convention into CI.
    #[test]
    fn every_ui_action_resolves() {
        let commands = crate::workflows::commands();
        let ws = immersion::Workspaces::new("test", Layout::single("runs"));
        // Settings with one favourite, so the favourites surface is exercised
        // rather than skipped on an empty default.
        let settings = serde_json::json!({
            "favorites": [{ "label": "Split", "action": "split",
                            "params": { "id": 1, "dir": "row" } }]
        });

        let mut actions: Vec<(String, String)> = Vec::new(); // (surface, action)
        for b in immersion::default_keymap() {
            actions.push(("keymap".into(), b.action));
        }
        for p in palette_items(&ws) {
            actions.push(("palette".into(), p.action));
        }
        for a in menu_actions(&window_menu(0, false)) {
            actions.push(("window menu".into(), a));
        }
        for a in menu_actions(&edit_menu(false)) {
            actions.push(("edit menu".into(), a));
        }
        for a in menu_actions(&help_menu(false)) {
            actions.push(("help menu".into(), a));
        }
        for a in menu_actions(&pie_menu_json()) {
            actions.push(("pie".into(), a));
        }
        for a in menu_actions(&favorites_menu_json(&settings)) {
            actions.push(("favorites".into(), a));
        }
        let kinds = kinds();
        for a in menu_actions(&immersion::editor_menu_json(1, &kinds, "runs")) {
            actions.push(("editor menu".into(), a));
        }
        for a in menu_actions(&immersion::view_menu_json(1, true, true, true)) {
            actions.push(("view menu".into(), a));
        }
        for a in menu_actions(&immersion::area_menu_json(1)) {
            actions.push(("area menu".into(), a));
        }

        assert!(
            actions.len() > 30,
            "harvest looks broken: {}",
            actions.len()
        );
        let mut orphans = Vec::new();
        for (surface, action) in &actions {
            let known = commands.get(action).is_some() || !matches!(route(action), Route::Bus);
            if !known {
                orphans.push(format!("{surface}: {action}"));
            }
        }
        assert!(
            orphans.is_empty(),
            "actions with no resolution (bus, host, or client-view):\n  {}",
            orphans.join("\n  ")
        );
    }

    /// The complement: a host action that stops being routed is as broken as
    /// an unrouted menu item. The on_action / on_menu matches cannot be
    /// inspected directly, so this pins the contract they implement.
    #[test]
    fn host_and_client_lists_do_not_overlap_or_shadow_the_bus() {
        let commands = crate::workflows::commands();
        for a in host_actions() {
            assert!(
                !client_view_actions().contains(&a),
                "{a} is both a host action and client-view state"
            );
            assert!(
                commands.get(a).is_none(),
                "{a} is both a host action and a bus command — one must win"
            );
        }
        for a in client_view_actions() {
            assert!(
                commands.get(a).is_none(),
                "{a} is both client-view and a bus command — one must win"
            );
        }
    }
}
