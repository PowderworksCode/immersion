//! Workspaces: named layout trees, switched by tabs.
//!
//! Blender's topbar tabs — Layout, Modeling, Shading — are named arrangements
//! you flip between, each remembering its own split tree. Here they are the
//! same: a `Workspaces` is an ordered list of `(name, Layout)` plus which one
//! is active, and switching is picking an index. The whole thing is one serde
//! value, so persistence and multi-client convergence are unchanged from a
//! single layout — it is just a bigger value in the same kv row.
//!
//! Every mutation keeps the invariant that there is always at least one
//! workspace and `active` always points at a real one, because the UI renders
//! the active tree unconditionally and a screen must show something.

use serde::{Deserialize, Serialize};

use crate::area::Layout;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Workspace {
    pub name: String,
    pub layout: Layout,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Workspaces {
    pub tabs: Vec<Workspace>,
    pub active: usize,
}

impl Workspaces {
    /// A single workspace around a starting layout.
    pub fn new(name: &str, layout: Layout) -> Self {
        Workspaces {
            tabs: vec![Workspace {
                name: name.to_string(),
                layout,
            }],
            active: 0,
        }
    }

    /// The active workspace. Always valid — every mutation preserves that.
    pub fn current(&self) -> &Workspace {
        &self.tabs[self.active]
    }

    /// The active layout, to mutate in place. Splits, joins and resizes land
    /// on whichever workspace is showing, which is the only one the gestures
    /// can reach.
    pub fn current_layout_mut(&mut self) -> &mut Layout {
        &mut self.tabs[self.active].layout
    }

    pub fn switch(&mut self, index: usize) -> bool {
        if index < self.tabs.len() {
            self.active = index;
            true
        } else {
            false
        }
    }

    /// Blender's Ctrl-PageUp/Down: wrap around the ends rather than stop, so
    /// the gesture is a rotation, not a bounded scan.
    pub fn cycle(&mut self, delta: i32) {
        let n = self.tabs.len() as i32;
        self.active = (((self.active as i32 + delta) % n + n) % n) as usize;
    }

    /// Add a workspace and switch to it — you add one to use it. The new tab
    /// starts from `layout` (the host decides: a fresh single area, or a copy
    /// of the current tree).
    pub fn add(&mut self, name: &str, layout: Layout) {
        self.tabs.push(Workspace {
            name: name.to_string(),
            layout,
        });
        self.active = self.tabs.len() - 1;
    }

    pub fn rename(&mut self, index: usize, name: &str) -> bool {
        match self.tabs.get_mut(index) {
            Some(w) if !name.trim().is_empty() => {
                w.name = name.trim().to_string();
                true
            }
            _ => false,
        }
    }

    /// Close a workspace. Refused when it is the last one — there is always a
    /// tab. `active` is kept in range and, when the closed tab was at or
    /// before it, shifted so it points at the same neighbour a user would
    /// expect rather than jumping.
    /// Move a tab to a new position, carrying the active selection with it —
    /// reordering must not change which workspace you are looking at, which
    /// is what happens if the index is left pointing at a neighbour.
    pub fn move_tab(&mut self, from: usize, to: usize) -> bool {
        let last = self.tabs.len().saturating_sub(1);
        if from > last || to > last || from == to {
            return false;
        }
        let active_name_is = |ws: &Self, i: usize| i == ws.active;
        let was_active = active_name_is(self, from);
        let tab = self.tabs.remove(from);
        self.tabs.insert(to, tab);
        self.active = if was_active {
            to
        } else {
            // The active tab did not move, but the indices around it did.
            let mut a = self.active;
            if from < a {
                a -= 1;
            }
            if to <= a {
                a += 1;
            }
            a.min(last)
        };
        true
    }

    pub fn close(&mut self, index: usize) -> bool {
        if self.tabs.len() <= 1 || index >= self.tabs.len() {
            return false;
        }
        self.tabs.remove(index);
        if self.active >= index && self.active > 0 {
            self.active -= 1;
        }
        self.active = self.active.min(self.tabs.len() - 1);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ws() -> Workspaces {
        let mut w = Workspaces::new("one", Layout::single("a"));
        w.add("two", Layout::single("b"));
        w.add("three", Layout::single("c"));
        w // active = 2 (three)
    }

    #[test]
    fn add_switches_to_the_new_tab() {
        let w = ws();
        assert_eq!(w.active, 2);
        assert_eq!(w.current().name, "three");
    }

    #[test]
    fn cycle_wraps_both_ways() {
        let mut w = ws();
        w.cycle(1); // 2 -> 0
        assert_eq!(w.active, 0);
        w.cycle(-1); // 0 -> 2
        assert_eq!(w.active, 2);
    }

    #[test]
    fn close_keeps_at_least_one_and_never_leaves_active_dangling() {
        let mut w = ws();
        // Close the active (index 2); active must fall back in range.
        assert!(w.close(2));
        assert_eq!(w.tabs.len(), 2);
        assert!(w.active < w.tabs.len());
        assert_eq!(w.current().name, "two");
        // Close down to the last, then refuse.
        assert!(w.close(1));
        assert!(!w.close(0));
        assert_eq!(w.tabs.len(), 1);
    }

    #[test]
    fn closing_before_active_shifts_active_to_track_the_same_tab() {
        let mut w = ws();
        w.switch(2); // on "three"
        w.close(0); // remove "one"
        // "three" is now at index 1; active should still show it.
        assert_eq!(w.current().name, "three");
    }

    #[test]
    fn rename_rejects_blank() {
        let mut w = ws();
        assert!(!w.rename(0, "   "));
        assert!(w.rename(0, "renamed"));
        assert_eq!(w.tabs[0].name, "renamed");
    }

    #[test]
    fn a_workspace_set_round_trips_through_json() {
        let w = ws();
        let json = serde_json::to_string(&w).unwrap();
        assert_eq!(w, serde_json::from_str::<Workspaces>(&json).unwrap());
    }
}

#[cfg(test)]
mod move_tests {
    use super::*;
    use crate::Layout;

    fn ws(n: usize) -> Workspaces {
        let mut w = Workspaces::new("0", Layout::single("runs"));
        for i in 1..n {
            w.add(&i.to_string(), Layout::single("runs"));
        }
        w
    }

    fn names(w: &Workspaces) -> Vec<String> {
        w.tabs.iter().map(|t| t.name.clone()).collect()
    }

    #[test]
    fn a_tab_moves_and_takes_the_selection_with_it() {
        // Reordering must not change which workspace you are looking at —
        // dragging the tab you are on to the front and finding yourself in a
        // different workspace is the bug this exists to prevent.
        let mut w = ws(4);
        w.active = 2;
        assert!(w.move_tab(2, 0));
        assert_eq!(names(&w), ["2", "0", "1", "3"]);
        assert_eq!(w.active, 0, "the moved tab is still the active one");
    }

    #[test]
    fn moving_another_tab_leaves_the_selection_where_it_looks() {
        let mut w = ws(4);
        w.active = 1; // looking at "1"
        assert!(w.move_tab(3, 0)); // drag the last tab to the front
        assert_eq!(names(&w), ["3", "0", "1", "2"]);
        assert_eq!(w.tabs[w.active].name, "1", "still looking at the same tab");

        let mut w = ws(4);
        w.active = 2; // looking at "2"
        assert!(w.move_tab(0, 3)); // drag the first tab to the end
        assert_eq!(names(&w), ["1", "2", "3", "0"]);
        assert_eq!(w.tabs[w.active].name, "2", "still looking at the same tab");
    }

    #[test]
    fn a_move_that_cannot_happen_is_refused() {
        let mut w = ws(3);
        assert!(!w.move_tab(0, 0), "a tab does not move onto itself");
        assert!(!w.move_tab(5, 0), "no such tab");
        assert!(!w.move_tab(0, 5), "no such position");
        assert_eq!(names(&w), ["0", "1", "2"], "and nothing changed");
    }
}
