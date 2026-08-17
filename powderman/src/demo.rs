//! Demo mode: a public instance with a believable history.
//!
//! A Fly deploy has no herdr and no systemd, so the workbench there is honest
//! and empty — no fleet, no runs, a CPU line flat at idle. An empty workbench
//! demonstrates nothing. `POWDERMAN_DEMO=1` seeds a synthetic few days: runs
//! with steps and errors, a fleet of agents, a metrics history with the shape
//! a 06:00 sweep actually leaves.
//!
//! All of it is fabricated, and the instance says so rather than letting the
//! numbers pass for a live box: the status bar carries a DEMO badge and
//! triggers are refused ([`refuse_trigger`]), because a trigger here would
//! reach for a herdr socket and a git worktree that do not exist.
//!
//! Seeding is idempotent — it only fires when `runs` is empty — so a machine
//! that restarts keeps whatever the last visitor did to the layout.

use crate::engine::{Db, now_ms};
use crate::metrics::{FleetAgent, FleetProc};

/// Is this instance a demo? Read once per call; it is only consulted at
/// startup and on the trigger path.
pub fn enabled() -> bool {
    std::env::var("POWDERMAN_DEMO").as_deref() == Ok("1")
}

/// Why a trigger is refused here. The UI surfaces this as the run's error, so
/// a visitor who presses Run gets an explanation rather than a silent nothing.
pub const REFUSAL: &str =
    "this is a demo instance — no herdr fleet and no worktrees, so workflows cannot run here";

const MIN: i64 = 60_000;

/// One seeded run: `ago` and `dur` are minutes before boot, so the history
/// reads as recent no matter when the machine happens to start.
struct Run {
    id: &'static str,
    workflow: &'static str,
    input: &'static str,
    status: &'static str,
    note: Option<&'static str>,
    error: Option<&'static str>,
    ago: i64,
    dur: i64,
    /// `(name, result, error)` — one row per completed step, spread evenly
    /// across the run's span.
    steps: &'static [(&'static str, Option<&'static str>, Option<&'static str>)],
}

const RUNS: &[Run] = &[
    Run {
        id: "9f3a1c2e4b6d8a0f1e3c5a7b9d1f3e5c",
        workflow: "treebank_sweep",
        input: r#"{"limit":100}"#,
        status: "done",
        note: Some("41 grammars, 3 with gaps"),
        error: None,
        ago: 1_580,
        dur: 34,
        steps: &[
            ("pull", Some(r#"{"changed":true,"head":"2128ef5"}"#), None),
            ("build", Some(r#"{"ok":true,"secs":96}"#), None),
            ("rank", Some(r#"{"languages":41}"#), None),
            (
                "fetch:python",
                Some(r#"{"files":812,"bytes":9418233}"#),
                None,
            ),
            ("materialize:python", Some(r#"{"trees":812}"#), None),
            ("sweep:python", Some(r#"{"nodes":141822,"gaps":0}"#), None),
            ("fetch:rust", Some(r#"{"files":640,"bytes":7733901}"#), None),
            ("materialize:rust", Some(r#"{"trees":640}"#), None),
            ("sweep:rust", Some(r#"{"nodes":98410,"gaps":3}"#), None),
            (
                "fetch:javascript",
                Some(r#"{"files":915,"bytes":6120044}"#),
                None,
            ),
            ("materialize:javascript", Some(r#"{"trees":915}"#), None),
            (
                "sweep:javascript",
                Some(r#"{"nodes":120553,"gaps":1}"#),
                None,
            ),
            ("handoff", Some(r#"{"queued":["rust","javascript"]}"#), None),
        ],
    },
    Run {
        id: "c4d6e8f0a2b4c6d8e0f2a4b6c8d0e2f4",
        workflow: "treebank_fix",
        input: r#"{"lang":"rust"}"#,
        status: "done",
        note: Some("PR #93 — 3 node kinds"),
        error: None,
        ago: 1_540,
        dur: 71,
        steps: &[
            (
                "worktree",
                Some(r#"{"path":"/home/exedev/treebank-fix-rust"}"#),
                None,
            ),
            ("agent", Some(r#"{"pane":"%17","model":"opus"}"#), None),
            ("wait", Some(r#"{"settled_after_s":2860}"#), None),
            (
                "verify",
                Some(r#"{"parsed":640,"errors":0,"gaps":0}"#),
                None,
            ),
            (
                "pr",
                Some(r#"{"url":"https://github.com/PowderworksCode/treebank/pull/93"}"#),
                None,
            ),
        ],
    },
    Run {
        id: "1a2b3c4d5e6f7081920a3b4c5d6e7f80",
        workflow: "treebank_fix",
        input: r#"{"lang":"javascript"}"#,
        status: "failed",
        note: None,
        error: Some("verify: 14 files still parse with errors after 2 agent passes"),
        ago: 1_490,
        dur: 96,
        steps: &[
            (
                "worktree",
                Some(r#"{"path":"/home/exedev/treebank-fix-javascript"}"#),
                None,
            ),
            ("agent", Some(r#"{"pane":"%18","model":"opus"}"#), None),
            ("wait", Some(r#"{"settled_after_s":4102}"#), None),
            (
                "verify",
                None,
                Some(r#"{"parsed":915,"errors":14,"sample":"jsx spread in call position"}"#),
            ),
        ],
    },
    Run {
        id: "77e9d1b3f5a7c9e1d3b5f7a9c1e3d5b7",
        workflow: "agent",
        input: r#"{"prompt":"port the keymap help overlay to the new widget kit"}"#,
        status: "done",
        note: Some("settled after 22m"),
        error: None,
        ago: 980,
        dur: 23,
        steps: &[
            (
                "workspace",
                Some(r#"{"name":"immersion-keymap","pane":"%21"}"#),
                None,
            ),
            ("prompt", Some(r#"{"chars":184}"#), None),
            ("settle", Some(r#"{"idle_s":90,"turns":31}"#), None),
        ],
    },
    Run {
        id: "b8c0d2e4f6a8b0c2d4e6f8a0b2c4d6e8",
        workflow: "shell",
        input: "null",
        status: "failed",
        note: None,
        error: Some("step 2 exited 1: cargo clippy -- -D warnings"),
        ago: 640,
        dur: 3,
        steps: &[
            (
                "cmd:0",
                Some(r#"{"exit":0,"cmd":"cargo fmt --check"}"#),
                None,
            ),
            ("cmd:1", None, Some(r#"{"exit":1,"cmd":"cargo clippy"}"#)),
        ],
    },
    Run {
        id: "33f5a7c9e1d3b5f7a9c1e3d5b7f9a1c3",
        workflow: "needs_a_human",
        input: r#"{"why":"grammar bump changes 41 fixtures — wants a look"}"#,
        status: "suspended",
        note: Some("waiting on a human since 04:12"),
        error: None,
        ago: 310,
        dur: 1,
        steps: &[("park", Some(r#"{"reason":"review"}"#), None)],
    },
    Run {
        id: "5c7e9a1b3d5f7091a3c5e7d9b1f3a5c7",
        workflow: "treebank_sweep",
        input: r#"{"limit":100}"#,
        status: "running",
        note: Some("sweeping — 18 of 41"),
        error: None,
        ago: 26,
        dur: 26,
        steps: &[
            ("pull", Some(r#"{"changed":false,"head":"4820176"}"#), None),
            ("build", Some(r#"{"ok":true,"secs":88}"#), None),
            ("rank", Some(r#"{"languages":41}"#), None),
            ("fetch:go", Some(r#"{"files":701,"bytes":8210553}"#), None),
            ("materialize:go", Some(r#"{"trees":701}"#), None),
            ("sweep:go", Some(r#"{"nodes":110284,"gaps":0}"#), None),
        ],
    },
];

/// The fleet a demo shows. Real [`crate::metrics::fleet`] asks herdr over a
/// socket; there is no herdr here, so the shape is hand-written to match what
/// that call returns for a box with four agents up.
pub fn fleet() -> Vec<FleetAgent> {
    let agent =
        |name: &str, status: &str, cwd: &str, pane: &str, procs: &[(u32, &str, f64)]| FleetAgent {
            name: name.to_string(),
            status: status.to_string(),
            cwd: cwd.to_string(),
            pane: pane.to_string(),
            procs: procs
                .iter()
                .map(|(pid, name, rss)| FleetProc {
                    pid: *pid,
                    name: name.to_string(),
                    rss: *rss,
                })
                .collect(),
        };
    vec![
        agent(
            "treebank-sweep",
            "busy",
            "/home/exedev/treebank",
            "%14",
            &[
                (48211, "claude", 412_000_000.0),
                (48260, "tree-sitter", 96_400_000.0),
            ],
        ),
        agent(
            "immersion-widgets",
            "idle",
            "/home/exedev/powderworks/immersion",
            "%21",
            &[(31904, "claude", 388_100_000.0)],
        ),
        agent(
            "immersion-ci",
            "busy",
            "/home/exedev/powderworks/immersion",
            "%22",
            &[
                (52017, "claude", 401_700_000.0),
                (52240, "cargo", 1_240_000_000.0),
            ],
        ),
        agent(
            "treebank-fix-rust",
            "waiting",
            "/home/exedev/treebank-fix-rust",
            "%17",
            &[(44182, "claude", 355_900_000.0)],
        ),
    ]
}

/// Seed the history, once. No-op if `runs` already has rows, so a restart —
/// or a visitor's own triggered run — is never overwritten.
pub fn seed(db: &Db) {
    let now = now_ms();
    let conn = db.lock().expect("db");
    let empty: i64 = conn
        .query_row("SELECT COUNT(*) FROM runs", [], |r| r.get(0))
        .unwrap_or(1);
    if empty > 0 {
        return;
    }

    for r in RUNS {
        let start = now - r.ago * MIN;
        let end = start + r.dur * MIN;
        // No deadline, ever. `needs_a_human` parks indefinitely in reality,
        // and a wake_at here would eventually have the tick drive a fabricated
        // run against a box with no herdr.
        let wake_at: Option<i64> = None;
        let _ = conn.execute(
            "INSERT INTO runs (id, workflow, input, status, wake_at, note, pane_id, error, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?8, ?9)",
            rusqlite::params![r.id, r.workflow, r.input, r.status, wake_at, r.note, r.error, start, end],
        );
        let n = r.steps.len().max(1) as i64;
        for (i, (name, result, error)) in r.steps.iter().enumerate() {
            let ended = start + (end - start) * (i as i64 + 1) / n;
            let _ = conn.execute(
                "INSERT INTO steps (run_id, key, name, result, error, ended_at)
                 VALUES (?1, ?2, ?2, ?3, ?4, ?5)",
                rusqlite::params![r.id, name, result, error, ended],
            );
        }
    }

    backfill_samples(&conn, now);
    println!("demo mode: seeded {} runs", RUNS.len());
}

/// Six hours of box metrics at one-minute resolution, so the charts open with
/// a history instead of drawing themselves a pixel at a time. The shape is the
/// one a sweep leaves: idle, a long CPU plateau while grammars build, a
/// memory ramp that releases at the end.
fn backfill_samples(conn: &rusqlite::Connection, now: i64) {
    const POINTS: i64 = 360;
    const MEM_TOTAL: f64 = 16.0 * 1024.0 * 1024.0 * 1024.0;
    const DISK_TOTAL: f64 = 200.0 * 1024.0 * 1024.0 * 1024.0;

    // Deterministic jitter: a sampled line with no noise reads as a drawing,
    // not a measurement, and Math.random has no place in a reproducible seed.
    let mut lcg: u64 = 0x2545_F491_4F6C_DD1D;
    let mut noise = || {
        lcg = lcg
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((lcg >> 33) as f64 / (1u64 << 31) as f64) - 0.5
    };

    for i in 0..POINTS {
        let at = now - (POINTS - 1 - i) * MIN;
        // Two builds in the window: a long one early, a shorter one running
        // now — which is the run the fleet shows as busy.
        let busy = (60..140).contains(&i) || i > 330;
        let cpu = if busy { 74.0 } else { 8.0 } + noise() * 9.0;
        let mem = if busy { 0.62 } else { 0.34 } + noise() * 0.03;
        let rows: [(&str, f64); 5] = [
            ("box.cpu_pct", cpu.clamp(0.5, 99.0)),
            // Load trails CPU rather than tracking it: the sweep is four
            // parallel builds, so a busy minute sits above 1.0 per core-ish.
            ("box.load1", (cpu / 22.0).max(0.05)),
            ("box.mem_used", MEM_TOTAL * mem),
            ("box.mem_total", MEM_TOTAL),
            ("box.disk_used", DISK_TOTAL * (0.41 + i as f64 * 0.00004)),
        ];
        for (name, value) in rows {
            let _ = conn.execute(
                "INSERT INTO samples (at, name, label, value) VALUES (?1, ?2, '', ?3)",
                rusqlite::params![at, name, value],
            );
        }
        let _ = conn.execute(
            "INSERT INTO samples (at, name, label, value) VALUES (?1, 'box.disk_total', '', ?2)",
            rusqlite::params![at, DISK_TOTAL],
        );
    }
}

/// Keep the synthetic series moving. The real [`crate::metrics::sample_loop`]
/// is not run in demo mode: a Fly machine's actual idle CPU and 512MB of RAM
/// appended to a seeded 16GB box would put a cliff in the middle of every
/// chart. This continues the fabricated line instead, one point a minute, so a
/// chart left open still redraws.
pub async fn sample_loop(db: Db) {
    const MEM_TOTAL: f64 = 16.0 * 1024.0 * 1024.0 * 1024.0;
    const DISK_TOTAL: f64 = 200.0 * 1024.0 * 1024.0 * 1024.0;
    let mut lcg: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut tick: u64 = 0;
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        tick += 1;
        lcg = lcg
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let n = ((lcg >> 33) as f64 / (1u64 << 31) as f64) - 0.5;
        // Roughly twenty minutes busy in every hour, so a visitor who leaves
        // the machine view open sees the line move rather than flatline.
        let busy = tick % 60 < 20;
        let cpu: f64 = if busy { 74.0 } else { 8.0 } + n * 9.0;
        let mem = if busy { 0.62 } else { 0.34 } + n * 0.03;
        let at = now_ms();
        let conn = db.lock().expect("db");
        for (name, value) in [
            ("box.cpu_pct", cpu.clamp(0.5, 99.0)),
            ("box.load1", (cpu / 22.0).max(0.05)),
            ("box.mem_used", MEM_TOTAL * mem),
            ("box.mem_total", MEM_TOTAL),
            ("box.disk_used", DISK_TOTAL * 0.42),
            ("box.disk_total", DISK_TOTAL),
        ] {
            let _ = conn.execute(
                "INSERT INTO samples (at, name, label, value) VALUES (?1, ?2, '', ?3)",
                rusqlite::params![at, name, value],
            );
        }
    }
}

/// A fabricated file tree. The demo must not list the machine it runs on —
/// names and sizes alone map a container, and the code viewer will make what
/// the browser lists readable — so the public instance browses a plausible
/// checkout that does not exist. Same contract as the seeded runs: it looks
/// like the real thing and is entirely made up.
///
/// `(path, contents)`; a path ending in `/` is a directory and its contents
/// are ignored. Sizes shown in the browser are derived from the contents, so
/// what the file browser claims and what the code viewer shows agree — a demo
/// that listed a 4 KB file and then displayed six lines would be a demo of a
/// bug.
const FILES: &[(&str, &str)] = &[
    ("/README.md", README_MD),
    ("/Cargo.toml", CARGO_TOML),
    ("/docs/", ""),
    ("/docs/roadmap.md", ROADMAP_MD),
    ("/immersion/", ""),
    ("/immersion/src/", ""),
    ("/immersion/src/tree.rs", TREE_RS),
    ("/immersion/src/area.rs", AREA_RS),
    ("/immersion/ts/", ""),
    ("/immersion/ts/gestures.ts", GESTURES_TS),
    ("/powderman/", ""),
    ("/powderman/src/", ""),
    ("/powderman/src/daemon.rs", DAEMON_RS),
    ("/powderman/src/workflows.rs", WORKFLOWS_RS),
];

const README_MD: &str = r#"# powderman

Durable workflows on a Blender-style workbench.

## What this is

A daemon that runs long jobs and survives restarts, and a tiled workbench for
watching them. Every mutation goes through one command bus, so a header
button, a keystroke and an agent's tool call are the same operation.

## Running it

    cargo run -p powderman -- --db ~/.powderman.db --port 7777

Schedules are on by default. `POWDERMAN_SCHEDULES=0` makes an instance
view-only on the clock, which is what a second instance beside the real one
should be.

## The demo

`POWDERMAN_DEMO=1` seeds a fabricated history and refuses to run anything.
The instance you are reading this on is one of those: the runs, the fleet,
the metrics and this file are all made up.
"#;

const CARGO_TOML: &str = r#"[workspace]
members = ["immersion", "powderman"]
resolver = "2"

[workspace.package]
edition = "2024"
license = "Apache-2.0"

[profile.release]
lto = "thin"
codegen-units = 1
"#;

const ROADMAP_MD: &str = r#"# The workbench, honestly

## Where we are

Areas with n-ary splits, regions, workspaces, menus, a command palette, a
keymap with rebinding, a widget kit bound to serde documents by pointer, and
a tree view with two editors over it.

## Where we are going

1. Parity made honest — enforced, not asserted.
2. Errors surface — one type, two surfaces.
3. The tree view, and the data editor over it.
4. Editors have targets.
5. Code and diff viewers.
6. Charts as Vega-Lite documents.

## What we are not doing

Runtime extensibility. Hosts add editors and commands at compile time; there
is no plugin loading and no runtime schema registry.
"#;

const TREE_RS: &str = r#"//! The tree view: expandable rows over any hierarchical value.
//!
//! One component, many editors. The host supplies children through a
//! callback, so the same view walks a serde document, a directory, or
//! anything else that answers "what is under this node".

use dioxus::prelude::*;

/// One row the host hands back from its children callback.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeRow {
    /// Where this node lives. Opaque to the component.
    pub pointer: String,
    /// The name shown on the row.
    pub label: String,
    /// A short value preview, dimmed, after the label.
    pub preview: String,
    /// Whether the row can expand.
    pub has_children: bool,
}

/// Children of a node inside a serde document.
pub fn value_children(doc: &serde_json::Value, pointer: &str) -> Vec<TreeRow> {
    let Some(node) = doc.pointer(pointer) else {
        return Vec::new();
    };
    match node {
        serde_json::Value::Object(map) => map
            .iter()
            .map(|(k, v)| TreeRow {
                pointer: format!("{pointer}/{k}"),
                label: k.clone(),
                preview: preview(v),
                has_children: branches(v),
            })
            .collect(),
        _ => Vec::new(),
    }
}
"#;

const AREA_RS: &str = r#"//! The layout tree: areas, splits, and the operations on them.
//!
//! An area is a leaf with an editor, or a split with children and the sizes
//! between them. That is the whole model — no tabs, no floating panels, no
//! z-order — so persistence and undo are serialization, not integration.

use serde::{Deserialize, Serialize};

pub type AreaId = u64;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Area {
    Leaf {
        id: AreaId,
        editor: String,
        arg: Option<String>,
    },
    Split {
        id: AreaId,
        dir: Dir,
        sizes: Vec<f32>,
        children: Vec<Area>,
    },
}

impl Layout {
    /// Split an area in two. Returns the new leaf's id, or None if the
    /// target is not a leaf.
    pub fn split(&mut self, target: AreaId, dir: Dir, ratio: f32) -> Option<AreaId> {
        let id = self.next_id;
        self.next_id += 1;
        Some(id)
    }
}
"#;

const GESTURES_TS: &str = r#"// The gesture shim: corner drags, seam drags, and the drop targets.
//
// Everything here is frame-path work — pointer moves, a preview rectangle —
// and it commits exactly one message on release. The server never sees the
// drag, only its result.

import { once } from "./types";
import type { Gesture as GestureMsg } from "./generated/Gesture";

const THRESHOLD = 4;

if (once("__imGestures")) {
  let seam: { splitId: number; index: number } | null = null;

  const send = (msg: GestureMsg): void => {
    try {
      dioxus.send(JSON.stringify(msg));
    } catch {
      /* channel gone; a reload re-installs */
    }
  };

  document.addEventListener("pointerup", (e) => {
    if (!seam) return;
    send({ t: "ratio", id: seam.splitId, index: seam.index, ratio: seamRatio(e) });
    seam = null;
  });
}
"#;

const DAEMON_RS: &str = r#"//! The daemon: state, the command bus, and the routes.

use anyhow::Result;

/// Run a layout command and hand back the new workbench. THE one write path:
/// every UI mutation arrives here as a named command, is applied to the
/// workspace value, and persisted.
pub fn dispatch_from(
    source: &str,
    name: &str,
    params: serde_json::Value,
) -> Result<immersion::Workspaces> {
    let s = shared();
    let mut w = s.workspaces.lock().expect("workspaces");
    // The command runs against a clone, so a failure leaves the live
    // workspace untouched and undo is recorded only on success.
    let mut candidate = w.clone();
    let outcome = s.commands.run(&mut candidate, name, &params);
    log_command(source, name, params.clone(), outcome.is_ok());
    outcome?;
    if s.commands.records_undo(name) {
        s.undo.lock().expect("undo").push(w.clone());
        s.redo.lock().expect("redo").clear();
    }
    *w = candidate;
    persist_workspaces(&s, &w);
    Ok(w.clone())
}
"#;

const WORKFLOWS_RS: &str = r#"//! The workflow registry: what this daemon knows how to run.

use crate::engine::{Registry, WorkflowDef};

pub fn registry() -> Registry {
    let mut r = Registry::new();
    r.insert(
        "treebank_sweep",
        WorkflowDef {
            description: "Pull, build, then rank every grammar; hand any with gaps to a fix run.",
            example: Some("{\"limit\": 100}"),
            schedule: Some("0 6 * * *"),
            cwd: None,
            run: crate::treebank::sweep_all,
        },
    );
    r
}
"#;

/// A file the demo presents as changed, with the patch to show for it. Two
/// entries, so the diff editor has both a modified file and (for everything
/// else) the "matches HEAD" case to demonstrate.
const DIFFS: &[(&str, &str)] = &[
    (
        "/immersion/src/tree.rs",
        r#"diff --git a/immersion/src/tree.rs b/immersion/src/tree.rs
--- a/immersion/src/tree.rs
+++ b/immersion/src/tree.rs
@@ -10,7 +10,7 @@ use dioxus::prelude::*;
 /// One row the host hands back from its children callback.
 #[derive(Debug, Clone, PartialEq)]
 pub struct TreeRow {
-    /// Where this node lives.
+    /// Where this node lives. Opaque to the component.
     pub pointer: String,
     /// The name shown on the row.
     pub label: String,
@@ -20,6 +20,8 @@ pub struct TreeRow {
     pub preview: String,
     /// Whether the row can expand.
     pub has_children: bool,
+    /// Set when the host could not read this node's children.
+    pub unreadable: bool,
 }
"#,
    ),
    (
        "/powderman/src/daemon.rs",
        r#"diff --git a/powderman/src/daemon.rs b/powderman/src/daemon.rs
--- a/powderman/src/daemon.rs
+++ b/powderman/src/daemon.rs
@@ -14,6 +14,7 @@ pub fn dispatch_from(
     let mut candidate = w.clone();
     let outcome = s.commands.run(&mut candidate, name, &params);
     log_command(source, name, params.clone(), outcome.is_ok());
+    // Logged either way; surface the error after recording it.
     outcome?;
"#,
    ),
];

/// Which fabricated files are presented as changed. The diff editor's picker
/// lists these, so finding something to look at does not mean guessing which
/// two of fourteen files have a patch.
pub fn changed_files() -> Vec<&'static str> {
    DIFFS.iter().map(|(p, _)| *p).collect()
}

/// The fabricated contents of one file, for the code viewer.
pub fn file_source(path: &str) -> Option<&'static str> {
    FILES
        .iter()
        .find(|(p, _)| *p == path && !p.ends_with('/'))
        .map(|(_, body)| *body)
}

/// The fabricated patch for one file. `Some(None)` means the file exists and
/// matches HEAD; `None` means there is no such file.
pub fn file_diff(path: &str) -> Option<Option<&'static str>> {
    file_source(path)?;
    Some(DIFFS.iter().find(|(p, _)| *p == path).map(|(_, d)| *d))
}

/// Children of one directory in the fabricated tree.
/// Children of one directory in the fabricated tree.
pub fn file_children(pointer: &str) -> Vec<immersion::TreeRow> {
    let mut rows = fabricated_rows(pointer);
    // Directories first, like the real browser: two file views that sort
    // differently read as two different tools.
    rows.sort_by_key(|r| (!r.has_children, r.label.clone()));
    rows
}

fn fabricated_rows(pointer: &str) -> Vec<immersion::TreeRow> {
    let prefix = if pointer.is_empty() {
        "/".to_string()
    } else {
        format!("{pointer}/")
    };
    FILES
        .iter()
        .filter_map(|(path, body)| {
            let rest = path.strip_prefix(&prefix)?;
            // One level only: a deeper path belongs to a child directory, and
            // the trailing slash of a directory row is not a level of its own.
            let name = rest.trim_end_matches('/');
            if name.is_empty() || name.contains('/') {
                return None;
            }
            let is_dir = path.ends_with('/');
            Some(immersion::TreeRow {
                pointer: format!("{prefix}{name}"),
                label: if is_dir {
                    format!("{name}/")
                } else {
                    name.to_string()
                },
                preview: if is_dir {
                    String::new()
                } else {
                    crate::editors::human_size(body.len() as u64)
                },
                has_children: is_dir,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn db() -> Db {
        Arc::new(Mutex::new(crate::db::open_in_memory().expect("open")))
    }

    fn count(db: &Db, sql: &str) -> i64 {
        db.lock()
            .expect("db")
            .query_row(sql, [], |r| r.get(0))
            .expect("count")
    }

    #[test]
    fn the_fabricated_tree_walks_one_level_at_a_time() {
        let root = file_children("");
        let names: Vec<&str> = root.iter().map(|r| r.label.as_str()).collect();
        assert!(names.contains(&"docs/"), "{names:?}");
        assert!(names.contains(&"README.md"), "{names:?}");
        assert!(
            !names.iter().any(|n| n.contains('/') && !n.ends_with('/')),
            "a deeper path leaked into the root level: {names:?}"
        );
        let src = file_children("/immersion/src");
        assert!(src.iter().all(|r| !r.has_children), "no dirs at that level");
        assert!(src.iter().any(|r| r.label == "tree.rs"));
        // Nothing outside the fabricated set, whatever is asked for.
        assert!(file_children("/etc").is_empty());
        assert!(file_children("/../..").is_empty());
    }

    #[test]
    fn seeding_twice_seeds_once() {
        // A machine restarts more often than it deploys; a second seed would
        // fail on the primary key and, worse, double the history if the ids
        // ever stopped being fixed.
        let db = db();
        seed(&db);
        let after_first = count(&db, "SELECT COUNT(*) FROM runs");
        seed(&db);
        assert_eq!(after_first, RUNS.len() as i64);
        assert_eq!(count(&db, "SELECT COUNT(*) FROM runs"), after_first);
    }

    #[test]
    fn every_seeded_run_has_steps_and_a_status_the_ui_draws() {
        let db = db();
        seed(&db);
        let conn = db.lock().expect("db");
        for r in RUNS {
            let steps: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM steps WHERE run_id = ?1",
                    [r.id],
                    |x| x.get(0),
                )
                .expect("steps");
            assert!(steps > 0, "{} has no steps", r.workflow);
            assert!(
                matches!(r.status, "done" | "failed" | "running" | "suspended"),
                "{} has status {}, which the runs list has no colour for",
                r.workflow,
                r.status
            );
        }
    }

    #[test]
    fn no_seeded_run_is_ever_due() {
        // The tick drives suspended runs whose wake_at has passed. A demo has
        // no herdr to drive them with, so none may carry a deadline.
        let db = db();
        seed(&db);
        assert_eq!(
            count(&db, "SELECT COUNT(*) FROM runs WHERE wake_at IS NOT NULL"),
            0
        );
    }

    #[test]
    fn the_machine_view_finds_every_tile_it_reads() {
        // box_now looks up a fixed list of names; one missing is a tile that
        // silently reads zero, which is how the LOAD tile shipped blank.
        let db = db();
        seed(&db);
        let now = crate::metrics::box_now(&db);
        for k in [
            "box.cpu_pct",
            "box.load1",
            "box.mem_used",
            "box.mem_total",
            "box.disk_used",
            "box.disk_total",
        ] {
            assert!(now.contains_key(k), "no sample for {k}");
        }
    }
}

#[cfg(test)]
mod demo_files {
    /// The demo's browser, code viewer and diff viewer read one table, so
    /// what the browser lists is exactly what the viewers can open. A path in
    /// the listing with no contents is the bug this catches.
    #[test]
    fn everything_listed_can_be_opened() {
        fn walk(dir: &str, seen: &mut usize) {
            for row in super::file_children(dir) {
                if row.has_children {
                    walk(&row.pointer, seen);
                } else {
                    let body = super::file_source(&row.pointer)
                        .unwrap_or_else(|| panic!("{} is listed but has no contents", row.pointer));
                    assert!(!body.is_empty(), "{} is empty", row.pointer);
                    // The size the browser shows comes from the same string.
                    assert_eq!(row.preview, crate::editors::human_size(body.len() as u64));
                    *seen += 1;
                }
            }
        }
        let mut seen = 0;
        walk("", &mut seen);
        assert!(seen >= 8, "only {seen} files in the fabricated tree");
    }

    #[test]
    fn the_diff_viewer_has_both_cases_to_show() {
        // A changed file and an unchanged one, so the editor demonstrates
        // its patch rendering and its "matches HEAD" state.
        let changed = super::file_diff("/immersion/src/tree.rs")
            .expect("the file exists")
            .expect("and is presented as changed");
        assert!(changed.starts_with("diff --git "), "git header: {changed}");
        assert!(changed.contains("@@"), "a hunk header");
        assert!(changed.contains("\n+"), "an added line");
        assert!(
            super::file_diff("/README.md").expect("exists").is_none(),
            "an unchanged file matches HEAD"
        );
        assert!(super::file_diff("/etc/passwd").is_none(), "no such file");
    }
}
