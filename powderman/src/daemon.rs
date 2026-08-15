//! One process that owns the database, drives the clock, and serves the UI.
//!
//! Single writer by design: the tick and the UI both live here, so SQLite
//! never sees two writers and WAL is the whole of the concurrency story.
//!
//! The clock is a one-second tick doing two things — firing any schedule whose
//! cron expression matches this minute, and re-invoking any suspended run
//! whose deadline has passed. A run parked with no deadline stays parked until
//! a person or an event moves it.

use anyhow::{Result, anyhow};
use axum::{
    Router,
    extract::{Path, WebSocketUpgrade},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
};
use chrono::Local;
use rusqlite::params;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};

use crate::cron::{self, Cron};

/// Compiled schedules, by workflow name.
type Schedules = HashMap<&'static str, Cron>;
use crate::engine::{self, Db, Registry};
use crate::ui::{self, Annotation, RunView, State, StepView, WorkflowView};

/// Process-wide handles. The UI component is spawned by the liveview pool and
/// has no way to be handed state, so it reaches for these.
struct Shared {
    db: Db,
    registry: Registry,
    schedules: Schedules,
    herdr: Mutex<Option<String>>,
    /// The live fleet, refreshed on a timer rather than per page render:
    /// pane_procs is a socket round trip per agent and the UI polls every
    /// second.
    fleet: Mutex<Vec<crate::metrics::FleetAgent>>,
    /// The workbench: a set of named layout trees. Server truth, shared by
    /// every client, persisted like everything else.
    workspaces: Mutex<immersion::Workspaces>,
    /// The one write path for layout. Built once; fn-pointer commands, so it
    /// is cheap to hold and Send + Sync without a lock of its own.
    commands: immersion::Commands,
    /// Undo is a stack of past workbench values — the layout is one serde
    /// value, so a snapshot is the whole history entry, no diffing. Redo holds
    /// what undo popped, cleared the moment a new edit lands.
    undo: Mutex<Vec<immersion::Workspaces>>,
    redo: Mutex<Vec<immersion::Workspaces>>,
    timers: Mutex<Vec<crate::ui::TimerRow>>,
    in_flight: Mutex<HashSet<String>>,
    /// Every command that ran, newest last — Blender's Info log. Capped; it is
    /// a record to read, not state to replay.
    log: Mutex<Vec<crate::ui::LogEntry>>,
}

static SHARED: OnceLock<Arc<Shared>> = OnceLock::new();

fn shared() -> Arc<Shared> {
    SHARED.get().expect("daemon not started").clone()
}

/// Drive a run to its next parking point, at most once concurrently.
async fn drive(id: String) {
    let s = shared();
    {
        let mut f = s.in_flight.lock().expect("in_flight");
        if !f.insert(id.clone()) {
            return;
        }
    }
    let outcome = engine::run(&s.db, &s.registry, &id).await;
    s.in_flight.lock().expect("in_flight").remove(&id);
    match outcome {
        Ok(o) => println!(
            "run {} -> {}{}",
            &id[..8],
            o.status.as_str(),
            o.note.map(|n| format!(" ({n})")).unwrap_or_default()
        ),
        Err(e) => eprintln!("run {} threw outside the workflow: {e:?}", &id[..8]),
    }
}

/// The starter workbench: an "Overview" workspace with machine on top, runs
/// below-left, fleet below-right. Only used when the database has none yet.
fn default_workspaces() -> immersion::Workspaces {
    let mut l = immersion::Layout::single("machine");
    if let Some(bottom) = l.split(1, immersion::Dir::Col, 0.45) {
        l.set_editor(bottom, "runs");
        if let Some(right) = l.split(bottom, immersion::Dir::Row, 0.6) {
            l.set_editor(right, "fleet");
        }
    }
    immersion::Workspaces::new("Overview", l)
}

fn load_workspaces(conn: &rusqlite::Connection) -> immersion::Workspaces {
    conn.query_row("SELECT value FROM kv WHERE key = 'workspaces'", [], |r| {
        r.get::<_, String>(0)
    })
    .ok()
    .and_then(|json| serde_json::from_str(&json).ok())
    .unwrap_or_else(default_workspaces)
}

pub fn workspaces() -> immersion::Workspaces {
    shared().workspaces.lock().expect("workspaces").clone()
}

/// The workbench settings document — a small JSON value the widget-based
/// Settings editor edits by pointer. Defaults fill any key the stored doc is
/// missing, so adding a setting never needs a migration.
pub fn settings() -> serde_json::Value {
    let defaults = serde_json::json!({
        "accent": "#5680c2",
        "splash_on_start": true,
        "poll_ms": 1000,
        "sweep_limit": 100,
        "density": "cozy",
        "tooltips_on": true,
        "theme": "Blender Dark",
        "ui_scale": 1.0,
        // A vector setting: the chart window as [hours, samples, smoothing].
        "chart_window": [1, 60, 3],
        // Quick Favourites (Q). Seeded with a few useful ones; right-clicking
        // any menu row adds to this list.
        "favorites": [
            {"label": "Command palette", "action": "palette", "params": null},
            {"label": "Maximize area", "action": "maximize", "params": null},
            {"label": "Adjust last operation", "action": "adjust_last", "params": null}
        ]
    });
    let s = shared();
    let stored: serde_json::Value = {
        let conn = s.db.lock().expect("db");
        conn.query_row("SELECT value FROM kv WHERE key = 'settings'", [], |r| {
            r.get::<_, String>(0)
        })
        .ok()
        .and_then(|v| serde_json::from_str(&v).ok())
        .unwrap_or(serde_json::Value::Null)
    };
    let mut doc = defaults;
    if let (Some(d), Some(st)) = (doc.as_object_mut(), stored.as_object()) {
        for (k, v) in st {
            d.insert(k.clone(), v.clone());
        }
    }
    doc
}

/// Apply one widget edit to the settings document and persist. Not on the
/// layout undo stack — a preference is not something you undo with the same
/// Ctrl-Z that reverts a split.
pub fn set_setting(pointer: &str, value: serde_json::Value) -> serde_json::Value {
    let mut doc = settings();
    immersion::apply_edit(&mut doc, pointer, value);
    let s = shared();
    if let Ok(json) = serde_json::to_string(&doc) {
        let conn = s.db.lock().expect("db");
        let _ = conn.execute(
            "INSERT INTO kv (key, value) VALUES ('settings', ?1)
             ON CONFLICT(key) DO UPDATE SET value = ?1",
            rusqlite::params![json],
        );
    }
    doc
}

/// Run a layout command and hand back the new workbench. THE one write path:
/// every UI mutation — button, dropdown, gesture, tab — arrives here as a
/// named command, is applied to the workspace value, and persisted. The
/// write-through is the point: a workbench that lived only in memory would
/// reset on every deploy, the failure the boot-id reload would otherwise cause
/// daily.
pub fn dispatch(name: &str, params: serde_json::Value) -> immersion::Workspaces {
    // The UI path: a bad command from a button or gesture is a bug in our own
    // wiring, so log it and leave the workbench unchanged. The agent path wants
    // the error instead — see `dispatch_checked`.
    match dispatch_checked(name, params) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("command {name} failed: {e}");
            workspaces()
        }
    }
}

/// Run a command and hand back the new workbench, or the command's error. The
/// error-returning core of [`dispatch`]: the agent route surfaces a bad name or
/// bad params to the caller rather than swallowing it. Atomic — the command
/// runs against a clone, so a failure leaves the live workspace untouched (no
/// half-applied split), and undo is recorded only on success.
pub fn dispatch_checked(name: &str, params: serde_json::Value) -> Result<immersion::Workspaces> {
    let s = shared();
    let mut w = s.workspaces.lock().expect("workspaces");
    let mut candidate = w.clone();
    let outcome = s.commands.run(&mut candidate, name, &params);
    // The Info log: every command through the one write path — UI and MCP
    // alike — with whether it took. Newest last, capped.
    {
        let mut log = s.log.lock().expect("log");
        log.push(crate::ui::LogEntry {
            name: name.to_string(),
            params: params.clone(),
            at: engine::now_ms(),
            ok: outcome.is_ok(),
        });
        let len = log.len();
        if len > 200 {
            log.drain(0..len - 200);
        }
    }
    outcome?; // logged either way; surface the error after recording it
    // Success. Record for undo — but not for pure navigation (switching a tab
    // is not something you undo) — capping depth so history is bounded.
    if s.commands.records_undo(name) {
        let mut u = s.undo.lock().expect("undo");
        u.push(w.clone());
        if u.len() > 100 {
            u.remove(0);
        }
        s.redo.lock().expect("redo").clear();
    }
    *w = candidate;
    persist_workspaces(&s, &w);
    Ok(w.clone())
}

/// The Info log — every command that ran, newest last.
pub fn command_log() -> Vec<crate::ui::LogEntry> {
    shared().log.lock().expect("log").clone()
}

/// The most recent command that changed the layout — name and params — for
/// Adjust Last to re-run with edits. Same filter as repeat_last.
pub fn last_command() -> Option<(String, serde_json::Value)> {
    let s = shared();
    let log = s.log.lock().expect("log");
    log.iter()
        .rev()
        .find(|e| e.ok && s.commands.records_undo(&e.name))
        .map(|e| (e.name.clone(), e.params.clone()))
}

/// Re-run the most recent command that changed the layout (Blender's Repeat
/// Last, Shift+R). Navigation and failed commands are skipped — repeating a
/// tab-switch or a command that already errored is not what the key means.
pub fn repeat_last() -> immersion::Workspaces {
    let s = shared();
    let last = {
        let log = s.log.lock().expect("log");
        log.iter()
            .rev()
            .find(|e| e.ok && s.commands.records_undo(&e.name))
            .cloned()
    };
    match last {
        Some(e) => dispatch(&e.name, e.params),
        None => workspaces(),
    }
}

/// Replace the whole workbench from imported JSON (the layout export round
/// trips back in here). A parse failure is a no-op — a malformed upload must
/// not blank the workspace — and the swap is recorded for undo.
pub fn set_workspaces_from_json(json: &str) -> immersion::Workspaces {
    let s = shared();
    match serde_json::from_str::<immersion::Workspaces>(json) {
        Ok(new) => {
            let mut w = s.workspaces.lock().expect("workspaces");
            s.undo.lock().expect("undo").push(w.clone());
            *w = new;
            persist_workspaces(&s, &w);
            w.clone()
        }
        Err(e) => {
            eprintln!("layout import failed: {e}");
            workspaces()
        }
    }
}

fn persist_workspaces(s: &Shared, w: &immersion::Workspaces) {
    if let Ok(json) = serde_json::to_string(w) {
        let conn = s.db.lock().expect("db");
        let _ = conn.execute(
            "INSERT INTO kv (key, value) VALUES ('workspaces', ?1)
             ON CONFLICT(key) DO UPDATE SET value = ?1",
            rusqlite::params![json],
        );
    }
}

/// Step back one edit. The current value goes onto the redo stack, the last
/// undo value becomes current. A no-op with an empty stack.
pub fn undo() -> immersion::Workspaces {
    let s = shared();
    let mut w = s.workspaces.lock().expect("workspaces");
    if let Some(prev) = s.undo.lock().expect("undo").pop() {
        s.redo.lock().expect("redo").push(w.clone());
        *w = prev;
        persist_workspaces(&s, &w);
    }
    w.clone()
}

pub fn redo() -> immersion::Workspaces {
    let s = shared();
    let mut w = s.workspaces.lock().expect("workspaces");
    if let Some(next) = s.redo.lock().expect("redo").pop() {
        s.undo.lock().expect("undo").push(w.clone());
        *w = next;
        persist_workspaces(&s, &w);
    }
    w.clone()
}

/// Release a parked run and drive it. The UI's resume button.
pub fn resume(id: &str) -> Result<()> {
    let s = shared();
    // Same reason as trigger_with: the seeded runs are rows, not work. Driving
    // one would replay `shell` or `needs_a_human` against a box with neither.
    if crate::demo::enabled() {
        return Err(anyhow!("{}", crate::demo::REFUSAL));
    }
    engine::release_parks(&s.db, id)?;
    tokio::spawn(drive(id.to_string()));
    Ok(())
}

pub fn trigger_with(name: &str, input: Value) -> Result<()> {
    let s = shared();
    if !s.registry.contains_key(name) {
        return Err(anyhow!("unknown workflow {name}"));
    }
    // Every caller — a header button, an MCP tool, POST /trigger — lands here,
    // so one guard covers the lot. A demo instance has no herdr socket and no
    // checkout; starting a run would only manufacture a failed row.
    if crate::demo::enabled() {
        return Err(anyhow!("{}", crate::demo::REFUSAL));
    }
    let id = engine::create_run(&s.db, name, input)?;
    tokio::spawn(drive(id));
    Ok(())
}

/// Everything the UI draws, in one query pass.
pub fn snapshot() -> State {
    let s = shared();
    let conn = s.db.lock().expect("db");

    let mut runs: Vec<RunView> = Vec::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT id, workflow, status, note, error, updated_at FROM runs ORDER BY created_at DESC LIMIT 20",
    )
        && let Ok(rows) = stmt.query_map([], |r| {
            Ok(RunView {
                id: r.get(0)?,
                workflow: r.get(1)?,
                status: r.get(2)?,
                note: r.get(3)?,
                error: r.get(4)?,
                updated_at: r.get(5)?,
                steps: Vec::new(),
            })
        }) {
            runs = rows.filter_map(|r| r.ok()).collect();
        }

    let mut by_run: HashMap<String, Vec<StepView>> = HashMap::new();
    if let Ok(mut stmt) =
        conn.prepare("SELECT run_id, key, result, error FROM steps ORDER BY ended_at")
        && let Ok(rows) = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                StepView {
                    key: r.get(1)?,
                    result: r.get(2)?,
                    error: r.get(3)?,
                },
            ))
        })
    {
        for (run_id, step) in rows.flatten() {
            by_run.entry(run_id).or_default().push(step);
        }
    }
    for r in &mut runs {
        r.steps = by_run.remove(&r.id).unwrap_or_default();
    }

    let mut workflows: Vec<WorkflowView> = s
        .registry
        .iter()
        .map(|(name, def)| WorkflowView {
            name: (*name).to_string(),
            description: def.description.to_string(),
            example: def.example.map(str::to_string),
            schedule: def.schedule.map(str::to_string),
            next: s
                .schedules
                .get(name)
                .and_then(|c| c.next_after(Local::now()).ok())
                .map(|t| t.format("%H:%M").to_string()),
        })
        .collect();
    workflows.sort_by(|a, b| a.name.cmp(&b.name));

    // An hour of history is what a "what happened at 07:00" question needs;
    // longer windows belong to a range picker nobody has asked for yet.
    let since = engine::now_ms() - 3_600_000;
    drop(conn);

    // Newest first, last 50 — the Info editor shows recent history.
    let mut log = command_log();
    log.reverse();
    log.truncate(50);
    State {
        herdr: s.herdr.lock().expect("herdr").clone(),
        log,
        workflows,
        runs,
        machine: crate::metrics::box_now(&s.db),
        cpu: crate::metrics::series(&s.db, "box.cpu_pct", "", since),
        mem: crate::metrics::series(&s.db, "box.mem_used", "", since),
        annotations: crate::metrics::annotations(&s.db, since)
            .into_iter()
            .map(|(id, wf, from, to, status)| Annotation {
                id,
                workflow: wf,
                from,
                to,
                status,
            })
            .collect(),
        fleet: s.fleet.lock().expect("fleet").clone(),
        timers: s.timers.lock().expect("timers").clone(),
        window: (since, engine::now_ms()),
    }
}

fn fire_schedules() {
    let s = shared();
    let now = Local::now();
    let minute = cron::minute_of(now);
    for (name, c) in &s.schedules {
        if !c.matches(now) {
            continue;
        }
        let already = {
            let conn = s.db.lock().expect("db");
            conn.query_row(
                "SELECT last_minute FROM schedules WHERE workflow = ?1",
                params![name],
                |r| r.get::<_, Option<i64>>(0),
            )
            .unwrap_or(None)
        };
        if already == Some(minute) {
            continue; // already fired in this wall-clock minute
        }
        {
            let conn = s.db.lock().expect("db");
            let _ = conn.execute(
                "INSERT INTO schedules (workflow, expression, last_minute) VALUES (?1, ?2, ?3)
                 ON CONFLICT(workflow) DO UPDATE SET last_minute = ?3, expression = ?2",
                params![name, c.source, minute],
            );
        }
        match engine::create_run(&s.db, name, serde_json::json!({ "trigger": "schedule" })) {
            Ok(id) => {
                println!("schedule {name} fired -> run {}", &id[..8]);
                tokio::spawn(drive(id));
            }
            Err(e) => eprintln!("schedule {name}: could not create a run: {e}"),
        }
    }
}

async fn tick() {
    loop {
        fire_schedules();
        let s = shared();
        match engine::due(&s.db, engine::now_ms()) {
            Ok(ids) => {
                for id in ids {
                    tokio::spawn(drive(id));
                }
            }
            Err(e) => eprintln!("tick: {e}"),
        }
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
    }
}

/// herdr's identity, refreshed rather than held: the server can restart under
/// us and the UI should say so instead of showing a stale version forever.
async fn watch_herdr() {
    loop {
        let pong = tokio::task::spawn_blocking(crate::herdr::ping).await;
        let text = match pong {
            Ok(Ok(p)) => Some(format!("herdr {} · protocol {}", p.version, p.protocol)),
            _ => None,
        };
        *shared().herdr.lock().expect("herdr") = text;
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}

/// The websocket URL must be a PATH, not an absolute URL.
///
/// dioxus-liveview's glue special-cases a leading `/`: it derives the socket
/// address from `window.location`, picking `wss:` when the page was served
/// over https. Hand it an absolute `ws://host/ws` and it uses that verbatim —
/// which points at the viewer's own machine when the page is reached through
/// a proxy, and is refused as mixed content from an https page either way.
/// Everything on this box that fires on a clock.
///
/// powderman's own schedules are only part of the answer. systemd user timers
/// and cron entries fire too, and after moving treebank's job off cron the
/// question "what still runs unattended here" had no single place to look. It
/// does now, and the crontab line it shows is the comment pointing here.
async fn watch_timers() {
    loop {
        let mut rows: Vec<crate::ui::TimerRow> = Vec::new();
        let s = shared();
        for (name, cron) in &s.schedules {
            rows.push(crate::ui::TimerRow {
                source: "powderman".into(),
                name: (*name).to_string(),
                schedule: cron.source.clone(),
                next: cron
                    .next_after(Local::now())
                    .map(|t| t.format("%a %H:%M").to_string())
                    .unwrap_or_default(),
            });
        }
        let sh = |cmd: &str, args: &[&str]| -> String {
            std::process::Command::new(cmd)
                .args(args)
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
                .unwrap_or_default()
        };
        for line in sh(
            "systemctl",
            &[
                "--user",
                "list-timers",
                "--all",
                "--no-legend",
                "--no-pager",
            ],
        )
        .lines()
        {
            // NEXT(3 cols) LEFT UNIT ACTIVATES …; the unit name is what matters.
            let f: Vec<&str> = line.split_whitespace().collect();
            if let Some(unit) = f.iter().find(|w| w.ends_with(".timer")) {
                rows.push(crate::ui::TimerRow {
                    source: "systemd".into(),
                    name: (*unit).to_string(),
                    schedule: String::new(),
                    next: f.get(0..3).map(|w| w.join(" ")).unwrap_or_default(),
                });
            }
        }
        for line in sh("crontab", &["-l"]).lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let fields: Vec<&str> = line.split_whitespace().collect();
            let cut = 5.min(fields.len());
            let (sched, cmd) = fields.split_at(cut);
            rows.push(crate::ui::TimerRow {
                source: "cron".into(),
                name: cmd.join(" "),
                schedule: sched.join(" "),
                next: String::new(),
            });
        }
        rows.sort_by(|a, b| a.source.cmp(&b.source).then(a.name.cmp(&b.name)));
        *s.timers.lock().expect("timers") = rows;
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    }
}

/// Keep a recent picture of the fleet without paying for it on every render.
async fn watch_fleet() {
    loop {
        if let Ok(f) = tokio::task::spawn_blocking(crate::metrics::fleet).await {
            *shared().fleet.lock().expect("fleet") = f;
        }
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    }
}

/// The process's start time. A page whose socket died has no way to know the
/// daemon restarted — dioxus-liveview does not reconnect — so it polls this
/// and reloads when it changes. Without it every deploy leaves a page that
/// looks live and is not, which for a monitoring UI is the worst failure it
/// can have.
static BOOT_ID: OnceLock<String> = OnceLock::new();

/// A liveness probe for the Fly health check and the preview workflow. Cheap,
/// unauthenticated, and independent of herdr — a preview has no herdr, and the
/// point is only "is the server up and serving".
async fn health_route() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        "{\"ok\":true}\n",
    )
}

async fn boot_route() -> impl IntoResponse {
    BOOT_ID.get().cloned().unwrap_or_default()
}

async fn index() -> impl IntoResponse {
    Html(format!(
        r#"<!doctype html><html><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>powderman</title></head><body><div id="main"></div>{glue}
<script>
  // Reload when the daemon restarts, and when the socket has been shut long
  // enough that the render is stale.
  let boot = null;
  setInterval(async () => {{
    try {{
      const r = await fetch("/boot", {{cache: "no-store"}});
      const b = await r.text();
      if (boot === null) boot = b;
      else if (b !== boot) location.reload();
    }} catch (e) {{ /* daemon down; try again shortly */ }}
  }}, 4000);
</script>
</body></html>"#,
        glue = dioxus_liveview::interpreter_glue("/ws")
    ))
}

/// Start a run from outside the UI: curl, a GitHub webhook later, or a
/// person at a terminal. The UI's buttons are not a control plane.
async fn trigger_route(Path(name): Path<String>, body: String) -> impl IntoResponse {
    // A JSON body becomes the run's input. No body is fine — most workflows
    // take none — but a workflow like treebank_fix needs {"lang": "rust"} and
    // silently dropping it would start the wrong work.
    let input: Value = if body.trim().is_empty() {
        Value::Null
    } else {
        match serde_json::from_str(&body) {
            Ok(v) => v,
            Err(e) => return (StatusCode::BAD_REQUEST, format!("body is not JSON: {e}\n")),
        }
    };
    match trigger_with(&name, input) {
        Ok(()) => (StatusCode::ACCEPTED, format!("triggered {name}\n")),
        Err(e) => (StatusCode::NOT_FOUND, format!("{e}\n")),
    }
}

/// Let a parked run carry on.
///
/// This is the other half of `park` and the reason it exists: without a door
/// out, "waiting for a human" is indistinguishable from "abandoned". Releasing
/// the parks and re-driving is the whole operation — replay walks the run back
/// to where it stopped and continues.
async fn resume_route(Path(id): Path<String>) -> impl IntoResponse {
    let s = shared();
    if crate::demo::enabled() {
        return (StatusCode::FORBIDDEN, format!("{}\n", crate::demo::REFUSAL));
    }
    match engine::release_parks(&s.db, &id) {
        Ok(n) => {
            tokio::spawn(drive(id.clone()));
            (
                StatusCode::ACCEPTED,
                format!("resumed {id} ({n} park(s) released)\n"),
            )
        }
        Err(e) => (StatusCode::NOT_FOUND, format!("{e}\n")),
    }
}

pub async fn serve(db_path: &std::path::Path, port: u16) -> Result<()> {
    crate::herdr::ensure_socket_env();

    let db: Db = Arc::new(Mutex::new(crate::db::open(db_path)?));
    let initial_workspaces = {
        let conn = db.lock().expect("db");
        load_workspaces(&conn)
    };
    let registry = crate::workflows::registry();

    // A second instance run beside the real daemon — a dev build on another
    // port — must not fire schedules, or 06:00 sweeps twice and two fix
    // agents fight over one worktree path. POWDERMAN_SCHEDULES=0 makes an
    // instance view-only on the clock; triggers still work.
    let schedules_on = std::env::var("POWDERMAN_SCHEDULES").as_deref() != Ok("0");

    // Parsed once, at startup, so a malformed expression is a loud failure now
    // rather than a schedule that silently never fires.
    let mut schedules = HashMap::new();
    for (name, def) in &registry {
        if !schedules_on {
            println!("schedules disabled (POWDERMAN_SCHEDULES=0) — dev instance");
            break;
        }
        if let Some(expr) = def.schedule {
            let c = cron::parse(expr)?;
            println!(
                "schedule {name}: {} (next {})",
                c.source,
                c.next_after(Local::now())?.format("%Y-%m-%d %H:%M")
            );
            schedules.insert(*name, c);
        }
    }

    SHARED
        .set(Arc::new(Shared {
            db,
            registry,
            schedules,
            herdr: Mutex::new(None),
            fleet: Mutex::new(Vec::new()),
            workspaces: Mutex::new(initial_workspaces),
            commands: crate::workflows::commands(),
            undo: Mutex::new(Vec::new()),
            redo: Mutex::new(Vec::new()),
            timers: Mutex::new(Vec::new()),
            in_flight: Mutex::new(HashSet::new()),
            log: Mutex::new(Vec::new()),
        }))
        .map_err(|_| anyhow!("daemon started twice"))?;

    // A run left `running` was interrupted — the daemon was restarted or the
    // box went down mid-flight. Nothing else would ever pick it up: the tick
    // only looks at suspended runs with a due deadline. Re-driving it is
    // exactly what replay is for, and costs nothing for work already recorded.
    if !crate::demo::enabled() {
        let s = shared();
        let orphans: Vec<String> = {
            let conn = s.db.lock().expect("db");
            let mut stmt = conn.prepare("SELECT id FROM runs WHERE status = 'running'")?;
            let ids = stmt.query_map([], |r| r.get::<_, String>(0))?;
            ids.collect::<rusqlite::Result<Vec<_>>>()?
        };
        for id in orphans {
            println!("resuming interrupted run {}", &id[..8]);
            tokio::spawn(drive(id));
        }
    }

    let _ = BOOT_ID.set(engine::now_ms().to_string());

    tokio::spawn(tick());
    tokio::spawn(watch_timers());
    if crate::demo::enabled() {
        // No herdr to watch and no box worth sampling; the fleet is a fixed
        // list rather than a socket poll, so it is set once here.
        let s = shared();
        crate::demo::seed(&s.db);
        *s.fleet.lock().expect("fleet") = crate::demo::fleet();
        *s.herdr.lock().expect("herdr") = Some("demo fleet".to_string());
        tokio::spawn(crate::demo::sample_loop(s.db.clone()));
    } else {
        tokio::spawn(watch_herdr());
        tokio::spawn(crate::metrics::sample_loop(shared().db.clone()));
        tokio::spawn(watch_fleet());
    }

    let addr = format!("0.0.0.0:{port}");
    let view = dioxus_liveview::LiveViewPool::new();
    let pool = Arc::new(view);

    let app = Router::new()
        .route("/", get(index))
        .route("/trigger/{name}", post(trigger_route))
        .route("/resume/{id}", post(resume_route))
        .route("/boot", get(boot_route))
        .route("/health", get(health_route))
        .nest_service("/mcp", crate::mcp::service())
        .route(
            "/ws",
            get(move |ws: WebSocketUpgrade| {
                let pool = pool.clone();
                async move {
                    ws.on_upgrade(move |socket| async move {
                        _ = pool
                            .launch(dioxus_liveview::axum_socket(socket), ui::App)
                            .await;
                    })
                }
            }),
        );

    println!(
        "powderman on http://localhost:{port}  db={}",
        db_path.display()
    );
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
