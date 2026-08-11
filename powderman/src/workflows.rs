//! The registry. Adding a workflow is adding an entry to `registry()`.
//!
//! A workflow is a `fn(Ctx) -> BoxFut<Result<Value>>` — a fn pointer, not a
//! closure, because the database stores `workflow = "daily"` and something
//! has to resolve that name back to code after a restart.

use anyhow::Result;
use serde_json::{Value, json};

use crate::engine::{BoxFut, Ctx, Registry, WorkflowDef};
use crate::herdr;

pub fn registry() -> Registry {
    let mut r = Registry::new();
    r.insert(
        "demo",
        WorkflowDef {
            description: "Steps, a twenty-second park, then more steps — watch a run suspend and resume.",
            example: None,
            schedule: None,
            cwd: None,
            run: demo,
        },
    );
    // Unscheduled. It ran every five minutes and buried everything else: 12
    // rows an hour of a demo workflow, against one real run a day. A demo
    // earns a button, not a place in the history.
    r.insert(
        "fleet",
        WorkflowDef {
            description: "Read the live herdr fleet over the socket and summarise it.",
            example: None,
            schedule: None,
            cwd: None,
            run: fleet,
        },
    );
    r.insert(
        "shell",
        WorkflowDef {
            description: "Run three commands under systemd-run, one of them failing on purpose.",
            example: None,
            schedule: None,
            cwd: None,
            run: shell,
        },
    );
    r.insert(
        "needs_a_human",
        WorkflowDef {
            description: "Park indefinitely. Use the resume button to release it.",
            example: None,
            schedule: None,
            cwd: None,
            run: needs_a_human,
        },
    );
    r.insert(
        "agent",
        WorkflowDef {
            description: "Start a Claude agent in its own herdr workspace, prompt it, wait for it to settle.",
            example: None,
            schedule: None,
            cwd: None,
            run: agent,
        },
    );
    // 06:00, the hour scripts/daily.sh used to own. The sweep hands each
    // language with gaps to its own treebank_fix run.
    r.insert(
        "treebank_sweep",
        WorkflowDef {
            description: "Pull, build, then rank/fetch/materialize/sweep every grammar; hand any with gaps to a fix run.",
            example: Some("{\"limit\": 100}"),
            schedule: Some("0 6 * * *"),
            cwd: None,
            run: crate::treebank::sweep_all,
        },
    );
    // Triggered with {"lang": "rust"} — a fix run is long, can park for hours
    // on a human, and should be startable for one language on its own.
    r.insert(
        "treebank_fix",
        WorkflowDef {
            description: "Give one language's grammar gaps to an agent in a worktree, verify, and open a stacked PR.",
            example: Some("{\"lang\": \"rust\"}"),
            schedule: None,
            cwd: None,
            run: crate::treebank::fix,
        },
    );
    r
}

/// Steps, a park on a timer, then more steps. Watch it suspend and resume.
fn demo(ctx: Ctx) -> BoxFut<'static, Result<Value>> {
    Box::pin(async move {
        let a: i64 = ctx.step("pick a number", |_| async { Ok(42) }).await?;
        ctx.step("think about it", move |_| async move {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            Ok(format!("{a} is the number"))
        })
        .await?;
        ctx.sleep("wait a bit", 20_000).await?;
        let b: i64 = ctx
            .step("double it", move |_| async move { Ok(a * 2) })
            .await?;
        Ok(json!({ "a": a, "b": b }))
    })
}

/// Reads the live fleet over the herdr socket.
///
/// The SDK is synchronous, so the call goes through spawn_blocking rather
/// than stalling a tokio worker on a blocking read.
fn fleet(ctx: Ctx) -> BoxFut<'static, Result<Value>> {
    Box::pin(async move {
        let live: Vec<Value> = ctx
            .step("agent.list", |_| async {
                let agents = tokio::task::spawn_blocking(herdr::agents).await??;
                Ok(agents
                    .into_iter()
                    .map(|a| json!({ "name": a.name, "status": a.status(), "cwd": a.cwd }))
                    .collect())
            })
            .await?;
        let busy = live.iter().filter(|a| a["status"] == "working").count();
        let n = live.len();
        ctx.step("summarise", move |_| async move {
            Ok(format!("{n} agents, {busy} working"))
        })
        .await?;
        Ok(json!(live))
    })
}

/// Real commands under systemd-run: real exit codes, journald, no helper.
fn shell(ctx: Ctx) -> BoxFut<'static, Result<Value>> {
    Box::pin(async move {
        let who = ctx.exec("whoami", vec!["whoami".into()], None).await?;
        let files = ctx
            .exec(
                "count files",
                vec!["bash".into(), "-lc".into(), "ls -1 ~ | wc -l".into()],
                None,
            )
            .await?;
        // A failing command is a result, not an exception: the exit code and
        // both streams come back and the workflow decides what it means.
        let missing = ctx
            .exec(
                "deliberate failure",
                vec!["ls".into(), "/no/such/path".into()],
                None,
            )
            .await?;
        Ok(json!({
            "user": who.stdout.trim(),
            "home_entries": files.stdout.trim(),
            "failed_as_expected": !missing.ok(),
            "exit_code": missing.code,
            "unit": missing.unit,
        }))
    })
}

/// Parks forever, so the UI has something in the state the design exists for.
fn needs_a_human(ctx: Ctx) -> BoxFut<'static, Result<Value>> {
    Box::pin(async move {
        ctx.step("get ready", |_| async { Ok("ready".to_string()) })
            .await?;
        ctx.park("human", "waiting for someone to answer").await?;
        Ok(json!("released"))
    })
}

/// Launch a coding agent in herdr and wait for it to settle.
///
/// This is the case the whole design exists for. Under `--permission-mode
/// auto` an agent stops and waits for a person, and that wait is measured in
/// someone's sleep — so the workflow parks between checks rather than holding
/// a process, and every check is a `poll` rather than a `step`, because a
/// recorded status would replay as `blocked` forever and the loop would never
/// end.
///
/// The agent outlives this run. That is deliberate: if powderman restarts, the
/// agent keeps working and the resumed run finds it by name.
fn agent(ctx: Ctx) -> BoxFut<'static, Result<Value>> {
    Box::pin(async move {
        let name = format!("pm-{}", &ctx.run_id[..8]);
        // Where the agent starts matters more than it looks. Claude asks
        // "do you trust this folder?" the first time it sees a directory, and
        // reports as `blocked` until a human answers — so an unattended
        // workflow should start somewhere already trusted. That is a safety
        // feature, not an obstacle to engineer around: the first run in a new
        // directory will always need one person, once.
        let cwd = std::env::var("HOME")
            .map(|h| format!("{h}/powderworks"))
            .unwrap_or_else(|_| "/tmp".into());

        // Creating the workspace is a step: replaying it would open a second
        // one every time the run was re-invoked.
        let n = name.clone();
        let c = cwd.clone();
        let pane: String = ctx
            .step("workspace", move |_| async move {
                tokio::task::spawn_blocking(move || {
                    herdr::open_workspace(&format!("agent {n}"), &c)
                })
                .await?
            })
            .await?;

        let n = name.clone();
        // Wait for the pane to become an available shell — INSIDE a step.
        //
        // This looked like a job for a poll loop in the body, and that is a
        // trap. `poll` re-reads the world on every invocation, so a loop built
        // on it takes a different number of iterations each replay. Once the
        // agent is running, the pane stops being an "available shell" — the
        // very condition being waited on inverts — so on the next replay the
        // loop never exits and the run can never reach its own recorded
        // `start agent` step. It spins until the timeout, forever.
        //
        // Recorded once, it replays as "ready" and the run moves on. A bounded
        // wait belongs inside a step; `sleep` and `park` are for the long ones.
        let p = pane.clone();
        ctx.step("pane ready", move |_| async move {
            for _ in 0..30 {
                let p2 = p.clone();
                if tokio::task::spawn_blocking(move || herdr::pane_ready(&p2)).await?? {
                    return Ok(true);
                }
                tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
            }
            Err(anyhow::anyhow!("pane never became an available shell"))
        })
        .await?;

        let cleanup = pane.clone();
        let start_pane = pane.clone();
        let started = ctx
            .step("start agent", move |_| async move {
                let args = vec![
                    "--permission-mode".to_string(),
                    "auto".to_string(),
                    "--remote-control".to_string(),
                    n.clone(),
                ];
                tokio::task::spawn_blocking(move || {
                    herdr::start_agent(&n, "claude", &start_pane, &args)
                })
                .await??;
                Ok(true)
            })
            .await;
        if started.is_err() {
            let _ = tokio::task::spawn_blocking(move || herdr::close_workspace(&cleanup)).await;
            started?;
        }

        // `agent.start` returning does NOT mean the agent can be prompted.
        // It returns once herdr sees the process, but a fresh agent in a new
        // directory sits on its own startup modal — Claude's "do you trust
        // this folder?" — and herdr reports that as `blocked`. Prompting then
        // fails with "not an active named agent". So wait for readiness, and
        // treat a startup block as what it is: a person needs to answer it.
        // `agent.start` returns immediately with the agent `launch_pending`,
        // and it stays that way until something polls `agent.get` for it —
        // see herdr::agent_get. Until then `agent.prompt` refuses the agent as
        // "not an active named agent" even though it is sitting idle at its
        // prompt. Recorded as a step, so a replay does not wait again.
        let n = name.clone();
        ctx.step("wait until ready", move |_| async move {
            tokio::task::spawn_blocking(move || {
                herdr::wait_until_ready(&n, std::time::Duration::from_secs(120))
            })
            .await??;
            Ok(true)
        })
        .await?;

        let n = name.clone();
        ctx.step("prompt", move |_| async move {
            let text = "Reply with one short sentence describing what directory you are in, \
                        then stop. Do not create or modify any files.";
            tokio::task::spawn_blocking(move || herdr::prompt_agent(&n, text)).await??;
            Ok(true)
        })
        .await?;

        let mut waited = 0i64;
        loop {
            let n = name.clone();
            let status: Option<String> = ctx
                .poll("agent status", move || async move {
                    tokio::task::spawn_blocking(move || herdr::agent_status(&n)).await?
                })
                .await?;

            match status.as_deref() {
                // Gone means it exited or was never detected; either way there
                // is nothing left to wait for.
                None => break,
                Some("idle") | Some("done") => break,
                Some("blocked") => {
                    // A person is being waited on. Say so and keep checking —
                    // parking here would need something to un-park it, and
                    // re-checking costs one socket call.
                    println!("agent {name} is blocked — a human needs to answer it");
                }
                _ => {}
            }
            if waited > 600_000 {
                ctx.park("stuck agent", "agent has not settled in ten minutes")
                    .await?;
            }
            ctx.sleep("check again", 10_000).await?;
            waited += 10_000;
        }

        let n = name.clone();
        let final_status: Option<String> = ctx
            .step("final status", move |_| async move {
                tokio::task::spawn_blocking(move || herdr::agent_status(&n)).await?
            })
            .await?;

        // Tidy up only on a clean finish: a workspace left standing is how a
        // person inspects what went wrong.
        let p = pane.clone();
        ctx.step("close workspace", move |_| async move {
            tokio::task::spawn_blocking(move || herdr::close_workspace(&p)).await??;
            Ok(true)
        })
        .await?;

        Ok(json!({ "agent": name, "settled_as": final_status }))
    })
}

/// The command registry: the library's built-in layout commands plus
/// powderman's own. `open_run` splits an area and points the new half at a
/// run — a compound the UI needs and an agent, later, gets for free.
pub fn commands() -> immersion::Commands {
    fn open_run(ws: &mut immersion::Workspaces, p: &serde_json::Value) -> anyhow::Result<()> {
        let area = p
            .get("area")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| anyhow::anyhow!("open_run needs an integer area"))?;
        let run = p
            .get("run")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("open_run needs a run id"))?;
        let l = ws.current_layout_mut();
        if let Some(new) = l.split(area, immersion::Dir::Row, 0.5) {
            l.set_editor_arg(new, "run", run);
        }
        Ok(())
    }
    immersion::Commands::builtin().with(immersion::Command {
        name: "open_run",
        description: "Open a run in a new area beside the list",
        navigational: false,
        run: open_run,
    })
}
