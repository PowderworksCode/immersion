//! herdr, for the half of the work a person might want to take over.
//!
//! Thin on purpose. herdr-sdk already types the socket; this adds only what
//! powderman needs, plus one thing the SDK cannot do for us: it resolves the
//! socket from `HERDR_SOCKET_PATH` with no fallback, so a daemon started by
//! systemd — which inherits none of a login shell's environment — must be
//! told where the socket is. `ensure_socket_env` does that once at startup so
//! every later call works, rather than each one failing with "not running
//! under Herdr".

use anyhow::{Context, Result};
use herdr_sdk::Client;
use serde_json::json;

/// herdr's default socket for the default session.
fn default_socket() -> String {
    format!(
        "{}/.config/herdr/herdr.sock",
        std::env::var("HOME").unwrap_or_default()
    )
}

/// Point herdr-sdk at a socket if the environment has not already.
pub fn ensure_socket_env() {
    if std::env::var_os("HERDR_SOCKET_PATH").is_none() {
        // Safety: called once, at startup, before any threads are spawned.
        unsafe { std::env::set_var("HERDR_SOCKET_PATH", default_socket()) };
    }
}

pub fn client() -> Result<Client> {
    Client::connect().context("connecting to herdr — is the server running?")
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Pong {
    pub version: String,
    pub protocol: u32,
}

pub fn ping() -> Result<Pong> {
    client()?.call::<Pong>("ping", json!({}))
}

/// What powderman needs to know about an agent.
///
/// Not `herdr_sdk::model::Agent`: that struct omits `name`, and the name is
/// the only stable handle we have — it is what a workflow chose, it survives
/// a pane moving, and it is what `agent.get` takes as a target. The SDK models
/// what a plugin needs; a scheduler needs the key.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentInfo {
    pub name: Option<String>,
    /// Whether herdr considers the agent ready to receive input.
    ///
    /// Distinct from `agent_status`. An agent can report `idle` while still
    /// booting, and `agent.prompt` then refuses it with "not an active named
    /// agent" — a confusing error, since the agent plainly exists and is in
    /// the list. This is the field that actually answers "can I prompt it".
    #[serde(default)]
    pub interactive_ready: Option<bool>,
    pub agent: Option<String>,
    pub agent_status: Option<String>,
    pub pane_id: String,
    pub workspace_id: String,
    pub cwd: Option<String>,
}

impl AgentInfo {
    pub fn status(&self) -> &str {
        self.agent_status.as_deref().unwrap_or("unknown")
    }
}

#[derive(serde::Deserialize)]
struct AgentList {
    #[serde(default)]
    agents: Vec<AgentInfo>,
}

pub fn agents() -> Result<Vec<AgentInfo>> {
    Ok(client()?.call::<AgentList>("agent.list", json!({}))?.agents)
}

#[derive(serde::Deserialize)]
struct WorkspaceCreated {
    root_pane: RootPane,
}
#[derive(serde::Deserialize)]
struct RootPane {
    pane_id: String,
}

/// A workspace for a run's interactive work. Returns its root pane id.
pub fn open_workspace(label: &str, cwd: &str) -> Result<String> {
    let created = client()?.call::<WorkspaceCreated>(
        "workspace.create",
        json!({ "label": label, "cwd": cwd, "focus": false }),
    )?;
    Ok(created.root_pane.pane_id)
}

pub fn close_workspace(pane_id: &str) -> Result<()> {
    let workspace_id = pane_id.split(':').next().unwrap_or(pane_id).to_string();
    client()?
        .call::<serde_json::Value>("workspace.close", json!({ "workspace_id": workspace_id }))?;
    Ok(())
}

/// Is this pane an *available shell* — at its own prompt, with nothing else
/// in the foreground?
///
/// `agent.start` requires one and refuses with "is not an available shell"
/// otherwise. A freshly created workspace is not one for the first fraction
/// of a second: `workspace.create` returns when the pane exists, which is
/// before its shell has exec'd. Polling this closes that race.
///
/// Note the nesting: the fields live under `process_info`, not at the top of
/// the result. Reading them from the top yields `None` for everything, which
/// looks exactly like a pane that is never ready.
pub fn pane_ready(pane_id: &str) -> Result<bool> {
    #[derive(serde::Deserialize)]
    struct Resp {
        process_info: Info,
    }
    #[derive(serde::Deserialize)]
    struct Info {
        shell_pid: Option<u32>,
        foreground_process_group_id: Option<u32>,
    }
    let r: Resp = client()?.call("pane.process_info", json!({ "pane_id": pane_id }))?;
    Ok(
        match (
            r.process_info.shell_pid,
            r.process_info.foreground_process_group_id,
        ) {
            // The shell is its own foreground process group: nothing is running.
            (Some(shell), Some(fg)) => shell == fg,
            // A shell with no foreground command at all is also available.
            (Some(_), None) => true,
            _ => false,
        },
    )
}

/// A git worktree with a herdr workspace on it, in one call.
///
/// Returns `(workspace_id, pane_id, checkout_path)`. herdr puts the worktree
/// under `~/.herdr/worktrees/<repo>/<branch-slug>` — outside the repo, which
/// matters: a worktree nested inside its own checkout leaves that checkout
/// permanently dirty.
pub fn create_worktree(
    repo: &str,
    branch: &str,
    base: &str,
    label: &str,
    path: &str,
) -> Result<(String, String, String)> {
    #[derive(serde::Deserialize)]
    struct Created {
        workspace: Workspace,
        root_pane: RootPane,
    }
    #[derive(serde::Deserialize)]
    struct Workspace {
        workspace_id: String,
        worktree: Worktree,
    }
    #[derive(serde::Deserialize)]
    struct Worktree {
        checkout_path: String,
    }
    let c: Created = client()?.call(
        "worktree.create",
        json!({
            "cwd": repo, "branch": branch, "base": base,
            "label": label, "focus": false, "path": path,
        }),
    )?;
    Ok((
        c.workspace.workspace_id,
        c.root_pane.pane_id,
        c.workspace.worktree.checkout_path,
    ))
}

/// The pids herdr sees in a pane: its shell plus anything in the foreground.
///
/// Agents have no cgroup of their own — a claude process reports
/// `0::/init.scope` because it belongs to the herdr server's scope — so this
/// is the only way to attribute CPU and memory to a named agent.
pub fn pane_procs(pane_id: &str) -> Result<Vec<(u32, String)>> {
    #[derive(serde::Deserialize)]
    struct Resp {
        process_info: Info,
    }
    #[derive(serde::Deserialize)]
    struct Info {
        shell_pid: Option<u32>,
        #[serde(default)]
        foreground_processes: Vec<Proc>,
    }
    #[derive(serde::Deserialize)]
    struct Proc {
        pid: u32,
        name: String,
    }
    let r: Resp = client()?.call("pane.process_info", json!({ "pane_id": pane_id }))?;
    let mut out: Vec<(u32, String)> = r
        .process_info
        .foreground_processes
        .into_iter()
        .map(|p| (p.pid, p.name))
        .collect();
    if let Some(shell) = r.process_info.shell_pid
        && !out.iter().any(|(p, _)| *p == shell)
    {
        out.push((shell, "shell".to_string()));
    }
    Ok(out)
}

pub fn pane_pids(pane_id: &str) -> Result<Vec<u32>> {
    Ok(pane_procs(pane_id)?.into_iter().map(|(p, _)| p).collect())
}

/// Start an agent in an existing shell pane.
///
/// The parameter is `pane_id`, not `pane` — the CLI's flag is `--pane` and
/// guessing from it costs a round trip that fails as "connection closed with
/// no response", because a params-level rejection never gets far enough to
/// produce an error message. The schema is the authority, not the CLI.
/// `timeout_ms` is not a safety net, it is the readiness gate.
///
/// Without it `agent.start` returns as soon as the process is spawned, and the
/// name is not yet bound: `agent.prompt` then fails with `agent_not_ready` —
/// "not an active named agent" — even though the agent is visibly sitting at
/// its prompt and appears in `agent.list` under that very name. With it, the
/// call blocks until herdr has bound the name and considers the agent ready
/// for input, which is the thing a caller actually wants to wait for.
pub fn start_agent(name: &str, kind: &str, pane_id: &str, args: &[String]) -> Result<()> {
    // Retry "not an available shell" rather than trying to predict it.
    //
    // pane_ready() checks that the shell is its own foreground process group,
    // which is necessary but not sufficient: herdr wants more before it calls
    // a pane available, and a check that returns true can still be followed by
    // a start that is refused. Predicting another process's readiness is a
    // losing game; asking again a moment later is not.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        let err = match client()?.call::<serde_json::Value>(
            "agent.start",
            json!({
                "name": name,
                "kind": kind,
                "pane_id": pane_id,
                "args": args,
                "timeout_ms": 120_000,
            }),
        ) {
            Ok(_) => return Ok(()),
            Err(e) => e,
        };
        if !err.to_string().contains("not an available shell")
            || std::time::Instant::now() >= deadline
        {
            return Err(err);
        }
        std::thread::sleep(std::time::Duration::from_millis(1000));
    }
}

pub fn prompt_agent(target: &str, text: &str) -> Result<()> {
    client()?
        .call::<serde_json::Value>("agent.prompt", json!({ "target": target, "text": text }))?;
    Ok(())
}

/// An agent's lifecycle state, or `None` if there is no such live agent.
///
/// This is what nothing else on the box can tell you, and the reason agents
/// live in herdr rather than under systemd: `blocked` means a human is being
/// waited on.
pub fn agent_status(name: &str) -> Result<Option<String>> {
    Ok(agents()?
        .into_iter()
        .find(|a| a.name.as_deref() == Some(name))
        .map(|a| a.status().to_string()))
}

/// Fetch one agent by name, via `agent.get`.
///
/// Not a convenience over `agent.list` — the two do not agree. A freshly
/// started agent carries `launch_pending: true` and no `interactive_ready`,
/// and `agent.list` shows it that way forever no matter how long you watch.
/// Polling `agent.get` with the name as target is what settles it: within a
/// few seconds `launch_pending` disappears, `interactive_ready` becomes true,
/// and only then will `agent.prompt` accept the agent instead of refusing it
/// with "not an active named agent".
///
/// herdr's own CLI does exactly this — `agent.start` then `agent.get` in a
/// loop — which is why starting an agent from the CLI works and starting one
/// from a naive client does not.
pub fn agent_get(name: &str) -> Result<Option<AgentInfo>> {
    #[derive(serde::Deserialize)]
    struct Got {
        agent: AgentInfo,
    }
    match client()?.call::<Got>("agent.get", json!({ "target": name })) {
        Ok(g) => Ok(Some(g.agent)),
        // "no such agent" is an ordinary answer, not a failure.
        Err(_) => Ok(None),
    }
}

/// Block until herdr will accept prompts for this agent.
pub fn wait_until_ready(name: &str, timeout: std::time::Duration) -> Result<()> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        match agent_get(name)? {
            Some(a) if a.interactive_ready == Some(true) => return Ok(()),
            Some(a) if a.status() == "blocked" => {
                println!("agent {name}: blocked — a human needs to answer it")
            }
            _ => {}
        }
        if std::time::Instant::now() >= deadline {
            anyhow::bail!("agent {name} never became ready for input");
        }
        std::thread::sleep(std::time::Duration::from_millis(1500));
    }
}

/// `(status, ready_for_input)` for a live agent, or `None` if there is none.
///
/// Note `interactive_ready` is not always reported — it was absent for an
/// agent that was plainly idle at its prompt — so do not gate on it. Use
/// `agent.start`'s timeout to wait for readiness instead.
#[allow(dead_code)]
pub fn agent_state(name: &str) -> Result<Option<(String, bool)>> {
    Ok(agents()?
        .into_iter()
        .find(|a| a.name.as_deref() == Some(name))
        .map(|a| (a.status().to_string(), a.interactive_ready.unwrap_or(false))))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Live, read-only, and skipped when no server is reachable.
    fn live() -> bool {
        ensure_socket_env();
        ping().is_ok()
    }

    #[test]
    fn ping_reports_a_version_and_protocol() {
        if !live() {
            eprintln!("herdr not reachable — skipping");
            return;
        }
        let pong = ping().unwrap();
        assert!(!pong.version.is_empty());
        assert!(pong.protocol > 0);
    }

    #[test]
    fn agent_list_returns_the_live_fleet() {
        if !live() {
            return;
        }
        for a in agents().unwrap() {
            assert!(!a.pane_id.is_empty());
            // The field the SDK's own Agent lacks, and the reason for AgentInfo.
            assert!(a.name.is_some(), "agent.list should carry a name");
        }
    }

    #[test]
    fn a_name_that_is_not_running_has_no_status() {
        if !live() {
            return;
        }
        assert_eq!(agent_status("definitely-not-an-agent").unwrap(), None);
    }
}
