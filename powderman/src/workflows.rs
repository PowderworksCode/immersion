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
    /// Select a thing, and point every unpinned area of its kind at whichever
    /// is now active.
    ///
    /// Blender's distinction: many things selected, exactly one active. A
    /// plain click replaces the selection, ctrl-click extends it, and the last
    /// one picked is the active one — the thing a detail pane should show.
    /// Selection is what an operation applies *across*; active is what you are
    /// looking *at*.
    ///
    /// The pointing is still a write into the layout rather than a lookup when
    /// an area is drawn, so the document keeps saying exactly what each area
    /// shows and every reader of it needs no new concept.
    fn select(ws: &mut immersion::Workspaces, p: &serde_json::Value) -> anyhow::Result<()> {
        let kind = p
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("select needs a kind, e.g. \"file\""))?
            .to_string();
        let value = p
            .get("value")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("select needs a value"))?
            .to_string();
        let how = immersion::Pick::parse(
            p.get("mode")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("replace"),
        );

        let index = ws.active;
        let active = ws.tabs[index].pick(&kind, &value, how);
        // Toggling the last selected thing off leaves nothing active. The
        // areas keep showing what they had rather than blanking: "nothing is
        // selected" is not the same statement as "look at nothing", and a
        // detail pane that empties itself when you deselect is a pane that
        // loses your place.
        let Some(active) = active else {
            return Ok(());
        };
        let layout = &mut ws.tabs[index].layout;
        let followers: Vec<immersion::AreaId> = layout
            .root
            .leaves()
            .into_iter()
            .filter(|id| !layout.is_pinned(*id))
            .filter(|id| match layout.root.find(*id) {
                Some(immersion::Area::Leaf { editor, .. }) => {
                    crate::editors::target_kind(editor) == Some(kind.as_str())
                }
                _ => false,
            })
            .collect();
        for id in &followers {
            layout.set_target(*id, &active);
        }
        // Nothing followed is not an error: selecting a file in a workspace
        // with no viewer open is a reasonable thing to do, and saying so
        // would be noise on an ordinary click.
        Ok(())
    }

    /// Freeze an area on what it is looking at, or let it follow again.
    fn set_pinned(ws: &mut immersion::Workspaces, p: &serde_json::Value) -> anyhow::Result<()> {
        let id = p
            .get("id")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| anyhow::anyhow!("set_pinned needs an integer id"))?;
        let pinned = p
            .get("pinned")
            .and_then(serde_json::Value::as_bool)
            .ok_or_else(|| anyhow::anyhow!("set_pinned needs a boolean pinned"))?;
        if !ws.current_layout_mut().set_pinned(id, pinned) {
            anyhow::bail!("no area {id}");
        }
        Ok(())
    }

    immersion::Commands::builtin()
        .with(immersion::Command {
            name: "open_run",
            description: "Open a run in a new area beside the list",
            navigational: false,
            // Always: any area can be split to hold a run, and whether the run
            // exists is the run function's business.
            poll: immersion::always,
            run: open_run,
        })
        .with(immersion::Command {
            name: "select",
            description: "Point every unpinned area of a kind (file, run, chart) at one thing",
            // Navigational: a click in a list is not an edit, and putting one
            // on the undo stack per click would bury the splits and joins
            // that are.
            navigational: true,
            // Always, deliberately. Selecting a file in a workspace with no
            // viewer open writes nothing, and a click that reports an error
            // for doing something reasonable is worse than one that quietly
            // does nothing — see the test that pins this.
            poll: immersion::always,
            run: select,
        })
        .with(immersion::Command {
            name: "set_pinned",
            description: "Freeze an area on what it is showing, or let it follow the selection",
            navigational: false,
            // Always: poll answers about the workbench, not about params. A
            // bad id is the run function's error, with its own message.
            poll: immersion::always,
            run: set_pinned,
        })
}

#[cfg(test)]
mod linked_targets {
    use immersion::{Area, Dir, Layout, Workspaces};
    use serde_json::json;

    /// A workspace with a browser, a code viewer and a diff viewer — the
    /// Code and Changes arrangements, in one tree.
    fn workbench() -> Workspaces {
        let mut l = Layout::single("files");
        let code = l.split(1, Dir::Row, 0.3).expect("a second area");
        l.set_editor(code, "code");
        let diff = l.split(code, Dir::Col, 0.5).expect("a third area");
        l.set_editor(diff, "diff");
        Workspaces::new("test", l)
    }

    fn target(ws: &Workspaces, id: u64) -> Option<String> {
        ws.current().layout.target_of(id)
    }

    fn ids(ws: &Workspaces) -> (u64, u64, u64) {
        let leaves = ws.current().layout.root.leaves();
        let of = |editor: &str| {
            *leaves
                .iter()
                .find(|id| {
                    matches!(ws.current().layout.root.find(**id),
                        Some(Area::Leaf { editor: e, .. }) if e == editor)
                })
                .unwrap_or_else(|| panic!("no {editor} area"))
        };
        (of("files"), of("code"), of("diff"))
    }

    /// The payoff, and the reason kind is not the same thing as the noun the
    /// picker shows: a code viewer and a diff viewer both point at a path, so
    /// one click drives both.
    #[test]
    fn one_pick_moves_every_area_that_points_at_that_kind() {
        let commands = super::commands();
        let mut ws = workbench();
        let (files, code, diff) = ids(&ws);
        commands
            .run(
                &mut ws,
                "select",
                &json!({ "kind": "file", "value": "src/main.rs" }),
            )
            .expect("select runs");
        assert_eq!(target(&ws, code).as_deref(), Some("src/main.rs"));
        assert_eq!(target(&ws, diff).as_deref(), Some("src/main.rs"));
        // And the browser is untouched. Its target is the folder it is rooted
        // at; re-rooting it onto the file you just clicked would collapse the
        // tree you clicked it in.
        assert_eq!(target(&ws, files), None, "the browser followed a file");
    }

    /// Pinning is the only thing that stops an area following, and it has to
    /// stop it completely — a pinned viewer that drifts is worse than no pin.
    #[test]
    fn a_pinned_area_stays_where_it_was_put() {
        let commands = super::commands();
        let mut ws = workbench();
        let (_, code, diff) = ids(&ws);
        commands
            .run(
                &mut ws,
                "select",
                &json!({ "kind": "file", "value": "a.rs" }),
            )
            .expect("select runs");
        commands
            .run(
                &mut ws,
                "set_pinned",
                &json!({ "id": code, "pinned": true }),
            )
            .expect("pin runs");
        commands
            .run(
                &mut ws,
                "select",
                &json!({ "kind": "file", "value": "b.rs" }),
            )
            .expect("select runs");
        assert_eq!(target(&ws, code).as_deref(), Some("a.rs"), "the pin leaked");
        assert_eq!(target(&ws, diff).as_deref(), Some("b.rs"));

        // Unpinning does not retroactively move it; the next selection does.
        commands
            .run(
                &mut ws,
                "set_pinned",
                &json!({ "id": code, "pinned": false }),
            )
            .expect("unpin runs");
        assert_eq!(target(&ws, code).as_deref(), Some("a.rs"));
        commands
            .run(
                &mut ws,
                "select",
                &json!({ "kind": "file", "value": "c.rs" }),
            )
            .expect("select runs");
        assert_eq!(target(&ws, code).as_deref(), Some("c.rs"));
    }

    /// Selecting a kind nothing shows is a normal thing to do — a file picked
    /// in a workspace with no viewer open. It must not be an error, or the UI
    /// reports a failure for a click that was fine.
    #[test]
    fn selecting_a_kind_nothing_shows_is_not_a_failure() {
        let commands = super::commands();
        let mut ws = Workspaces::new("test", Layout::single("runs"));
        commands
            .run(
                &mut ws,
                "select",
                &json!({ "kind": "file", "value": "x.rs" }),
            )
            .expect("a selection nothing follows is still a selection");
        // But a malformed one is an error, because that is a caller bug.
        assert!(
            commands
                .run(&mut ws, "select", &json!({ "kind": "file" }))
                .is_err()
        );
        assert!(
            commands
                .run(&mut ws, "set_pinned", &json!({ "id": 99, "pinned": true }))
                .is_err()
        );
    }

    /// Splitting is how you get a second view of *this* thing. A fresh half
    /// that immediately followed the next selection away would be the
    /// opposite of what the gesture means.
    #[test]
    fn the_half_you_split_off_keeps_what_it_was_showing() {
        let commands = super::commands();
        let mut ws = workbench();
        let (_, code, _) = ids(&ws);
        commands
            .run(
                &mut ws,
                "select",
                &json!({ "kind": "file", "value": "a.rs" }),
            )
            .expect("select runs");
        let new = ws
            .current_layout_mut()
            .split(code, Dir::Col, 0.5)
            .expect("split");
        commands
            .run(
                &mut ws,
                "select",
                &json!({ "kind": "file", "value": "b.rs" }),
            )
            .expect("select runs");
        assert_eq!(
            target(&ws, new).as_deref(),
            Some("a.rs"),
            "the new half drifted"
        );
        assert_eq!(target(&ws, code).as_deref(), Some("b.rs"));
    }

    /// Every layout saved before pinning existed reads as unpinned, which is
    /// the behaviour those layouts had. A default that came out the other way
    /// would freeze every existing workbench silently.
    #[test]
    fn a_layout_saved_before_pins_existed_follows() {
        // Exactly what a workbench wrote before this field existed: no
        // `pinned` key anywhere.
        let json = r#"{"root":{"kind":"leaf","id":1,"editor":"code","arg":"old.rs"},"next_id":2}"#;
        let layout: Layout = serde_json::from_str(json).expect("an old layout still loads");
        assert!(!layout.is_pinned(1), "an old layout came back pinned");
    }
}

#[cfg(test)]
mod multi_select {
    use immersion::{Area, Dir, Layout, Workspaces};
    use serde_json::json;

    fn bench() -> Workspaces {
        let mut l = Layout::single("runs");
        let detail = l.split(1, Dir::Row, 0.5).expect("a second area");
        l.set_editor(detail, "run");
        Workspaces::new("test", l)
    }

    fn detail_id(ws: &Workspaces) -> u64 {
        *ws.current()
            .layout
            .root
            .leaves()
            .iter()
            .find(|id| {
                matches!(ws.current().layout.root.find(**id),
                    Some(Area::Leaf { editor, .. }) if editor == "run")
            })
            .expect("a run area")
    }

    /// Extending selects more without moving what the detail pane shows off
    /// the thing you last touched. That is the point of active-vs-selected:
    /// you can build a set of five while still looking at one of them.
    #[test]
    fn extending_the_selection_leaves_the_detail_pane_on_the_active_one() {
        let commands = super::commands();
        let mut ws = bench();
        let detail = detail_id(&ws);
        for (value, mode) in [("r1", "replace"), ("r2", "extend"), ("r3", "extend")] {
            commands
                .run(
                    &mut ws,
                    "select",
                    &json!({ "kind": "run", "value": value, "mode": mode }),
                )
                .expect("select runs");
        }
        assert_eq!(ws.current().selection("run"), ["r1", "r2", "r3"]);
        assert_eq!(
            ws.current().layout.target_of(detail).as_deref(),
            Some("r3"),
            "the pane should show the active one"
        );
    }

    /// Deselecting everything must not blank the areas. "Nothing is selected"
    /// is not the same statement as "look at nothing", and a detail pane that
    /// empties itself when you deselect is a pane that loses your place.
    #[test]
    fn clearing_the_selection_leaves_the_areas_where_they_were() {
        let commands = super::commands();
        let mut ws = bench();
        let detail = detail_id(&ws);
        commands
            .run(&mut ws, "select", &json!({ "kind": "run", "value": "r1" }))
            .expect("select runs");
        commands
            .run(
                &mut ws,
                "select",
                &json!({ "kind": "run", "value": "r1", "mode": "toggle" }),
            )
            .expect("toggle runs");
        assert!(ws.current().selection("run").is_empty());
        assert_eq!(ws.current().layout.target_of(detail).as_deref(), Some("r1"));
    }

    /// A selection belongs to the workspace it was made in. Two arrangements
    /// looking at different runs is the normal case, not a bug.
    #[test]
    fn a_selection_does_not_leak_between_workspaces() {
        let commands = super::commands();
        let mut ws = bench();
        commands
            .run(&mut ws, "select", &json!({ "kind": "run", "value": "r1" }))
            .expect("select runs");
        ws.add("other", Layout::single("runs"));
        assert!(ws.current().selection("run").is_empty());
        ws.switch(0);
        assert_eq!(ws.current().selection("run"), ["r1"]);
    }
}
