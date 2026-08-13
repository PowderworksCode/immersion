//! The client bundle installer.
//!
//! [`Chrome`] installs [`client.js`] once — the tooltip, the slider live
//! preview, and the status-bar chord prettify, the three behaviours that need
//! no server round trip and used to be three separate shims on three separate
//! components. Renders nothing; place it once, like [`crate::Keymap`]. Its one
//! input is the tooltip on/off flag.

use dioxus::prelude::*;

const CLIENT_JS: &str = include_str!("client.js");

#[derive(Props, Clone, PartialEq)]
pub struct ChromeProps {
    /// The global tooltip enable switch. Defaults on.
    #[props(default = true)]
    pub tooltips_enabled: bool,
}

/// Installs the client-side chrome bundle. Toggling `tooltips_enabled` flips the
/// flag the bundle reads.
#[component]
pub fn Chrome(props: ChromeProps) -> Element {
    let enabled = props.tooltips_enabled;
    use_effect(move || {
        // Set the flag, then run the bundle. Installing twice is a no-op (the
        // bundle guards itself), so this doubles as the flag update.
        let setup = format!("window.__imTooltipsEnabled = {enabled};\n{CLIENT_JS}");
        dioxus::document::eval(&setup);
    });
    rsx! {}
}
