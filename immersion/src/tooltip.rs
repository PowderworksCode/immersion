//! Tooltips: Blender's hover help, on the liveview budget.
//!
//! Hover is continuous, and a tooltip that asked the server on every pointer
//! move would be exactly the round-trip the rest of the library refuses. So
//! this component renders nothing itself — it installs a client-side shim
//! ([`tooltip.js`]) that reads what is already in the DOM (a control's native
//! `title`, or a richer `data-tip` / `data-tip-key` / `data-tip-desc`) and
//! draws one shared styled tooltip after a short delay. The server's only part
//! is the on/off flag.
//!
//! Because the shim upgrades the plain `title` attribute, every control that
//! already carries one gets a styled tooltip for free — no markup to change.

use dioxus::prelude::*;

const TOOLTIP_JS: &str = include_str!("tooltip.js");

#[derive(Props, Clone, PartialEq)]
pub struct TooltipsProps {
    /// The global enable switch (Blender's tooltip preference). Defaults on.
    #[props(default = true)]
    pub enabled: bool,
}

/// Installs the tooltip shim. Renders nothing; place it once, anywhere in the
/// tree. Toggling `enabled` flips the shared flag the shim reads.
#[component]
pub fn Tooltips(props: TooltipsProps) -> Element {
    let enabled = props.enabled;
    // Re-eval when the flag changes: set the global the shim consults, and
    // install the shim on first run. Installing twice is harmless — the shim's
    // listeners are idempotent enough that a reinstall just re-registers them —
    // but the flag write is what a toggle actually needs.
    use_effect(move || {
        let setup = format!("window.__imTooltipsEnabled = {enabled};\n{TOOLTIP_JS}");
        dioxus::document::eval(&setup);
    });
    rsx! {}
}
