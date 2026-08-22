//! The info editor.

use dioxus::prelude::*;

use crate::ui::State;
use crate::ui::{hhmmss, short};

/// The Info log: every command that ran, newest first — Blender's Info editor.
/// The workbench's own audit trail, and the more useful now that an agent
/// drives the same command bus: this is where you see what it did.
pub(crate) fn ed_info(s: &State) -> Element {
    rsx! {
        div { class: "info-log",
            if s.log.is_empty() {
                div { class: "note", "no commands yet" }
            }
            for (i, e) in s.log.iter().enumerate() {
                div {
                    class: if e.ok { "log-row" } else { "log-row failed" },
                    key: "{i}-{e.at}",
                    "data-im-menu": "{row_menu(e)}",
                    "data-filter-text": "{e.name} {e.source} {e.params}",
                    span { class: "when", "{hhmmss(e.at)}" }
                    span { class: "src {e.source}", "{e.source}" }
                    span { class: "k", "{e.name}" }
                    span { class: "note", "{short(&e.params.to_string(), 80)}" }
                }
            }
        }
    }
}

/// A log row's menu: the call that would do this again.
///
/// Blender's Info editor shows every operator as the Python that ran it, and
/// that is what makes it more than a receipt — you can copy a line out of the
/// log and into a script. Ours is the same idea in this workbench's language:
/// the MCP tool name and the params, which is the sentence an agent is given.
fn row_menu(e: &crate::ui::LogEntry) -> String {
    let call = agent_call(&e.name, &e.params);
    immersion::menu_json(&[
        immersion::MenuItem::new(
            "Copy as agent call",
            "copy_value",
            serde_json::json!({ "value": call }),
        ),
        immersion::MenuItem::new(
            "Copy parameters",
            "copy_value",
            serde_json::json!({ "value": e.params.to_string() }),
        ),
    ])
}

/// `workspace.add {"name":"x"}` as `workspace_add {"name":"x"}` — the tool
/// spelling, because the point is to paste it somewhere an agent reads.
pub(crate) fn agent_call(name: &str, params: &serde_json::Value) -> String {
    let tool = crate::mcp::tool_name(name);
    if params.is_null() {
        tool
    } else {
        format!("{tool} {params}")
    }
}

/// This editor's entry in the registry: what it is called, how it is drawn in
/// a header, whether it takes a target, and what the status bar says while it
/// has focus. Declared beside the editor so adding one is one file.
pub(crate) fn kind() -> immersion::EditorKind {
    immersion::EditorKind {
        id: "info",
        label: "Info log",
        icon: "info-circle",
        hints: &[("Type", "Filter")],
        targets: false,
    }
}

#[cfg(test)]
mod agent_call_tests {
    use super::agent_call;

    /// The whole value of the row is that what you copy is what an agent
    /// runs. MCP spells `workspace.add` as `workspace_add`, so a log row that
    /// copied the command name verbatim would hand over a call that does not
    /// exist — and the mistake is invisible until someone pastes it.
    #[test]
    fn a_copied_row_is_spelled_the_way_the_tool_is() {
        assert_eq!(
            agent_call("workspace.add", &serde_json::json!({ "name": "x" })),
            r#"workspace_add {"name":"x"}"#
        );
        assert_eq!(
            agent_call("split", &serde_json::json!({ "id": 1, "dir": "row" })),
            // serde_json orders object keys alphabetically, which is a fine
            // and stable thing for something meant to be pasted.
            r#"split {"dir":"row","id":1}"#
        );
        // Undo takes nothing, and `undo null` is not a call anyone would run.
        assert_eq!(agent_call("undo", &serde_json::Value::Null), "undo");
    }

    /// And the tool it names is one that exists. A row offering a call the
    /// server does not answer is worse than no row.
    #[test]
    fn the_tool_it_names_is_one_the_server_has() {
        for c in crate::workflows::commands().iter() {
            let call = agent_call(c.name, &serde_json::Value::Null);
            assert!(
                crate::mcp::tools().iter().any(|t| t.name == call.as_str())
                    || call == "load_layout",
                "{} copies as {call}, which is not a tool",
                c.name
            );
        }
    }
}
