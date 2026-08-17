//! The diff editor.

use dioxus::prelude::*;

use super::code::read_source;
use super::files::{changed_files, files_root};
use super::stamp_of;

/// The diff viewer: what changed, drawn by the same renderer as the code
/// viewer so a file and a change to it look like the same thing.
///
/// The diff itself is computed here — `similar`, in Rust — and crosses as a
/// unified patch, the format the renderer parses. That keeps the division
/// honest: the server knows what changed, the browser draws it.
pub(crate) fn ed_diff(target: Option<String>, split: bool) -> Element {
    let Some(path) = target.filter(|t| !t.is_empty()) else {
        return rsx! {
            div { class: "empty",
                if changed_files().is_empty() {
                    "Nothing has changed \u{2014} the working tree matches HEAD."
                } else {
                    "Pick a changed file \u{2014} the target chip in the header."
                }
            }
        };
    };
    match git_diff(&path) {
        Ok(None) => {
            rsx! { div { class: "empty", "{path} matches HEAD \u{2014} nothing to show." } }
        }
        Ok(Some(patch)) => {
            let stamp = stamp_of(&path, &patch);
            rsx! {
                div { class: "code-view",
                    div { class: "code-path", "{path} \u{2014} working tree vs HEAD" }
                    pre { class: "code-src-payload", "data-im-code-src": "{stamp}", "{patch}" }
                    div {
                        class: "code-host",
                        "data-im-code": "{stamp}",
                        "data-im-code-path": "{path}",
                        "data-im-code-kind": "diff",
                        "data-im-code-layout": if split { "split" } else { "unified" },
                    }
                }
            }
        }
        Err(e) => rsx! { div { class: "empty", "{e}" } },
    }
}

/// A unified patch of one file against HEAD, or None when it matches. Reads
/// the committed blob with `git show` rather than reimplementing object
/// lookup; a path outside a repository is an error the viewer reports.
fn git_diff(rel: &str) -> Result<Option<String>, String> {
    // Same reason, and there is no repository on a demo machine to ask.
    if crate::demo::enabled() {
        return crate::demo::file_diff(rel)
            .map(|d| d.map(str::to_string))
            .ok_or_else(|| format!("no such file: {rel}"));
    }
    let working = read_source(rel)?;
    let root = files_root();
    let inside = rel.trim_start_matches('/');
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(&root)
        .arg("show")
        .arg(format!("HEAD:{inside}"))
        .output()
        .map_err(|e| format!("git: {e}"))?;
    // A file git does not know is new work, which diffs against nothing.
    let head = if out.status.success() {
        String::from_utf8_lossy(&out.stdout).into_owned()
    } else {
        String::new()
    };
    if head == working {
        return Ok(None);
    }
    let patch = similar::TextDiff::from_lines(&head, &working)
        .unified_diff()
        .context_radius(3)
        .header(&format!("a/{inside}"), &format!("b/{inside}"))
        .to_string();
    // The renderer wants the git file header, which unified_diff omits.
    Ok(Some(format!(
        "diff --git a/{inside} b/{inside}\n--- a/{inside}\n+++ b/{inside}\n{}",
        patch
            .split_once("+++ ")
            .and_then(|(_, rest)| rest.split_once('\n'))
            .map(|(_, body)| body.to_string())
            .unwrap_or(patch)
    )))
}

/// This editor's entry in the registry: what it is called, how it is drawn in
/// a header, whether it takes a target, and what the status bar says while it
/// has focus. Declared beside the editor so adding one is one file.
pub(crate) fn kind() -> immersion::EditorKind {
    immersion::EditorKind {
        id: "diff",
        label: "Diff",
        icon: "file-diff",
        hints: &[("Chip", "Pick changed file")],
        targets: true,
    }
}

#[cfg(test)]
mod diff_viewer {
    /// The renderer parses a git-style unified patch, so what we emit has to
    /// carry the headers it looks for — a bare `similar` unified_diff does
    /// not, and the panel comes up empty when it is missing.
    #[test]
    fn the_patch_carries_the_headers_the_renderer_parses() {
        let head = "one\ntwo\nthree\n";
        let work = "one\ntwo point five\nthree\n";
        let patch = similar::TextDiff::from_lines(head, work)
            .unified_diff()
            .context_radius(3)
            .header("a/x.rs", "b/x.rs")
            .to_string();
        let full = format!(
            "diff --git a/x.rs b/x.rs\n--- a/x.rs\n+++ b/x.rs\n{}",
            patch
                .split_once("+++ ")
                .and_then(|(_, r)| r.split_once('\n'))
                .map(|(_, body)| body.to_string())
                .unwrap_or(patch)
        );
        assert!(full.starts_with("diff --git a/x.rs b/x.rs\n"));
        assert!(full.contains("@@"), "a hunk header: {full}");
        assert!(full.contains("-two\n"), "the removed line: {full}");
        assert!(full.contains("+two point five\n"), "the added line: {full}");
        assert_eq!(full.matches("--- ").count(), 1, "headers are not doubled");
        assert_eq!(full.matches("+++ ").count(), 1, "headers are not doubled");
    }
}
