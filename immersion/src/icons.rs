//! Icons: a small owned sprite.
//!
//! Blender gives every editor an icon and shows it in the header's editor
//! selector; the old immersion did the same. A name alone makes a header read
//! as a form, and makes a menu of twelve editors a wall of words — an icon is
//! what lets you find the Runs area without reading.
//!
//! The set is deliberately tiny and vendored as path data rather than pulled
//! from an icon package: fourteen glyphs is not a dependency's worth, and a
//! sprite in the binary needs no fetch, no build step, and no version to keep
//! in sync. The paths are from Tabler Icons (MIT), drawn on a 24x24 grid with
//! a 2px stroke, and `currentColor` means an icon inherits the text colour
//! wherever it is placed.

/// `(name, svg body)`. The body is whatever goes inside `<svg>` — strokes,
/// not fills, so one glyph works on any background the theme has.
const ICONS: &[(&str, &str)] = &[
    (
        "binary-tree",
        "<path d=\"M6 20a2 2 0 1 0 -4 0a2 2 0 0 0 4 0\" /> <path d=\"M16 4a2 2 0 1 0 -4 0a2 2 0 0 0 4 0\" /> <path d=\"M16 20a2 2 0 1 0 -4 0a2 2 0 0 0 4 0\" /> <path d=\"M11 12a2 2 0 1 0 -4 0a2 2 0 0 0 4 0\" /> <path d=\"M21 12a2 2 0 1 0 -4 0a2 2 0 0 0 4 0\" /> <path d=\"M5.058 18.306l2.88 -4.606\" /> <path d=\"M10.061 10.303l2.877 -4.604\" /> <path d=\"M10.065 13.705l2.876 4.6\" /> <path d=\"M15.063 5.7l2.881 4.61\" />",
    ),
    (
        "braces",
        "<path d=\"M7 4a2 2 0 0 0 -2 2v3a2 3 0 0 1 -2 3a2 3 0 0 1 2 3v3a2 2 0 0 0 2 2\" /> <path d=\"M17 4a2 2 0 0 1 2 2v3a2 3 0 0 0 2 3a2 3 0 0 0 -2 3v3a2 2 0 0 1 -2 2\" />",
    ),
    (
        "brackets",
        "<path d=\"M8 4h-3v16h3\" /> <path d=\"M16 4h3v16h-3\" />",
    ),
    (
        "chart-line",
        "<path d=\"M4 19l16 0\" /> <path d=\"M4 15l4 -6l4 2l4 -5l4 4\" />",
    ),
    ("chevron-down", "<path d=\"M6 9l6 6l6 -6\" />"),
    (
        "circle-dot",
        "<path d=\"M11 12a1 1 0 1 0 2 0a1 1 0 1 0 -2 0\" /> <path d=\"M3 12a9 9 0 1 0 18 0a9 9 0 1 0 -18 0\" />",
    ),
    (
        "clock",
        "<path d=\"M3 12a9 9 0 1 0 18 0a9 9 0 0 0 -18 0\" /> <path d=\"M12 7v5l3 3\" />",
    ),
    (
        "code",
        "<path d=\"M7 8l-4 4l4 4\" /> <path d=\"M17 8l4 4l-4 4\" /> <path d=\"M14 4l-4 16\" />",
    ),
    (
        "file",
        "<path d=\"M14 3v4a1 1 0 0 0 1 1h4\" /> <path d=\"M17 21h-10a2 2 0 0 1 -2 -2v-14a2 2 0 0 1 2 -2h7l5 5v11a2 2 0 0 1 -2 2\" />",
    ),
    (
        "file-diff",
        "<path d=\"M14 3v4a1 1 0 0 0 1 1h4\" /> <path d=\"M17 21h-10a2 2 0 0 1 -2 -2v-14a2 2 0 0 1 2 -2h7l5 5v11a2 2 0 0 1 -2 2\" /> <path d=\"M12 10l0 4\" /> <path d=\"M10 12l4 0\" /> <path d=\"M10 17l4 0\" />",
    ),
    (
        "file-text",
        "<path d=\"M14 3v4a1 1 0 0 0 1 1h4\" /> <path d=\"M17 21h-10a2 2 0 0 1 -2 -2v-14a2 2 0 0 1 2 -2h7l5 5v11a2 2 0 0 1 -2 2\" /> <path d=\"M9 9l1 0\" /> <path d=\"M9 13l6 0\" /> <path d=\"M9 17l6 0\" />",
    ),
    (
        "folder",
        "<path d=\"M5 4h4l3 3h7a2 2 0 0 1 2 2v8a2 2 0 0 1 -2 2h-14a2 2 0 0 1 -2 -2v-11a2 2 0 0 1 2 -2\" />",
    ),
    (
        "folder-open",
        "<path d=\"M5 19l2.757 -7.351a1 1 0 0 1 .936 -.649h12.307a1 1 0 0 1 .986 1.164l-.996 5.211a2 2 0 0 1 -1.964 1.625h-14.026a2 2 0 0 1 -2 -2v-11a2 2 0 0 1 2 -2h4l3 3h7a2 2 0 0 1 2 2v2\" />",
    ),
    (
        "hash",
        "<path d=\"M5 9l14 0\" /> <path d=\"M5 15l14 0\" /> <path d=\"M11 4l-4 16\" /> <path d=\"M17 4l-4 16\" />",
    ),
    (
        "info-circle",
        "<path d=\"M3 12a9 9 0 1 0 18 0a9 9 0 0 0 -18 0\" /> <path d=\"M12 9h.01\" /> <path d=\"M11 12h1v4h1\" />",
    ),
    (
        "keyboard",
        "<path d=\"M2 8a2 2 0 0 1 2 -2h16a2 2 0 0 1 2 2v8a2 2 0 0 1 -2 2h-16a2 2 0 0 1 -2 -2l0 -8\" /> <path d=\"M6 10l0 .01\" /> <path d=\"M10 10l0 .01\" /> <path d=\"M14 10l0 .01\" /> <path d=\"M18 10l0 .01\" /> <path d=\"M6 14l0 .01\" /> <path d=\"M18 14l0 .01\" /> <path d=\"M10 14l4 .01\" />",
    ),
    (
        "layout-columns",
        "<path d=\"M4 6a2 2 0 0 1 2 -2h12a2 2 0 0 1 2 2v12a2 2 0 0 1 -2 2h-12a2 2 0 0 1 -2 -2l0 -12\" /> <path d=\"M12 4l0 16\" />",
    ),
    (
        "layout-rows",
        "<path d=\"M4 6a2 2 0 0 1 2 -2h12a2 2 0 0 1 2 2v12a2 2 0 0 1 -2 2h-12a2 2 0 0 1 -2 -2l0 -12\" /> <path d=\"M4 12l16 0\" />",
    ),
    (
        "letter-case",
        "<path d=\"M14 15.5a3.5 3.5 0 1 0 7 0a3.5 3.5 0 1 0 -7 0\" /> <path d=\"M3 19v-10.5a3.5 3.5 0 0 1 7 0v10.5\" /> <path d=\"M3 13h7\" /> <path d=\"M21 12v7\" />",
    ),
    (
        "list-details",
        "<path d=\"M13 5h8\" /> <path d=\"M13 9h5\" /> <path d=\"M13 15h8\" /> <path d=\"M13 19h5\" /> <path d=\"M3 5a1 1 0 0 1 1 -1h4a1 1 0 0 1 1 1v4a1 1 0 0 1 -1 1h-4a1 1 0 0 1 -1 -1l0 -4\" /> <path d=\"M3 15a1 1 0 0 1 1 -1h4a1 1 0 0 1 1 1v4a1 1 0 0 1 -1 1h-4a1 1 0 0 1 -1 -1l0 -4\" />",
    ),
    ("player-play", "<path d=\"M7 4v16l13 -8l-13 -8\" />"),
    (
        "search",
        "<path d=\"M3 10a7 7 0 1 0 14 0a7 7 0 1 0 -14 0\" /> <path d=\"M21 21l-6 -6\" />",
    ),
    (
        "server-2",
        "<path d=\"M3 7a3 3 0 0 1 3 -3h12a3 3 0 0 1 3 3v2a3 3 0 0 1 -3 3h-12a3 3 0 0 1 -3 -3v-2\" /> <path d=\"M3 15a3 3 0 0 1 3 -3h12a3 3 0 0 1 3 3v2a3 3 0 0 1 -3 3h-12a3 3 0 0 1 -3 -3l0 -2\" /> <path d=\"M7 8l0 .01\" /> <path d=\"M7 16l0 .01\" /> <path d=\"M11 8h6\" /> <path d=\"M11 16h6\" />",
    ),
    (
        "settings",
        "<path d=\"M10.325 4.317c.426 -1.756 2.924 -1.756 3.35 0a1.724 1.724 0 0 0 2.573 1.066c1.543 -.94 3.31 .826 2.37 2.37a1.724 1.724 0 0 0 1.065 2.572c1.756 .426 1.756 2.924 0 3.35a1.724 1.724 0 0 0 -1.066 2.573c.94 1.543 -.826 3.31 -2.37 2.37a1.724 1.724 0 0 0 -2.572 1.065c-.426 1.756 -2.924 1.756 -3.35 0a1.724 1.724 0 0 0 -2.573 -1.066c-1.543 .94 -3.31 -.826 -2.37 -2.37a1.724 1.724 0 0 0 -1.065 -2.572c-1.756 -.426 -1.756 -2.924 0 -3.35a1.724 1.724 0 0 0 1.066 -2.573c-.94 -1.543 .826 -3.31 2.37 -2.37c1 .608 2.296 .07 2.572 -1.065\" /> <path d=\"M9 12a3 3 0 1 0 6 0a3 3 0 0 0 -6 0\" />",
    ),
    (
        "square-toggle",
        "<path d=\"M12 2l0 20\" /> <path d=\"M14 20h-8a2 2 0 0 1 -2 -2v-12a2 2 0 0 1 2 -2h8\" /> <path d=\"M20 6a2 2 0 0 0 -2 -2\" /> <path d=\"M18 20a2 2 0 0 0 2 -2\" /> <path d=\"M20 10l0 4\" />",
    ),
    (
        "toggle-left",
        "<path d=\"M6 12a2 2 0 1 0 4 0a2 2 0 1 0 -4 0\" /> <path d=\"M2 12a6 6 0 0 1 6 -6h8a6 6 0 0 1 6 6a6 6 0 0 1 -6 6h-8a6 6 0 0 1 -6 -6\" />",
    ),
    ("x", "<path d=\"M18 6l-12 12\" /> <path d=\"M6 6l12 12\" />"),
];

/// One icon as an inline `<svg>`, or an empty string for a name nobody drew.
/// Empty rather than a placeholder: a missing icon should leave a header
/// looking ordinary, not decorated with a question mark.
pub fn icon(name: &str) -> String {
    let Some((_, body)) = ICONS.iter().find(|(n, _)| *n == name) else {
        return String::new();
    };
    format!(
        r#"<svg class="im-icon" viewBox="0 0 24 24" width="1em" height="1em" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{body}</svg>"#
    )
}

/// Whether a glyph exists, for callers that lay out differently without one.
pub fn has_icon(name: &str) -> bool {
    ICONS.iter().any(|(n, _)| *n == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_icon_is_inline_svg_that_inherits_its_colour() {
        let svg = icon("chart-line");
        assert!(svg.starts_with("<svg"), "{svg}");
        assert!(
            svg.contains("currentColor"),
            "an icon takes the text colour"
        );
        assert!(
            svg.contains("viewBox=\"0 0 24 24\""),
            "one grid for all of them"
        );
        assert!(svg.contains("<path"), "and has something to draw");
    }

    #[test]
    fn a_name_nobody_drew_is_nothing_rather_than_a_placeholder() {
        assert_eq!(icon("no-such-icon"), "");
        assert!(!has_icon("no-such-icon"));
        assert!(has_icon("folder"));
    }

    #[test]
    fn every_glyph_is_drawable_and_none_is_duplicated() {
        let mut names: Vec<&str> = ICONS.iter().map(|(n, _)| *n).collect();
        let before = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), before, "an icon name is listed twice");
        for (name, body) in ICONS {
            assert!(
                body.contains("<path") || body.contains("<circle"),
                "{name} draws nothing"
            );
            assert!(
                !body.contains("stroke=\"none\""),
                "{name} kept the sizing stub"
            );
        }
    }
}
