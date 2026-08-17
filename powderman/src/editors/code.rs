//! The code editor.

use dioxus::prelude::*;

use super::files::files_root;
use super::human_size;
use super::stamp_of;

/// The code viewer: one file, highlighted, read-only.
///
/// The server reads the file and hands over its text; the vendored renderer
/// draws it. The two never contend for the same DOM: the framework owns the
/// payload element and the (empty) host element, and the renderer owns only
/// what it appends inside the host, keyed by a stamp so a redraw happens
/// exactly when the source changes.
pub(crate) fn ed_code(target: Option<String>) -> Element {
    let Some(path) = target.filter(|t| !t.is_empty()) else {
        return rsx! {
            div { class: "empty", "Pick a file to show here \u{2014} the target chip in the header." }
        };
    };
    match read_source(&path) {
        Ok(src) => {
            let stamp = stamp_of(&path, &src);
            rsx! {
                div { class: "code-view",
                    div { class: "code-path", "{path}" }
                    // The payload. Hidden, and never touched by the renderer.
                    pre { class: "code-src-payload", "data-im-code-src": "{stamp}", "{src}" }
                    div {
                        class: "code-host",
                        "data-im-code": "{stamp}",
                        "data-im-code-path": "{path}",
                        "data-im-code-kind": "file",
                    }
                }
            }
        }
        Err(e) => rsx! { div { class: "empty", "{e}" } },
    }
}

/// Read a file for display, with the limits a viewer needs: inside the root,
/// small enough to render, and text.
pub(crate) fn read_source(rel: &str) -> Result<String, String> {
    // The demo browses a fabricated checkout, so it has to be able to read
    // one too: a picker offering files whose contents do not exist is a
    // picker that does nothing.
    if crate::demo::enabled() {
        return crate::demo::file_source(rel)
            .map(str::to_string)
            .ok_or_else(|| format!("no such file: {rel}"));
    }
    // 2 MB: past that the page is the bottleneck, not the reading, and a
    // viewer that hangs the workbench is worse than one that declines.
    const MAX: u64 = 2 * 1024 * 1024;
    let root = files_root();
    let path = root.join(rel.trim_start_matches('/'));
    let path = path
        .canonicalize()
        .map_err(|_| format!("no such file: {rel}"))?;
    if !path.starts_with(&root) {
        return Err("outside the file root".to_string());
    }
    let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
    if meta.is_dir() {
        return Err(format!("{rel} is a directory"));
    }
    if meta.len() > MAX {
        return Err(format!(
            "{rel} is {} \u{2014} too large to display",
            human_size(meta.len())
        ));
    }
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    // A NUL in the first block is the usual tell, and it is the check `grep`
    // makes: rendering a binary as text produces a page of replacement
    // characters and no information.
    if bytes.iter().take(8000).any(|b| *b == 0) {
        return Err(format!("{rel} is binary"));
    }
    String::from_utf8(bytes).map_err(|_| format!("{rel} is not valid UTF-8"))
}

/// This editor's entry in the registry: what it is called, how it is drawn in
/// a header, whether it takes a target, and what the status bar says while it
/// has focus. Declared beside the editor so adding one is one file.
pub(crate) fn kind() -> immersion::EditorKind {
    immersion::EditorKind {
        id: "code",
        label: "Code",
        icon: "code",
        hints: &[("Chip", "Pick file")],
        targets: true,
    }
}

#[cfg(test)]
mod code_viewer {
    /// The viewer reads files, so its limits are its security surface: inside
    /// the root, not a directory, not binary, not enormous.
    #[test]
    fn it_declines_what_it_should_not_show() {
        let _guard = crate::editors::ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let tmp = std::env::temp_dir().join("im-code-view-test");
        std::fs::create_dir_all(tmp.join("dir")).expect("mkdir");
        std::fs::write(tmp.join("ok.rs"), b"fn main() {}\n").expect("write");
        std::fs::write(tmp.join("bin.dat"), [0x7f, 0x45, 0x00, 0x01]).expect("write");
        // SAFETY: single-threaded test; nothing else here reads the env.
        unsafe { std::env::set_var("POWDERMAN_FILES_ROOT", &tmp) };

        assert_eq!(
            super::read_source("/ok.rs").as_deref(),
            Ok("fn main() {}\n")
        );
        for (path, expect) in [
            ("/dir", "is a directory"),
            ("/bin.dat", "is binary"),
            ("/nope.rs", "no such file"),
            ("/../etc/passwd", "no such file"),
        ] {
            let err = super::read_source(path).expect_err(path);
            assert!(err.contains(expect), "{path}: got {err:?}");
        }
        unsafe { std::env::remove_var("POWDERMAN_FILES_ROOT") };
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn the_stamp_tracks_path_and_content() {
        // The renderer redraws when the stamp changes and skips when it does
        // not, so the stamp has to move for both a new file and an edit.
        let a = crate::editors::stamp_of("/a.rs", "fn a() {}");
        assert_eq!(a, crate::editors::stamp_of("/a.rs", "fn a() {}"), "stable");
        assert_ne!(
            a,
            crate::editors::stamp_of("/b.rs", "fn a() {}"),
            "path matters"
        );
        assert_ne!(
            a,
            crate::editors::stamp_of("/a.rs", "fn b() {}"),
            "content matters"
        );
    }
}
