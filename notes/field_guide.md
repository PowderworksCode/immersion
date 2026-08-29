# Agent field guide

Durable notes for anyone — human or agent — starting work in this repository.
Append what you learn; keep it to things that are true and not obvious from the
code.

## What this is

Two crates in one workspace, and they are not peers.

- `immersion/` is the library: Blender-style tiling areas as a binary tree of
  editors, held as **server-side** state over Dioxus liveview. The layout is a
  value, every mutation is a named command, rendering is a component over the
  value. It is host-agnostic.
- `powderman/` is the first host and the reason the library exists: a durable
  workflow daemon on SQLite, with editors for machine, fleet, runs, actions,
  timers, run detail, settings, and an in-process MCP server that exposes the
  same command bus as agent tools.

The point of the command bus is that there is one write path. A button, a chord,
a gesture and an agent all arrive as the same command, which is what makes the
MCP surface and the UI stay in step rather than drifting into two half-features.
`docs/roadmap.md` is unusually honest about where that is a convention rather
than an enforced guarantee — read its "where we are lying to ourselves" section
before assuming an invariant holds.

## Building and testing

There is **no `rust-toolchain.toml`**. The workspace declares
`edition = "2024"` and `rust-version = "1.95"`, and CI uses whatever stable
ships on `ubuntu-latest`. `fleet-lint.yml`'s hawk job separately installs 1.98.0
for itself; that is not the project toolchain.

`cargo build` needs **no JavaScript toolchain**. That is deliberate — the
container image that ships powderman has none — and it is why the compiled `.js`
and the vendored bundles are committed. The JS toolchain is only needed when you
change the TypeScript.

The four gates, which are the same four commands to run locally:

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
# plus straitjacket, which CI fetches as a prebuilt binary
```

Both `Cargo.lock` and `bun.lock` are committed and CI installs with
`--frozen-lockfile`.

`fleet-lint.yml` and `.github/dependabot.yml` are distributed by conf. Edit them
there; a local change is drift the next fleet sync reports.

## The generated-file rule, and the three ways to violate it

Three categories of file are generated, committed, and checked for drift by
`.github/workflows/shims.yml`. A red Shims job almost always means "you changed
a source and did not commit the regenerated output".

| generated | regenerate with | source |
| --- | --- | --- |
| `immersion/ts/generated/*.ts` (wire types) | `cargo test -p immersion export_bindings` | Rust types via `ts-rs` |
| `immersion/src/*.js` (client shims) | `bun run build` | `immersion/ts/*.ts` |
| `immersion/vendor/{diffs,vega,icons}/` | `bun run build:vendor` | `immersion/ts/vendor/*` and node_modules |

The job runs them in that order on purpose: the wire types are regenerated
*before* the typecheck, so a Rust-side change the TypeScript has not caught up
with fails in CI rather than at runtime.

**The Bun version is an input, not a detail.** `package.json`'s
`packageManager: "bun@1.3.14"` and the `oven-sh/setup-bun` pin in `shims.yml`
must name the same version. The lockfile pins what goes *into* the vendor
bundle; the bundler is an input too, and different Bun releases mangle
identifiers differently — so an unpinned bump makes the committed bundle stale
against itself and turns every open pull request red with nobody having changed a
line. That already happened once. Bumping Bun is a real change: bump both places,
run `bun run build:vendor`, and commit the rebuilt bundle in the same commit.

**`docs/reference.md` is generated too**, from the command bus, MCP router,
keymap and editor registry — regenerate with
`UPDATE_DOCS=1 cargo test -p powderman`. A stale file fails
`cargo test`, not the Shims job, so this one shows up as a Rust test failure.
Adding a command, a tool, a keybinding or an editor means regenerating it.

**`.ignore` is not a `.gitignore`.** It lists `immersion/vendor/` so ripgrep-family
tools (straitjacket included) skip it. Those files are committed; they are simply
minified Apache-2.0 third-party output that nobody here can fix, and linting them
buries the real findings.

## Landmines

**`scripts/build-diffs.ts` does more than bundle.** It prunes shiki's grammars
and themes down to a hardcoded `KEEP` allowlist (~10 MB to ~4 MB), then does its
own static-analysis pass over the *bundled output*, regex-scanning for relative
import strings, to prove pruning never removed a chunk something still statically
imports. Bumping `@pierre/diffs` or wanting a new highlighted language means
editing that allowlist, not just the dependency.

The script also writes `immersion/vendor/diffs/manifest.json`, with a comment
saying "a manifest the Rust side asserts against". As of this writing no Rust
file reads it — `grep -r manifest --include=*.rs` finds nothing but
`CARGO_MANIFEST_DIR`. The guard the comment describes is not wired up, so do not
rely on it; the drift check in `shims.yml` is what actually catches a stale
bundle.

**A transient systemd unit inherits nothing.** `powderman/src/exec.rs` runs
headless work with `systemd-run --user --pipe --wait --collect`, and the unit
does not get the daemon's `PATH` or `HOME` unless they are passed with
`--setenv`. Without them `cargo`, `npx` and `tree-sitter` silently vanish and the
failure reads as "toolchain missing" when it is actually "environment not
passed". The same class of problem is why `powderman/src/herdr.rs` has
`ensure_socket_env()` — `herdr-sdk` has no fallback for `HERDR_SOCKET_PATH`, and
a systemd-launched daemon inherits no login shell. `powderman/systemd/powderman.service`
encodes all of this as comments; read it before debugging a "works in my shell"
failure.

There is a test in `exec.rs` that asserts the passed `PATH` survives, and it runs
`printenv PATH` rather than `bash -lc 'echo $PATH'` — a login shell re-sources
the profile and would make the test pass for the wrong reason.

**Fly runs in demo mode, and cannot do the real work.** `fly.toml` and
`fly.preview.toml` both set `POWDERMAN_DEMO=1`, and production also sets
`POWDERMAN_SCHEDULES=0` as belt and braces against the 06:00 sweep firing with no
checkout. A Fly machine has no herdr and no systemd, so fleet and run execution
are refused there while the workbench, command bus, palette, widgets and MCP
server all work. The database is on ephemeral disk with no volume, so every
deploy resets state — that is intended, not a bug to fix.

`POWDERMAN_MCP_ALLOWED_HOSTS=*` in the Dockerfile is also load-bearing: rmcp's
DNS-rebinding guard defaults to loopback only, so any non-localhost deployment
needs it set or `/mcp` refuses the request.

**`navigator.platform` is deprecated and can return empty**, which silently makes
`isMac` false and breaks every Cmd chord on a Mac. The keymap shim checks
`navigator.userAgent` as well. `docs/keymap-web-safety.md` records which Blender
chords had to be remapped because the browser or OS reserves them — maximize
moved off `Ctrl+Space`, workspace cycling onto `Alt+PageUp/PageDown`.

## Where the surprising code lives

- `powderman/src/exec.rs` — the deliberate split between systemd-run for headless
  work and herdr for anything a human or agent might watch. The reasoning is in
  the module doc: a pane hands back text, so "the build failed" and "the build
  printed the word failed" look identical, and a real exit code is worth the
  extra machinery.
- `immersion/src/vendor.rs` — embeds the vendored bundles with `include_dir!` and
  serves them, with path-traversal guards that are actually tested. The same
  defensive shape appears in `powderman/src/editors/files.rs`.
- `powderman/src/treebank.rs` — a partial port of an existing `scripts/daily.sh`.
  It documents what it does *not* yet replicate and runs unscheduled specifically
  so it cannot collide with the live cron job it is meant to replace.
- `powderman/src/reference.rs` — the generator behind `docs/reference.md`, and the
  test that holds the file to the code.

## docs/

- `docs/reference.md` — generated; every command, MCP tool, keybinding, editor.
- `docs/roadmap.md` — what is built versus aspirational, and the ordered plan.
- `docs/visual-study.md` — Blender vs. the old TypeScript immersion vs. this one,
  with the chrome backlog.
- `docs/keymap-web-safety.md` — the chords the browser will not let you have.
