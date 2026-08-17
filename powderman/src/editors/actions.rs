//! The actions editor.

use dioxus::prelude::*;

use crate::ui::{State, WorkflowView, run_with};

pub(crate) fn ed_actions(s: &State) -> Element {
    rsx! {
        div { class: "actions",
            for w in s.workflows.iter().cloned() {
                {action_row(w)}
            }
        }
    }
}

/// One workflow: what it is, and a form to run it. Extracted from `ed_actions`
/// so the loop body stays shallow.
fn action_row(w: WorkflowView) -> Element {
    let example = w.example.clone();
    let name = w.name.clone();
    rsx! {
        div { class: "action", key: "{w.name}",
            div { class: "who",
                span { class: "wf", "{w.name}" }
                if let Some(sched) = w.schedule.clone() {
                    span { class: "sched", "{sched}" }
                }
            }
            div { class: "note", "{w.description}" }
            form {
                class: "go",
                onsubmit: move |e| {
                    let name = name.clone();
                    // One message on submit, not a round trip per keystroke.
                    let input = form_input(&e);
                    async move { let _ = run_with(name, input).await; }
                },
                if let Some(ex) = example.clone() {
                    input { name: "input", value: "{ex}", spellcheck: "false",
                            autocomplete: "off", class: "arg" }
                }
                button { r#type: "submit", "run" }
            }
        }
    }
}

/// The named field's text, or empty. Kept out of the closure so the onsubmit
/// body is one line.
fn form_input(e: &dioxus::prelude::Event<dioxus::events::FormData>) -> String {
    match e.get_first("input") {
        Some(dioxus::events::FormValue::Text(t)) => t,
        _ => String::new(),
    }
}

/// This editor's entry in the registry: what it is called, how it is drawn in
/// a header, whether it takes a target, and what the status bar says while it
/// has focus. Declared beside the editor so adding one is one file.
pub(crate) fn kind() -> immersion::EditorKind {
    immersion::EditorKind {
        id: "actions",
        label: "Actions",
        icon: "player-play",
        hints: &[("Click", "Trigger")],
        targets: false,
    }
}
