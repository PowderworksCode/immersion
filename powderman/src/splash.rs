//! The splash: what powderman offers a visitor on the way in.
//!
//! Blender's startup screen is new-file templates beside recent .blend files,
//! with its links underneath. This is the same three things, run-shaped: the
//! arrangements worth starting from, the runs worth going back to, and the
//! footer that says where to read more, which chords to learn, and how to
//! hand the page to an agent.
//!
//! The library draws it; everything here is what a host puts in it.

use immersion::{Dir, Layout, Region, SplashRecent, SplashRow, SplashSection, Template};

use crate::ui::{State, hhmmss, short};

/// The preset layouts offered on the splash. Each is a small tree the "New
/// workspace" column builds from.
pub(crate) fn templates() -> Vec<Template> {
    // Each arrangement opens the N panel on its lead area. The region has
    // existed since the areas did, but nothing shipped with it on, so the
    // sidebar was a feature you had to already know about to ever see —
    // Blender opens the Properties editor on its default screen for the same
    // reason. One area per workspace, not all of them: it is a panel beside
    // the thing you are working on, not a second column everywhere.
    let overview = {
        let mut l = Layout::single("machine");
        if let Some(b) = l.split(1, Dir::Col, 0.45) {
            l.set_editor(b, "runs");
            if let Some(r) = l.split(b, Dir::Row, 0.6) {
                l.set_editor(r, "fleet");
            }
        }
        l.toggle_region(1, Region::Sidebar);
        l
    };
    let runs_focus = {
        let mut l = Layout::single("runs");
        if let Some(r) = l.split(1, Dir::Row, 0.5) {
            l.set_editor(r, "run");
        }
        // The list is the thing you scan; the tally belongs beside it.
        l.toggle_region(1, Region::Sidebar);
        l
    };
    let monitoring = {
        let mut l = Layout::single("machine");
        if let Some(r) = l.split(1, Dir::Row, 0.62) {
            l.set_editor(r, "fleet");
        }
        l.toggle_region(1, Region::Sidebar);
        l
    };
    // A browser beside a viewer, twice: the same arrangement reads a file or
    // reads what changed in it, and both are editors that take a target, so
    // the pair is the one the target chip was built for.
    let beside_the_browser = |editor: &str| {
        let mut l = Layout::single("files");
        if let Some(r) = l.split(1, Dir::Row, 0.3) {
            l.set_editor(r, editor);
            l.toggle_region(r, Region::Sidebar);
        }
        l
    };
    let charts = {
        // One chart with its spec open beside it. The sidebar is the document
        // that makes the drawing, so this arrangement is the whole argument
        // for charts being documents rather than a drawing routine.
        let mut l = Layout::single("chart");
        l.set_target(1, "/charts/cpu");
        l.toggle_region(1, Region::Sidebar);
        l
    };
    let shortcuts = {
        let mut l = Layout::single("keymap");
        if let Some(r) = l.split(1, Dir::Col, 0.62) {
            l.set_editor(r, "info");
        }
        l.toggle_region(1, Region::Sidebar);
        l
    };
    vec![
        Template {
            name: "Overview".into(),
            icon: "layout-dashboard".into(),
            hint: "machine, runs and fleet at a glance".into(),
            layout: overview,
        },
        Template {
            name: "Runs".into(),
            icon: "list-details".into(),
            hint: "the run list beside a detail pane".into(),
            layout: runs_focus,
        },
        Template {
            name: "Monitoring".into(),
            icon: "activity-heartbeat".into(),
            hint: "machine graphs and the live fleet".into(),
            layout: monitoring,
        },
        Template {
            name: "Code".into(),
            icon: "file-code".into(),
            hint: "the file browser beside a viewer".into(),
            layout: beside_the_browser("code"),
        },
        Template {
            name: "Changes".into(),
            icon: "git-compare".into(),
            hint: "what changed, file by file".into(),
            layout: beside_the_browser("diff"),
        },
        Template {
            name: "Charts".into(),
            icon: "chart-line".into(),
            hint: "a plot beside the spec that makes it".into(),
            layout: charts,
        },
        Template {
            name: "Shortcuts".into(),
            icon: "keyboard".into(),
            hint: "the keymap over the command log".into(),
            layout: shortcuts,
        },
        Template {
            name: "Single".into(),
            icon: "square".into(),
            hint: "one area to split as you like".into(),
            layout: {
                let mut l = Layout::single("machine");
                l.toggle_region(1, Region::Sidebar);
                l
            },
        },
    ]
}

/// The splash footer: where to read more, the chords worth knowing, and the
/// line that hands this page to an agent — the three things the old
/// Immersion splash put under its columns, because a splash is the one screen
/// everyone sees and the only place onboarding is free.
///
/// The keys come from the status bar's own list rather than a copy, so the
/// two can never name different chords for the same thing.
pub(crate) fn splash_foot(mac: bool) -> Vec<SplashSection> {
    vec![
        SplashSection {
            title: "Links".into(),
            rows: vec![
                SplashRow::Link {
                    label: "GitHub repo".into(),
                    href: "https://github.com/PowderworksCode/immersion".into(),
                    icon: "brand-github".into(),
                },
                SplashRow::Link {
                    label: "Roadmap".into(),
                    href: "https://github.com/PowderworksCode/immersion/blob/main/docs/roadmap.md"
                        .into(),
                    icon: "book".into(),
                },
            ],
        },
        SplashSection {
            title: "Keyboard basics".into(),
            rows: crate::status::status_hints(mac, None)
                .into_iter()
                .map(|(chord, label)| SplashRow::Key { chord, label })
                .chain(std::iter::once(SplashRow::Key {
                    chord: "Right-click".into(),
                    label: "Context menus".into(),
                }))
                .collect(),
        },
        SplashSection {
            title: "Hand it to an agent".into(),
            rows: vec![
                SplashRow::Note {
                    text: "Everything here is a command an agent can run too.".into(),
                },
                SplashRow::Copy {
                    text: format!(
                        "claude mcp add --transport http powderman {}/mcp",
                        crate::daemon::public_url()
                    ),
                },
            ],
        },
    ]
}

/// Recent runs to jump back into — the "Recent files" column, run-shaped.
pub(crate) fn recents(s: &State) -> Vec<SplashRecent> {
    s.runs
        .iter()
        .take(8)
        .map(|r| SplashRecent {
            label: r.workflow.clone(),
            sub: format!("{} · {}", short(&r.id, 8), hhmmss(r.updated_at)),
            status: r.status.clone(),
            key: r.id.clone(),
        })
        .collect()
}

#[cfg(test)]
mod splash_tests {
    use super::*;

    /// A template naming an icon the set does not have draws a row with a gap
    /// where its glyph should be — the same net the editor registry has.
    #[test]
    fn every_splash_row_names_an_icon_that_exists() {
        for t in templates() {
            assert!(
                immersion::has_icon(&t.icon),
                "the {} template names the icon {:?}, which is not in the set",
                t.name,
                t.icon
            );
        }
        for section in splash_foot(false) {
            for row in section.rows {
                if let SplashRow::Link { icon, label, .. } = row {
                    assert!(
                        icon.is_empty() || immersion::has_icon(&icon),
                        "the {label} link names the icon {icon:?}, which is not in the set"
                    );
                }
            }
        }
    }

    /// The handoff line is the whole point of the column: an address a
    /// visitor can paste. One that says localhost on a deployed instance is
    /// worse than no line, so the URL comes from the environment the server
    /// is actually reachable at.
    #[test]
    fn the_agent_handoff_line_names_this_instance() {
        let foot = splash_foot(false);
        let agent = foot
            .iter()
            .find(|s| s.title == "Hand it to an agent")
            .expect("the handoff column");
        let line = agent
            .rows
            .iter()
            .find_map(|r| match r {
                SplashRow::Copy { text } => Some(text.clone()),
                _ => None,
            })
            .expect("a line to copy");
        assert!(line.starts_with("claude mcp add"), "{line}");
        assert!(
            line.ends_with("/mcp"),
            "the MCP mount, not the page: {line}"
        );
    }

    /// The splash used to name its own chords, which is one more place for a
    /// rebind to be missed. They come from the status bar's list now, so this
    /// checks the two agree rather than that either says a literal.
    #[test]
    fn the_keyboard_column_is_the_status_bars_own_list() {
        let bar = crate::status::status_hints(true, None);
        let keys: Vec<(String, String)> = splash_foot(true)
            .into_iter()
            .find(|s| s.title == "Keyboard basics")
            .expect("the keyboard column")
            .rows
            .into_iter()
            .filter_map(|r| match r {
                SplashRow::Key { chord, label } => Some((chord, label)),
                _ => None,
            })
            .collect();
        for hint in &bar {
            assert!(keys.contains(hint), "the splash lost {hint:?}");
        }
        assert!(
            keys.iter().any(|(c, _)| c == "Right-click"),
            "and keeps the one the keymap cannot list"
        );
    }
}
