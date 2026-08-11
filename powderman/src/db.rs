//! The whole persistent state of the engine, which is two tables and a bit.
//!
//! A run is a workflow invocation. A step is one recorded decision inside it.
//! Resuming is not a special code path: the runner re-invokes the workflow
//! from the top, and every step that already has a row returns its recorded
//! result instead of executing. The steps table *is* the resume mechanism.

use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::Path;

pub fn open(path: &Path) -> Result<Connection> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).ok();
    }
    let conn = Connection::open(path).with_context(|| format!("opening {}", path.display()))?;
    prepare(&conn)?;
    crate::metrics::migrate(&conn)?;
    Ok(conn)
}

/// Used by the engine tests; kept out of the daemon path deliberately.
#[allow(dead_code)]
pub fn open_in_memory() -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    prepare(&conn)?;
    crate::metrics::migrate(&conn)?;
    Ok(conn)
}

fn prepare(conn: &Connection) -> Result<()> {
    // WAL so a reader (the UI) never blocks the writer. There is still
    // exactly one writer process by design — the daemon owns this file.
    conn.pragma_update(None, "journal_mode", "WAL").ok();
    conn.pragma_update(None, "busy_timeout", 5000)?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS runs (
          id         TEXT PRIMARY KEY,
          workflow   TEXT NOT NULL,
          input      TEXT NOT NULL DEFAULT 'null',
          status     TEXT NOT NULL,
          wake_at    INTEGER,
          note       TEXT,
          pane_id    TEXT,
          error      TEXT,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS steps (
          run_id   TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
          key      TEXT NOT NULL,
          name     TEXT NOT NULL,
          result   TEXT,
          error    TEXT,
          ended_at INTEGER NOT NULL,
          PRIMARY KEY (run_id, key)
        );

        -- The scheduler's only query: what is due?
        CREATE INDEX IF NOT EXISTS runs_due
          ON runs (status, wake_at) WHERE status = 'suspended';

        -- Small singletons: the workbench layout lives here as JSON. A table
        -- per singleton would be schema noise.
        CREATE TABLE IF NOT EXISTS kv (
          key   TEXT PRIMARY KEY,
          value TEXT NOT NULL
        );

        -- The last wall-clock minute each schedule fired in, so neither a
        -- tick that runs twice in a minute nor a daemon restart can
        -- double-fire one.
        CREATE TABLE IF NOT EXISTS schedules (
          workflow    TEXT PRIMARY KEY,
          expression  TEXT NOT NULL,
          last_minute INTEGER
        );
        "#,
    )?;
    Ok(())
}
