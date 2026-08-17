//! The help editor.
//!
//! The same value `docs/reference.md` is rendered from, drawn in an area. Not
//! the file — the registries — so the page in the workbench cannot disagree
//! with the file in the repository, and neither can be out of date with the
//! code, because there is no third copy to update.

use dioxus::prelude::*;
use immersion::{FilterBox, Panel};

use crate::editors::Draw;
use crate::reference::{Entry, Section, reference};

pub(crate) fn ed_help(_d: &Draw) -> Element {
    let sections = reference();
    rsx! {
        div { class: "help im-filter-scope",
            div { class: "keymap-head",
                FilterBox { placeholder: "filter the reference…" }
            }
            for section in sections.into_iter() {
                {section_panel(section)}
            }
        }
    }
}

/// One section, collapsible. Open by default: a reference that starts closed
/// is a reference you have to unpack before you can search it, and the filter
/// box above only narrows what is on the page.
fn section_panel(section: Section) -> Element {
    rsx! {
        Panel { title: "{section.title}",
            div { class: "help-about", "{section.about}" }
            for entry in section.entries.into_iter() {
                {entry_row(entry)}
            }
        }
    }
}

fn entry_row(entry: Entry) -> Element {
    // Everything the row says goes into the filter's haystack, so searching
    // for "sidebar" finds the command whose name never mentions it.
    let haystack = format!(
        "{} {} {}",
        entry.name,
        entry.detail,
        entry
            .params
            .iter()
            .map(|p| format!("{} {}", p.name, p.about))
            .collect::<Vec<_>>()
            .join(" ")
    );
    rsx! {
        div { class: "help-entry", key: "{entry.name}", "data-filter-text": "{haystack}",
            div { class: "help-name",
                code { "{entry.name}" }
                span { class: "im-row-actions",
                    button {
                        class: "im-row-btn im-copy",
                        title: "copy the name",
                        "data-im-copy": "{entry.name}",
                        dangerous_inner_html: "{immersion::icon(\"copy\")}",
                    }
                }
            }
            if !entry.detail.is_empty() {
                div { class: "help-detail", "{entry.detail}" }
            }
            for p in entry.params.into_iter() {
                div { class: "help-param",
                    code { class: "help-param-name", "{p.name}" }
                    if !p.kind.is_empty() {
                        span { class: "help-param-kind", "{p.kind}" }
                    }
                    if p.required {
                        span { class: "help-param-req", "required" }
                    }
                    span { class: "help-param-about", "{p.about}" }
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
        id: "help",
        label: "Help",
        icon: "help",
        hints: &[("Type", "Filter the reference")],
        targets: false,
    }
}

/// What the reference is, beside it — the counts, which are the one thing the
/// list itself does not say.
pub(crate) fn sidebar(_d: &Draw) -> Element {
    let sections = reference();
    rsx! {
        div { class: "area-props",
            Panel { title: "Reference",
                for s in sections.iter() {
                    {crate::editors::prop(s.title, s.entries.len().to_string())}
                }
            }
        }
    }
}

pub(crate) fn footer(_d: &Draw) -> String {
    let sections = reference();
    let total: usize = sections.iter().map(|s| s.entries.len()).sum();
    format!("{total} entries · {} sections", sections.len())
}
