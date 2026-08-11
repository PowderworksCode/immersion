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
use serde::Deserialize;

use crate::area::{Area, AreaId, Dir, Layout};

/// What the gesture shim commits, one message per completed drag. Each maps to
/// a bus command, so a drag and a header-button click are the same operation.
#[derive(Deserialize)]
#[serde(tag = "t", rename_all = "lowercase")]
enum Gesture {
    Ratio { id: AreaId, ratio: f32 },
    Split { id: AreaId, dir: Dir, frac: f32 },
    Join { survivor: AreaId, victim: AreaId },
}

impl Gesture {
    fn command(self) -> (&'static str, serde_json::Value) {
        match self {
            Gesture::Ratio { id, ratio } => {
                ("ratio", serde_json::json!({ "id": id, "ratio": ratio }))
            }
            Gesture::Split { id, dir, frac } => {
                let d = if matches!(dir, Dir::Row) {
                    "row"
                } else {
                    "col"
                };
                (
                    "split",
                    serde_json::json!({ "id": id, "dir": d, "frac": frac }),
                )
            }
            Gesture::Join { survivor, victim } => (
                "join_into",
                serde_json::json!({ "survivor": survivor, "victim": victim }),
            ),
        }
    }
}

const GESTURES_JS: &str = include_str!("gestures.js");

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
    /// Renders a leaf's body from `(area id, editor kind, argument)`. The host
    /// closes over its own state here; the library never sees it. The area id
    /// lets an editor act on its own area — a list that opens an item into a
    /// split needs to know where it is.
    pub render: Callback<(AreaId, String, Option<String>), Element>,
    /// The single write path: `(command name, JSON params)`. The header
    /// buttons, the editor dropdown, and the gesture shim all emit through
    /// here, so there is exactly one way to mutate the layout — the property
    /// that lets undo, the keymap, and a future agent reach everything.
    pub on_command: Callback<(String, serde_json::Value)>,
}

impl PartialEq for AreasProps {
    fn eq(&self, other: &Self) -> bool {
        // The callback is identity-stable per mount; the layout and kinds are
        // what decide whether a re-render matters.
        self.layout == other.layout && self.kinds == other.kinds
    }
}

/// The whole tiling surface. Give it the tree and the callbacks; it owns
/// nothing.
#[component]
pub fn Areas(props: AreasProps) -> Element {
    let on_command = props.on_command;

    // The shim installs once per page and speaks back over the eval channel:
    // one JSON message per completed drag, mapped to a bus command and sent to
    // the same callback the header buttons use. The gesture is client-side;
    // the mutation is a command.
    use_future(move || async move {
        let mut channel = dioxus::document::eval(GESTURES_JS);
        loop {
            let Ok(raw) = channel.recv::<String>().await else {
                return; // channel closed; a reload re-installs
            };
            if let Ok(g) = serde_json::from_str::<Gesture>(&raw) {
                let (name, params) = g.command();
                on_command.call((name.to_string(), params));
            }
        }
    });

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
        Area::Leaf { id, editor, arg } => render_leaf(*id, editor, arg.clone(), props, lone),
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
            let (handle_dir, handle_pos) = match dir {
                Dir::Row => ("row", format!("left: {pct}%")),
                Dir::Col => ("col", format!("top: {pct}%")),
            };
            rsx! {
                div { class: "{cls}", key: "{id}",
                    div { class: "im-cell", style: "flex-basis: {pct}%",
                        {render_node(a, props, false)}
                    }
                    div { class: "im-cell", style: "flex-basis: {100.0 - pct}%",
                        {render_node(b, props, false)}
                    }
                    // The resize handle rides the seam. It is chrome, not
                    // layout: absolutely positioned over the border so the
                    // cells' geometry stays a pure function of the ratio.
                    div {
                        class: "im-seam-handle im-seam-{handle_dir}",
                        style: "{handle_pos}",
                        "data-im-seam": "{id}",
                        "data-im-dir": "{handle_dir}",
                    }
                }
            }
        }
    }
}

fn render_leaf(
    id: AreaId,
    editor: &str,
    arg: Option<String>,
    props: &AreasProps,
    lone: bool,
) -> Element {
    let kinds = props.kinds.clone();
    let editor_owned = editor.to_string();
    let cmd = props.on_command;
    let body = props.render.call((id, editor_owned.clone(), arg));

    rsx! {
        div { class: "im-area", key: "{id}", "data-im-area": "{id}",
            // Corner grips: invisible hit-zones in all four corners, per the
            // locked decision — no visual reveal in any state; the diagonal
            // resize cursor is the only affordance. Drag inward to split,
            // outward over a neighbour to join.
            span { class: "im-grip im-grip-tl", "data-im-grip": "tl" }
            span { class: "im-grip im-grip-tr", "data-im-grip": "tr" }
            span { class: "im-grip im-grip-bl", "data-im-grip": "bl" }
            span { class: "im-grip im-grip-br", "data-im-grip": "br" }
            div { class: "im-header",
                // The editor-type selector — Blender's leftmost header button.
                // A native <select> for phase 1: it is keyboard-accessible and
                // costs one message on change, which is the liveview budget.
                select {
                    class: "im-kind",
                    onchange: move |e| cmd.call(("set_editor".to_string(), serde_json::json!({ "id": id, "editor": e.value() }))),
                    for k in kinds.iter() {
                        option {
                            key: "{k.id}",
                            value: "{k.id}",
                            selected: k.id == editor_owned,
                            "{k.label}"
                        }
                    }
                }
                span { class: "im-tools",
                    button { class: "im-btn", title: "split horizontally",
                        onclick: move |_| cmd.call(("split".to_string(), serde_json::json!({ "id": id, "dir": "row" }))), "⬒" }
                    button { class: "im-btn", title: "split vertically",
                        onclick: move |_| cmd.call(("split".to_string(), serde_json::json!({ "id": id, "dir": "col" }))), "◧" }
                    // Close-is-join: the sibling absorbs the space. The last
                    // area has no sibling, so it gets no close button rather
                    // than a button that refuses.
                    if !lone {
                        button { class: "im-btn", title: "close (join into neighbor)",
                            onclick: move |_| cmd.call(("join".to_string(), serde_json::json!({ "id": id }))), "✕" }
                    }
                }
            }
            div { class: "im-body", {body} }
        }
    }
}

/// The topbar workspace tabs — Blender's named layouts.
///
/// A row of tabs plus a `+`. Click to switch, double-click to rename in
/// place, the `✕` to close. Rename is a local `<input>` that commits on
/// blur or Enter, so the round trip is one message per rename, not per
/// keystroke — the same liveview budget the gesture shim and the action
/// forms hold to.
#[derive(Props, Clone)]
pub struct WorkspaceTabsProps {
    pub names: Vec<String>,
    pub active: usize,
    /// `(command name, params)`, same bus as the areas. Add is a host concern
    /// (it needs a layout to add), so it stays its own callback.
    pub on_command: Callback<(String, serde_json::Value)>,
    pub on_add: Callback<()>,
}

impl PartialEq for WorkspaceTabsProps {
    fn eq(&self, other: &Self) -> bool {
        self.names == other.names && self.active == other.active
    }
}

#[component]
pub fn WorkspaceTabs(props: WorkspaceTabsProps) -> Element {
    let mut editing = use_signal(|| None::<usize>);
    // The in-progress rename text. oninput keeps it current locally; the
    // commit reads it once on blur/Enter, so the round trip is one message,
    // not one per keystroke.
    let mut draft = use_signal(String::new);
    let cmd = props.on_command;
    let on_add = props.on_add;
    let multi = props.names.len() > 1;

    rsx! {
        div { class: "im-tabs",
            for (i, name) in props.names.iter().cloned().enumerate() {
                if editing() == Some(i) {
                    input {
                        class: "im-tab-edit",
                        value: "{draft}",
                        autofocus: true,
                        oninput: move |e| draft.set(e.value()),
                        onblur: move |_| {
                            cmd.call(("workspace.rename".to_string(), serde_json::json!({ "index": i, "name": draft() })));
                            editing.set(None);
                        },
                        // Enter commits, Escape abandons the draft. Flat
                        // if/else rather than a match so the closure body does
                        // not nest another two levels inside the view tree.
                        onkeydown: move |e| {
                            let k = e.key();
                            if k == Key::Enter {
                                cmd.call(("workspace.rename".to_string(), serde_json::json!({ "index": i, "name": draft() })));
                            }
                            if k == Key::Enter || k == Key::Escape {
                                editing.set(None);
                            }
                        },
                    }
                } else {
                    div {
                        class: if i == props.active { "im-tab active" } else { "im-tab" },
                        onclick: move |_| cmd.call(("workspace.switch".to_string(), serde_json::json!({ "index": i }))),
                        ondoubleclick: {
                            let name = name.clone();
                            move |_| { draft.set(name.clone()); editing.set(Some(i)); }
                        },
                        span { class: "im-tab-name", "{name}" }
                        if multi {
                            button {
                                class: "im-tab-x",
                                onclick: move |e| { e.stop_propagation(); cmd.call(("workspace.close".to_string(), serde_json::json!({ "index": i }))); },
                                "✕"
                            }
                        }
                    }
                }
            }
            button { class: "im-tab-add", title: "new workspace",
                onclick: move |_| on_add.call(()), "+" }
        }
    }
}
