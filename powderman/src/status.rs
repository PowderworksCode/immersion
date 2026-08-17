//! What the status bar says: the hints on the left, the running task on the
//! right.
//!
//! Both answer questions about right now rather than about the document — what
//! can I do here, and what is this daemon busy with — which is why they live
//! together and away from the workbench frame.

use immersion::pretty_chord;

use crate::ui::{RunView, State};

/// The key hints the status bar keeps in view — the chords worth knowing, in
/// grammar form (the bar's shim renders `Mod` as the platform glyph). Global
/// only for now; area-scoped hints arrive with regions.
/// The status bar's left slot: what you can do here. Blender's shows the
/// active area's own shortcuts ahead of the global ones, which is the
/// difference between a status bar that teaches and one that decorates.
///
/// The area-specific entries are not all chords — some are the gesture or the
/// click that an editor answers to, which is exactly what a newcomer is
/// looking for and what no keymap lists.
pub(crate) fn status_hints(mac: bool, editor: Option<&str>) -> Vec<(String, String)> {
    let mut hints: Vec<(String, String)> = editor
        .map(editor_hints)
        .unwrap_or_default()
        .into_iter()
        .map(|(c, l)| (pretty_chord(c, mac), l.to_string()))
        .collect();
    hints.extend(
        [
            ("Mod+Z", "Undo"),
            ("F3", "Commands"),
            ("Mod+Shift+Space", "Maximize"),
        ]
        .into_iter()
        .map(|(c, l)| (pretty_chord(c, mac), l.to_string())),
    );
    hints
}

/// What each editor answers to, in its own words.
fn editor_hints(editor: &str) -> Vec<(&'static str, &'static str)> {
    match editor {
        "runs" => vec![("Click", "Open run"), ("Type", "Filter")],
        "run" => vec![("Chip", "Pick run")],
        "data" => vec![("Click", "Expand"), ("Right-click", "Copy data path")],
        "files" => vec![("Click", "Expand"), ("Chip", "Root here")],
        "code" | "diff" => vec![("Chip", "Pick file")],
        "chart" => vec![("Chip", "Pick chart"), ("N", "Edit spec")],
        "settings" => vec![("Drag", "Scrub number"), ("Type", "3*2 works")],
        "keymap" => vec![("Set", "Rebind"), ("Type", "Filter")],
        "actions" => vec![("Click", "Trigger")],
        "info" => vec![("Type", "Filter")],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod hint_tests {
    use super::*;

    #[test]
    fn the_hints_lead_with_the_focused_editor() {
        // The global hints are always there; the area's own come first,
        // because the question a status bar answers is "what can I do here".
        let global = status_hints(false, None);
        assert!(global.iter().any(|(_, l)| l == "Undo"));

        let runs = status_hints(false, Some("runs"));
        assert_eq!(runs[0].1, "Open run", "the editor's own hint leads");
        assert!(runs.len() > global.len(), "and the global ones remain");

        // An editor with nothing of its own still gets the globals rather
        // than an empty bar.
        assert_eq!(status_hints(false, Some("nope")), global);
    }

    #[test]
    fn every_registered_editor_is_accounted_for() {
        // A new editor with no hints is a status bar that says nothing about
        // it. Not every editor needs one, but the omission should be a
        // decision — this lists the ones that have deliberately none.
        const NO_HINTS: &[&str] = &["machine", "fleet", "timers"];
        for k in crate::ui::kinds() {
            let has = !editor_hints(k.id).is_empty();
            assert!(
                has || NO_HINTS.contains(&k.id),
                "{} has no status hints; add some or list it in NO_HINTS",
                k.id
            );
        }
    }
}

/// The status bar's progress slot: what this daemon is busy with.
///
/// A run's size is not known in advance — a sweep's step count depends on how
/// many grammars have gaps — so the fraction is only offered when the run
/// itself says so, in a `note` that reads "N of M". Otherwise the bar says
/// that work is happening without inventing a position for it, which is the
/// honest shape for indeterminate work.
pub(crate) fn running_task(s: &State) -> Option<(String, Option<f32>)> {
    let running: Vec<&RunView> = s.runs.iter().filter(|r| r.status == "running").collect();
    let first = running.first()?;
    let label = match running.len() {
        1 => first.workflow.clone(),
        n => format!("{} +{}", first.workflow, n - 1),
    };
    Some((label, first.note.as_deref().and_then(progress_of)))
}

/// The fraction in a note like "sweeping — 18 of 41". Nothing else is a
/// progress claim: a note is prose, and reading a number out of arbitrary
/// prose would put a wrong bar on the screen with confidence.
fn progress_of(note: &str) -> Option<f32> {
    let (before, after) = note.split_once(" of ")?;
    let done: f32 = before
        .rsplit(|c: char| !c.is_ascii_digit())
        .next()?
        .parse()
        .ok()?;
    let total: f32 = after
        .split(|c: char| !c.is_ascii_digit())
        .find(|t| !t.is_empty())?
        .parse()
        .ok()?;
    (total > 0.0).then(|| (done / total).clamp(0.0, 1.0))
}

#[cfg(test)]
mod task_tests {
    use super::*;

    fn run(status: &str, workflow: &str, note: Option<&str>) -> RunView {
        RunView {
            id: workflow.into(),
            workflow: workflow.into(),
            status: status.into(),
            note: note.map(str::to_string),
            error: None,
            updated_at: 0,
            steps: vec![],
        }
    }

    #[test]
    fn the_bar_appears_only_while_something_runs() {
        let idle = State {
            runs: vec![run("done", "sweep", None), run("failed", "fix", None)],
            ..Default::default()
        };
        assert!(running_task(&idle).is_none(), "nothing is running");

        let busy = State {
            runs: vec![run(
                "running",
                "treebank_sweep",
                Some("sweeping — 18 of 41"),
            )],
            ..Default::default()
        };
        let (label, done) = running_task(&busy).expect("a task");
        assert_eq!(label, "treebank_sweep");
        assert_eq!(done, Some(18.0 / 41.0));
    }

    #[test]
    fn several_runs_are_counted_not_hidden() {
        let s = State {
            runs: vec![
                run("running", "sweep", None),
                run("running", "fix", None),
                run("done", "agent", None),
            ],
            ..Default::default()
        };
        let (label, done) = running_task(&s).expect("a task");
        assert_eq!(label, "sweep +1", "the others are visible in the count");
        assert_eq!(done, None, "and an unmeasured run claims no position");
    }

    #[test]
    fn only_a_real_count_becomes_a_fraction() {
        assert_eq!(progress_of("sweeping — 18 of 41"), Some(18.0 / 41.0));
        assert_eq!(progress_of("step 3 of 4"), Some(0.75));
        // Prose that merely contains "of" is not a measurement.
        assert_eq!(progress_of("waiting on a human"), None);
        assert_eq!(progress_of("out of memory"), None);
        assert_eq!(progress_of("0 of 0"), None, "and nothing divides by zero");
    }
}
