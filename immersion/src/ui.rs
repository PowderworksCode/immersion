//! Rendering the tree, and the header chrome every area carries.
//!
//! Nested flex boxes: a Split is a flex row or column whose children get
//! `flex: ratio` and `flex: 1-ratio`; a Leaf is a header strip plus the body
//! the host renders. The seam between siblings is a 1px border, not a gap —
//! areas meet edge to edge, per the locked flat look.
//!
//! Phase-1 interactivity is deliberately button-shaped: the header carries the
//! editor dropdown, split-horizontal, split-vertical, and close. The corner
//! gestures replace the buttons in a later phase; the *mutations* they commit
//! are the same four callbacks, so the gesture work will not touch the host.

use dioxus::prelude::*;

use crate::area::{Area, AreaId, Dir, Layout};

/// One entry in the editor-type dropdown.
#[derive(Debug, Clone, PartialEq)]
pub struct EditorKind {
    /// Registry key, stored in the tree.
    pub id: &'static str,
    /// What the dropdown shows.
    pub label: &'static str,
}

#[derive(Props, Clone)]
pub struct AreasProps {
    pub layout: Layout,
    pub kinds: Vec<EditorKind>,
    /// Renders a leaf's body. The host closes over its own state here; the
    /// library never sees it.
    pub render: Callback<(AreaId, String), Element>,
    pub on_switch: Callback<(AreaId, String)>,
    pub on_split: Callback<(AreaId, Dir)>,
    pub on_join: Callback<AreaId>,
}

impl PartialEq for AreasProps {
    fn eq(&self, other: &Self) -> bool {
        // Callbacks are identity-stable per mount; the layout and kinds are
        // what decide whether a re-render matters.
        self.layout == other.layout && self.kinds == other.kinds
    }
}

/// The whole tiling surface. Give it the tree and the callbacks; it owns
/// nothing.
#[component]
pub fn Areas(props: AreasProps) -> Element {
    let root = props.layout.root.clone();
    let lone = matches!(root, Area::Leaf { .. });
    rsx! {
        div { class: "im-root",
            {render_node(&root, &props, lone)}
        }
    }
}

fn render_node(node: &Area, props: &AreasProps, lone: bool) -> Element {
    match node {
        Area::Leaf { id, editor } => render_leaf(*id, editor, props, lone),
        Area::Split {
            id,
            dir,
            ratio,
            a,
            b,
        } => {
            let (cls, pct) = match dir {
                Dir::Row => ("im-split im-row", ratio * 100.0),
                Dir::Col => ("im-split im-col", ratio * 100.0),
            };
            rsx! {
                div { class: "{cls}", key: "{id}",
                    div { class: "im-cell", style: "flex-basis: {pct}%",
                        {render_node(a, props, false)}
                    }
                    div { class: "im-cell", style: "flex-basis: {100.0 - pct}%",
                        {render_node(b, props, false)}
                    }
                }
            }
        }
    }
}

fn render_leaf(id: AreaId, editor: &str, props: &AreasProps, lone: bool) -> Element {
    let kinds = props.kinds.clone();
    let editor_owned = editor.to_string();
    let on_switch = props.on_switch;
    let on_split = props.on_split;
    let on_join = props.on_join;
    let body = props.render.call((id, editor_owned.clone()));
    let label = kinds
        .iter()
        .find(|k| k.id == editor)
        .map(|k| k.label)
        .unwrap_or(editor);

    rsx! {
        div { class: "im-area", key: "{id}",
            div { class: "im-header",
                // The editor-type selector — Blender's leftmost header button.
                // A native <select> for phase 1: it is keyboard-accessible and
                // costs one message on change, which is the liveview budget.
                select {
                    class: "im-kind",
                    onchange: move |e| on_switch.call((id, e.value())),
                    for k in kinds.iter() {
                        option {
                            key: "{k.id}",
                            value: "{k.id}",
                            selected: k.id == editor_owned,
                            "{k.label}"
                        }
                    }
                }
                span { class: "im-title", "{label}" }
                span { class: "im-tools",
                    button { class: "im-btn", title: "split horizontally",
                        onclick: move |_| on_split.call((id, Dir::Row)), "⬒" }
                    button { class: "im-btn", title: "split vertically",
                        onclick: move |_| on_split.call((id, Dir::Col)), "◧" }
                    // Close-is-join: the sibling absorbs the space. The last
                    // area has no sibling, so it gets no close button rather
                    // than a button that refuses.
                    if !lone {
                        button { class: "im-btn", title: "close (join into neighbor)",
                            onclick: move |_| on_join.call(id), "✕" }
                    }
                }
            }
            div { class: "im-body", {body} }
        }
    }
}
