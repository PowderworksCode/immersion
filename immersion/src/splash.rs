//! The startup splash: Blender's, and Immersion's before it.
//!
//! A centered modal over a dimmed backdrop with two columns — preset layout
//! templates on the left, recent things to jump back into on the right — and
//! a "don't show this again" opt-out. Blender's is new-file templates plus
//! recent .blend files; here it is starter arrangements plus whatever the host
//! calls recent.
//!
//! Like the rest of the library, this knows nothing of the host's domain. It
//! renders the templates and recents it is handed and reports which was
//! picked; the host owns what a template *is* (a `Layout`) and what a recent
//! *does* (an opaque key it maps back to an action). Show/dismiss is host
//! state, so the same splash can open on load, from a menu, or never.

use dioxus::prelude::*;

use crate::area::Layout;

/// A preset layout offered on the splash. The host builds the tree.
#[derive(Debug, Clone, PartialEq)]
pub struct Template {
    pub name: String,
    /// One line under the name — what the arrangement is for.
    pub hint: String,
    pub layout: Layout,
}

/// A recent item to jump back into. `key` is opaque to the library; the host
/// maps it to an action in `on_recent`.
#[derive(Debug, Clone, PartialEq)]
pub struct SplashRecent {
    pub label: String,
    pub sub: String,
    /// Drives the status dot's color via a `status-<value>` class, so it reads
    /// the same as a run's status elsewhere. Empty for none.
    pub status: String,
    pub key: String,
}

#[derive(Props, Clone)]
pub struct SplashProps {
    pub brand: String,
    pub subtitle: String,
    pub templates: Vec<Template>,
    pub recents: Vec<SplashRecent>,
    /// Picked a template by index — the host adds a workspace from its layout.
    pub on_template: Callback<usize>,
    /// Picked a recent by key.
    pub on_recent: Callback<String>,
    /// Backdrop click, Escape, or any pick — the host hides the splash.
    pub on_dismiss: Callback<()>,
    pub dont_show: bool,
    pub on_dont_show: Callback<bool>,
}

impl PartialEq for SplashProps {
    fn eq(&self, other: &Self) -> bool {
        self.brand == other.brand
            && self.subtitle == other.subtitle
            && self.templates == other.templates
            && self.recents == other.recents
            && self.dont_show == other.dont_show
    }
}

#[component]
pub fn Splash(props: SplashProps) -> Element {
    let on_template = props.on_template;
    let on_recent = props.on_recent;
    let on_dismiss = props.on_dismiss;
    let on_dont_show = props.on_dont_show;
    let dont_show = props.dont_show;

    rsx! {
        // The backdrop dismisses; the card stops the click so a miss inside it
        // does not close the splash.
        div {
            class: "im-splash-backdrop",
            onclick: move |_| on_dismiss.call(()),
            tabindex: "0",
            autofocus: true,
            onkeydown: move |e| {
                if e.key() == Key::Escape {
                    on_dismiss.call(());
                }
            },
            div {
                class: "im-splash",
                onclick: move |e| e.stop_propagation(),
                div { class: "im-splash-head",
                    span { class: "im-splash-brand", "{props.brand}" }
                    span { class: "im-splash-sub", "{props.subtitle}" }
                }
                div { class: "im-splash-cols",
                    div { class: "im-splash-col",
                        div { class: "im-splash-h", "New workspace" }
                        for (i, t) in props.templates.iter().cloned().enumerate() {
                            {template_row(i, t, on_template, on_dismiss)}
                        }
                    }
                    div { class: "im-splash-col",
                        div { class: "im-splash-h", "Recent" }
                        if props.recents.is_empty() {
                            div { class: "im-splash-empty", "nothing recent yet" }
                        }
                        for r in props.recents.iter().cloned() {
                            {recent_row(r, on_recent, on_dismiss)}
                        }
                    }
                }
                label { class: "im-splash-foot",
                    input {
                        r#type: "checkbox",
                        checked: dont_show,
                        onchange: move |e| on_dont_show.call(e.checked()),
                    }
                    "Don't show on startup"
                }
            }
        }
    }
}

fn template_row(
    i: usize,
    t: Template,
    on_template: Callback<usize>,
    on_dismiss: Callback<()>,
) -> Element {
    rsx! {
        div {
            class: "im-splash-item",
            onclick: move |_| {
                on_template.call(i);
                on_dismiss.call(());
            },
            div { class: "im-splash-name", "{t.name}" }
            div { class: "im-splash-hint", "{t.hint}" }
        }
    }
}

fn recent_row(r: SplashRecent, on_recent: Callback<String>, on_dismiss: Callback<()>) -> Element {
    let key = r.key.clone();
    rsx! {
        div {
            class: "im-splash-item",
            onclick: move |_| {
                on_recent.call(key.clone());
                on_dismiss.call(());
            },
            div { class: "im-splash-name",
                if !r.status.is_empty() {
                    span { class: "im-dot status {r.status}" }
                }
                "{r.label}"
            }
            div { class: "im-splash-hint", "{r.sub}" }
        }
    }
}
