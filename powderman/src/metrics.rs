//! Where the machine's time and memory went, and which run was responsible.
//!
//! Not a general metrics system. It answers one question — "the box was pegged
//! at 07:00, what was running?" — and the reason it can is that powderman
//! already knows what was running. Run and step boundaries are rows in the same
//! database as the samples, so an annotation is a join rather than an
//! integration.
//!
//! Three sources, and they are not equally cheap:
//!
//!   * **The box.** /proc/stat, /proc/loadavg, /proc/meminfo, statvfs. Free.
//!   * **Units.** Every ctx.exec runs in its own transient systemd unit, so
//!     `cpu.stat` and `memory.current` under its cgroup are that step's cost
//!     exactly, with no attribution guesswork. Read as files; shelling out to
//!     `systemctl show` every ten seconds would cost more than the thing being
//!     measured.
//!   * **Agents.** These do NOT get their own cgroup — a claude process
//!     reports `0::/init.scope`, because it belongs to the herdr server's
//!     scope rather than to anything per-agent. So agents are sampled by pid
//!     instead, via herdr's pane.process_info, and that costs socket calls.
//!     It is the one asymmetry in here and the reason agent sampling runs on a
//!     slower cadence than the rest.
//!
//! Samples are kept for a week and then deleted. There is no downsampling,
//! because at ten-second resolution a week is ~60k rows per series and SQLite
//! does not care.

use anyhow::Result;
use rusqlite::params;
use std::collections::HashMap;

use crate::engine::{Db, now_ms};

pub const RETENTION_DAYS: i64 = 7;
const SAMPLE_EVERY: u64 = 10;
/// Agents cost a socket round trip each, so they are sampled every Nth tick.
const AGENT_EVERY: u64 = 3;

pub fn migrate(conn: &rusqlite::Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS samples (
          at    INTEGER NOT NULL,
          name  TEXT NOT NULL,
          label TEXT NOT NULL DEFAULT '',
          value REAL NOT NULL
        );
        -- The two shapes of query: draw one series over a window, and delete
        -- everything older than a week.
        CREATE INDEX IF NOT EXISTS samples_series ON samples (name, label, at);
        CREATE INDEX IF NOT EXISTS samples_at ON samples (at);
        "#,
    )?;
    Ok(())
}

fn read(path: &str) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

/// Total and idle jiffies from /proc/stat's aggregate line.
fn cpu_totals() -> Option<(f64, f64)> {
    let stat = read("/proc/stat")?;
    let line = stat.lines().next()?;
    let v: Vec<f64> = line
        .split_whitespace()
        .skip(1)
        .filter_map(|x| x.parse().ok())
        .collect();
    if v.len() < 5 {
        return None;
    }
    let idle = v[3] + v[4]; // idle + iowait
    Some((v.iter().sum(), idle))
}

fn meminfo() -> Option<(f64, f64)> {
    let m = read("/proc/meminfo")?;
    let get = |k: &str| -> Option<f64> {
        m.lines()
            .find(|l| l.starts_with(k))?
            .split_whitespace()
            .nth(1)?
            .parse::<f64>()
            .ok()
            .map(|kb| kb * 1024.0)
    };
    Some((get("MemTotal:")?, get("MemAvailable:")?))
}

fn disk_used(path: &str) -> Option<(f64, f64)> {
    let out = std::process::Command::new("df")
        .args(["-B1", "--output=size,used", path])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().nth(1)?;
    let mut it = line.split_whitespace();
    let size: f64 = it.next()?.parse().ok()?;
    let used: f64 = it.next()?.parse().ok()?;
    Some((size, used))
}

fn app_slice() -> String {
    format!(
        "/sys/fs/cgroup/user.slice/user-{}.slice/user@{}.service/app.slice",
        nix_uid(),
        nix_uid()
    )
}

fn nix_uid() -> u32 {
    // No libc dependency for one number.
    read("/proc/self/status")
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("Uid:"))?
                .split_whitespace()
                .nth(1)?
                .parse()
                .ok()
        })
        .unwrap_or(1000)
}

/// `(cpu_usec, memory_bytes, tasks)` for one cgroup directory.
fn cgroup_stats(dir: &std::path::Path) -> Option<(f64, f64, f64)> {
    let cpu = read(dir.join("cpu.stat").to_str()?)?;
    let usec: f64 = cpu
        .lines()
        .find(|l| l.starts_with("usage_usec"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()?;
    let mem: f64 = read(dir.join("memory.current").to_str()?)?
        .trim()
        .parse()
        .unwrap_or(0.0);
    let tasks: f64 = read(dir.join("pids.current").to_str()?)
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0.0);
    Some((usec, mem, tasks))
}

/// CPU jiffies and RSS for a pid, from /proc.
fn pid_stats(pid: u32) -> Option<(f64, f64)> {
    let stat = read(&format!("/proc/{pid}/stat"))?;
    // Fields after the (possibly space-containing) comm, which is parenthesised.
    // The comm field is parenthesised and may contain spaces, so split from
    // the right on the closing paren rather than tokenising from the left.
    let rest = stat.rsplit_once(')')?.1;
    let f: Vec<&str> = rest.split_whitespace().collect();
    let utime: f64 = f.get(11)?.parse().ok()?;
    let stime: f64 = f.get(12)?.parse().ok()?;
    let rss_pages: f64 = f.get(21)?.parse().ok()?;
    Some((utime + stime, rss_pages * 4096.0))
}

struct Sampler {
    db: Db,
    last_cpu: Option<(f64, f64)>,
    tick: u64,
}

impl Sampler {
    fn write(&self, at: i64, rows: &[(String, String, f64)]) {
        let conn = self.db.lock().expect("db");
        for (name, label, value) in rows {
            let _ = conn.execute(
                "INSERT INTO samples (at, name, label, value) VALUES (?1, ?2, ?3, ?4)",
                params![at, name, label, value],
            );
        }
    }

    fn sample_box(&mut self, rows: &mut Vec<(String, String, f64)>) {
        if let Some((total, idle)) = cpu_totals() {
            if let Some((pt, pi)) = self.last_cpu {
                let dt = total - pt;
                let di = idle - pi;
                if dt > 0.0 {
                    rows.push(("box.cpu_pct".into(), String::new(), (1.0 - di / dt) * 100.0));
                }
            }
            self.last_cpu = Some((total, idle));
        }
        if let Some(v) =
            read("/proc/loadavg").and_then(|l| l.split_whitespace().next()?.parse::<f64>().ok())
        {
            rows.push(("box.load1".into(), String::new(), v));
        }
        if let Some((total, avail)) = meminfo() {
            rows.push(("box.mem_used".into(), String::new(), total - avail));
            rows.push(("box.mem_total".into(), String::new(), total));
        }
        if let Some((size, used)) = disk_used("/") {
            rows.push(("box.disk_used".into(), String::new(), used));
            rows.push(("box.disk_total".into(), String::new(), size));
        }
    }

    /// Units powderman owns: itself, and every transient exec unit alive now.
    fn sample_units(&self, rows: &mut Vec<(String, String, f64)>) {
        let slice = app_slice();
        let Ok(entries) = std::fs::read_dir(&slice) else {
            return;
        };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if !(name.starts_with("pm-") || name == "powderman.service") {
                continue;
            }
            let unit = name.trim_end_matches(".service").to_string();
            if let Some((cpu, mem, tasks)) = cgroup_stats(&e.path()) {
                rows.push(("unit.cpu_usec".into(), unit.clone(), cpu));
                rows.push(("unit.mem".into(), unit.clone(), mem));
                rows.push(("unit.tasks".into(), unit, tasks));
            }
        }
    }

    /// Agents, by pid, because they have no cgroup of their own.
    fn sample_agents(&self, rows: &mut Vec<(String, String, f64)>) {
        let Ok(agents) = crate::herdr::agents() else {
            return;
        };
        for a in agents {
            let Some(name) = a.name.clone() else { continue };
            let Ok(pids) = crate::herdr::pane_pids(&a.pane_id) else {
                continue;
            };
            let (mut cpu, mut rss) = (0.0, 0.0);
            for p in pids {
                if let Some((c, r)) = pid_stats(p) {
                    cpu += c;
                    rss += r;
                }
            }
            rows.push(("agent.cpu_jiffies".into(), name.clone(), cpu));
            rows.push(("agent.rss".into(), name, rss));
        }
    }

    fn prune(&self) {
        let cutoff = now_ms() - RETENTION_DAYS * 86_400_000;
        let conn = self.db.lock().expect("db");
        let _ = conn.execute("DELETE FROM samples WHERE at < ?1", params![cutoff]);
    }
}

/// Sample forever. Cheap enough to run on its own timer rather than the
/// workflow tick, so a slow herdr call cannot delay a scheduled run.
pub async fn sample_loop(db: Db) {
    let mut s = Sampler {
        db,
        last_cpu: None,
        tick: 0,
    };
    let mut pruned_day = -1i64;
    loop {
        let at = now_ms();
        let mut rows: Vec<(String, String, f64)> = Vec::new();
        s.sample_box(&mut rows);
        s.sample_units(&mut rows);
        if s.tick.is_multiple_of(AGENT_EVERY) {
            // Blocking socket calls; keep them off the async worker.
            let agent_rows = tokio::task::spawn_blocking({
                let db = s.db.clone();
                move || {
                    let dummy = Sampler {
                        db,
                        last_cpu: None,
                        tick: 0,
                    };
                    let mut r = Vec::new();
                    dummy.sample_agents(&mut r);
                    r
                }
            })
            .await
            .unwrap_or_default();
            rows.extend(agent_rows);
        }
        if !rows.is_empty() {
            s.write(at, &rows);
        }

        // Once a day, drop what aged out. No downsampling: a week at ten
        // seconds is ~60k rows per series, which SQLite does not notice.
        let day = at / 86_400_000;
        if day != pruned_day {
            s.prune();
            pruned_day = day;
        }

        s.tick = s.tick.wrapping_add(1);
        tokio::time::sleep(std::time::Duration::from_secs(SAMPLE_EVERY)).await;
    }
}

/// One series over a window, oldest first.
pub fn series(db: &Db, name: &str, label: &str, since: i64) -> Vec<(i64, f64)> {
    let conn = db.lock().expect("db");
    let Ok(mut stmt) = conn.prepare(
        "SELECT at, value FROM samples WHERE name = ?1 AND label = ?2 AND at >= ?3 ORDER BY at",
    ) else {
        return Vec::new();
    };
    stmt.query_map(params![name, label, since], |r| Ok((r.get(0)?, r.get(1)?)))
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default()
}

/// Every label seen for a metric in the window — which units and agents ran.
///
/// Not used by the current view, which charts the box and annotates it with
/// runs. It is what a per-unit or per-agent breakdown would be built on, and
/// the sampler is already recording those series.
#[allow(dead_code)]
pub fn labels(db: &Db, name: &str, since: i64) -> Vec<String> {
    let conn = db.lock().expect("db");
    let Ok(mut stmt) = conn
        .prepare("SELECT DISTINCT label FROM samples WHERE name = ?1 AND at >= ?2 ORDER BY label")
    else {
        return Vec::new();
    };
    stmt.query_map(params![name, since], |r| r.get::<_, String>(0))
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default()
}

/// Run boundaries in the window: the annotations.
pub fn annotations(db: &Db, since: i64) -> Vec<(String, String, i64, i64, String)> {
    let conn = db.lock().expect("db");
    let Ok(mut stmt) = conn.prepare(
        "SELECT id, workflow, created_at, updated_at, status FROM runs WHERE updated_at >= ?1 ORDER BY created_at",
    ) else {
        return Vec::new();
    };
    stmt.query_map(params![since], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
    })
    .map(|rows| rows.flatten().collect())
    .unwrap_or_default()
}

/// A live snapshot of the fleet — read on demand, never recorded.
///
/// The htop-ish half: what herdr has running right now and which processes
/// belong to it. Deliberately not a timeseries; recording every process would
/// be a hundred times the data for a question best answered by looking.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct FleetProc {
    pub pid: u32,
    pub name: String,
    pub rss: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct FleetAgent {
    pub name: String,
    pub status: String,
    pub cwd: String,
    pub pane: String,
    pub procs: Vec<FleetProc>,
}

pub fn fleet() -> Vec<FleetAgent> {
    let Ok(agents) = crate::herdr::agents() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for a in agents {
        let mut procs: Vec<FleetProc> = crate::herdr::pane_procs(&a.pane_id)
            .unwrap_or_default()
            .into_iter()
            .map(|(pid, name)| FleetProc {
                pid,
                name,
                rss: pid_stats(pid).map(|(_, r)| r).unwrap_or(0.0),
            })
            .collect();
        // Biggest first, so a capped view shows the ones that matter.
        procs.sort_by(|a, b| b.rss.total_cmp(&a.rss));
        out.push(FleetAgent {
            name: a.name.clone().unwrap_or_default(),
            status: a.status().to_string(),
            cwd: a.cwd.clone().unwrap_or_default(),
            pane: a.pane_id.clone(),
            procs,
        });
    }
    out
}

/// Totals for the header line.
pub fn box_now(db: &Db) -> HashMap<String, f64> {
    let mut m = HashMap::new();
    for k in [
        "box.cpu_pct",
        "box.load1",
        "box.mem_used",
        "box.mem_total",
        "box.disk_used",
        "box.disk_total",
    ] {
        if let Some((_, v)) = series(db, k, "", now_ms() - 120_000).last() {
            m.insert(k.to_string(), *v);
        }
    }
    m
}
