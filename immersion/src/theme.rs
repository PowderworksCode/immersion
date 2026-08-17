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
            ("header", "#33333a"),
            ("surface", "#232327"),
            ("input-bg", "#232327"),
            ("border", "#47474f"),
            ("seam", "#0e0e10"),
            ("text", "#e8e8e4"),
            ("text-muted", "#a2a29c"),
        ],
    },
    Theme {
        name: "Graphite",
        tokens: &[
            ("bg", "#202022"),
            ("panel", "#2c2c30"),
            ("header", "#3b3b41"),
            ("surface", "#28282b"),
            ("input-bg", "#252528"),
            ("border", "#4f4f56"),
            ("seam", "#141416"),
            ("text", "#ededed"),
            ("text-muted", "#aaaaa4"),
        ],
    },
    Theme {
        name: "Light",
        tokens: &[
            ("bg", "#d6d6d8"),
            ("panel", "#cacace"),
            ("header", "#cdcdd2"),
            ("surface", "#e6e6e8"),
            ("input-bg", "#f2f2f4"),
            ("border", "#9c9ca3"),
            ("seam", "#9a9a9e"),
            ("text", "#1c1c1f"),
            ("text-muted", "#54545b"),
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

    /// The module doc promises "Blender Dark" and no-theme-at-all look
    /// identical — the base stylesheet's `:root` fallbacks *are* that theme.
    /// Nothing held the two in step, so a contrast pass on one silently
    /// desynced the other. This is the thing that notices.
    #[test]
    fn the_default_theme_matches_the_stylesheet_fallbacks() {
        let fallbacks = root_tokens(crate::CSS);
        for (key, value) in THEMES[0].tokens {
            let name = format!("--im-{key}");
            let found = fallbacks
                .iter()
                .find(|(k, _)| k == &name)
                .unwrap_or_else(|| panic!("{name} has no fallback in immersion.css"));
            assert_eq!(
                &found.1, value,
                "{name}: immersion.css says {}, theme \"{}\" says {value}",
                found.1, THEMES[0].name,
            );
        }
    }

    /// `(--im-name, value)` pairs from the first `:root { … }` block.
    fn root_tokens(css: &str) -> Vec<(String, String)> {
        let start = css
            .find(":root {")
            .expect("immersion.css has a :root block");
        let block = &css[start + ":root {".len()..];
        let block = &block[..block.find('}').expect("the :root block closes")];
        block
            .split(';')
            .filter_map(|decl| decl.split_once(':'))
            .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
            .filter(|(k, _)| k.starts_with("--im-"))
            .collect()
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
