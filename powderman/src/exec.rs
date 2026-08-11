//! Running things, split by whether a human might ever need to intervene.
//!
//! **systemd-run** for headless work — a build, a sweep, an API call. It
//! returns the real exit code, captures both streams, puts the process in its
//! own cgroup, and logs to journald, all without a helper writing a status
//! file for one to be recovered from. This is what a pane cannot do: a pane
//! hands back text, so "the build failed" and "the build printed the word
//! failed" look identical.
//!
//! **herdr** for agents and anything a person may want to watch or take over.
//! That is the one thing systemd cannot offer, and the reason the fleet
//! exists.
//!
//! Splitting them means the fleet UI contains only things worth looking at.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::process::Stdio;
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Output {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
    /// The transient unit the command ran in, for `journalctl --user -u <it>`.
    pub unit: String,
}

impl Output {
    pub fn ok(&self) -> bool {
        self.code == 0
    }
}

/// Run a command under the user's systemd manager and wait for it.
///
/// `--pipe` gives us the streams, `--wait` blocks until it exits and
/// propagates the exit code, `--collect` removes the unit afterwards so a
/// failed run does not linger in `systemctl --user list-units`.
pub async fn run(unit: &str, cmd: &[String], cwd: Option<&str>) -> Result<Output> {
    let mut c = Command::new("systemd-run");
    c.arg("--user")
        .arg("--pipe")
        .arg("--wait")
        .arg("--collect")
        .arg("--quiet")
        .arg(format!("--unit={unit}"));
    if let Some(dir) = cwd {
        c.arg(format!("--working-directory={dir}"));
    }
    // A transient unit inherits the *user manager's* environment, not this
    // process's — so cargo, npx and tree-sitter are all missing from its PATH
    // even though they are plainly on ours. The failure reads "Failed to find
    // executable cargo", which sounds like the toolchain is absent rather than
    // like an environment that was never passed along.
    if let Ok(path) = std::env::var("PATH") {
        c.arg(format!("--setenv=PATH={path}"));
    }
    if let Ok(home) = std::env::var("HOME") {
        c.arg(format!("--setenv=HOME={home}"));
    }
    c.arg("--");
    c.args(cmd);
    c.stdout(Stdio::piped()).stderr(Stdio::piped());

    let out = c
        .output()
        .await
        .with_context(|| format!("spawning systemd-run for {cmd:?}"))?;

    Ok(Output {
        // A signalled process reports no code; -1 is not an exit status any
        // command can produce, so it cannot be confused with a real one.
        code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        unit: unit.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_name(n: &str) -> String {
        format!("pm-test-{n}-{}", std::process::id())
    }

    #[tokio::test]
    async fn captures_exit_code_and_both_streams() {
        let out = run(
            &unit_name("streams"),
            &[
                "bash".into(),
                "-lc".into(),
                "echo to-stdout; echo to-stderr >&2; exit 7".into(),
            ],
            None,
        )
        .await
        .unwrap();
        // The whole reason for systemd-run over a pane: a real exit code, and
        // the two streams kept apart.
        assert_eq!(out.code, 7);
        assert!(!out.ok());
        assert!(out.stdout.contains("to-stdout"));
        assert!(out.stderr.contains("to-stderr"));
        assert!(!out.stdout.contains("to-stderr"));
    }

    #[tokio::test]
    async fn the_callers_path_reaches_the_command() {
        // Guards the "Failed to find executable cargo" class of failure: a
        // transient unit does not inherit our PATH unless we pass it.
        //
        // `printenv`, not `bash -lc 'echo $PATH'` — a login shell re-sources
        // the profile and prepends to PATH, so that version of this test
        // fails against its own measurement rather than against the code.
        let out = run(
            &unit_name("path"),
            &["printenv".into(), "PATH".into()],
            None,
        )
        .await
        .unwrap();
        assert_eq!(out.stdout.trim(), std::env::var("PATH").unwrap());
    }

    #[tokio::test]
    async fn success_is_zero() {
        let out = run(&unit_name("ok"), &["true".into()], None).await.unwrap();
        assert_eq!(out.code, 0);
        assert!(out.ok());
    }

    #[tokio::test]
    async fn runs_in_the_requested_directory() {
        let out = run(&unit_name("cwd"), &["pwd".into()], Some("/tmp"))
            .await
            .unwrap();
        assert_eq!(out.stdout.trim(), "/tmp");
    }
}
