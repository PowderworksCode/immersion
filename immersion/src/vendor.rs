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

/// Where the bundles are served from. Their chunks import each other by
/// relative path, so each stays a directory of its own.
pub const MOUNT: &str = "/vendor";

/// The renderers, by bundle name. Two, for now: `diffs` draws code and
/// changes, `vega` draws charts. Both are entered through `entry.js`.
static DIFFS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/vendor/diffs");
static VEGA: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/vendor/vega");

/// The bundles a page loads, in order.
pub const BUNDLES: &[&str] = &["diffs", "vega"];

/// One vendored file, by bundle and name (`diffs`, `rust-a1b2c3.js`). `None`
/// for anything not in a bundle — a path from a request must never reach the
/// filesystem.
pub fn asset(bundle: &str, name: &str) -> Option<&'static [u8]> {
    // No traversal: each bundle is flat, so a name with a separator in it is
    // not one of ours whatever it resolves to.
    if name.contains('/') || name.contains("..") {
        return None;
    }
    let dir = match bundle {
        "diffs" => &DIFFS,
        "vega" => &VEGA,
        _ => return None,
    };
    dir.get_file(name).map(|f| f.contents())
}

/// The `<script>` tags the page needs. Modules, deferred by nature, so they
/// do not block the first paint.
pub fn script_tag() -> String {
    BUNDLES
        .iter()
        .map(|b| format!(r#"<script type="module" src="{MOUNT}/{b}/entry.js"></script>"#))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bundle_is_embedded_and_reachable() {
        for b in BUNDLES {
            let entry = asset(b, "entry.js")
                .unwrap_or_else(|| panic!("{b} is not vendored; run its build script"));
            assert!(
                entry.len() > 10_000,
                "{b} entry is truncated: {}",
                entry.len()
            );
            // Every bundle needs a script tag, or it is dead weight in the
            // binary that nothing ever loads.
            assert!(script_tag().contains(&format!("/vendor/{b}/entry.js")));
        }
        assert!(
            DIFFS.files().count() > 20,
            "grammar chunks are missing; run scripts/build-diffs.ts"
        );
    }

    #[test]
    fn a_request_cannot_leave_a_bundle() {
        for bad in ["../Cargo.toml", "a/b.js", "../../etc/passwd", "..%2fx"] {
            assert!(asset("diffs", bad).is_none(), "{bad} resolved");
        }
        assert!(asset("diffs", "nope.js").is_none());
        // And a bundle nobody vendored is not a directory to go looking in.
        assert!(asset("../vega", "entry.js").is_none());
        assert!(asset("nope", "entry.js").is_none());
    }
}
