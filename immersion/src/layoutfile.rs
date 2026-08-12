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
}

impl PartialEq for LayoutFileProps {
    fn eq(&self, other: &Self) -> bool {
        self.layout_json == other.layout_json
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
