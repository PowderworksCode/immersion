//! The files editor.

use dioxus::prelude::*;
use immersion::{FilterBox, Panel, TreeView};

use crate::editors::{Draw, prop};

use super::human_size;

/// The file browser: the tree view over a directory. Lazily loaded — a
/// directory reads only when its branch opens.
pub(crate) fn ed_files(target: Option<String>) -> Element {
    let root = target.unwrap_or_default();
    let children_of = Callback::new(move |pointer: String| {
        let at = if pointer.is_empty() {
            root.clone()
        } else {
            pointer
        };
        file_children(&at)
    });
    rsx! {
        div { class: "files-editor im-filter-scope",
            div { class: "keymap-head", FilterBox { placeholder: "filter files\u{2026}" } }
            TreeView { children_of }
        }
    }
}

/// Where the browser is rooted. `POWDERMAN_FILES_ROOT` confines it; without
/// one it is the daemon's working directory, which is where the things a run
/// touches live. Canonicalized, because containment is checked against it.
pub(crate) fn files_root() -> std::path::PathBuf {
    let raw = std::env::var("POWDERMAN_FILES_ROOT")
        .map(std::path::PathBuf::from)
        .or_else(|_| std::env::current_dir())
        .unwrap_or_else(|_| std::path::PathBuf::from("/"));
    raw.canonicalize().unwrap_or(raw)
}

/// One directory level. The demo serves a fabricated tree instead of the
/// machine's own: a public instance listing its container's filesystem is an
/// invitation, and it only gets worse when the code viewer can read what the
/// browser lists.
pub(crate) fn file_children(pointer: &str) -> Vec<immersion::TreeRow> {
    if crate::demo::enabled() {
        return crate::demo::file_children(pointer);
    }
    const CAP: usize = 500;
    let root = files_root();
    let dir = root.join(pointer.trim_start_matches('/'));
    // Containment, checked rather than assumed: resolve the path and require
    // it to still be under the root. A "../.." in a crafted pointer and a
    // symlink pointing outward both fail here, which is why the check is on
    // the resolved path and not on the text of the pointer.
    let Ok(dir) = dir.canonicalize() else {
        return Vec::new();
    };
    if !dir.starts_with(&root) {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut rows: Vec<(bool, bool, String, u64)> = entries
        .filter_map(|e| {
            let e = e.ok()?;
            let name = e.file_name().to_string_lossy().into_owned();
            // Symlinks are leaves: a link's target is better opened where it
            // really lives, and not following one keeps the walk inside root
            // without a second canonicalize per entry.
            let meta = e.path().symlink_metadata().ok()?;
            Some((meta.is_dir(), name.starts_with('.'), name, meta.len()))
        })
        .collect();
    rows.sort_by(|a, b| (!a.0, a.1, &a.2).cmp(&(!b.0, b.1, &b.2)));
    let extra = rows.len().saturating_sub(CAP);
    let mut out: Vec<immersion::TreeRow> = rows
        .into_iter()
        .take(CAP)
        .map(|(is_dir, _, name, len)| immersion::TreeRow {
            icon: if is_dir { "folder" } else { "file" }.to_string(),
            pointer: format!("{pointer}/{name}"),
            label: if is_dir { format!("{name}/") } else { name },
            preview: if is_dir {
                String::new()
            } else {
                human_size(len)
            },
            has_children: is_dir,
        })
        .collect();
    if extra > 0 {
        out.push(immersion::TreeRow {
            pointer: format!("{pointer}/\u{2026}"),
            label: format!("\u{2026} {extra} more"),
            preview: String::new(),
            has_children: false,
            icon: String::new(),
        });
    }
    out
}

/// The files that differ from HEAD, as pickable rows. On a demo these are
/// the fabricated ones; on a real instance `git status` answers, which is the
/// same question a person would ask the terminal.
pub(crate) fn changed_files() -> Vec<immersion::TreeRow> {
    let row = |path: String, state: &str| immersion::TreeRow {
        icon: "file-diff".to_string(),
        label: path.rsplit('/').next().unwrap_or(&path).to_string(),
        preview: format!("{state}  {path}"),
        pointer: path,
        has_children: false,
    };
    if crate::demo::enabled() {
        return crate::demo::changed_files()
            .into_iter()
            .map(|p| row(p.to_string(), "modified"))
            .collect();
    }
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(files_root())
        .arg("status")
        .arg("--porcelain")
        .output();
    let Ok(out) = out else { return Vec::new() };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| {
            // "XY path", where X is the index state and Y the worktree's.
            // A rename reads "R  old -> new"; the new name is the one to show.
            let (flags, path) = line.split_at(line.len().min(3));
            let path = path.rsplit(" -> ").next()?.trim();
            if path.is_empty() {
                return None;
            }
            let state = match flags.trim() {
                "??" => "new",
                "D" | " D" => "deleted",
                "A" | "A " => "added",
                _ => "modified",
            };
            Some(row(format!("/{path}"), state))
        })
        .collect()
}

/// This editor's entry in the registry: what it is called, how it is drawn in
/// a header, whether it takes a target, and what the status bar says while it
/// has focus. Declared beside the editor so adding one is one file.
pub(crate) fn kind() -> immersion::EditorKind {
    immersion::EditorKind {
        id: "files",
        label: "Files",
        icon: "folder",
        hints: &[("Click", "Expand"), ("Chip", "Root here")],
        targets: true,
    }
}

/// What the browser is looking at. `has_children` is the only thing a row
/// says about its kind, so it is what the folder/file split counts.
pub(crate) fn sidebar(d: &Draw) -> Element {
    let root = d.arg.clone().unwrap_or_default();
    let rows = file_children(&root);
    let folders = rows.iter().filter(|r| r.has_children).count();
    rsx! {
        div { class: "area-props",
            Panel { title: "Folder",
                {prop("Root", if root.is_empty() { "/".to_string() } else { root.clone() })}
                {prop("Entries", rows.len().to_string())}
                {prop("Folders", folders.to_string())}
                {prop("Files", (rows.len() - folders).to_string())}
            }
            Panel { title: "Changed", open: false,
                {prop("Files", changed_files().len().to_string())}
            }
        }
    }
}

/// The bottom line: how much is in the folder being browsed.
pub(crate) fn footer(d: &Draw) -> String {
    let rows = file_children(&d.arg.clone().unwrap_or_default());
    let folders = rows.iter().filter(|r| r.has_children).count();
    format!(
        "{} entries · {folders} folders · {} files",
        rows.len(),
        rows.len() - folders
    )
}

#[cfg(test)]
mod file_browser {
    /// Containment is the property that matters: whatever pointer arrives,
    /// the browser must not read outside its root. Checked on the resolved
    /// path, so a crafted "../.." fails even though the text looks harmless
    /// after joining.
    #[test]
    fn a_pointer_cannot_climb_out_of_the_root() {
        let _guard = crate::editors::ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let tmp = std::env::temp_dir().join("im-files-root-test");
        let inner = tmp.join("inside");
        std::fs::create_dir_all(&inner).expect("mkdir");
        std::fs::write(inner.join("kept.txt"), b"hi").expect("write");
        // SAFETY: single-threaded test process; the var is read on the next
        // line and nothing else in this test touches the environment.
        unsafe { std::env::set_var("POWDERMAN_FILES_ROOT", &tmp) };

        let inside = super::file_children("/inside");
        assert!(
            inside.iter().any(|r| r.label == "kept.txt"),
            "the root's own files list: {inside:?}"
        );
        for escape in ["/..", "/../..", "/inside/../..", "/../etc"] {
            assert!(
                super::file_children(escape).is_empty(),
                "{escape} escaped the root"
            );
        }
        unsafe { std::env::remove_var("POWDERMAN_FILES_ROOT") };
        std::fs::remove_dir_all(&tmp).ok();
    }
}
