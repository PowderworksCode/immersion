//! treebank's daily sweep, as a workflow.
//!
//! This is the port of the first half of `scripts/daily.sh`: pull, build, and
//! then for every vendored grammar rank → fetch → materialize → sweep, ending
//! with a report of which languages have grammar gaps. The fix-agent half —
//! worktree, agent, verify, stacked PR — is not here yet, and until it is this
//! runs unscheduled so it cannot collide with the cron job that still owns
//! 06:00.
//!
//! What the shell version needs and this does not: a flock (a run is a row, and
//! the daemon drives one at a time), a `--reap` mode (an interrupted run
//! resumes by replay), and the launch/reap idempotence dance that existed only
//! because a shell script cannot pause and be resumed.
//!
//! Note what stayed in the shell: nothing. Every command here goes through
//! systemd-run, so each gets an exit code, its own cgroup, and a journald unit
//! you can read afterwards — `journalctl --user -u pm-<run>-<step>`.

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::engine::{BoxFut, Ctx};
use crate::herdr;

fn repo() -> String {
    std::env::var("TREEBANK_REPO")
        .unwrap_or_else(|_| format!("{}/treebank", std::env::var("HOME").unwrap_or_default()))
}

/// One grammar, as the ledger describes it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Grammar {
    /// The canonical language name — the ledger's `grammar` field, which is
    /// what `--lang` and `corpus/<lang>/` both use.
    lang: String,
    /// Repo-relative directory, e.g. `crates/treebank-csharp`.
    reldir: String,
}

/// What a sweep wrote.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Sweep {
    #[serde(default)]
    files: i64,
    #[serde(default)]
    passed: i64,
    #[serde(default)]
    failed: i64,
    #[serde(default)]
    gap_files: i64,
    #[serde(default)]
    noise_files: i64,
    /// `clusters` is a LIST of clusters, not a count — deserializing it as a
    /// number fails with "invalid type: sequence, expected i64", which points
    /// at the reader rather than at the assumption behind it.
    #[serde(default)]
    clusters: Vec<serde_json::Value>,
}

/// Read the ledgers directly rather than shelling out to `jq`.
///
/// The grammar's directory comes from the ledger's own path, never from
/// `treebank-<lang>`: the two disagree — grammar `csharp` used to live in a
/// directory the derived name did not produce — and a path derived from a
/// name can be wrong in a way a dirname cannot.
fn discover(root: &str) -> Result<Vec<Grammar>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(format!("{root}/crates"))? {
        let dir = entry?.path();
        let ledger = dir.join("ledger.json");
        if !ledger.is_file() {
            continue;
        }
        let text = std::fs::read_to_string(&ledger)?;
        let v: Value = serde_json::from_str(&text)?;
        let lang = v["grammar"]
            .as_str()
            .ok_or_else(|| anyhow!("{}: no grammar field", ledger.display()))?
            .to_string();
        let reldir = dir
            .strip_prefix(root)
            .unwrap_or(&dir)
            .to_string_lossy()
            .into_owned();
        out.push(Grammar { lang, reldir });
    }
    // Sorted so the step sequence is identical on every replay.
    out.sort_by(|a, b| a.lang.cmp(&b.lang));
    Ok(out)
}

pub fn sweep_all(ctx: Ctx) -> BoxFut<'static, Result<Value>> {
    Box::pin(async move {
        let root = repo();
        let cwd = Some(root.clone());
        // {"limit": 1000} sweeps deeper than the daily default. The corpus a
        // language is measured against is a choice, not a constant.
        let limit = ctx.input["limit"].as_i64().unwrap_or(100);

        // Pull merged work in, but never over a dirty tree: an agent's
        // unreviewed changes are not discarded to make room for a pull.
        let dirty = ctx
            .exec(
                "check clean",
                vec![
                    "git".into(),
                    "status".into(),
                    "--porcelain".into(),
                    "--untracked-files=no".into(),
                ],
                cwd.clone(),
            )
            .await?;
        if dirty.stdout.trim().is_empty() {
            ctx.exec(
                "pull",
                vec![
                    "git".into(),
                    "pull".into(),
                    "--ff-only".into(),
                    "--quiet".into(),
                ],
                cwd.clone(),
            )
            .await?;
        }

        // A pull that moves a submodule pointer does not move the submodule
        // working tree, and materialize refuses to run when the checked-out
        // sha is not the pinned one.
        ctx.exec(
            "submodules",
            vec![
                "git".into(),
                "submodule".into(),
                "update".into(),
                "--init".into(),
                "--quiet".into(),
            ],
            cwd.clone(),
        )
        .await?;

        // Fatal, unlike the per-grammar failures below: every later step runs
        // the binary this produces.
        let built = ctx
            .exec(
                "build",
                vec![
                    "cargo".into(),
                    "build".into(),
                    "--release".into(),
                    "--quiet".into(),
                ],
                cwd.clone(),
            )
            .await?;
        if !built.ok() {
            return Err(anyhow!("cargo build failed: {}", built.stderr.trim()));
        }

        // Recorded, so a replay iterates exactly the same grammars in the same
        // order — the step keys below depend on it.
        let root_for_discover = root.clone();
        let grammars: Vec<Grammar> = ctx
            .step("discover grammars", move |_| async move {
                discover(&root_for_discover)
            })
            .await?;

        let tb = format!("{root}/target/release/treebank");
        let mut report = serde_json::Map::new();

        for g in &grammars {
            let lang = g.lang.clone();

            // rank can legitimately fail — a language with no ranking path
            // falls back to the list already on disk, and only a language with
            // neither is a problem.
            let ranked = ctx
                .exec(
                    &format!("rank {lang}"),
                    vec![
                        tb.clone(),
                        "rank".into(),
                        "--lang".into(),
                        lang.clone(),
                        "--k".into(),
                        "1000".into(),
                    ],
                    cwd.clone(),
                )
                .await?;
            let have_list =
                std::path::Path::new(&format!("{root}/corpus/{lang}/top-k.json")).exists();
            if !ranked.ok() && !have_list {
                report.insert(
                    lang.clone(),
                    json!({ "skipped": "no package list and cannot rank" }),
                );
                continue;
            }

            ctx.exec(
                &format!("fetch {lang}"),
                vec![
                    tb.clone(),
                    "fetch".into(),
                    "--lang".into(),
                    lang.clone(),
                    "--limit".into(),
                    limit.to_string(),
                ],
                cwd.clone(),
            )
            .await?;

            let materialized = ctx
                .exec(
                    &format!("materialize {lang}"),
                    // Absolute: systemd resolves the executable before
                    // --working-directory applies, so a relative path is
                    // looked up on PATH and reported as "Failed to find
                    // executable scripts/materialize.sh".
                    vec![format!("{root}/scripts/materialize.sh"), g.reldir.clone()],
                    cwd.clone(),
                )
                .await?;
            if !materialized.ok() {
                report.insert(
                    lang.clone(),
                    json!({ "error": "materialize failed", "stderr": materialized.stderr.trim() }),
                );
                continue;
            }

            let swept = ctx
                .exec(
                    &format!("sweep {lang}"),
                    vec![
                        tb.clone(),
                        "sweep".into(),
                        "--lang".into(),
                        lang.clone(),
                        "--grammar".into(),
                        format!("{}/build", g.reldir),
                    ],
                    cwd.clone(),
                )
                .await?;
            if !swept.ok() {
                report.insert(
                    lang.clone(),
                    json!({ "error": "sweep failed", "stderr": swept.stderr.trim() }),
                );
                continue;
            }

            let path = format!("{root}/corpus/{lang}/reports/sweep.json");
            let s: Sweep = ctx
                .step(&format!("read sweep {lang}"), move |_| async move {
                    let text = std::fs::read_to_string(&path)?;
                    Ok(serde_json::from_str(&text)?)
                })
                .await?;
            report.insert(
                lang.clone(),
                json!({
                    "files": s.files, "passed": s.passed, "failed": s.failed,
                    "gap_files": s.gap_files, "noise_files": s.noise_files,
                    "clusters": s.clusters.len(),
                }),
            );
        }

        let with_gaps: Vec<String> = report
            .iter()
            .filter(|(_, v)| v.get("gap_files").and_then(Value::as_i64).unwrap_or(0) > 0)
            .map(|(k, _)| k.clone())
            .collect();

        // Hand each language with gaps to its own fix run, rather than fixing
        // inline. A fix is long and can park for hours on a human; the sweep
        // should not still be open while that happens, and one language
        // stalling must not hold up the others.
        //
        // A step, so a replayed sweep does not launch a second set of agents.
        let to_fix: Vec<String> = with_gaps
            .iter()
            .filter(|l| !NO_AGENT.contains(&l.as_str()))
            .cloned()
            .collect();
        let launch = to_fix.clone();
        let launched: Vec<String> = ctx
            .step("launch fixes", move |_| async move {
                let mut ok = Vec::new();
                for lang in launch {
                    match crate::daemon::trigger_with("treebank_fix", json!({ "lang": lang })) {
                        Ok(()) => ok.push(lang),
                        Err(e) => eprintln!("could not start a fix run for {lang}: {e}"),
                    }
                }
                Ok(ok)
            })
            .await?;
        Ok(json!({
            "grammars": grammars.len(),
            "gaps_in": with_gaps,
            "fixes_launched": launched,
            "report": report,
        }))
    })
}

/// Languages that are swept and reported but never handed to a fix agent.
///
/// C# reports ~7,148 gap files across 838 clusters every single day, and by
/// its own LOCAL-PATCHES.md roughly two thirds are the inherent `#if` class:
/// Roslyn adjudicates the active preprocessor branch while tree-sitter parses
/// every branch into one tree. An agent pointed at that backlog would burn a
/// session a day, stack PRs to the depth cap, and then block the language —
/// working hard on the part that cannot move.
const NO_AGENT: &[&str] = &["csharp"];

const STACK_MAX: usize = 3;

#[derive(Debug, Deserialize)]
struct Pr {
    number: i64,
    #[serde(rename = "headRefName")]
    head_ref_name: String,
    url: String,
}

/// Fix one language's grammar gaps: worktree, agent, verify, stacked PR.
///
/// Triggered with `{"lang": "rust"}`. Deliberately a separate workflow from
/// the sweep rather than a branch inside it — a fix run is long, can park for
/// hours on a human, and should be startable on its own for one language.
pub fn fix(ctx: Ctx) -> BoxFut<'static, Result<Value>> {
    Box::pin(async move {
        let root = repo();
        let cwd = Some(root.clone());
        let lang = ctx.input["lang"]
            .as_str()
            .ok_or_else(|| anyhow!("fix needs an input like {{\"lang\":\"rust\"}}"))?
            .to_string();

        if NO_AGENT.contains(&lang.as_str()) {
            return Ok(json!({ "lang": lang, "skipped": "excluded from agent fixes" }));
        }

        let grammars: Vec<Grammar> = {
            let r = root.clone();
            ctx.step("discover grammars", move |_| async move { discover(&r) })
                .await?
        };
        let g = grammars
            .into_iter()
            .find(|g| g.lang == lang)
            .ok_or_else(|| anyhow!("no grammar for {lang}"))?;

        // An agent still live for this language means its work has not reached
        // a PR yet; a second one would duplicate it.
        let agent_name = format!("tbfix-{}", lang.replace('_', "-"));
        // A STEP, not a poll. This is a decision taken once at the start of a
        // run, not a condition to re-observe: as a poll it re-evaluates on
        // every replay, and once this run starts its own agent the guard sees
        // that agent and skips — the workflow tripping over the thing it just
        // created, one sleep after creating it.
        let n = agent_name.clone();
        if let Some(a) = ctx
            .step("existing agent", move |_| async move {
                tokio::task::spawn_blocking(move || herdr::agent_get(&n)).await?
            })
            .await?
        {
            return Ok(
                json!({ "lang": lang, "skipped": format!("{agent_name} is still {}", a.status()) }),
            );
        }

        // The open fix PRs for a language ARE its stack, bottom to top. A new
        // run branches off the top, so the agent starts from a tree with
        // yesterday's patches applied: its sweep reports only what is still
        // broken and its patches/NNNN numbering continues rather than
        // colliding. Stacks merge bottom-up, so an unreviewed one blocks the
        // language — hence the cap.
        let prs_out = ctx
            .exec(
                "list prs",
                vec![
                    "gh".into(),
                    "pr".into(),
                    "list".into(),
                    "--state".into(),
                    "open".into(),
                    "--limit".into(),
                    "100".into(),
                    "--json".into(),
                    "number,headRefName,url".into(),
                ],
                cwd.clone(),
            )
            .await?;
        if !prs_out.ok() {
            return Err(anyhow!("cannot reach GitHub: {}", prs_out.stderr.trim()));
        }
        let all: Vec<Pr> = serde_json::from_str(&prs_out.stdout).unwrap_or_default();
        let prefix = format!("grammar-fixes/{lang}-");
        let mut stack: Vec<Pr> = all
            .into_iter()
            .filter(|p| p.head_ref_name.starts_with(&prefix))
            .collect();
        stack.sort_by_key(|p| p.number);

        if stack.len() >= STACK_MAX {
            return Ok(json!({
                "lang": lang,
                "skipped": format!("fix stack is {} deep (max {STACK_MAX})", stack.len()),
                "blocking": stack.first().map(|p| p.url.clone()),
            }));
        }

        let trunk = ctx
            .exec(
                "trunk",
                vec![
                    "git".into(),
                    "rev-parse".into(),
                    "--abbrev-ref".into(),
                    "HEAD".into(),
                ],
                cwd.clone(),
            )
            .await?;
        let trunk = trunk.stdout.trim().to_string();
        let base = stack
            .last()
            .map(|p| p.head_ref_name.clone())
            .unwrap_or_else(|| trunk.clone());

        // The branch name is a step: it embeds a timestamp, and computing it
        // in the body would produce a different name on every replay.
        let branch: String = ctx
            .step("branch name", |_| async {
                Ok(format!(
                    "grammar-fixes/{}-{}",
                    lang,
                    chrono::Local::now().format("%Y%m%d-%H%M")
                ))
            })
            .await?;

        // A STABLE path per language, not one derived from the branch.
        //
        // Claude asks "do you trust this folder?" the first time it sees a
        // directory and reports as `blocked` until a human answers — and a
        // path derived from the branch is a new directory every run, so it
        // would ask every run, forever. Trust is keyed by path string in
        // ~/.claude.json and survives the directory being deleted and
        // recreated, so a stable path means exactly one human confirmation per
        // language, ever.
        let wt_path = format!(
            "{}/.herdr/worktrees/treebank/tbfix-{lang}",
            std::env::var("HOME").unwrap_or_default()
        );

        // A worktree left by an earlier run holds that path. Clear it first,
        // deiniting its submodules or `git worktree remove` refuses.
        if std::path::Path::new(&wt_path).exists() {
            ctx.exec(
                "clear stale worktree",
                vec![
                    "bash".into(),
                    "-c".into(),
                    format!(
                        "git -C {wt_path} submodule deinit -f --all; \
                         git -C {root} worktree remove --force {wt_path}; \
                         git -C {root} worktree prune"
                    ),
                ],
                cwd.clone(),
            )
            .await?;
        }

        let (ws, pane, wt) = {
            let (r, b, ba, l, p) = (
                root.clone(),
                branch.clone(),
                base.clone(),
                format!("tbfix {lang}"),
                wt_path.clone(),
            );
            ctx.step("worktree", move |_| async move {
                tokio::task::spawn_blocking(move || herdr::create_worktree(&r, &b, &ba, &l, &p))
                    .await?
            })
            .await?
        };

        // A fresh worktree is not usable as-is: no submodule (materialize
        // refuses without it at the pinned sha), and corpus/ and target/ are
        // gitignored so they exist only in the main checkout — but check.sh
        // sweeps the corpus and resolves TREEBANK_BIN through target/.
        ctx.exec(
            "worktree submodule",
            vec![
                "git".into(),
                "-C".into(),
                wt.clone(),
                "submodule".into(),
                "update".into(),
                "--init".into(),
                format!("{}/upstream", g.reldir),
            ],
            cwd.clone(),
        )
        .await?;
        ctx.exec(
            "worktree links",
            vec![
                "bash".into(),
                "-c".into(),
                format!("ln -sfn {root}/corpus {wt}/corpus && ln -sfn {root}/target {wt}/target"),
            ],
            cwd.clone(),
        )
        .await?;

        // The PR body is computed from the sweep that actually ran, rather
        // than recalled by the agent afterwards.
        let body_path = format!("{root}/corpus/{lang}/reports/PR-BODY.md");
        {
            let (rt, lg, bp) = (root.clone(), lang.clone(), body_path.clone());
            ctx.step("pr body", move |_| async move {
                let sweep: Sweep = serde_json::from_str(&std::fs::read_to_string(format!(
                    "{rt}/corpus/{lg}/reports/sweep.json"
                ))?)?;
                std::fs::write(
                    &bp,
                    format!(
                        "Automated fix run.\n\n                         - Corpus: **{}** files, {} passed / {} failed\n                         - Gap files: **{}** across {} clusters\n                         - Patches and evidence live in the grammar's `patches/`, `ledger.json` and `LOCAL-PATCHES.md`.\n                         - CI re-proves materialization, corpus tests and the negative corpus.\n",
                        sweep.files, sweep.passed, sweep.failed, sweep.gap_files, sweep.clusters.len()
                    ),
                )?;
                Ok(true)
            })
            .await?;
        }

        let agent = agent_name.clone();
        let p = pane.clone();
        ctx.step("start agent", move |_| async move {
            // --remote-control so the agent is reachable from the Claude app
            // the moment it starts, named after itself so it is identifiable
            // among the fleet. A nightly run that parks at 06:45 is only
            // answerable if you can get at it from wherever you are; opening
            // the session after the fact is not possible, so it has to be on
            // at launch.
            let args = vec![
                "--permission-mode".to_string(),
                "auto".to_string(),
                "--remote-control".to_string(),
                agent.clone(),
            ];
            tokio::task::spawn_blocking(move || herdr::start_agent(&agent, "claude", &p, &args))
                .await??;
            Ok(true)
        })
        .await?;

        // Without this the prompt is refused as "not an active named agent":
        // agent.start leaves the agent launch_pending until something polls
        // agent.get for it. See herdr::agent_get.
        let agent = agent_name.clone();
        ctx.step("wait until ready", move |_| async move {
            tokio::task::spawn_blocking(move || {
                herdr::wait_until_ready(&agent, std::time::Duration::from_secs(180))
            })
            .await??;
            Ok(true)
        })
        .await?;

        let agent = agent_name.clone();
        let prompt = fix_prompt(&lang, &g.reldir);
        ctx.step("prompt", move |_| async move {
            tokio::task::spawn_blocking(move || herdr::prompt_agent(&agent, &prompt)).await??;
            Ok(true)
        })
        .await?;

        // Wait for the prompt to REGISTER before waiting for it to finish.
        //
        // An agent is still `idle` for a second or two after being prompted —
        // herdr has not seen it start work yet. Going straight into the settle
        // loop reads that idle as "already done", and the run tears the
        // worktree down while the agent is working in it, leaving an agent
        // whose cwd reads `(deleted)`. Observed, not theoretical.
        let agent = agent_name.clone();
        ctx.step("prompt registered", move |_| async move {
            for _ in 0..40 {
                let a = agent.clone();
                let st = tokio::task::spawn_blocking(move || herdr::agent_get(&a)).await??;
                match st.as_ref().map(|a| a.status().to_string()).as_deref() {
                    Some("working") | Some("blocked") => return Ok(true),
                    None => return Ok(false), // gone; the settle loop will notice
                    _ => {}
                }
                tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
            }
            // A trivial prompt can finish inside the window; not an error.
            Ok(false)
        })
        .await?;

        // Now wait for it to settle. Every check is a poll, never a step: a
        // recorded status would replay as `working` forever and the loop would
        // never end. The run parks between checks rather than holding a
        // process, so an agent that blocks for hours costs nothing.
        let mut waited = 0i64;
        loop {
            let agent = agent_name.clone();
            let state = ctx
                .poll("agent state", move || async move {
                    tokio::task::spawn_blocking(move || herdr::agent_get(&agent)).await?
                })
                .await?;
            match state.as_ref().map(|a| a.status().to_string()).as_deref() {
                None | Some("idle") | Some("done") => break,
                Some("blocked") => println!("{agent_name}: blocked — a human needs to answer it"),
                _ => {}
            }
            if waited > 4 * 3_600_000 {
                ctx.park("stuck agent", "agent has not settled in four hours")
                    .await?;
            }
            ctx.sleep("check agent", 30_000).await?;
            waited += 30_000;
        }

        // Trust nothing the agent reported: re-prove the invariant against
        // what it actually committed.
        let verified = ctx
            .exec(
                "verify",
                vec![
                    format!("{root}/scripts/verify.sh"),
                    format!("{wt}/{}", g.reldir),
                ],
                Some(root.clone()),
            )
            .await?;
        if !verified.ok() {
            return Err(anyhow!(
                "verify failed on {branch}; worktree kept at {wt}: {}",
                verified.stderr.trim()
            ));
        }

        let committed = ctx
            .exec(
                "count commits",
                vec![
                    "git".into(),
                    "-C".into(),
                    wt.clone(),
                    "rev-list".into(),
                    "--count".into(),
                    format!("{trunk}..{branch}"),
                ],
                Some(root.clone()),
            )
            .await?;
        let commits: i64 = committed.stdout.trim().parse().unwrap_or(0);
        if commits == 0 {
            teardown_worktree(&ctx, &root, &wt, &branch).await?;
            return Ok(json!({ "lang": lang, "result": "agent committed nothing" }));
        }

        // `gh stack submit` opens a full-screen editor on a TTY; --auto is
        // what makes it usable unattended and --open stops every layer being
        // created as a draft. The extension is young, so a plain PR against
        // the layer below is the fallback — a day of work is not worth losing
        // to it.
        let mut below: Vec<String> = stack.iter().map(|p| p.head_ref_name.clone()).collect();
        below.push(branch.clone());
        let mut init = vec!["gh".to_string(), "stack".into(), "init".into()];
        init.extend(below.clone());
        init.extend(["--base".into(), trunk.clone()]);
        let stacked = ctx.exec("stack init", init, Some(wt.clone())).await?;
        let submitted = if stacked.ok() {
            ctx.exec(
                "stack submit",
                vec![
                    "gh".into(),
                    "stack".into(),
                    "submit".into(),
                    "--auto".into(),
                    "--open".into(),
                ],
                Some(wt.clone()),
            )
            .await?
        } else {
            ctx.exec(
                "plain pr",
                vec![
                    "bash".into(), "-c".into(),
                    format!(
                        "git -C {wt} push -u origin {branch} && cd {wt} && gh pr create --base {base} --head {branch} --title '{} grammar fixes' --body-file {body_path}",
                        g.reldir
                    ),
                ],
                Some(wt.clone()),
            )
            .await?
        };
        if !submitted.ok() {
            return Err(anyhow!(
                "{commits} commit(s) on {branch} but no PR: {}",
                submitted.stderr.trim()
            ));
        }

        teardown_worktree(&ctx, &root, &wt, &branch).await?;

        Ok(json!({
            "lang": lang, "branch": branch, "base": base,
            "stack_depth": stack.len() + 1, "commits": commits,
            "workspace": ws,
        }))
    })
}

fn fix_prompt(lang: &str, reldir: &str) -> String {
    format!(
        "Read corpus/{lang}/reports/REPORT.md and fix ALL of its gap clusters, one at a \
         time, exactly per the report's instructions. Edit grammar sources in \
         {reldir}/build/ (the materialized tree — see GRAMMARS.md). After each fix, run \
         ../../scripts/check.sh from {reldir} until it prints CHECK OK. Capture each fix \
         as patches/NNNN-*.patch with a ledger.json entry and a LOCAL-PATCHES.md note, per \
         GRAMMARS.md, numbering after the highest patch already there. Update the ledger's \
         corpus.sweep_patched numbers, then run scripts/verify.sh {reldir} from the repo \
         root — it must pass.\n\n\
         Finally: git add {reldir} and git commit it, with a message naming the clusters \
         you fixed. Commit only — do NOT push and do NOT open a PR; the job does that so \
         the stack wiring stays deterministic.\n\n\
         You are in a git worktree of your own; nothing here touches the main checkout. If \
         a cluster is genuinely beyond a minimal grammar change, skip it and say so."
    )
}

/// Remove a fix worktree and its branch.
///
/// `git worktree remove` fails outright on a worktree with initialized
/// submodules — including with `--force` — so they are deinited first. Every
/// grammar worktree has one, so this is the only path that works.
pub async fn teardown_worktree(ctx: &Ctx, root: &str, wt: &str, branch: &str) -> Result<()> {
    ctx.exec(
        "deinit submodules",
        vec![
            "git".into(),
            "-C".into(),
            wt.to_string(),
            "submodule".into(),
            "deinit".into(),
            "-f".into(),
            "--all".into(),
        ],
        Some(root.to_string()),
    )
    .await?;
    ctx.exec(
        "remove worktree",
        vec![
            "git".into(),
            "-C".into(),
            root.to_string(),
            "worktree".into(),
            "remove".into(),
            "--force".into(),
            wt.to_string(),
        ],
        Some(root.to_string()),
    )
    .await?;
    // The branch stays when a PR points at it; only an abandoned run deletes it.
    let _ = branch;
    Ok(())
}
