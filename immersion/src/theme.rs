//! Themes: the flat skin as swappable token sets.
//!
//! Every colour in the library is an `--im-*` custom property with a fallback,
//! so the base stylesheet already renders standalone. A theme is just a named
//! set of values for those properties, emitted as a `:root { … }` block the
//! host injects after the base CSS — defining the property makes the fallback
//! yield, so the whole workbench recolours the moment the block changes. No
//! per-element work, no continuous input: picking a theme is one value.
//!
//! Accent is deliberately NOT a theme token — it is the user's own highlight
//! colour (a separate setting, applied over the theme), orthogonal to the
//! grays-and-text a theme decides.

/// A named palette: `(token-without-`--im-`-prefix, value)` pairs.
pub struct Theme {
    pub name: &'static str,
    tokens: &'static [(&'static str, &'static str)],
}

/// The built-in presets. The first is the default (Blender's dark), and its
/// values match the base stylesheet's fallbacks, so "Blender Dark" and no theme
/// look identical.
pub fn themes() -> &'static [Theme] {
    THEMES
}

/// The `:root { … }` block for a named theme, to inject after the base CSS. An
/// unknown name falls back to the first preset, so a stale setting never yields
/// an unstyled page.
pub fn theme_css(name: &str) -> String {
    let theme = THEMES.iter().find(|t| t.name == name).unwrap_or(&THEMES[0]);
    let body = theme
        .tokens
        .iter()
        .map(|(k, v)| format!("--im-{k}: {v};"))
        .collect::<Vec<_>>()
        .join(" ");
    format!(":root {{ {body} }}")
}

const THEMES: &[Theme] = &[
    Theme {
        name: "Blender Dark",
        tokens: &[
            ("bg", "#1a1a1d"),
            ("panel", "#2b2b30"),
            ("header", "#2b2b30"),
            ("surface", "#232327"),
            ("input-bg", "#232327"),
            ("border", "#3a3a40"),
            ("seam", "#0e0e10"),
            ("text", "#e8e8e4"),
            ("text-muted", "#8b8b85"),
        ],
    },
    Theme {
        name: "Graphite",
        tokens: &[
            ("bg", "#202022"),
            ("panel", "#2c2c30"),
            ("header", "#323236"),
            ("surface", "#28282b"),
            ("input-bg", "#252528"),
            ("border", "#45454a"),
            ("seam", "#141416"),
            ("text", "#ededed"),
            ("text-muted", "#9a9a95"),
        ],
    },
    Theme {
        name: "Light",
        tokens: &[
            ("bg", "#d6d6d8"),
            ("panel", "#cacace"),
            ("header", "#c0c0c4"),
            ("surface", "#e6e6e8"),
            ("input-bg", "#f2f2f4"),
            ("border", "#adadb2"),
            ("seam", "#9a9a9e"),
            ("text", "#1c1c1f"),
            ("text-muted", "#63636a"),
        ],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_css_defines_tokens_and_falls_back() {
        let css = theme_css("Light");
        assert!(css.starts_with(":root {"));
        assert!(css.contains("--im-bg: #d6d6d8;"));
        // An unknown name yields the first preset, never an empty block.
        assert_eq!(theme_css("nope"), theme_css("Blender Dark"));
    }

    #[test]
    fn every_theme_defines_the_same_token_set() {
        let base: Vec<&str> = THEMES[0].tokens.iter().map(|(k, _)| *k).collect();
        for t in THEMES {
            let keys: Vec<&str> = t.tokens.iter().map(|(k, _)| *k).collect();
            assert_eq!(keys, base, "theme {} has a different token set", t.name);
        }
    }
}
