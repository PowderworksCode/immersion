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
    /// An icon name from the library's set. Empty draws the row without one,
    /// so a host that has not picked icons still gets a working splash.
    pub icon: String,
    pub layout: Layout,
}

/// A footer column. Blender's splash keeps its links down here; the old
/// Immersion kept links, the three chords worth knowing, and the line that
/// hands the page to an agent. All of them are a heading over a few rows, so
/// this is one type and the rows carry the difference.
#[derive(Debug, Clone, PartialEq)]
pub struct SplashSection {
    pub title: String,
    pub rows: Vec<SplashRow>,
}

/// A row in a footer column.
#[derive(Debug, Clone, PartialEq)]
pub enum SplashRow {
    /// A link out. `icon` may be empty.
    Link {
        label: String,
        href: String,
        icon: String,
    },
    /// A chord and what it does, drawn like the status bar's hints so the
    /// same key looks the same in both places.
    Key { chord: String, label: String },
    /// A line meant to be copied — a command, an address. Shown monospace
    /// with a copy button; the copy is client-side, so it costs no message.
    Copy { text: String },
    /// Plain prose under a heading.
    Note { text: String },
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
    /// A small line above the brand — what this thing *is*, in the register
    /// of a strapline. Empty for none.
    #[props(default)]
    pub eyebrow: String,
    /// Shown beside the brand. Empty for none.
    #[props(default)]
    pub version: String,
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
    /// Footer columns, left to right. Empty draws no footer at all.
    #[props(default)]
    pub foot: Vec<SplashSection>,
}

impl PartialEq for SplashProps {
    fn eq(&self, other: &Self) -> bool {
        self.brand == other.brand
            && self.subtitle == other.subtitle
            && self.eyebrow == other.eyebrow
            && self.version == other.version
            && self.templates == other.templates
            && self.recents == other.recents
            && self.dont_show == other.dont_show
            && self.foot == other.foot
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
                    if !props.eyebrow.is_empty() {
                        div { class: "im-splash-eyebrow", "{props.eyebrow}" }
                    }
                    div { class: "im-splash-brandline",
                        span { class: "im-splash-brand", "{props.brand}" }
                        if !props.version.is_empty() {
                            span { class: "im-splash-version", "{props.version}" }
                        }
                    }
                    div { class: "im-splash-sub", "{props.subtitle}" }
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
                if !props.foot.is_empty() {
                    div { class: "im-splash-foot-cols",
                        for section in props.foot.iter().cloned() {
                            {foot_col(section)}
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
    let glyph = crate::icons::icon(&t.icon);
    rsx! {
        div {
            class: "im-splash-item",
            onclick: move |_| {
                on_template.call(i);
                on_dismiss.call(());
            },
            if !glyph.is_empty() {
                span { class: "im-splash-icon", dangerous_inner_html: "{glyph}" }
            }
            div { class: "im-splash-itembody",
                div { class: "im-splash-name", "{t.name}" }
                div { class: "im-splash-hint", "{t.hint}" }
            }
        }
    }
}

/// One footer column: a heading over its rows. Its own function because the
/// rows-inside-a-column-inside-the-footer nest is three levels the splash's
/// view does not need to carry.
fn foot_col(section: SplashSection) -> Element {
    rsx! {
        div { class: "im-splash-foot-col", key: "{section.title}",
            div { class: "im-splash-h", "{section.title}" }
            for (i, row) in section.rows.into_iter().enumerate() {
                {foot_row(i, row)}
            }
        }
    }
}

/// One footer row. The copy button carries its payload in `data-im-copy`; the
/// client bundle handles the click, so copying never reaches the server.
fn foot_row(i: usize, row: SplashRow) -> Element {
    match row {
        SplashRow::Link { label, href, icon } => {
            let glyph = crate::icons::icon(&icon);
            rsx! {
                a {
                    class: "im-splash-link",
                    key: "{i}",
                    href: "{href}",
                    target: "_blank",
                    rel: "noreferrer",
                    if !glyph.is_empty() {
                        span { class: "im-splash-linkicon", dangerous_inner_html: "{glyph}" }
                    }
                    "{label}"
                }
            }
        }
        SplashRow::Key { chord, label } => rsx! {
            div { class: "im-splash-key", key: "{i}",
                span { class: "im-hint-key", "{chord}" }
                span { class: "im-splash-hint", "{label}" }
            }
        },
        SplashRow::Copy { text } => rsx! {
            div { class: "im-splash-copy", key: "{i}",
                code { "{text}" }
                button {
                    class: "im-copy",
                    "data-im-copy": "{text}",
                    title: "copy",
                    dangerous_inner_html: "{crate::icons::icon(\"copy\")}",
                }
            }
        },
        SplashRow::Note { text } => rsx! {
            div { class: "im-splash-note im-splash-hint", key: "{i}", "{text}" }
        },
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
