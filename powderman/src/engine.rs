//! The engine: four primitives, and a runner that re-invokes.
//!
//! A workflow is an async function. `ctx.step()` marks a durable boundary —
//! its result is written to SQLite, and on any later invocation of the same
//! run it returns that recorded value without executing. A run that was
//! interrupted resumes by being called again from the top: work already done
//! replays from the database in microseconds, and execution continues at the
//! first step with no row.
//!
//! That has one consequence, and it is the price of workflows being ordinary
//! functions rather than a declared graph: **the body must be deterministic
//! outside step()**. Branching on the clock, on a random number, or on a file
//! read done in the body means a replay can take a different path than the
//! original run, and then recorded steps line up with the wrong calls. Read
//! the world inside `step` or `poll`, never in the body.

use anyhow::{Result, anyhow};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

pub type Db = Arc<Mutex<Connection>>;

/// A recorded step, as replay sees it: the JSON result, or the error that was
/// recorded instead. Exactly one is `Some`.
type Recorded = (Option<String>, Option<String>);
pub type BoxFut<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
pub type Runner = fn(Ctx) -> BoxFut<'static, Result<Value>>;

#[derive(Clone)]
#[allow(dead_code)] // `cwd` is read once a workflow declares one
pub struct WorkflowDef {
    /// One line: what running this does. Shown beside the button, because a
    /// name alone is not an answer to "what happens if I press this".
    pub description: &'static str,
    /// An example input, for a workflow that needs one. `None` means it takes
    /// none, and the difference matters: a workflow that silently does nothing
    /// without input looks broken.
    pub example: Option<&'static str>,
    /// Five-field cron. `None` for a workflow that only runs on demand.
    pub schedule: Option<&'static str>,
    /// Where its commands run. `None` means the daemon's cwd.
    pub cwd: Option<&'static str>,
    pub run: Runner,
}

pub type Registry = HashMap<&'static str, WorkflowDef>;

/// Unwinds a workflow that is parked. Never escapes `run`.
#[derive(Debug)]
pub struct Suspend {
    pub wake_at: Option<i64>,
    pub note: String,
}

impl std::fmt::Display for Suspend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "suspended: {}", self.note)
    }
}
impl std::error::Error for Suspend {}

pub fn now_ms() -> i64 {
    chrono::Local::now().timestamp_millis()
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[allow(dead_code)] // Running is a database state, never an Outcome
pub enum Status {
    Running,
    Suspended,
    Done,
    Failed,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Running => "running",
            Status::Suspended => "suspended",
            Status::Done => "done",
            Status::Failed => "failed",
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)] // the daemon logs status; the rest is for callers
pub struct Outcome {
    pub status: Status,
    pub result: Option<Value>,
    pub error: Option<String>,
    pub wake_at: Option<i64>,
    pub note: Option<String>,
}

/// What a workflow body is handed.
///
/// Cheap to clone; the counters and the replay flag are shared, because a
/// workflow that splits work across helper functions still has to produce
/// one stable sequence of step keys.
#[derive(Clone)]
pub struct Ctx {
    pub run_id: String,
    /// Whatever the trigger passed in. Read by workflow bodies.
    #[allow(dead_code)]
    pub input: Value,
    db: Db,
    recorded: Arc<HashMap<String, Recorded>>,
    seen: Arc<Mutex<HashMap<String, u32>>>,
    replaying: Arc<Mutex<bool>>,
}

impl Ctx {
    /// The nth occurrence of a name within a run is its identity. Stable
    /// across replays precisely because the body is deterministic.
    fn next_key(&self, name: &str) -> String {
        let mut seen = self.seen.lock().expect("seen");
        let n = seen.entry(name.to_string()).or_insert(0);
        *n += 1;
        format!("{name}#{n}")
    }

    fn record(
        &self,
        key: &str,
        name: &str,
        result: Option<String>,
        error: Option<&str>,
    ) -> Result<()> {
        let conn = self.db.lock().expect("db");
        conn.execute(
            "INSERT OR REPLACE INTO steps (run_id, key, name, result, error, ended_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![self.run_id, key, name, result, error, now_ms()],
        )?;
        Ok(())
    }

    /// True while replaying recorded steps. For logging only.
    #[allow(dead_code)]
    pub fn replaying(&self) -> bool {
        *self.replaying.lock().expect("replaying")
    }

    /// Do something once, durably. The result is recorded; later invocations
    /// of this run return it without calling `f` again.
    pub async fn step<T, F, Fut>(&self, name: &str, f: F) -> Result<T>
    where
        T: Serialize + DeserializeOwned,
        F: FnOnce(String) -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let key = self.next_key(name);
        if let Some((result, error)) = self.recorded.get(&key) {
            if let Some(e) = error {
                return Err(anyhow!(e.clone()));
            }
            let raw = result.as_deref().unwrap_or("null");
            return Ok(serde_json::from_str(raw)?);
        }
        *self.replaying.lock().expect("replaying") = false;
        match f(key.clone()).await {
            Ok(value) => {
                self.record(&key, name, Some(serde_json::to_string(&value)?), None)?;
                Ok(value)
            }
            Err(e) => {
                // A Suspend is not a failure — it is the workflow parking. It
                // must not be recorded as a failed step, or the run could
                // never make progress again.
                if e.downcast_ref::<Suspend>().is_some() {
                    return Err(e);
                }
                self.record(&key, name, None, Some(&e.to_string()))?;
                Err(e)
            }
        }
    }

    /// Read the world, every time, recording nothing.
    ///
    /// The deliberate hole in the replay model, and agent workflows need it:
    /// `step("agent state", …)` memoizes the first answer, so a loop waiting
    /// for an agent to stop being `blocked` would see `blocked` forever.
    pub async fn poll<T, F, Fut>(&self, _name: &str, f: F) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        *self.replaying.lock().expect("replaying") = false;
        f().await
    }

    /// Park until at least `ms` after the first time this line was reached.
    pub async fn sleep(&self, name: &str, ms: i64) -> Result<()> {
        // The deadline is itself a step: computed once, replayed thereafter,
        // so a run does not restart its own timer every time it is poked.
        let deadline: i64 = self
            .step(
                &format!("sleep:{name}"),
                |_| async move { Ok(now_ms() + ms) },
            )
            .await?;
        if now_ms() < deadline {
            return Err(anyhow!(Suspend {
                wake_at: Some(deadline),
                note: format!("sleeping until {}", fmt_ms(deadline)),
            }));
        }
        Ok(())
    }

    /// Run a command under systemd-run and wait for it.
    ///
    /// Durable like `step`: a resumed run returns the recorded result rather
    /// than running the command again — the difference between replaying a
    /// push and doing it twice.
    pub async fn exec(
        &self,
        name: &str,
        cmd: Vec<String>,
        cwd: Option<String>,
    ) -> Result<crate::exec::Output> {
        let run_id = self.run_id.clone();
        self.step(name, move |key| async move {
            // Stable across replays because the step key is, which keeps the
            // transient unit name stable too.
            let slug: String = key
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
                .collect();
            let unit = format!("pm-{}-{}", &run_id[..8], slug);
            crate::exec::run(&unit, &cmd, cwd.as_deref()).await
        })
        .await
    }

    /// Park until a human releases this run.
    ///
    /// A park has to be *satisfiable*, or resuming is impossible: an
    /// unconditional throw would simply be reached again on the next
    /// invocation and park forever, which is what the first version did. So a
    /// park is a step, and its recorded value is the state of the wait —
    /// `"parked"` until something releases the run, `"released"` after. Replay
    /// then walks straight past it, exactly as it walks past any other step
    /// whose answer is already known.
    pub async fn park(&self, name: &str, note: &str) -> Result<()> {
        let key = self.next_key(name);
        if let Some((result, error)) = self.recorded.get(&key) {
            if let Some(e) = error {
                return Err(anyhow!(e.clone()));
            }
            if result.as_deref() == Some("\"released\"") {
                return Ok(());
            }
            // Still parked: re-park against the same note rather than
            // recording a second one.
            return Err(anyhow!(Suspend {
                wake_at: None,
                note: note.to_string()
            }));
        }
        *self.replaying.lock().expect("replaying") = false;
        self.record(&key, name, Some("\"parked\"".to_string()), None)?;
        Err(anyhow!(Suspend {
            wake_at: None,
            note: note.to_string()
        }))
    }
}

fn fmt_ms(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|t| {
            t.with_timezone(&chrono::Local)
                .format("%H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|| ms.to_string())
}

pub fn create_run(db: &Db, workflow: &str, input: Value) -> Result<String> {
    let id = uuid_v4();
    let t = now_ms();
    let conn = db.lock().expect("db");
    conn.execute(
        "INSERT INTO runs (id, workflow, input, status, created_at, updated_at)
         VALUES (?1, ?2, ?3, 'running', ?4, ?5)",
        params![id, workflow, input.to_string(), t, t],
    )?;
    Ok(id)
}

/// Invoke (or re-invoke) a run to its next parking point.
///
/// Safe to call repeatedly: recorded steps replay, so calling this on a
/// finished run is a no-op and calling it on an interrupted one picks up
/// where it stopped.
pub async fn run(db: &Db, registry: &Registry, run_id: &str) -> Result<Outcome> {
    let (workflow, input, status): (String, String, String) = {
        let conn = db.lock().expect("db");
        conn.query_row(
            "SELECT workflow, input, status FROM runs WHERE id = ?1",
            params![run_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?
        .ok_or_else(|| anyhow!("no such run {run_id}"))?
    };
    if status == "done" || status == "failed" {
        return Ok(Outcome {
            status: if status == "done" {
                Status::Done
            } else {
                Status::Failed
            },
            result: None,
            error: None,
            wake_at: None,
            note: None,
        });
    }

    let def = registry
        .get(workflow.as_str())
        .ok_or_else(|| anyhow!("run {run_id} names unknown workflow {workflow}"))?
        .clone();

    let recorded = {
        let conn = db.lock().expect("db");
        let mut stmt = conn.prepare("SELECT key, result, error FROM steps WHERE run_id = ?1")?;
        let rows = stmt.query_map(params![run_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                (
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ),
            ))
        })?;
        rows.collect::<rusqlite::Result<HashMap<_, _>>>()?
    };

    let ctx = Ctx {
        run_id: run_id.to_string(),
        input: serde_json::from_str(&input).unwrap_or(Value::Null),
        db: db.clone(),
        recorded: Arc::new(recorded),
        seen: Arc::new(Mutex::new(HashMap::new())),
        replaying: Arc::new(Mutex::new(true)),
    };

    match (def.run)(ctx).await {
        Ok(result) => {
            let conn = db.lock().expect("db");
            conn.execute(
                "UPDATE runs SET status='done', wake_at=NULL, note=NULL, updated_at=?1 WHERE id=?2",
                params![now_ms(), run_id],
            )?;
            Ok(Outcome {
                status: Status::Done,
                result: Some(result),
                error: None,
                wake_at: None,
                note: None,
            })
        }
        Err(e) => {
            if let Some(s) = e.downcast_ref::<Suspend>() {
                let conn = db.lock().expect("db");
                conn.execute(
                    "UPDATE runs SET status='suspended', wake_at=?1, note=?2, updated_at=?3 WHERE id=?4",
                    params![s.wake_at, s.note, now_ms(), run_id],
                )?;
                return Ok(Outcome {
                    status: Status::Suspended,
                    result: None,
                    error: None,
                    wake_at: s.wake_at,
                    note: Some(s.note.clone()),
                });
            }
            let message = format!("{e:?}");
            let conn = db.lock().expect("db");
            conn.execute(
                "UPDATE runs SET status='failed', error=?1, wake_at=NULL, updated_at=?2 WHERE id=?3",
                params![message, now_ms(), run_id],
            )?;
            Ok(Outcome {
                status: Status::Failed,
                result: None,
                error: Some(message),
                wake_at: None,
                note: None,
            })
        }
    }
}

/// Release every park in a run, so a resumed run walks past them.
///
/// All of them, not the newest: a run parked more than once is waiting on
/// everything it asked about, and a human saying "carry on" means all of it.
pub fn release_parks(db: &Db, run_id: &str) -> Result<usize> {
    let conn = db.lock().expect("db");
    let n = conn.execute(
        "UPDATE steps SET result = '\"released\"' WHERE run_id = ?1 AND result = '\"parked\"'",
        params![run_id],
    )?;
    conn.execute(
        "UPDATE runs SET status='running', wake_at=NULL, note=NULL, error=NULL, updated_at=?1 WHERE id=?2",
        params![now_ms(), run_id],
    )?;
    Ok(n)
}

/// Suspended runs whose deadline has passed. Parked-without-deadline runs are
/// deliberately excluded: only a trigger or an event moves those.
pub fn due(db: &Db, at: i64) -> Result<Vec<String>> {
    let conn = db.lock().expect("db");
    let mut stmt = conn.prepare(
        "SELECT id FROM runs WHERE status='suspended' AND wake_at IS NOT NULL AND wake_at <= ?1 ORDER BY wake_at",
    )?;
    let ids = stmt
        .query_map(params![at], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(ids)
}

/// A v4 UUID without pulling in a crate for it.
fn uuid_v4() -> String {
    let mut b = [0u8; 16];
    getrandom(&mut b);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

fn getrandom(buf: &mut [u8]) {
    use std::io::Read;
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(buf))
        .expect("/dev/urandom");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    fn mem() -> Db {
        Arc::new(Mutex::new(crate::db::open_in_memory().unwrap()))
    }

    fn reg(name: &'static str, run: Runner) -> Registry {
        let mut r = Registry::new();
        r.insert(
            name,
            WorkflowDef {
                description: "test",
                example: None,
                schedule: None,
                cwd: None,
                run,
            },
        );
        r
    }

    static CALLS: AtomicUsize = AtomicUsize::new(0);

    #[tokio::test]
    async fn step_executes_once_then_replays_from_the_database() {
        fn wf(ctx: Ctx) -> BoxFut<'static, Result<Value>> {
            Box::pin(async move {
                let n: usize = ctx
                    .step("count", |_| async {
                        Ok(CALLS.fetch_add(1, Ordering::SeqCst) + 1)
                    })
                    .await?;
                Ok(serde_json::json!(n))
            })
        }
        CALLS.store(0, Ordering::SeqCst);
        let db = mem();
        let r = reg("w", wf);
        let id = create_run(&db, "w", Value::Null).unwrap();
        assert_eq!(
            run(&db, &r, &id).await.unwrap().result,
            Some(serde_json::json!(1))
        );
        assert_eq!(CALLS.load(Ordering::SeqCst), 1);

        // Re-invoking a finished run must not re-execute anything. This is
        // the property the whole resume design rests on.
        run(&db, &r, &id).await.unwrap();
        assert_eq!(CALLS.load(Ordering::SeqCst), 1);
    }

    static BOOM: AtomicUsize = AtomicUsize::new(0);

    #[tokio::test]
    async fn a_failed_step_fails_the_run() {
        fn wf(ctx: Ctx) -> BoxFut<'static, Result<Value>> {
            Box::pin(async move {
                let _: () = ctx
                    .step("boom", |_| async {
                        BOOM.fetch_add(1, Ordering::SeqCst);
                        Err(anyhow!("nope"))
                    })
                    .await?;
                Ok(Value::Null)
            })
        }
        BOOM.store(0, Ordering::SeqCst);
        let db = mem();
        let out = {
            let r = reg("w", wf);
            let id = create_run(&db, "w", Value::Null).unwrap();
            run(&db, &r, &id).await.unwrap()
        };
        assert_eq!(out.status, Status::Failed);
        assert!(out.error.unwrap().contains("nope"));
        assert_eq!(BOOM.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn the_same_name_in_a_loop_makes_distinct_steps() {
        fn wf(ctx: Ctx) -> BoxFut<'static, Result<Value>> {
            Box::pin(async move {
                let mut seen = Vec::new();
                for i in 0..3u32 {
                    seen.push(ctx.step("tick", move |_| async move { Ok(i) }).await?);
                }
                Ok(serde_json::json!(seen))
            })
        }
        let db = mem();
        let r = reg("w", wf);
        let id = create_run(&db, "w", Value::Null).unwrap();
        let out = run(&db, &r, &id).await.unwrap();
        assert_eq!(out.result, Some(serde_json::json!([0, 1, 2])));

        let conn = db.lock().unwrap();
        let mut stmt = conn.prepare("SELECT key FROM steps ORDER BY key").unwrap();
        let keys: Vec<String> = stmt
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(keys, vec!["tick#1", "tick#2", "tick#3"]);
    }

    static DONE: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
    static EXPLODE: AtomicBool = AtomicBool::new(true);

    #[tokio::test]
    async fn work_done_before_a_crash_is_not_repeated_after_it() {
        fn wf(ctx: Ctx) -> BoxFut<'static, Result<Value>> {
            Box::pin(async move {
                ctx.step("fetch", |_| async {
                    DONE.lock().unwrap().push("fetch");
                    Ok(())
                })
                .await?;
                ctx.step("sweep", |_| async {
                    DONE.lock().unwrap().push("sweep");
                    Ok(())
                })
                .await?;
                if EXPLODE.load(Ordering::SeqCst) {
                    return Err(anyhow!("power cut"));
                }
                ctx.step("pr", |_| async {
                    DONE.lock().unwrap().push("pr");
                    Ok(())
                })
                .await?;
                Ok(serde_json::json!("complete"))
            })
        }
        DONE.lock().unwrap().clear();
        EXPLODE.store(true, Ordering::SeqCst);
        let db = mem();
        let r = reg("w", wf);
        let id = create_run(&db, "w", Value::Null).unwrap();

        assert_eq!(run(&db, &r, &id).await.unwrap().status, Status::Failed);
        assert_eq!(*DONE.lock().unwrap(), vec!["fetch", "sweep"]);

        // The supervisor resets a failed run for retry.
        db.lock()
            .unwrap()
            .execute(
                "UPDATE runs SET status='running', error=NULL WHERE id=?1",
                params![id],
            )
            .unwrap();
        EXPLODE.store(false, Ordering::SeqCst);

        let out = run(&db, &r, &id).await.unwrap();
        assert_eq!(out.status, Status::Done);
        // fetch and sweep did NOT run again: a fetch or an agent launch must
        // not happen twice because the box rebooted.
        assert_eq!(*DONE.lock().unwrap(), vec!["fetch", "sweep", "pr"]);
    }

    static AFTER: AtomicUsize = AtomicUsize::new(0);

    #[tokio::test]
    async fn sleep_suspends_and_does_not_restart_its_own_timer() {
        fn wf(ctx: Ctx) -> BoxFut<'static, Result<Value>> {
            Box::pin(async move {
                ctx.step("before", |_| async { Ok("b".to_string()) })
                    .await?;
                ctx.sleep("wait", 400).await?;
                let n = ctx
                    .step("after", |_| async {
                        Ok(AFTER.fetch_add(1, Ordering::SeqCst) + 1)
                    })
                    .await?;
                Ok(serde_json::json!(n))
            })
        }
        AFTER.store(0, Ordering::SeqCst);
        let db = mem();
        let r = reg("w", wf);
        let id = create_run(&db, "w", Value::Null).unwrap();

        let first = run(&db, &r, &id).await.unwrap();
        assert_eq!(first.status, Status::Suspended);
        assert!(due(&db, now_ms()).unwrap().is_empty());

        // Poking it early parks again against the ORIGINAL deadline.
        let early = run(&db, &r, &id).await.unwrap();
        assert_eq!(early.wake_at, first.wake_at);

        tokio::time::sleep(std::time::Duration::from_millis(450)).await;
        assert_eq!(due(&db, now_ms()).unwrap(), vec![id.clone()]);
        assert_eq!(run(&db, &r, &id).await.unwrap().status, Status::Done);
        assert_eq!(AFTER.load(Ordering::SeqCst), 1);
    }

    static STATE: Mutex<String> = Mutex::new(String::new());

    #[tokio::test]
    async fn poll_rereads_the_world_so_a_wait_loop_can_end() {
        fn wf(ctx: Ctx) -> BoxFut<'static, Result<Value>> {
            Box::pin(async move {
                loop {
                    let s: String = ctx
                        .poll("agent", || async { Ok(STATE.lock().unwrap().clone()) })
                        .await?;
                    if s != "blocked" {
                        return Ok(serde_json::json!(s));
                    }
                    ctx.sleep("retry", 100).await?;
                }
            })
        }
        *STATE.lock().unwrap() = "blocked".to_string();
        let db = mem();
        let r = reg("w", wf);
        let id = create_run(&db, "w", Value::Null).unwrap();
        assert_eq!(run(&db, &r, &id).await.unwrap().status, Status::Suspended);

        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        assert_eq!(run(&db, &r, &id).await.unwrap().status, Status::Suspended);

        *STATE.lock().unwrap() = "idle".to_string();
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        let out = run(&db, &r, &id).await.unwrap();
        assert_eq!(out.status, Status::Done);
        assert_eq!(out.result, Some(serde_json::json!("idle")));
    }

    static PAST_PARK: AtomicUsize = AtomicUsize::new(0);

    #[tokio::test]
    async fn a_released_park_lets_the_run_continue() {
        // The whole point of park: a human answers, and the run carries on
        // from where it stopped. Without release this loops forever — an
        // unconditional park is reached again on every invocation, which is
        // exactly what the first version did and why resume was impossible.
        fn wf(ctx: Ctx) -> BoxFut<'static, Result<Value>> {
            Box::pin(async move {
                ctx.step("before", |_| async { Ok("b".to_string()) })
                    .await?;
                ctx.park("human", "waiting for someone").await?;
                let n = ctx
                    .step("after", |_| async {
                        Ok(PAST_PARK.fetch_add(1, Ordering::SeqCst) + 1)
                    })
                    .await?;
                Ok(serde_json::json!(n))
            })
        }
        PAST_PARK.store(0, Ordering::SeqCst);
        let db = mem();
        let r = reg("w", wf);
        let id = create_run(&db, "w", Value::Null).unwrap();

        let first = run(&db, &r, &id).await.unwrap();
        assert_eq!(first.status, Status::Suspended);
        assert_eq!(first.wake_at, None);
        assert_eq!(PAST_PARK.load(Ordering::SeqCst), 0);

        // Re-driving without releasing must park again, not slip past.
        assert_eq!(run(&db, &r, &id).await.unwrap().status, Status::Suspended);
        assert_eq!(PAST_PARK.load(Ordering::SeqCst), 0);

        assert_eq!(release_parks(&db, &id).unwrap(), 1);
        let done = run(&db, &r, &id).await.unwrap();
        assert_eq!(done.status, Status::Done);
        assert_eq!(done.result, Some(serde_json::json!(1)));
        // Work before the park replayed rather than repeating.
        assert_eq!(PAST_PARK.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn park_is_never_due() {
        fn wf(ctx: Ctx) -> BoxFut<'static, Result<Value>> {
            Box::pin(async move {
                ctx.step("get ready", |_| async { Ok("ready".to_string()) })
                    .await?;
                ctx.park("human", "waiting for a human").await?;
                Ok(Value::Null)
            })
        }
        let db = mem();
        let r = reg("w", wf);
        let id = create_run(&db, "w", Value::Null).unwrap();
        let out = run(&db, &r, &id).await.unwrap();
        assert_eq!(out.status, Status::Suspended);
        assert_eq!(out.wake_at, None);
        assert_eq!(out.note.as_deref(), Some("waiting for a human"));
        assert!(due(&db, now_ms() + 86_400_000).unwrap().is_empty());
    }
}
