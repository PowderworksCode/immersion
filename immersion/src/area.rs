//! The area tree: Blender's windowing model as a data structure.
//!
//! The screen is a binary tree. A leaf is an area showing one editor; a split
//! divides its rectangle between two children at a ratio. That is the whole
//! model — no tabs, no floating panels, no z-order. Every operation the UI can
//! perform is a total function over this enum, and the layout *is* the value,
//! so persistence and undo are serialization, not integration.
//!
//! Nodes carry ids. The UI addresses "split area 7" long after the tree has
//! been reshaped around area 7, so identity has to be stable across mutations
//! — positional paths would go stale in the hand that holds them.

use serde::{Deserialize, Serialize};

pub type AreaId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Dir {
    /// Children side by side.
    Row,
    /// Children stacked.
    Col,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Area {
    Leaf {
        id: AreaId,
        /// The editor kind this area shows — a registry key the host resolves.
        editor: String,
        /// An optional argument the editor interprets: which run to show,
        /// which file to open. `None` is the bare editor (a list, a picker).
        /// serde(default) so layouts saved before this field still load.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        arg: Option<String>,
    },
    Split {
        id: AreaId,
        dir: Dir,
        /// Fraction of the rectangle the first child gets, clamped to keep
        /// both children grabbable.
        ratio: f32,
        a: Box<Area>,
        b: Box<Area>,
    },
}

/// Blender never lets an area collapse to nothing — there is always enough
/// seam left to grab. 5% keeps a child visible and clickable.
pub const MIN_RATIO: f32 = 0.05;

fn clamp(ratio: f32) -> f32 {
    ratio.clamp(MIN_RATIO, 1.0 - MIN_RATIO)
}

impl Area {
    pub fn id(&self) -> AreaId {
        match self {
            Area::Leaf { id, .. } | Area::Split { id, .. } => *id,
        }
    }

    pub fn find(&self, id: AreaId) -> Option<&Area> {
        if self.id() == id {
            return Some(self);
        }
        match self {
            Area::Leaf { .. } => None,
            Area::Split { a, b, .. } => a.find(id).or_else(|| b.find(id)),
        }
    }

    fn find_mut(&mut self, id: AreaId) -> Option<&mut Area> {
        if self.id() == id {
            return Some(self);
        }
        match self {
            Area::Leaf { .. } => None,
            Area::Split { a, b, .. } => a.find_mut(id).or_else(|| b.find_mut(id)),
        }
    }

    pub fn leaves(&self) -> Vec<AreaId> {
        match self {
            Area::Leaf { id, .. } => vec![*id],
            Area::Split { a, b, .. } => {
                let mut v = a.leaves();
                v.extend(b.leaves());
                v
            }
        }
    }

    fn max_id(&self) -> AreaId {
        match self {
            Area::Leaf { id, .. } => *id,
            Area::Split { id, a, b, .. } => (*id).max(a.max_id()).max(b.max_id()),
        }
    }
}

/// The tree plus the id counter that names new nodes.
///
/// The counter is part of the persisted value, not process state: two loads of
/// the same layout must go on to mint the same ids, or a stale client's
/// "split area 12" could land on a different area 12.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    pub root: Area,
    next_id: AreaId,
}

impl Layout {
    pub fn single(editor: &str) -> Self {
        Layout {
            root: Area::Leaf {
                id: 1,
                editor: editor.to_string(),
                arg: None,
            },
            next_id: 2,
        }
    }

    /// Rebuild the counter from a tree that arrived without one — an imported
    /// layout, or a hand-written default.
    pub fn from_root(root: Area) -> Self {
        let next_id = root.max_id() + 1;
        Layout { root, next_id }
    }

    fn mint(&mut self) -> AreaId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Divide a leaf in two. The original keeps its id and its editor and
    /// becomes the first child; the new area shows the same editor — splitting
    /// duplicates what you are looking at, which is Blender's behaviour and
    /// the useful one: you split *because* you want another view of this.
    ///
    /// Returns the new leaf's id, or None if `target` is not a leaf.
    pub fn split(&mut self, target: AreaId, dir: Dir, ratio: f32) -> Option<AreaId> {
        let split_id = self.mint();
        let new_id = self.mint();
        let node = self.root.find_mut(target)?;
        let Area::Leaf { id, editor, arg } = node else {
            return None;
        };
        let original = Area::Leaf {
            id: *id,
            editor: editor.clone(),
            arg: arg.clone(),
        };
        let fresh = Area::Leaf {
            id: new_id,
            editor: editor.clone(),
            arg: arg.clone(),
        };
        *node = Area::Split {
            id: split_id,
            dir,
            ratio: clamp(ratio),
            a: Box::new(original),
            b: Box::new(fresh),
        };
        Some(new_id)
    }

    /// Close an area: its sibling subtree takes the whole rectangle. This is
    /// Blender's "closing areas (via join)" — there is no close that leaves a
    /// hole. Joining the last area is refused; a screen must show something.
    pub fn join(&mut self, victim: AreaId) -> bool {
        fn walk(node: &mut Area, victim: AreaId) -> bool {
            if let Area::Split { a, b, .. } = node {
                if a.id() == victim {
                    *node = (**b).clone();
                    return true;
                }
                if b.id() == victim {
                    *node = (**a).clone();
                    return true;
                }
                return walk(a, victim) || walk(b, victim);
            }
            false
        }
        if self.root.id() == victim {
            return false;
        }
        walk(&mut self.root, victim)
    }

    /// The join gesture: drag from `survivor` over `victim`; the survivor
    /// takes the space. Valid only for sibling leaves — the one case where
    /// "A absorbs B" has an unambiguous meaning in a binary tree. Blender's
    /// broader rule (any two areas sharing a full edge) needs rectangle
    /// geometry the tree alone does not carry; that refinement belongs to the
    /// gesture phase that can compute it, and until then refusing is honest
    /// where guessing would corrupt someone's layout.
    pub fn join_into(&mut self, survivor: AreaId, victim: AreaId) -> bool {
        fn is_leaf(a: &Area) -> bool {
            matches!(a, Area::Leaf { .. })
        }
        fn walk(node: &mut Area, survivor: AreaId, victim: AreaId) -> bool {
            if let Area::Split { a, b, .. } = node {
                let siblings = (a.id() == survivor && b.id() == victim)
                    || (a.id() == victim && b.id() == survivor);
                if siblings && is_leaf(a) && is_leaf(b) {
                    let keep = if a.id() == survivor {
                        (**a).clone()
                    } else {
                        (**b).clone()
                    };
                    *node = keep;
                    return true;
                }
                return walk(a, survivor, victim) || walk(b, survivor, victim);
            }
            false
        }
        walk(&mut self.root, survivor, victim)
    }

    /// Move the seam of a split.
    pub fn set_ratio(&mut self, split: AreaId, ratio: f32) -> bool {
        match self.root.find_mut(split) {
            Some(Area::Split { ratio: r, .. }) => {
                *r = clamp(ratio);
                true
            }
            _ => false,
        }
    }

    /// Change what an area shows, in place. The area survives — same id, same
    /// rectangle — which is the whole point of the editor dropdown. Switching
    /// kind clears any argument: picking "Run detail" from the dropdown gives
    /// you the picker, not whatever run the area showed before.
    pub fn set_editor(&mut self, leaf: AreaId, editor: &str) -> bool {
        match self.root.find_mut(leaf) {
            Some(Area::Leaf { editor: e, arg, .. }) => {
                *e = editor.to_string();
                *arg = None;
                true
            }
            _ => false,
        }
    }

    /// Point an area at a specific thing: editor kind plus its argument. This
    /// is how "open run 0f6a43ae here" lands — set the kind to the run-detail
    /// editor and the arg to the id.
    pub fn set_editor_arg(&mut self, leaf: AreaId, editor: &str, arg: &str) -> bool {
        match self.root.find_mut(leaf) {
            Some(Area::Leaf {
                editor: e, arg: a, ..
            }) => {
                *e = editor.to_string();
                *a = Some(arg.to_string());
                true
            }
            _ => false,
        }
    }

    /// Exchange what two areas show — editor and argument both. Corner-dragging
    /// one area onto another with the command key swaps them, so a run detail
    /// and a list can trade places without a join. Both must be leaves; a
    /// no-op (returns false) if either is missing or they are the same area.
    pub fn swap_editors(&mut self, a: AreaId, b: AreaId) -> bool {
        if a == b {
            return false;
        }
        let read = |node: &Area| match node {
            Area::Leaf { editor, arg, .. } => Some((editor.clone(), arg.clone())),
            _ => None,
        };
        let Some(ea) = self.root.find(a).and_then(read) else {
            return false;
        };
        let Some(eb) = self.root.find(b).and_then(read) else {
            return false;
        };
        if let Some(Area::Leaf { editor, arg, .. }) = self.root.find_mut(a) {
            *editor = eb.0;
            *arg = eb.1;
        }
        if let Some(Area::Leaf { editor, arg, .. }) = self.root.find_mut(b) {
            *editor = ea.0;
            *arg = ea.1;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor_of(l: &Layout, id: AreaId) -> String {
        match l.root.find(id) {
            Some(Area::Leaf { editor, .. }) => editor.clone(),
            other => panic!("area {id} is not a leaf: {other:?}"),
        }
    }

    #[test]
    fn swap_editors_exchanges_two_leaves() {
        let mut l = Layout::single("runs");
        let new = l
            .split(1, Dir::Row, 0.5)
            .expect("split makes a second leaf");
        l.set_editor(new, "fleet");
        assert!(l.swap_editors(1, new));
        assert_eq!(editor_of(&l, 1), "fleet");
        assert_eq!(editor_of(&l, new), "runs");
        // same-area and missing-area swaps are no-ops.
        assert!(!l.swap_editors(1, 1));
        assert!(!l.swap_editors(1, 9999));
    }

    #[test]
    fn split_keeps_the_original_and_duplicates_its_editor() {
        let mut l = Layout::single("machine");
        let new = l.split(1, Dir::Row, 0.6).unwrap();
        assert_eq!(l.root.leaves(), vec![1, new]);
        // Splitting duplicates the current editor — you split because you
        // want another view of this, not a blank.
        assert_eq!(editor_of(&l, 1), "machine");
        assert_eq!(editor_of(&l, new), "machine");
    }

    #[test]
    fn ids_survive_reshaping_around_them() {
        let mut l = Layout::single("a");
        let b = l.split(1, Dir::Row, 0.5).unwrap();
        l.set_editor(b, "b");
        let c = l.split(b, Dir::Col, 0.5).unwrap();
        l.set_editor(c, "c");
        // Area 1 has been re-parented twice; its id still finds it.
        assert_eq!(editor_of(&l, 1), "a");
        assert_eq!(l.root.leaves(), vec![1, b, c]);
    }

    #[test]
    fn join_gives_the_space_to_the_sibling_subtree() {
        let mut l = Layout::single("a");
        let b = l.split(1, Dir::Row, 0.5).unwrap();
        l.set_editor(b, "b");
        let c = l.split(b, Dir::Col, 0.5).unwrap();
        l.set_editor(c, "c");
        // Join away area 1: the b/c split takes the whole screen.
        assert!(l.join(1));
        assert_eq!(l.root.leaves(), vec![b, c]);
        // And the tree is a plain split at the root, not a wrapper.
        assert!(matches!(l.root, Area::Split { .. }));
    }

    #[test]
    fn the_last_area_cannot_be_joined_away() {
        let mut l = Layout::single("a");
        assert!(!l.join(1));
        assert_eq!(l.root.leaves(), vec![1]);
    }

    #[test]
    fn ratios_never_let_an_area_vanish() {
        let mut l = Layout::single("a");
        l.split(1, Dir::Row, 0.5).unwrap();
        let split_id = l.root.id();
        assert!(l.set_ratio(split_id, 0.0001));
        let Area::Split { ratio, .. } = l.root else {
            unreachable!()
        };
        assert!(ratio >= MIN_RATIO);
    }

    #[test]
    fn minted_ids_are_deterministic_across_a_save_load_cycle() {
        let mut l = Layout::single("a");
        l.split(1, Dir::Row, 0.5).unwrap();
        let json = serde_json::to_string(&l).unwrap();
        let mut reloaded: Layout = serde_json::from_str(&json).unwrap();
        let mut original = l.clone();
        // The counter travelled with the value, so both mint the same id.
        assert_eq!(
            original.split(1, Dir::Col, 0.5),
            reloaded.split(1, Dir::Col, 0.5)
        );
        assert_eq!(original, reloaded);
    }

    #[test]
    fn join_into_keeps_the_survivor_and_only_works_on_sibling_leaves() {
        let mut l = Layout::single("a");
        let b = l.split(1, Dir::Row, 0.5).unwrap();
        l.set_editor(b, "b");
        let c = l.split(b, Dir::Col, 0.5).unwrap();
        l.set_editor(c, "c");
        // 1 and b are not siblings any more (b sits inside a nested split).
        assert!(!l.join_into(1, b));
        // b and c are sibling leaves: c drags over b, c survives.
        assert!(l.join_into(c, b));
        assert_eq!(l.root.leaves(), vec![1, c]);
        assert_eq!(editor_of(&l, c), "c");
    }

    #[test]
    fn arg_rides_the_area_and_clears_on_a_kind_switch() {
        let mut l = Layout::single("runs");
        // Open a run into the area.
        assert!(l.set_editor_arg(1, "run", "0f6a43ae"));
        let json = serde_json::to_string(&l).unwrap();
        assert!(json.contains("0f6a43ae"), "arg must persist: {json}");
        assert_eq!(l, serde_json::from_str(&json).unwrap());
        // Splitting a run-detail area duplicates its arg — two views of the
        // same run, which is the point of a split.
        let twin = l.split(1, Dir::Row, 0.5).unwrap();
        assert!(
            serde_json::to_string(l.root.find(twin).unwrap())
                .unwrap()
                .contains("0f6a43ae")
        );
        // Switching kind from the dropdown drops the arg.
        assert!(l.set_editor(1, "fleet"));
        assert!(
            !serde_json::to_string(l.root.find(1).unwrap())
                .unwrap()
                .contains("0f6a43ae")
        );
    }

    #[test]
    fn a_layout_saved_before_arg_existed_still_loads() {
        // No `arg` key, as older persisted trees have.
        let json = r#"{"root":{"kind":"leaf","id":1,"editor":"runs"},"next_id":2}"#;
        let l: Layout = serde_json::from_str(json).unwrap();
        assert_eq!(l.root.leaves(), vec![1]);
    }

    #[test]
    fn splitting_a_split_is_refused() {
        let mut l = Layout::single("a");
        l.split(1, Dir::Row, 0.5).unwrap();
        let split_id = l.root.id();
        assert_eq!(l.split(split_id, Dir::Row, 0.5), None);
    }
}
