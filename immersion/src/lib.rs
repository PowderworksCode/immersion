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
mod client;
mod command;
mod contextmenu;
mod filter;
mod icons;
mod keymap;
mod layoutfile;
mod palette;
mod panel;
mod splash;
mod statusbar;
mod theme;
mod tree;
mod ui;
mod vendor;
mod widget;
mod workspace;

pub use area::{Area, AreaId, Dir, Layout, MIN_RATIO, Region};
pub use client::{Chrome, ChromeProps};
pub use command::{Command, Commands};
pub use contextmenu::{
    ContextMenu, ContextMenuProps, MenuItem, area_menu_json, editor_menu_json, menu_json,
    view_menu_json,
};
pub use filter::{FilterBox, FilterBoxProps};
pub use icons::{has_icon, icon};
pub use keymap::{
    Binding, Keymap, KeymapHelp, KeymapHelpProps, KeymapProps, Platform, PlatformProps,
    default_keymap, pretty_chord,
};
pub use layoutfile::{LayoutFile, LayoutFileProps};
pub use palette::{Palette, PaletteItem, PaletteProps};
pub use panel::{Panel, PanelProps};
pub use splash::{Splash, SplashProps, SplashRecent, Template};
pub use statusbar::{StatusBar, StatusBarProps};
pub use theme::{Theme, theme_css, themes};
pub use tree::{TreeRow, TreeView, TreeViewProps, value_children, value_icon};
pub use ui::{Areas, AreasProps, EditorKind, WorkspaceTabs, WorkspaceTabsProps};
pub use vendor::{MOUNT as VENDOR_MOUNT, asset as vendor_asset, script_tag as vendor_script_tag};
pub use widget::{
    EditorError, Field, FieldKind, PropertyEditor, PropertyEditorProps, apply_edit, eval_expr,
    eval_number,
};
pub use workspace::{Workspace, Workspaces};

/// The library's chrome styles. The host concatenates this with its own CSS —
/// tokens (`--im-*`) have fallbacks, so it works standalone and themes if the
/// host defines them.
pub const CSS: &str = include_str!("immersion.css");
