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
