//! Export / import the workbench layout as JSON.
//!
//! The layout is already one serde value, so saving it is a download and
//! loading it is an upload — Blender's File ▸ Save/Load of a workspace. Export
//! is entirely client-side (the current layout rides on the button as
//! `data-layout`); import reads the chosen file locally and hands its text to
//! the host, which parses and replaces the workspaces. The component only opens
//! the eval channel the import commits over.

use dioxus::prelude::*;

const LAYOUTFILE_JS: &str = include_str!("layoutfile.js");

#[derive(Props, Clone)]
pub struct LayoutFileProps {
    /// The current workspaces, serialized — what Export downloads.
    pub layout_json: String,
    /// The uploaded file's text; the host parses and applies it.
    pub on_import: Callback<String>,
    /// Draw the Save / Load buttons. A host that offers both from a File menu
    /// wants the component's machinery — the eval channel the import commits
    /// over, and the element the current layout rides on — without a second
    /// pair of buttons in its chrome. `false` keeps the machinery and hides
    /// the controls.
    #[props(default = true)]
    pub buttons: bool,
}

impl PartialEq for LayoutFileProps {
    fn eq(&self, other: &Self) -> bool {
        self.layout_json == other.layout_json && self.buttons == other.buttons
    }
}

/// Export and import buttons. Place it in the chrome (a topbar); the shim wires
/// the download and the file read.
#[component]
pub fn LayoutFile(props: LayoutFileProps) -> Element {
    let on_import = props.on_import;
    use_future(move || async move {
        let mut channel = dioxus::document::eval(LAYOUTFILE_JS);
        loop {
            let Ok(text) = channel.recv::<String>().await else {
                return;
            };
            on_import.call(text);
        }
    });
    // Without buttons, the layout still has to ride on an element the shim can
    // find — `im:layout-save` reads it off [data-im-export] — so the carrier
    // stays and only the controls go.
    if !props.buttons {
        return rsx! {
            span {
                class: "im-layoutfile im-layoutfile-headless",
                "data-im-export": "1",
                "data-layout": "{props.layout_json}",
            }
        };
    }
    rsx! {
        span { class: "im-layoutfile",
            button {
                class: "im-file-btn",
                title: "Save the workbench layout to a file",
                "data-im-export": "1",
                "data-layout": "{props.layout_json}",
                "⬇ Save"
            }
            // Plain button — the shim builds the file input in JS on click, so
            // liveview's own file handling never runs on it.
            button {
                class: "im-file-btn",
                title: "Load a workbench layout from a file",
                "data-im-import-trigger": "1",
                "⬆ Load"
            }
        }
    }
}
