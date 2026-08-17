//! The vendored renderer, embedded.
//!
//! `@pierre/diffs` (Apache-2.0) draws code and diffs. It is committed rather
//! than fetched — the same bargain the generated shims make, so a Rust build
//! needs no javascript toolchain — and embedded rather than read from disk, so
//! a deployed binary is still one file.
//!
//! It is code-split: the entry is small and each language grammar loads on
//! demand, which is the only reason a renderer this size is affordable. The
//! build prunes the grammars we never ask for (`scripts/build-diffs.ts`), and
//! the entry maps an unknown extension to plain text rather than requesting a
//! chunk that is not here.
//!
//! Serve it under [`MOUNT`]; the host adds one route.

use include_dir::{Dir, include_dir};

/// Where the bundle expects to be served from. Its chunks import each other by
/// relative path, so the directory has to stay a directory.
pub const MOUNT: &str = "/vendor/diffs";

static DIFFS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/vendor/diffs");

/// One vendored file, by name (`entry.js`, `rust-a1b2c3.js`). `None` for
/// anything not in the bundle — a path from a request must never reach the
/// filesystem.
pub fn asset(name: &str) -> Option<&'static [u8]> {
    // No traversal: the bundle is flat, so a name with a separator in it is
    // not one of ours whatever it resolves to.
    if name.contains('/') || name.contains("..") {
        return None;
    }
    DIFFS.get_file(name).map(|f| f.contents())
}

/// The `<script>` the page needs. A module, deferred by nature, so it does not
/// block the first paint.
pub fn script_tag() -> String {
    format!(r#"<script type="module" src="{MOUNT}/entry.js"></script>"#)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundle_is_embedded_and_reachable() {
        let entry = asset("entry.js").expect("the entry is vendored");
        assert!(
            entry.len() > 10_000,
            "entry looks truncated: {}",
            entry.len()
        );
        assert!(
            DIFFS.files().count() > 20,
            "grammar chunks are missing; run scripts/build-diffs.ts"
        );
    }

    #[test]
    fn a_request_cannot_leave_the_bundle() {
        for bad in ["../Cargo.toml", "a/b.js", "../../etc/passwd", "..%2fx"] {
            assert!(asset(bad).is_none(), "{bad} resolved");
        }
        assert!(asset("nope.js").is_none());
    }
}
