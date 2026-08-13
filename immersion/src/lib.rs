//! Blender-style areas for Dioxus liveview.
//!
//! The screen is a tiling tree of areas, each showing one editor. You split an
//! area to get another view, join to give the space back, and switch what an
//! area shows in place. The layout is a value (`Layout`), every mutation is a
//! method on it, and rendering is a component over it — there is no separate
//! layout engine to keep in sync.
//!
//! The library deliberately does not know what an editor *is*. The host hands
//! [`Areas`] a registry of kinds for the dropdown and one callback that renders
//! a leaf's body. That keeps host state out of the library and makes the seam
//! between the two exactly one function wide.
//!
//! Locked decisions inherited from the React Immersion, kept because they were
//! right: one editor per area and no tabs (switching is the dropdown), a flat
//! gap-free look with 1px seams, and close-is-join — no area ever leaves a
//! hole behind.

mod area;
mod command;
mod contextmenu;
mod keymap;
mod palette;
mod splash;
mod statusbar;
mod theme;
mod tooltip;
mod ui;
mod widget;
mod workspace;

pub use area::{Area, AreaId, Dir, Layout, MIN_RATIO};
pub use command::{Command, Commands};
pub use contextmenu::{ContextMenu, ContextMenuProps};
pub use keymap::{Binding, Keymap, KeymapHelp, KeymapHelpProps, KeymapProps, default_keymap};
pub use palette::{Palette, PaletteItem, PaletteProps};
pub use splash::{Splash, SplashProps, SplashRecent, Template};
pub use statusbar::{StatusBar, StatusBarProps};
pub use theme::{Theme, theme_css, themes};
pub use tooltip::{Tooltips, TooltipsProps};
pub use ui::{Areas, AreasProps, EditorKind, WorkspaceTabs, WorkspaceTabsProps};
pub use widget::{Field, FieldKind, PropertyEditor, PropertyEditorProps, apply_edit};
pub use workspace::{Workspace, Workspaces};

/// The library's chrome styles. The host concatenates this with its own CSS —
/// tokens (`--im-*`) have fallbacks, so it works standalone and themes if the
/// host defines them.
pub const CSS: &str = include_str!("immersion.css");
