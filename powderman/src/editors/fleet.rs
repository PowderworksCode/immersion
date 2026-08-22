//! The fleet editor.

use dioxus::prelude::*;

use immersion::Panel;

use crate::editors::{Draw, prop};
use crate::ui::State;
use crate::ui::{gib, short};

pub(crate) fn ed_fleet(s: &State) -> Element {
    rsx! {
        if s.fleet.is_empty() {
            div { class: "empty", "no agents" }
        }
        for a in s.fleet.iter() {
            // `im-row` marks the hover target for the row controls. The row
            // is a three-column grid (status, name, procs); the actions float
            // at its right edge rather than taking a fourth column, so the
            // columns stay put whether or not a pointer is near.
            div { class: "agent im-row", key: "{a.pane}",
                span { class: "status {a.status}", "{a.status}" }
                span { class: "wf", "{a.name}" }
                span { class: "procs",
                    // The two biggest by memory, not every pid. A full process
                    // table is htop's job; this row answers "what is this
                    // agent doing and how big is it".
                    for p in a.procs.iter().take(2) {
                        span { class: "proc", key: "{p.pid}", "{p.name}" }
                    }
                    if a.procs.len() > 2 {
                        span { class: "note", "+{a.procs.len() - 2}" }
                    }
                    span { class: "note", "{gib(a.procs.iter().map(|p| p.rss).sum::<f64>())}" }
                    span { class: "note", "{short(&a.cwd, 46)}" }
                }
                span { class: "im-row-actions",
                    // What an agent row is actually for, most of the time:
                    // the two strings on it that you need somewhere else —
                    // the directory it is working in, and the pane to attach
                    // to. Copying is client-side, so neither costs a message.
                    button {
                        class: "im-row-btn im-copy",
                        title: "copy this agent's working directory",
                        "data-im-copy": "{a.cwd}",
                        dangerous_inner_html: "{immersion::icon(\"folder\")}",
                    }
                    button {
                        class: "im-row-btn im-copy",
                        title: "copy this agent's pane id",
                        "data-im-copy": "{a.pane}",
                        dangerous_inner_html: "{immersion::icon(\"terminal-2\")}",
                    }
                }
            }
        }
    }
}

/// This editor's entry in the registry: what it is called, how it is drawn in
/// a header, whether it takes a target, and what the status bar says while it
/// has focus. Declared beside the editor so adding one is one file.
pub(crate) fn kind() -> immersion::EditorKind {
    immersion::EditorKind {
        id: "fleet",
        label: "Fleet",
        icon: "binary-tree",
        hints: &[],
        targets: false,
    }
}

/// The fleet as a tally. The list answers "what is each agent doing"; this
/// answers "how many, and how big".
pub(crate) fn sidebar(d: &Draw) -> Element {
    let s = &d.state;
    let n = |status: &str| s.fleet.iter().filter(|a| a.status == status).count();
    let procs: usize = s.fleet.iter().map(|a| a.procs.len()).sum();
    let rss: f64 = s
        .fleet
        .iter()
        .flat_map(|a| a.procs.iter())
        .map(|p| p.rss)
        .sum();
    rsx! {
        div { class: "area-props",
            Panel { title: "Fleet",
                {prop("Agents", s.fleet.len().to_string())}
                {prop("Busy", n("busy").to_string())}
                {prop("Idle", n("idle").to_string())}
                {prop("Waiting", n("waiting").to_string())}
            }
            Panel { title: "Processes", open: false,
                {prop("Tracked", procs.to_string())}
                {prop("Resident", gib(rss))}
            }
        }
    }
}

/// The bottom line: how many, and how many of them are working.
pub(crate) fn footer(d: &Draw) -> String {
    let busy = d.state.fleet.iter().filter(|a| a.status == "busy").count();
    format!("{} agents · {busy} busy", d.state.fleet.len())
}
