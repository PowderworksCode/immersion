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
        let Area::Leaf { id, editor } = node else {
            return None;
        };
        let original = Area::Leaf {
            id: *id,
            editor: editor.clone(),
        };
        let fresh = Area::Leaf {
            id: new_id,
            editor: editor.clone(),
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
    /// rectangle — which is the whole point of the editor dropdown.
    pub fn set_editor(&mut self, leaf: AreaId, editor: &str) -> bool {
        match self.root.find_mut(leaf) {
            Some(Area::Leaf { editor: e, .. }) => {
                *e = editor.to_string();
                true
            }
            _ => false,
        }
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
    fn splitting_a_split_is_refused() {
        let mut l = Layout::single("a");
        l.split(1, Dir::Row, 0.5).unwrap();
        let split_id = l.root.id();
        assert_eq!(l.split(split_id, Dir::Row, 0.5), None);
    }
}
