# The workbench, honestly — where immersion is, and where it goes

*Replaces the earlier "Blender-like interface on top of serde" design (PR #56),
which described a direction. This describes a position: what exists as of
August 2026, what is convention rather than guarantee, how far the Blender
chrome actually is, and which editors come next. Where a claim is checkable,
the file that proves it is named.*

## The thesis, restated in one paragraph

Every document the workbench touches — layout, settings, keymap, favourites,
and soon file trees, diffs and chart specs — is a serde value. Every mutation
goes through named commands (`immersion/src/command.rs`), so a header button,
a keystroke, a right-click menu and an MCP tool call are the same operation by
construction. The wire between server and browser is declared once in Rust and
the TypeScript is generated from it (`ts-rs`, checked in CI). Client-side code
exists only for frame-path work — drags, keystrokes, hovers — and commits one
message on release. Grammars we need, we declare rather than hand-roll
(`chumsky` for number expressions). This held up under ~40 merged PRs; it is
the part of the design that has stopped being a bet.

## Where we are

**The library** (`immersion/`, ~5,100 lines of Rust + TS): areas with n-ary
splits, seam drag, corner gestures for split/join/swap; per-area regions
(toolbar, sidebar, header hide/flip); workspaces with tabs; splash with
templates and recents; context menus, click-open menus, pie menus, quick
favourites; a command palette; menu accelerators; a keymap with Mac-correct
chords, a rebind editor with capture, and a help overlay; tooltips that name
their shortcut; a status bar with hints, a transient report, and an instance
badge; themes as token sets; resolution scale; a widget kit (slider, toggle,
radio, vector, color, text) bound to serde documents by JSON pointer, with
arithmetic expression entry via a declared grammar; snapshot undo/redo; layout
export/import.

**The host** (`powderman/`): nine editors — machine, fleet, runs, actions,
timers, run detail, settings, info, keymap — over a durable-workflow daemon,
plus 18 MCP tools mounted in-process. The host is the library's only real
consumer, on purpose (see "what we are not doing").

**The infrastructure**: CI gates every PR on fmt, clippy `-D warnings`, tests
and straitjacket; the shims workflow regenerates wire types and fails on
drift; Fly previews per PR and a public demo (`powderman-demo.fly.dev`) that
deploys from main, seeds believable data, badges itself DEMO, and refuses to
execute anything.

## Where we are lying to ourselves

An honest accounting names the gaps between the story above and the code.
The previous incarnation of this project (the TypeScript immersion) wrote a
plan of record whose sharpest lesson was: *"one authoritative mutation path"
was a convention, not an invariant, and every phase built on it inherited the
difference.* We are repeating that mistake in miniature:

1. **Not everything is on the bus.** Layout and editor commands route through
   `Commands::run`, but about ten host actions — undo, redo, repeat, maximize,
   fullscreen, set_setting, favorite_add, the pie and palette openers — are a
   bare `match` in `powderman/src/ui.rs:536`. A human can undo; an agent
   cannot. Nothing enforces parity, so it drifts exactly where it is
   inconvenient to maintain.
2. **Errors go nowhere.** `eval_expr` returns `{message, column}` and nothing
   displays it; a bad expression silently leaves the field unchanged. Command
   failures likewise: `Commands::run` returns `Result`, and most callers drop
   the error. The machinery for honesty exists; the surface does not.
3. **The library has one consumer.** Every API boundary decision is really a
   powderman decision until a second host exists. Accepted for this phase —
   extraction is deferred, not denied — but it should be said plainly.
4. **No integration tests.** The browser checks that caught the `type=number`
   operator-stripping bug and the n-ary layout-discard bug were run by hand.
   CI proves the code compiles and unit-passes; nothing in CI opens a page.

## How far the Blender chrome is

Measured against the interface map the old repo maintained (Blender 2.93,
feature by feature), counting only what applies to a web workbench:

**Done and faithful:** the three-part frame (topbar / areas / status bar),
workspace tabs with add/rename/duplicate/close/cycle, splash, area split/join/
swap/maximize by gesture and menu, regions with T/N toggles, header hide/flip,
editor switching in place, context + dropdown + pie menus, quick favourites
(Q), command palette, keymap rebinding with capture, tooltips with chords,
status-bar hints and report, themes, resolution scale, distraction-free
fullscreen, undo/redo, layout files.

**Missing, applicable, and worth having — the chrome backlog:**

- **Preferences editor** — Blender's Edit ▸ Preferences window. The settings
  editor exists; preferences-as-an-editor with sections (interface, themes,
  keymap, add-ons-equivalent) does not.
- **Error surfacing** — the red field and the Info report. Decided (below),
  not yet built. This is the largest gap in *feel*: Blender never swallows an
  error silently.
- **Running-task progress** — the status-bar progress bar with cancel. We
  have long-running runs and no progress affordance outside the runs editor.
- **Workspace tab reorder** (drag), **area-scoped status hints** (the hint
  line reflecting the hovered area's editor), **sweep-drag toggles** (drag
  across checkboxes to set many), **collapsible panels** inside regions —
  each small, each part of why Blender feels dense but calm.
- **Editor-owned header menus** — our headers offer View; Blender's editors
  own their menu sets. Needs the editor registry to carry menu declarations.

**Deliberately cut** (3D, filesystem, OS-window concerns) stays cut.

## The error pattern (decided)

One error type, two surfaces, Blender-style:

```rust
pub struct EditorError {          // EvalError generalized
    pub message: String,          // the parser's words, or the command's
    pub column: Option<usize>,    // where, when the source is a text field
}
```

- **Field-local**: the widget that produced the error flags itself —
  `im-invalid` class, message in `title` — and clears on the next successful
  commit. Requires `number_widget` and friends to become components with
  per-field state; this is the refactor, and it is worth it.
- **Status report**: the same error lands in the host's report slot (the
  existing `report()` path), the way Blender's Info line echoes warnings.
- **Command errors ride the same rails.** `Commands::run` failures stop being
  dropped: the host reports them, and an MCP caller gets them verbatim —
  agent parity for failure, not just success.

## The data model, made visible

Two additions extend the plan past Blender's chrome — and it turns out they
are not extensions at all, but Blender's own architecture followed further
than its screenshots. Blender's UI is generated from an underlying typed data
tree (RNA): the Outliner has a **Data API** display mode that exposes that
entire tree as browsable rows; every property answers right-click ▸ *Copy Full
Data Path*; and every editor's header carries a selector for *which*
data-block it is looking at, with a pin to hold it. We adopt all three,
translated to serde.

**The data editor.** Is the underlying structure a tree? Yes — a serde value
is strictly a tree (no cycles, no shared references), and the workbench holds
several of them. The data editor mounts them under one virtual root:

```
/layout      the workspace tree (what Areas renders)
/settings    the settings document
/keymap      overrides
/favorites   quick favourites
/state       the host snapshot (read-only): runs, fleet, machine
```

One editor shows that root as an expandable tree — Blender's Data API mode
for our world. Every row knows its JSON pointer; right-click ▸ *Copy data
path*. Read-only first; editing arrives later as ordinary bus commands
(`set_setting` already edits `/settings` by pointer — the data editor makes
the address space it operates on visible).

**Editor targets as locators.** An area leaf today is `{editor, arg}`; the
`arg` generalizes to a **target**: a pointer into the mounted root. The
header menu shows it as a breadcrumb chip — info first (what is this editor
looking at?), navigation second: clicking it opens a picker over the data
tree, and choosing a node retargets the editor through one bus command
(`set_target {id, pointer}`), which means agents retarget editors the same
way people do. The run-detail editor is the existing proof (its arg is a run
id — a locator by another name); the chart editor makes it essential (a chart
targets the subtree it plots).

## The next editors

Ordered so each builds on the last; read-only where "read" is meaningful.
Text *editing* stays out of this phase (the old immersion wrapped CodeMirror
for its CodePane — precedent if we ever choose wrapping deliberately).

1. **The tree view**, once, as a library component — expandable rows over any
   serde value, lazy children, filtering via FilterBox, selection on the bus.
   Two instances immediately: the **data editor** (the mounted root above)
   and the **file browser** (a directory tree is a lazily-loaded serde value
   fed by the host). One component, two editors, and the target-picker
   falls out of the same code.
2. **Editor targets** — the locator chip and picker, once the tree exists to
   pick from.
3. **Code viewer** — read-only, `syntect` highlighting server-side, line
   numbers, goto-line via the palette. Document: `{path, language, text}`.
4. **Diff viewer** — the diff is computed in Rust (`similar`) and is server
   truth; the *rendering* is [diff2html](https://github.com/rtfpessoa/diff2html),
   vendored (76 KB min / 20 KB gzip + 20 KB CSS, MIT). It takes exactly what
   `similar` emits — unified diff text — and draws GitHub-quality line-by-line
   or side-by-side HTML with intra-line highlights. This is a deliberate,
   sized exception to "own the drawing": diff *presentation* is a commodity
   with one dominant idiom, and 20 KB of vendored renderer beats reinventing
   line-matching. The shim receives the diff in one message and renders
   locally — the budget rule holds.
5. **Chart editor** — Vega-Lite, the real one. The criterion is not elegance
   but *training-data mass*: [Vega-Lite](https://vega.github.io/vega-lite/)
   is the grammar LLMs demonstrably know — it is what
   [VegaChat](https://arxiv.org/abs/2601.15385) generates, what
   [chat2plot](https://github.com/nyanp/chat2plot) emits, what Databricks'
   [agent pipelines](https://www.databricks.com/blog/bringing-visualizations-life-multi-agent-systems-vega-lite)
   speak, and the ecosystem's research default. An owned grammar would be
   cleaner and *unknown to every model* — wrong trade here. Concretely:
   - The chart document **is** a Vega-Lite spec (a serde value; `$schema`
     and all). Agents write specs over the MCP tools that already exist;
     `set_setting`-style pointer edits are the chart-editing API for free.
   - Rendering is the vendored Vega stack served from the binary
     (vega 512 KB + vega-lite 248 KB + vega-embed 60 KB ≈ 274 KB gzip
     total, BSD-3) — a client shim mounts the spec, and re-renders on
     commit. Data can be inline or a named feed the host resolves.
   - Specs are validated server-side against the published Vega-Lite JSON
     schema before they render; a bad spec surfaces through `EditorError`
     like any other bad input, instead of a blank panel.
   - `vl-convert` stays noted as the later server-side path for PNG/SVG
     export; it is not needed to ship the editor.
   - Powderman's machine charts become the first consumer: charts-as-specs
     replace charts-as-code, and the demo gets visibly better.

Vendoring note: both exceptions ride the existing pattern — assets served by
the daemon (`include_bytes!`), pinned versions committed with checksums, no
CDN at runtime.

## What we are not doing (this phase)

- **Runtime extensibility.** Hosts add editors, commands, themes and fields
  at compile time. No plugin loading, no runtime schema registry. The old
  repo's plan reserved "live schemas and views" for its horizon phases with
  zero seed in code; same here, same reason.
- **Text editing.** Read-only viewers first. Revisit only with a deliberate
  decision on own-vs-wrap.
- **PR viewer.** Deferred by decision. It remains the natural test of the
  composition claim (outliner + code + diff + gh feed, no new library
  surface), and everything it needs ships in steps 1–4 — but it is not this
  phase's work.
- **Publishing / second host.** Powderman-first, extract later. The cost —
  API decisions go untested by a second consumer — is accepted and named.
- **Multi-user rooms.** The old immersion had durable rooms and presence;
  liveview gives every visitor the same server truth, which is accidental
  multi-user. Deliberate multi-user (cursors, presence, per-user state) is
  horizon work.

## Sequence

Each step is a PR-sized unit; each leaves main coherent.

1. **Parity made honest** — move the ui.rs `match` actions onto the command
   bus; add the equivalence test (every palette entry and default binding
   resolves to a bus command); MCP gets undo/redo for free.
2. **Errors surface** — `EditorError`, widget components with invalid state,
   report-slot wiring, command failures reported. The expression parser's
   messages become visible, which is why the parser got good messages.
3. **Tree view** component → **data editor** (mounted root, copy-data-path)
   and **file browser** (lazy fs feed) as its first two instances.
4. **Editor targets** — locator chip in the header menu, picker over the
   tree, `set_target` on the bus.
5. **Code viewer** (`syntect`), then the **diff viewer** (`similar` +
   vendored diff2html).
6. **Chart editor** — Vega-Lite specs as documents, vendored Vega stack,
   schema validation through `EditorError`; powderman's machine charts
   migrate onto it.
7. **Chrome backlog** interleaved: preferences editor, task progress,
   tab reorder, sweep-drag, area-scoped hints, collapsible panels.

The order encodes the priorities: make the existing claims true (1–2) before
widening the surface, put the data model on screen before the editors that
navigate by it (3–4), and let the chrome polish ride along rather than lead.
