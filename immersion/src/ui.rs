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

use crate::area::{Area, AreaId, Dir, Layout, Region, Regions};

/// What the gesture shim commits, one message per completed drag. Each maps to
/// a bus command, so a drag and a header-button click are the same operation.
#[derive(Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../ts/generated/")]
#[serde(tag = "t", rename_all = "lowercase")]
pub enum Gesture {
    // Area ids are minted small and cross the wire as JSON numbers; without
    // this they would generate as `bigint`, which JSON.stringify refuses.
    Ratio {
        #[ts(type = "number")]
        id: AreaId,
        // The seam's position among the split's children — n-ary splits have
        // more than one, so the id alone no longer identifies it.
        #[ts(type = "number")]
        index: usize,
        ratio: f32,
    },
    Split {
        #[ts(type = "number")]
        id: AreaId,
        dir: Dir,
        frac: f32,
    },
    Join {
        #[ts(type = "number")]
        survivor: AreaId,
        #[ts(type = "number")]
        victim: AreaId,
    },
    Swap {
        #[ts(type = "number")]
        a: AreaId,
        #[ts(type = "number")]
        b: AreaId,
    },
    RegionWidth {
        #[ts(type = "number")]
        id: AreaId,
        region: crate::area::Region,
        w: u16,
    },
}

impl Gesture {
    fn command(self) -> (&'static str, serde_json::Value) {
        match self {
            Gesture::Ratio { id, index, ratio } => (
                "ratio",
                serde_json::json!({ "id": id, "index": index, "ratio": ratio }),
            ),
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
            Gesture::Swap { a, b } => ("swap", serde_json::json!({ "a": a, "b": b })),
            Gesture::RegionWidth { id, region, w } => (
                "set_region_width",
                serde_json::json!({ "id": id, "region": region.as_str(), "w": w }),
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
    /// The icon name from the library's sprite (`crate::icon`). Empty for an
    /// editor that has none; the header then shows its label alone.
    pub icon: &'static str,
    /// Whether this editor looks *at* something — a subtree, a file, a run.
    /// Blender's header carries a data-block selector only for editors that
    /// have one; ours shows the target chip on the same rule, so an editor
    /// with nothing to point at is not given a control that does nothing.
    pub targets: bool,
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
    /// Optional: renders a leaf's left toolbar region from `(area id, editor)`.
    /// When present, the area shows a T toggle; when absent, no toolbar.
    #[props(default)]
    pub render_toolbar: Option<Callback<(AreaId, String), Element>>,
    /// Optional: renders a leaf's right sidebar (N panel) — Blender's
    /// properties region — from `(area id, editor)`.
    #[props(default)]
    pub render_sidebar: Option<Callback<(AreaId, String), Element>>,
    /// The single write path: `(command name, JSON params)`. The header
    /// buttons, the editor dropdown, and the gesture shim all emit through
    /// here, so there is exactly one way to mutate the layout — the property
    /// that lets undo, the keymap, and a future agent reach everything.
    pub on_command: Callback<(String, serde_json::Value)>,
    /// An area was clicked: the host learns which one has focus. Blender's
    /// status bar shows the shortcuts for the area under the pointer; a click
    /// is the cheaper signal — one message when you act on an area, rather
    /// than one per mouse move — and it survives the pointer leaving.
    #[props(default)]
    pub on_focus: Option<Callback<AreaId>>,
    /// A target chip was clicked: the host opens its own picker (only it
    /// knows what can be pointed at) and answers with a `set_target` command.
    /// Without it, chips render as read-only labels.
    #[props(default)]
    pub on_pick_target: Option<Callback<AreaId>>,
    /// When set to a live area id, only that area is shown, filling the deck —
    /// Blender's maximize. View state, not layout: it is per-client and does
    /// not touch the tree, so it lives outside the command bus by design.
    #[props(default)]
    pub maximized: Option<AreaId>,
    /// `action -> chord` for the actions the chrome exposes, already written
    /// the platform's way. The header's buttons show theirs in the tooltip, so
    /// a control and its shortcut are learned together.
    #[props(default)]
    pub chords: std::collections::HashMap<String, String>,
    /// A version of whatever host state the editors read. The deck memoizes on
    /// its props, so without this a change the layout does not encode — a
    /// setting, a rebind — would leave the editor bodies stale until something
    /// else moved. The host bumps it; any value that changes will do.
    #[props(default)]
    pub revision: u64,
}

impl PartialEq for AreasProps {
    fn eq(&self, other: &Self) -> bool {
        // The callback is identity-stable per mount; the layout, kinds, and
        // maximize target are what decide whether a re-render matters.
        self.layout == other.layout
            && self.kinds == other.kinds
            && self.maximized == other.maximized
            && self.revision == other.revision
            && self.chords == other.chords
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
    // Maximize: if the target is a real leaf, show just it, full-deck. A leaf
    // shown alone gets no close button (see `lone`), which reads right — a
    // maximized area is momentarily the only one.
    let maxed = props.maximized.and_then(|id| match root.find(id) {
        Some(node @ Area::Leaf { .. }) => Some(node.clone()),
        _ => None,
    });
    let lone = matches!(root, Area::Leaf { .. }) || maxed.is_some();
    rsx! {
        div { class: "im-root",
            if let Some(node) = maxed {
                {render_node(&node, &props, true)}
            } else {
                {render_node(&root, &props, lone)}
            }
        }
    }
}

fn render_node(node: &Area, props: &AreasProps, lone: bool) -> Element {
    match node {
        Area::Leaf {
            id,
            editor,
            arg,
            regions,
        } => render_leaf(*id, editor, arg.clone(), regions.clone(), props, lone),
        Area::Split {
            id,
            dir,
            sizes,
            children,
        } => {
            let (cls, handle_dir) = match dir {
                Dir::Row => ("im-split im-row", "row"),
                Dir::Col => ("im-split im-col", "col"),
            };
            // Seams sit at the running total of the sizes before them, so a
            // split of three has two handles and each moves only its own pair.
            let mut cum = 0.0f32;
            let seams: Vec<(usize, f32)> = sizes
                .iter()
                .take(sizes.len().saturating_sub(1))
                .map(|s| {
                    cum += *s;
                    cum * 100.0
                })
                .enumerate()
                .collect();
            rsx! {
                div { class: "{cls}", key: "{id}",
                    for (i, child) in children.iter().enumerate() {
                        div {
                            class: "im-cell",
                            key: "{child.id()}",
                            style: "flex-basis: {sizes.get(i).copied().unwrap_or(0.0) * 100.0}%",
                            {render_node(child, props, false)}
                        }
                    }
                    // The resize handles ride the seams. They are chrome, not
                    // layout: absolutely positioned over each border so the
                    // cells' geometry stays a pure function of the sizes.
                    for (i, pos) in seams.iter().copied() {
                        div {
                            class: "im-seam-handle im-seam-{handle_dir}",
                            key: "seam-{i}",
                            style: if handle_dir == "row" { "left: {pos}%" } else { "top: {pos}%" },
                            "data-im-seam": "{id}",
                            "data-im-seam-index": "{i}",
                            "data-im-dir": "{handle_dir}",
                        }
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
    regions: Regions,
    props: &AreasProps,
    lone: bool,
) -> Element {
    let kinds = props.kinds.clone();
    let editor_owned = editor.to_string();
    let cmd = props.on_command;
    let body = props.render.call((id, editor_owned.clone(), arg.clone()));
    let menu = crate::contextmenu::area_menu_json(id);
    // Region content is the host's — a leaf shows a toolbar / sidebar only if
    // the host offers one and the region is toggled on.
    let has_toolbar = props.render_toolbar.is_some();
    let has_sidebar = props.render_sidebar.is_some();
    let toolbar = if regions.toolbar {
        props
            .render_toolbar
            .map(|cb| cb.call((id, editor_owned.clone())))
    } else {
        None
    };
    let sidebar = if regions.sidebar {
        props
            .render_sidebar
            .map(|cb| cb.call((id, editor_owned.clone())))
    } else {
        None
    };
    // Copied out of props before the view: a Callback is Copy, the props
    // reference is not, and the handler outlives this function.
    let on_focus = props.on_focus;
    let toolbar_style = if regions.toolbar_w > 0 {
        format!("width:{}px", regions.toolbar_w)
    } else {
        String::new()
    };
    let sidebar_style = if regions.sidebar_w > 0 {
        format!("width:{}px", regions.sidebar_w)
    } else {
        String::new()
    };

    rsx! {
        div {
            class: "im-area",
            key: "{id}",
            "data-im-area": "{id}",
            "data-im-menu": "{menu}",
            onpointerdown: move |_| {
                if let Some(focus) = on_focus {
                    focus.call(id);
                }
            },
            // Corner grips: invisible hit-zones in all four corners, per the
            // locked decision — no visual reveal in any state; the diagonal
            // resize cursor is the only affordance. Drag inward to split,
            // outward over a neighbour to join.
            span { class: "im-grip im-grip-tl", "data-im-grip": "tl" }
            span { class: "im-grip im-grip-tr", "data-im-grip": "tr" }
            span { class: "im-grip im-grip-bl", "data-im-grip": "bl" }
            span { class: "im-grip im-grip-br", "data-im-grip": "br" }
            if !regions.header_hidden && !regions.header_bottom {
                {leaf_header(LeafHeader {
                    id,
                    editor: editor_owned.clone(),
                    target: arg.clone(),
                    kinds: kinds.clone(),
                    cmd,
                    on_pick_target: props.on_pick_target,
                }, &regions, has_toolbar, has_sidebar, lone, &props.chords)}
            }
            if regions.header_hidden {
                button {
                    class: "im-header-stub",
                    title: "show header",
                    onclick: move |_| cmd.call(("toggle_region".to_string(), serde_json::json!({ "id": id, "region": "header" }))),
                    "▾"
                }
            }
            div { class: "im-area-main",
                if let Some(t) = toolbar {
                    div { class: "im-toolbar", style: "{toolbar_style}",
                        {t}
                        span { class: "im-region-handle im-region-handle-r", "data-im-region-handle": "{Region::Toolbar.as_str()}" }
                    }
                }
                div { class: "im-body", {body} }
                if let Some(sb) = sidebar {
                    div { class: "im-sidebar", style: "{sidebar_style}",
                        span { class: "im-region-handle im-region-handle-l", "data-im-region-handle": "{Region::Sidebar.as_str()}" }
                        {sb}
                    }
                }
            }
            if !regions.header_hidden && regions.header_bottom {
                {leaf_header(LeafHeader {
                    id,
                    editor: editor_owned.clone(),
                    target: arg.clone(),
                    kinds: kinds.clone(),
                    cmd,
                    on_pick_target: props.on_pick_target,
                }, &regions, has_toolbar, has_sidebar, lone, &props.chords)}
            }
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

/// The `data-im-menu` JSON for a workspace tab — duplicate it, or close it.
fn tab_menu_json(index: usize, last: usize) -> String {
    use crate::contextmenu::{MenuItem, menu_json};
    let mut items = vec![MenuItem::new(
        "Duplicate",
        "workspace.duplicate",
        serde_json::json!({}),
    )];
    // Reordering by menu as well as by drag: a drag is the quick way and a
    // menu is the one that works from a trackpad, a screen reader, or an
    // agent reading the same item list a person sees.
    if index > 0 {
        items.push(MenuItem::new(
            "Move left",
            "workspace.move",
            serde_json::json!({ "from": index, "to": index - 1 }),
        ));
    }
    if index < last {
        items.push(MenuItem::new(
            "Move right",
            "workspace.move",
            serde_json::json!({ "from": index, "to": index + 1 }),
        ));
    }
    items.push(MenuItem::sep());
    items.push(MenuItem::new(
        "Close",
        "workspace.close",
        serde_json::json!({ "index": index }),
    ));
    menu_json(&items)
}

#[component]
pub fn WorkspaceTabs(props: WorkspaceTabsProps) -> Element {
    let mut editing = use_signal(|| None::<usize>);
    // Which tab is being dragged. Per-client and never persisted: a drag in
    // one browser is not a drag in another, and the reorder itself commits
    // one command on drop.
    let mut dragging = use_signal(|| None::<usize>);
    // The in-progress rename text. oninput keeps it current locally; the
    // commit reads it once on blur/Enter, so the round trip is one message,
    // not one per keystroke.
    let mut draft = use_signal(String::new);
    let cmd = props.on_command;
    let on_add = props.on_add;
    let multi = props.names.len() > 1;
    // Its own callback rather than a closure in the view: the drop needs two
    // conditions and a command call, which inside the tab loop is four levels
    // deeper than anything else in this file.
    let drop_tab = use_callback(move |onto: usize| {
        let from = dragging();
        dragging.set(None);
        match from {
            Some(from) if from != onto => cmd.call((
                "workspace.move".to_string(),
                serde_json::json!({ "from": from, "to": onto }),
            )),
            _ => {}
        }
    });

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
                        "data-im-menu": "{tab_menu_json(i, props.names.len() - 1)}",
                        // Blender reorders workspace tabs by dragging them.
                        // The drag is the browser's own — no shim, no messages
                        // while it moves — and the drop commits one command.
                        draggable: true,
                        ondragstart: move |_| dragging.set(Some(i)),
                        ondragover: move |e| e.prevent_default(),
                        ondrop: move |e| {
                            e.prevent_default();
                            drop_tab.call(i);
                        },
                        ondragend: move |_| dragging.set(None),
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

/// The area header — editor switcher, View menu, and the tool buttons. Built
/// as its own function so it can be placed above or below the body (Blender's
/// flip) without duplicating the markup.
#[allow(clippy::too_many_arguments)]
/// What the header needs about the leaf it belongs to. A struct because the
/// argument list had reached the point where the next one would be a bug.
struct LeafHeader {
    id: AreaId,
    editor: String,
    target: Option<String>,
    kinds: Vec<EditorKind>,
    cmd: Callback<(String, serde_json::Value)>,
    on_pick_target: Option<Callback<AreaId>>,
}

fn leaf_header(
    leaf: LeafHeader,
    regions: &Regions,
    has_toolbar: bool,
    has_sidebar: bool,
    lone: bool,
    chords: &std::collections::HashMap<String, String>,
) -> Element {
    let LeafHeader {
        id,
        editor: editor_owned,
        target,
        kinds,
        cmd,
        on_pick_target,
    } = leaf;
    let targets = kinds
        .iter()
        .find(|k| k.id == editor_owned)
        .is_some_and(|k| k.targets);
    let tip = |action: &str| chords.get(action).cloned().unwrap_or_default();
    rsx! {
            div { class: "im-header",
                // The editor-type selector — Blender's leftmost header button.
                // A native <select> for phase 1: it is keyboard-accessible and
                // costs one message on change, which is the liveview budget.
                button {
                    class: "im-kind",
                    title: "editor type",
                    "data-im-menu-click": "{crate::contextmenu::editor_menu_json(id, &kinds, &editor_owned)}",
                    dangerous_inner_html: "{kind_button_html(&kinds, &editor_owned)}",
                }
                // The target chip — Blender's data-block selector. Shows what
                // this editor is looking at, and opens the host's picker.
                if targets {
                    button {
                        class: if target.is_some() { "im-target" } else { "im-target im-target-none" },
                        title: match &target {
                            Some(t) => format!("looking at {t} \u{2014} click to retarget"),
                            None => "nothing selected \u{2014} click to pick".to_string(),
                        },
                        disabled: on_pick_target.is_none(),
                        onclick: move |_| {
                            if let Some(pick) = on_pick_target {
                                pick.call(id);
                            }
                        },
                        {crumb(target.as_deref())}
                    }
                }
                button {
                    class: "im-viewmenu",
                    title: "view menu",
                    "data-tip": "View menu",
                    "data-tip-key": "{tip(\"pie\")}",
                    "data-im-menu-click": "{crate::contextmenu::view_menu_json(id, regions.toolbar, regions.sidebar, has_toolbar || has_sidebar)}",
                    "View"
                }
                span { class: "im-tools",
                    if has_toolbar {
                        button {
                            class: if regions.toolbar { "im-btn active" } else { "im-btn" },
                            title: "toggle toolbar",
                            "data-tip": "Toggle toolbar",
                            "data-tip-key": "{tip(\"toggle_toolbar\")}",
                            onclick: move |_| cmd.call(("toggle_region".to_string(), serde_json::json!({ "id": id, "region": Region::Toolbar.as_str() }))),
                            "T"
                        }
                    }
                    if has_sidebar {
                        button {
                            class: if regions.sidebar { "im-btn active" } else { "im-btn" },
                            title: "toggle sidebar",
                            "data-tip": "Toggle sidebar",
                            "data-tip-key": "{tip(\"toggle_sidebar\")}",
                            onclick: move |_| cmd.call(("toggle_region".to_string(), serde_json::json!({ "id": id, "region": Region::Sidebar.as_str() }))),
                            "N"
                        }
                    }
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
    }
}

/// A target as a breadcrumb: the last two segments, so a deep pointer still
/// fits a header. The full value is in the chip's tooltip.
fn crumb(target: Option<&str>) -> String {
    let Some(t) = target.filter(|t| !t.is_empty()) else {
        return "\u{25c7} none".to_string();
    };
    let parts: Vec<&str> = t.split('/').filter(|p| !p.is_empty()).collect();
    let tail = if parts.len() > 2 {
        format!("\u{2026}/{}", parts[parts.len() - 2..].join("/"))
    } else if parts.is_empty() {
        t.to_string()
    } else {
        parts.join("/")
    };
    // An opaque id — a run's, say — has no segments to trim, and a full one
    // is wider than the header it sits in. Its first characters identify it.
    let tail = if !tail.contains('/') && tail.chars().count() > 12 {
        format!("{}\u{2026}", tail.chars().take(8).collect::<String>())
    } else {
        tail
    };
    format!("\u{25c6} {tail}")
}

/// The editor-selector button's contents: the icon, the label, the chevron.
/// Built as markup rather than elements because the icon is markup — a sprite
/// glyph is an `<svg>` string, and threading it through rsx as children would
/// mean parsing it back into nodes for no gain.
fn kind_button_html(kinds: &[EditorKind], current: &str) -> String {
    let found = kinds.iter().find(|k| k.id == current);
    let label = found.map(|k| k.label).unwrap_or("editor");
    let icon = found
        .map(|k| crate::icons::icon(k.icon))
        .unwrap_or_default();
    format!(
        "{icon}<span class=\"im-kind-label\">{label}</span><span class=\"im-kind-caret\">▾</span>"
    )
}

#[cfg(test)]
mod crumb_tests {
    use super::crumb;

    #[test]
    fn a_deep_pointer_still_fits_a_header() {
        assert_eq!(crumb(None), "\u{25c7} none");
        assert_eq!(crumb(Some("")), "\u{25c7} none");
        assert_eq!(crumb(Some("/settings")), "\u{25c6} settings");
        assert_eq!(
            crumb(Some("/settings/favorites")),
            "\u{25c6} settings/favorites"
        );
        assert_eq!(
            crumb(Some("/state/runs/3/steps/7")),
            "\u{25c6} \u{2026}/steps/7",
            "long pointers keep the end, which is the part that identifies it"
        );
        // A run id is not a pointer: it keeps its own shape, shortened only
        // when a full one would be wider than the header.
        assert_eq!(crumb(Some("9f3a1c2e")), "\u{25c6} 9f3a1c2e");
        assert_eq!(
            crumb(Some("5c7e9a1b3d5f7091a3c5e7d9b1f3a5c7")),
            "\u{25c6} 5c7e9a1b\u{2026}"
        );
    }
}
