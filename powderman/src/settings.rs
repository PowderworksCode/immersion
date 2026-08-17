//! What is configurable: the settings document's fields, and how Preferences
//! groups them.
//!
//! One description of the workbench's options, reached two ways — the
//! Settings editor shows them as a list you can split beside your work, and
//! Preferences shows them sectioned in a window you open and close. Blender
//! has both, for the same reason.
//!
//! Split out of `ui.rs` to keep that file about the workbench frame.

use immersion::{Field, FieldKind};

/// The Settings editor's schema: which widget edits which pointer in the
/// settings document. This is the whole binding — a Field per knob, the doc is
/// serde, edits are (pointer, value).
/// The settings, grouped the way Blender's Preferences groups them: a list
/// of sections down the left, that section's fields on the right. The Settings
/// editor shows the same fields as one flat list, so there is one description
/// of what is configurable and two ways to reach it.
pub(crate) fn preference_sections() -> Vec<(&'static str, Vec<Field>)> {
    let all = settings_fields();
    let pick = |paths: &[&str]| -> Vec<Field> {
        paths
            .iter()
            .filter_map(|p| all.iter().find(|f| f.path == *p).cloned())
            .collect()
    };
    vec![
        (
            "Interface",
            pick(&["/theme", "/accent", "/ui_scale", "/density", "/tooltips_on"]),
        ),
        ("Editing", pick(&["/diff_split", "/chart_window"])),
        ("Startup", pick(&["/splash_on_start"])),
        ("System", pick(&["/poll_ms", "/sweep_limit"])),
    ]
}

pub(crate) fn settings_fields() -> Vec<Field> {
    vec![
        Field::new("/accent", "Accent color", FieldKind::Color)
            .with_hint("the widget-blue used across the workbench")
            .with_default(serde_json::json!("#5680c2")),
        Field::new(
            "/poll_ms",
            "Refresh interval",
            FieldKind::Slider {
                min: 250.0,
                max: 5000.0,
                step: 250.0,
            },
        )
        .with_hint("how often the page repolls, in ms")
        .with_default(serde_json::json!(1000)),
        Field::new("/splash_on_start", "Splash on startup", FieldKind::Bool)
            .with_default(serde_json::json!(true)),
        Field::new("/tooltips_on", "Tooltips", FieldKind::Toggle)
            .with_hint("hover help on the workbench controls")
            .with_default(serde_json::json!(true)),
        Field::new(
            "/sweep_limit",
            "Default sweep limit",
            FieldKind::Number {
                min: Some(1.0),
                max: Some(1000.0),
                step: Some(50.0),
            },
        )
        .with_hint("packages per ecosystem the daily sweep fetches")
        .with_default(serde_json::json!(100)),
        Field::new(
            "/theme",
            "Theme",
            FieldKind::Select(
                immersion::themes()
                    .iter()
                    .map(|t| (t.name.to_string(), t.name.to_string()))
                    .collect(),
            ),
        )
        .with_hint("the workbench palette; accent stays your own"),
        Field::new(
            "/chart_window",
            "Chart window",
            FieldKind::Vector {
                labels: vec!["H".into(), "N".into(), "S".into()],
                step: Some(1.0),
            },
        )
        .with_hint("hours shown, samples, smoothing")
        .with_default(serde_json::json!([1, 60, 3])),
        Field::new("/diff_split", "Split diffs", FieldKind::Toggle)
            .with_hint("show diffs side by side rather than stacked")
            .with_default(serde_json::json!(false)),
        Field::new(
            "/ui_scale",
            "Resolution scale",
            FieldKind::Slider {
                min: 0.8,
                max: 1.6,
                step: 0.05,
            },
        )
        .with_hint("size of the whole interface")
        .with_default(serde_json::json!(1.0)),
        Field::new(
            "/density",
            "Density",
            FieldKind::Radio(vec![
                ("cozy".into(), "Cozy".into()),
                ("compact".into(), "Compact".into()),
            ]),
        )
        .with_default(serde_json::json!("cozy")),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{ClientAction, Route, route};

    /// Preferences and the Settings editor describe the same document, so a
    /// field that exists in one and not the other is a setting someone can
    /// reach by one route and not the other — which is how a preference ends
    /// up unreachable after a rename.
    #[test]
    fn every_setting_appears_in_exactly_one_section() {
        let flat = settings_fields();
        let sections = preference_sections();
        let mut seen: Vec<&str> = Vec::new();
        for (_, fields) in &sections {
            for f in fields {
                assert!(
                    !seen.contains(&f.path.as_str()),
                    "{} appears in two sections",
                    f.path
                );
                seen.push(&f.path);
            }
        }
        for f in &flat {
            assert!(
                seen.contains(&f.path.as_str()),
                "{} is a setting no preferences section shows",
                f.path
            );
        }
        assert_eq!(seen.len(), flat.len(), "a section shows a field twice");
    }

    #[test]
    fn preferences_is_client_view_state() {
        // It is a window, not a document: opening it in one browser must not
        // open it in another, so it belongs with maximize and the palette
        // rather than on the bus.
        assert!(matches!(
            route("preferences"),
            Route::ClientView(ClientAction::Preferences)
        ));
    }
}
