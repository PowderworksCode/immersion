//! The machine editor.

use dioxus::prelude::*;

use immersion::Panel;

use crate::editors::{Draw, prop};
use crate::ui::State;
use crate::ui::{gib, tile};

pub(crate) fn ed_machine(s: &State, doc: &serde_json::Value) -> Element {
    rsx! {
        div { class: "tiles",
            {tile("cpu", format!("{:.0}%", s.machine.get("box.cpu_pct").copied().unwrap_or(0.0)), None)}
            {tile("load", format!("{:.2}", s.machine.get("box.load1").copied().unwrap_or(0.0)), None)}
            {tile("memory", gib(s.machine.get("box.mem_used").copied().unwrap_or(0.0)),
                  Some(format!("of {}", gib(s.machine.get("box.mem_total").copied().unwrap_or(0.0)))))}
            {tile("disk", gib(s.machine.get("box.disk_used").copied().unwrap_or(0.0)),
                  Some(format!("of {}", gib(s.machine.get("box.disk_total").copied().unwrap_or(0.0)))))}
            {tile("agents", format!("{}", s.fleet.len()), None)}
        }

        // The plots are documents now: /charts/cpu and /charts/memory. Editing
        // either in the chart editor changes what this editor draws, which is
        // the point of charts being specs rather than a drawing routine.
        div { class: "plot",
            div { class: "cap",
                b { "cpu" }
                span { "now " b { "{s.machine.get(\"box.cpu_pct\").copied().unwrap_or(0.0):.0}%" } }
            }
            {crate::charts::chart_element(s, doc, "cpu")}
        }
        div { class: "plot",
            div { class: "cap",
                b { "memory" }
                span { "now " b { "{gib(s.machine.get(\"box.mem_used\").copied().unwrap_or(0.0))}" } }
            }
            {crate::charts::chart_element(s, doc, "memory")}
        }
    }
}

/// This editor's entry in the registry: what it is called, how it is drawn in
/// a header, whether it takes a target, and what the status bar says while it
/// has focus. Declared beside the editor so adding one is one file.
pub(crate) fn kind() -> immersion::EditorKind {
    immersion::EditorKind {
        id: "machine",
        label: "Machine",
        icon: "server-2",
        hints: &[],
        targets: false,
    }
}

/// What the machine editor is showing, as numbers rather than plots. The
/// tiles round hard for legibility; this is where the reading is exact.
pub(crate) fn sidebar(d: &Draw) -> Element {
    let s = &d.state;
    let m = |k: &str| s.machine.get(k).copied().unwrap_or(0.0);
    let minutes = (s.window.1 - s.window.0).max(0) / 60_000;
    rsx! {
        div { class: "area-props",
            Panel { title: "Machine",
                {prop("CPU", format!("{:.1}%", m("box.cpu_pct")))}
                {prop("Load", format!("{:.2}", m("box.load1")))}
                {prop("Memory", format!("{} / {}", gib(m("box.mem_used")), gib(m("box.mem_total"))))}
                {prop("Disk", format!("{} / {}", gib(m("box.disk_used")), gib(m("box.disk_total"))))}
                {prop("Agents", s.fleet.len().to_string())}
            }
            Panel { title: "Samples", open: false,
                {prop("CPU points", s.cpu.len().to_string())}
                {prop("Memory points", s.mem.len().to_string())}
                {prop("Window", format!("{minutes} min"))}
                {prop("Annotations", s.annotations.len().to_string())}
            }
        }
    }
}

/// The bottom line: the two readings you glance at, without opening the N
/// panel to get them.
pub(crate) fn footer(d: &Draw) -> String {
    let m = |k: &str| d.state.machine.get(k).copied().unwrap_or(0.0);
    format!(
        "cpu {:.0}% · load {:.2} · {} agents",
        m("box.cpu_pct"),
        m("box.load1"),
        d.state.fleet.len()
    )
}
