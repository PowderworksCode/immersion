//! The machine editor.

use dioxus::prelude::*;

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
