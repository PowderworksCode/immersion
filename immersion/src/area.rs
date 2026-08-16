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

/// Which strip of an area a command is about. These were bare strings on the
/// wire and in eighteen call sites, so "sidbar" was a silent no-op rather than
/// a compile error; as an enum both ends have to spell it right.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../ts/generated/")]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Region {
    Toolbar,
    Sidebar,
    /// The header, hidden to a stub.
    Header,
    /// The header, moved to the opposite edge.
    HeaderFlip,
}

impl Region {
    /// The name this travels under, so the menu builders and the shim agree
    /// without either restating the spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Region::Toolbar => "toolbar",
            Region::Sidebar => "sidebar",
            Region::Header => "header",
            Region::HeaderFlip => "header_flip",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../ts/generated/")]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum Dir {
    /// Children side by side.
    Row,
    /// Children stacked.
    Col,
}

/// The collapsible strips around a leaf's body — Blender's regions. The
/// toolbar (T) is the narrow tool column on the left; the sidebar (N) is the
/// properties panel on the right. Both off by default, toggled per area.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Regions {
    #[serde(default)]
    pub toolbar: bool,
    #[serde(default)]
    pub sidebar: bool,
    /// Region widths in px; 0 means "use the default width".
    #[serde(default)]
    pub toolbar_w: u16,
    #[serde(default)]
    pub sidebar_w: u16,
    /// The header collapsed to a stub — Blender's Hide Header.
    #[serde(default)]
    pub header_hidden: bool,
    /// The header along the bottom edge instead of the top — Blender's Flip.
    #[serde(default)]
    pub header_bottom: bool,
}

impl Regions {
    fn is_default(&self) -> bool {
        !self.toolbar
            && !self.sidebar
            && self.toolbar_w == 0
            && self.sidebar_w == 0
            && !self.header_hidden
            && !self.header_bottom
    }
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
        /// The collapsible regions around the body.
        #[serde(default, skip_serializing_if = "Regions::is_default")]
        regions: Regions,
    },
    Split {
        id: AreaId,
        dir: Dir,
        /// Each child's fraction of the rectangle, in order. Same length as
        /// `children`, summing to 1.
        sizes: Vec<f32>,
        /// Two or more children. A split never contains a split of the same
        /// direction — `normalize` flattens those — so three areas in a row
        /// are one split of three, not a split of a split. That is what makes
        /// two layouts that look identical *be* identical: there is no hidden
        /// nesting left to tell them apart.
        children: Vec<Area>,
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
            Area::Split { children, .. } => children.iter().find_map(|c| c.find(id)),
        }
    }

    fn find_mut(&mut self, id: AreaId) -> Option<&mut Area> {
        if self.id() == id {
            return Some(self);
        }
        match self {
            Area::Leaf { .. } => None,
            Area::Split { children, .. } => children.iter_mut().find_map(|c| c.find_mut(id)),
        }
    }

    pub fn leaves(&self) -> Vec<AreaId> {
        match self {
            Area::Leaf { id, .. } => vec![*id],
            Area::Split { children, .. } => children.iter().flat_map(|c| c.leaves()).collect(),
        }
    }

    fn max_id(&self) -> AreaId {
        match self {
            Area::Leaf { id, .. } => *id,
            Area::Split { id, children, .. } => {
                children.iter().map(Area::max_id).fold(*id, AreaId::max)
            }
        }
    }

    /// Flatten same-direction nesting, so the value is canonical: a row of
    /// three areas is one split of three, however it was built. Without this,
    /// splitting A then B and splitting B then A produce identical-looking
    /// screens from different values — and then a seam drag or a join behaves
    /// differently depending on history the user cannot see. That is the
    /// complaint people actually have about tree-based tiling; this removes it
    /// without giving up the tree.
    fn normalize(&mut self) {
        let Area::Split {
            dir,
            sizes,
            children,
            ..
        } = self
        else {
            return;
        };
        for c in children.iter_mut() {
            c.normalize();
        }
        let dir = *dir;
        let mut new_sizes = Vec::with_capacity(children.len());
        let mut new_children = Vec::with_capacity(children.len());
        for (i, child) in children.drain(..).enumerate() {
            let slot = sizes.get(i).copied().unwrap_or(0.0);
            match child {
                // A child split of the same direction dissolves into this one,
                // its children's sizes scaled into the slot it occupied.
                Area::Split {
                    dir: cdir,
                    sizes: csizes,
                    children: cchildren,
                    ..
                } if cdir == dir => {
                    for (j, gc) in cchildren.into_iter().enumerate() {
                        new_sizes.push(csizes.get(j).copied().unwrap_or(0.0) * slot);
                        new_children.push(gc);
                    }
                }
                other => {
                    new_sizes.push(slot);
                    new_children.push(other);
                }
            }
        }
        *sizes = new_sizes;
        *children = new_children;
        renormalize(sizes);
        collapse(self);
    }
}

/// Drop a child and give its span to a neighbour, so the sizes still sum to 1.
fn remove_child(children: &mut Vec<Area>, sizes: &mut Vec<f32>, i: usize) {
    if i >= children.len() {
        return;
    }
    let gone = sizes.remove(i);
    children.remove(i);
    if sizes.is_empty() {
        return;
    }
    // The span goes to the neighbour it was taken from — the one before it,
    // or the first if it was the first.
    let neighbour = i.saturating_sub(1).min(sizes.len() - 1);
    sizes[neighbour] += gone;
    renormalize(sizes);
}

/// A split with one child is not a split; it is that child.
fn collapse(node: &mut Area) {
    let only = match node {
        Area::Split { children, .. } if children.len() == 1 => children.pop(),
        _ => None,
    };
    if let Some(child) = only {
        *node = child;
    }
}

/// Keep the sizes a partition of 1, so rendering never has to guess.
fn renormalize(sizes: &mut [f32]) {
    let total: f32 = sizes.iter().sum();
    if total <= 0.0 {
        let even = 1.0 / sizes.len().max(1) as f32;
        for s in sizes.iter_mut() {
            *s = even;
        }
        return;
    }
    for s in sizes.iter_mut() {
        *s /= total;
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
                regions: Regions::default(),
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
        let Area::Leaf {
            id,
            editor,
            arg,
            regions,
        } = node
        else {
            return None;
        };
        let original = Area::Leaf {
            id: *id,
            editor: editor.clone(),
            arg: arg.clone(),
            regions: regions.clone(),
        };
        let fresh = Area::Leaf {
            id: new_id,
            editor: editor.clone(),
            arg: arg.clone(),
            regions: Regions::default(),
        };
        let r = clamp(ratio);
        *node = Area::Split {
            id: split_id,
            dir,
            sizes: vec![r, 1.0 - r],
            children: vec![original, fresh],
        };
        // Flatten the split we may have just nested inside one of the same
        // direction, so the value stays canonical.
        self.root.normalize();
        Some(new_id)
    }

    /// Close an area: its sibling subtree takes the whole rectangle. This is
    /// Blender's "closing areas (via join)" — there is no close that leaves a
    /// hole. Joining the last area is refused; a screen must show something.
    pub fn join(&mut self, victim: AreaId) -> bool {
        fn walk(node: &mut Area, victim: AreaId) -> bool {
            let Area::Split {
                children, sizes, ..
            } = node
            else {
                return false;
            };
            if let Some(i) = children.iter().position(|c| c.id() == victim) {
                remove_child(children, sizes, i);
                collapse(node);
                return true;
            }
            children.iter_mut().any(|c| walk(c, victim))
        }
        if self.root.id() == victim {
            return false;
        }
        let hit = walk(&mut self.root, victim);
        if hit {
            self.root.normalize();
        }
        hit
    }

    /// The join gesture: drag from `survivor` over `victim`; the survivor
    /// takes the space. Valid only for sibling leaves — the one case where
    /// "A absorbs B" has an unambiguous meaning in a binary tree. Blender's
    /// broader rule (any two areas sharing a full edge) needs rectangle
    /// geometry the tree alone does not carry; that refinement belongs to the
    /// gesture phase that can compute it, and until then refusing is honest
    /// where guessing would corrupt someone's layout.
    pub fn join_into(&mut self, survivor: AreaId, victim: AreaId) -> bool {
        fn walk(node: &mut Area, survivor: AreaId, victim: AreaId) -> bool {
            let Area::Split {
                children, sizes, ..
            } = node
            else {
                return false;
            };
            let s_i = children.iter().position(|c| c.id() == survivor);
            let v_i = children.iter().position(|c| c.id() == victim);
            if let (Some(si), Some(vi)) = (s_i, v_i)
                && si.abs_diff(vi) == 1
            {
                // The survivor takes the victim's span, which is what the
                // gesture showed while the drag was live.
                sizes[si] += sizes[vi];
                remove_child(children, sizes, vi);
                collapse(node);
                return true;
            }
            children.iter_mut().any(|c| walk(c, survivor, victim))
        }
        let hit = walk(&mut self.root, survivor, victim);
        if hit {
            self.root.normalize();
        }
        hit
    }

    /// Move a seam. `index` names the boundary between children `index` and
    /// `index + 1`, and `pos` is where it lands as a fraction of the whole
    /// split — which is exactly what the drag measured. Only the two children
    /// either side of the seam change; the rest keep their sizes, which is the
    /// behaviour a row of three areas should have and the binary tree could
    /// not give.
    pub fn set_seam(&mut self, split: AreaId, index: usize, pos: f32) -> bool {
        let Some(Area::Split { sizes, .. }) = self.root.find_mut(split) else {
            return false;
        };
        if index + 1 >= sizes.len() {
            return false;
        }
        let lo: f32 = sizes[..index].iter().sum();
        let span = sizes[index] + sizes[index + 1];
        let hi = lo + span;
        let pos = pos.clamp(lo + MIN_RATIO * span, hi - MIN_RATIO * span);
        sizes[index] = pos - lo;
        sizes[index + 1] = hi - pos;
        true
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
    /// Set a leaf's toolbar or sidebar width (px), clamped to a grabbable range.
    pub fn set_region_width(&mut self, leaf: AreaId, region: Region, w: u16) -> bool {
        match self.root.find_mut(leaf) {
            Some(Area::Leaf { regions, .. }) => {
                let w = w.clamp(32, 500);
                match region {
                    Region::Toolbar => regions.toolbar_w = w,
                    Region::Sidebar => regions.sidebar_w = w,
                    // Only the side strips have a width.
                    _ => return false,
                }
                true
            }
            _ => false,
        }
    }

    /// Toggle a leaf's toolbar (T) or sidebar (N) region.
    pub fn toggle_region(&mut self, leaf: AreaId, region: Region) -> bool {
        match self.root.find_mut(leaf) {
            Some(Area::Leaf { regions, .. }) => {
                match region {
                    Region::Toolbar => regions.toolbar = !regions.toolbar,
                    Region::Sidebar => regions.sidebar = !regions.sidebar,
                    Region::Header => regions.header_hidden = !regions.header_hidden,
                    Region::HeaderFlip => regions.header_bottom = !regions.header_bottom,
                }
                true
            }
            _ => false,
        }
    }

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

    /// The point of the whole n-ary change: two ways of building the same
    /// three-column screen produce the same value, so nothing about a layout's
    /// history can leak into how it behaves.
    #[test]
    fn three_in_a_row_is_one_split_however_it_was_built() {
        // Split the left area, then split the left one again.
        let mut a = Layout::single("x");
        let second = a.split(1, Dir::Row, 0.5).unwrap();
        a.split(1, Dir::Row, 0.5).unwrap();
        // Split the left area, then split the RIGHT one.
        let mut b = Layout::single("x");
        let b2 = b.split(1, Dir::Row, 0.5).unwrap();
        b.split(b2, Dir::Row, 0.5).unwrap();

        // Both are one split of three leaves — no nesting either way.
        for l in [&a, &b] {
            match &l.root {
                Area::Split {
                    children, sizes, ..
                } => {
                    assert_eq!(children.len(), 3, "three columns, one split");
                    assert_eq!(sizes.len(), 3);
                    assert!(children.iter().all(|c| matches!(c, Area::Leaf { .. })));
                }
                other => panic!("expected one split, got {other:?}"),
            }
        }
        let _ = second;
    }

    #[test]
    fn a_seam_moves_only_its_own_pair() {
        let mut l = Layout::single("x");
        l.split(1, Dir::Row, 0.5).unwrap();
        l.split(1, Dir::Row, 0.5).unwrap();
        let (id, before) = match &l.root {
            Area::Split { id, sizes, .. } => (*id, sizes.clone()),
            _ => panic!("split"),
        };
        // Move the first seam; the third column keeps its size.
        assert!(l.set_seam(id, 0, 0.1));
        match &l.root {
            Area::Split { sizes, .. } => {
                assert!(
                    (sizes[2] - before[2]).abs() < 1e-6,
                    "outer column untouched"
                );
                assert!(
                    (sizes.iter().sum::<f32>() - 1.0).abs() < 1e-5,
                    "still a partition"
                );
            }
            _ => panic!("split"),
        }
    }

    #[test]
    fn joining_a_middle_area_leaves_two() {
        let mut l = Layout::single("x");
        l.split(1, Dir::Row, 0.5).unwrap();
        let mid = l.split(1, Dir::Row, 0.5).unwrap();
        assert!(l.join(mid));
        match &l.root {
            Area::Split {
                children, sizes, ..
            } => {
                assert_eq!(children.len(), 2);
                assert!((sizes.iter().sum::<f32>() - 1.0).abs() < 1e-5);
            }
            other => panic!("expected a split of two, got {other:?}"),
        }
        // And joining down to one collapses the split away entirely.
        let last = l.root.leaves()[1];
        assert!(l.join(last));
        assert!(matches!(l.root, Area::Leaf { .. }));
    }

    #[test]
    fn a_cross_direction_split_still_nests() {
        let mut l = Layout::single("x");
        let right = l.split(1, Dir::Row, 0.5).unwrap();
        l.split(right, Dir::Col, 0.5).unwrap();
        match &l.root {
            Area::Split { dir, children, .. } => {
                assert_eq!(*dir, Dir::Row);
                assert_eq!(children.len(), 2, "a column inside a row is still nesting");
                assert!(matches!(children[1], Area::Split { dir: Dir::Col, .. }));
            }
            other => panic!("expected a row, got {other:?}"),
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
    fn seams_never_let_an_area_vanish() {
        let mut l = Layout::single("a");
        l.split(1, Dir::Row, 0.5).unwrap();
        let split_id = l.root.id();
        assert!(l.set_seam(split_id, 0, 0.0001));
        let Area::Split { sizes, .. } = l.root else {
            unreachable!()
        };
        assert!(
            sizes[0] >= MIN_RATIO,
            "the squeezed area keeps a grabbable slice"
        );
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
