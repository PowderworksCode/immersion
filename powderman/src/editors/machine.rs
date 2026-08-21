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

/// The window the plots cover, in the header — the one control that changes
/// what a machine area is *about* rather than how it looks, which is what a
/// Blender header carries.
///
/// It writes `/chart_window/0`, the hours, and leaves the sample count and
/// smoothing beside it alone.
pub(crate) fn header(d: &Draw) -> Element {
    let hours = window_hours(&d.settings);
    let on_setting = d.on_setting;
    rsx! {
        for choice in [1_i64, 6, 24] {
            button {
                key: "{choice}",
                class: if choice == hours { "hdr-choice is-on" } else { "hdr-choice" },
                title: "show the last {choice}h",
                onclick: move |_| {
                    on_setting.call(("/chart_window/0".to_string(), serde_json::json!(choice)));
                },
                "{choice}h"
            }
        }
    }
}

/// How far back a machine area looks, from the settings document. One place,
/// because the snapshot and the header have to mean the same thing by it —
/// and because the setting existed for a while reading nothing at all.
pub(crate) fn window_hours(settings: &serde_json::Value) -> i64 {
    settings
        .pointer("/chart_window/0")
        .and_then(|h| h.as_i64())
        .filter(|h| *h > 0)
        .unwrap_or(1)
}

#[cfg(test)]
mod window_tests {
    use super::window_hours;

    /// `chart_window` sat in the settings document, offered by the
    /// preferences window, read by nothing: the snapshot was pinned to an
    /// hour. A control that appears to work is worse than one that is not
    /// there, so the reader and the writer are pinned to the same pointer.
    #[test]
    fn the_window_comes_from_the_setting_and_survives_nonsense() {
        assert_eq!(
            window_hours(&serde_json::json!({ "chart_window": [6, 60, 3] })),
            6
        );
        // The default, for a document that has never been written.
        assert_eq!(window_hours(&serde_json::json!({})), 1);
        // A zero or negative window would ask the database for a range that
        // ends before it starts, which draws nothing and looks like a bug in
        // the metrics rather than in the setting.
        for bad in [
            serde_json::json!({ "chart_window": [0, 60, 3] }),
            serde_json::json!({ "chart_window": [-4, 60, 3] }),
            serde_json::json!({ "chart_window": "an hour" }),
            serde_json::json!({ "chart_window": [] }),
        ] {
            assert_eq!(window_hours(&bad), 1, "{bad} should fall back");
        }
    }
}
