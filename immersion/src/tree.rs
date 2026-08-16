//! The tree view: expandable rows over any hierarchical value.
//!
//! One component, many editors. The host supplies children through a callback
//! — given a row's pointer, return that row's children — so the same view
//! walks a serde document (the data editor), a directory (the file browser),
//! or anything else that answers "what is under this node". Nothing loads
//! until its branch opens, which is what makes a directory tree affordable.
//!
//! Every row carries its pointer, and right-click offers *Copy data path* —
//! Blender's Copy Full Data Path, translated. The copy happens client-side in
//! the menu shim's clipboard handler; opening and browsing never touch the
//! wire beyond the expand clicks themselves (a click is commit-path, so each
//! toggle is one message).
//!
//! Expansion and selection are per-client view state, like maximize: two
//! browsers looking at one document may open different branches. If shared
//! expansion turns out to be wanted — Blender persists outliner state per
//! area — it moves into the layout tree later without changing this API.

use std::collections::HashSet;

use dioxus::prelude::*;

use crate::contextmenu::{MenuItem, menu_json};

/// One row the host hands back from its children callback.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeRow {
    /// Where this node lives — a JSON pointer, a relative path, whatever the
    /// host's children callback understands. Opaque to the component.
    pub pointer: String,
    /// The name shown on the row.
    pub label: String,
    /// A short value preview, dimmed, after the label. Empty shows nothing.
    pub preview: String,
    /// Whether the row can expand. A branch with no children yet still shows
    /// a caret; the callback answers when it opens.
    pub has_children: bool,
}

#[derive(Props, Clone, PartialEq)]
pub struct TreeViewProps {
    /// Children of a pointer; `""` is the root. Called when a branch opens.
    pub children_of: Callback<String, Vec<TreeRow>>,
    /// A row was clicked. Fires for branches and leaves alike, after the
    /// expand toggle. The host that cares about selection listens here.
    #[props(default)]
    pub on_pick: Option<Callback<TreeRow>>,
}

/// The tree. Place a [`crate::FilterBox`] beside it (inside one
/// `.im-filter-scope`) and rows filter client-side by label, like every other
/// filterable list.
#[component]
pub fn TreeView(props: TreeViewProps) -> Element {
    let open = use_signal(HashSet::<String>::new);
    let selected = use_signal(|| None::<String>);
    rsx! {
        div { class: "im-tree",
            {branch(String::new(), 0, open, selected, props.children_of, props.on_pick)}
        }
    }
}

/// One level of rows, recursing into open branches. A function rather than a
/// component so the signals stay the tree's own — every row toggles the same
/// two sets.
fn branch(
    pointer: String,
    depth: usize,
    open: Signal<HashSet<String>>,
    selected: Signal<Option<String>>,
    children_of: Callback<String, Vec<TreeRow>>,
    on_pick: Option<Callback<TreeRow>>,
) -> Element {
    let rows = children_of.call(pointer);
    rsx! {
        for row in rows {
            {tree_row(row, depth, open, selected, children_of, on_pick)}
        }
    }
}

fn tree_row(
    row: TreeRow,
    depth: usize,
    mut open: Signal<HashSet<String>>,
    mut selected: Signal<Option<String>>,
    children_of: Callback<String, Vec<TreeRow>>,
    on_pick: Option<Callback<TreeRow>>,
) -> Element {
    let is_open = row.has_children && open.read().contains(&row.pointer);
    let is_sel = selected.read().as_deref() == Some(row.pointer.as_str());
    let caret = if !row.has_children {
        "\u{00a0}"
    } else if is_open {
        "\u{25be}"
    } else {
        "\u{25b8}"
    };
    let menu = menu_json(&[MenuItem::new(
        "Copy data path",
        "copy_value",
        serde_json::json!({ "value": row.pointer }),
    )]);
    let click_row = row.clone();
    let sub = row.pointer.clone();
    rsx! {
        div {
            class: if is_sel { "im-tree-row is-sel" } else { "im-tree-row" },
            key: "{row.pointer}",
            style: "padding-left: {depth * 14 + 6}px",
            "data-filter-text": "{row.label}",
            "data-im-menu": "{menu}",
            onclick: move |_| {
                if click_row.has_children {
                    let mut o = open.write();
                    if !o.remove(&click_row.pointer) {
                        o.insert(click_row.pointer.clone());
                    }
                }
                selected.set(Some(click_row.pointer.clone()));
                if let Some(cb) = on_pick {
                    cb.call(click_row.clone());
                }
            },
            span { class: "im-tree-caret", "{caret}" }
            span { class: "im-tree-label", "{row.label}" }
            if !row.preview.is_empty() {
                span { class: "im-tree-preview", "{row.preview}" }
            }
        }
        if is_open {
            {branch(sub, depth + 1, open, selected, children_of, on_pick)}
        }
    }
}

/// Children of a node inside a serde document — the traversal every
/// serde-backed tree shares. `pointer` addresses within `doc` (`""` is the
/// document root); returned rows carry pointers extended the same way, so a
/// host mounting several documents under prefixes re-prefixes the result.
pub fn value_children(doc: &serde_json::Value, pointer: &str) -> Vec<TreeRow> {
    let Some(node) = doc.pointer(pointer) else {
        return Vec::new();
    };
    match node {
        serde_json::Value::Object(map) => map
            .iter()
            .map(|(k, v)| TreeRow {
                pointer: format!("{pointer}/{}", escape_pointer(k)),
                label: k.clone(),
                preview: preview(v),
                has_children: branches(v),
            })
            .collect(),
        serde_json::Value::Array(items) => items
            .iter()
            .enumerate()
            .map(|(i, v)| TreeRow {
                pointer: format!("{pointer}/{i}"),
                label: format!("[{i}]"),
                preview: preview(v),
                has_children: branches(v),
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn branches(v: &serde_json::Value) -> bool {
    match v {
        serde_json::Value::Object(m) => !m.is_empty(),
        serde_json::Value::Array(a) => !a.is_empty(),
        _ => false,
    }
}

/// The dimmed after-label glimpse of a value. Scalars show themselves;
/// containers show their size — the contents are one click away.
fn preview(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "null".into(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => {
            let mut out: String = s.chars().take(40).collect();
            if s.chars().count() > 40 {
                out.push('\u{2026}');
            }
            format!("\u{201c}{out}\u{201d}")
        }
        serde_json::Value::Array(a) => format!("[{}]", a.len()),
        serde_json::Value::Object(m) => format!("{{{}}}", m.len()),
    }
}

/// JSON-pointer escaping (RFC 6901): `~` is `~0`, `/` is `~1`. A key with a
/// slash in it must not read as two path segments.
fn escape_pointer(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_document_walks_level_by_level() {
        let doc = json!({ "b": { "x": 1 }, "a": [true, "hi"], "n": 7 });
        let top = value_children(&doc, "");
        let labels: Vec<&str> = top.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(labels, ["a", "b", "n"], "objects list in key order");
        assert!(top[0].has_children && top[1].has_children && !top[2].has_children);

        let a = value_children(&doc, "/a");
        assert_eq!(a[0].pointer, "/a/0");
        assert_eq!(a[1].preview, "\u{201c}hi\u{201d}");
    }

    #[test]
    fn a_key_with_a_slash_stays_one_segment() {
        let doc = json!({ "a/b": { "c": 1 } });
        let top = value_children(&doc, "");
        assert_eq!(top[0].pointer, "/a~1b");
        // and the escaped pointer resolves back to the same node
        let below = value_children(&doc, "/a~1b");
        assert_eq!(below[0].pointer, "/a~1b/c");
        assert_eq!(below[0].preview, "1");
    }

    #[test]
    fn previews_stay_short() {
        let long = "x".repeat(100);
        let doc = json!({ "s": long, "arr": [1, 2, 3], "obj": { "a": 1, "b": 2 } });
        let rows = value_children(&doc, "");
        assert_eq!(rows[0].preview, "[3]");
        assert_eq!(rows[1].preview, "{2}");
        assert!(rows[2].preview.chars().count() <= 43);
    }
}
