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

## The next editors

Ordered so each builds on the last. All read-only where "read" is meaningful;
text *editing* is explicitly out of this phase (the liveview budget rule makes
an editing buffer a client-side project, and wrapping CodeMirror inside a
codebase whose rule is "own your grammars" is a decision to take deliberately
or not at all).

1. **Outliner** — Blender's Outliner generalized: a tree view over any serde
   value, with expand/collapse state as layout data, filtering via the
   existing FilterBox, selection as a command (`select`, on the bus, so an
   agent can navigate). The filesystem viewer is the first instance (a
   directory tree is a lazily-loaded serde value); the settings document and
   a run's step tree are the second and third, free. Everything later —
   diffs, PRs, charts — wants a tree somewhere.
2. **Code viewer** — read-only, server-side syntax highlighting (`syntect`),
   line numbers, goto-line via palette. The document is `{path, language,
   text}`; scrolling is frame-path (client), everything else is server truth.
3. **Diff viewer** — unified and side-by-side over two texts; then `git diff`
   as the obvious host feed. Together with the outliner this makes powderman
   able to *show its work* — a fix run's worktree, its diff — inside the
   workbench instead of a terminal.
4. **PR viewer** — a powderman editor, not a library one: `gh`-backed list /
   detail / files, composed from outliner + code + diff. This is the proof
   that the library pieces compose into a host-specific tool without new
   library surface — the "toolkit" claim, tested.
5. **Data display** — the chart editor. The design follows from the research
   below: the document is a *declarative chart spec* (a serde value like
   everything else), the editor renders it to SVG server-side, and commands
   mutate the spec. Which spec grammar — a small owned one in the house
   style, versus embedding a Vega-Lite subset — is the one open question, and
   the doc takes a position: start with a small owned grammar (bar, line,
   scatter, table; explicit axes; theme tokens for color), shaped so that a
   Vega-Lite-subset importer is a mapping, not a rewrite.

### Why declarative specs, and the LLM angle

The charting ecosystem converged during 2025–26 on exactly the pattern this
codebase already uses: **LLMs and agents author declarative JSON specs; a
renderer owns the drawing.** [VegaChat](https://arxiv.org/abs/2601.15385)
generates and validates Vega-Lite from natural language;
[chat2plot](https://github.com/nyanp/chat2plot) has the LLM emit a JSON plot
spec precisely because generated *code* is unsafe and unvalidatable;
[Highcharts](https://5of10.com/articles/best-chart-library-for-llm-output/)
now ships production MCP servers; Databricks builds
[multi-agent Vega-Lite pipelines](https://www.databricks.com/blog/bringing-visualizations-life-multi-agent-systems-vega-lite).
The shared reasons: a spec can be schema-validated before it renders, edited
incrementally, diffed, and carried in a constrained context window —
[Vega-Lite](https://vega.github.io/vega-lite/)'s compactness is cited as the
deciding property.

For immersion this is not an adaptation, it is the default: a chart spec is a
serde document, mutated by commands, readable and writable by an agent over
the MCP tools that already exist. `set_setting` with a pointer into a chart
spec *is* the chart-editing API. Rust-side rendering has precedent if we ever
want Vega-Lite itself ([vl-convert](https://github.com/vega/vl-convert),
[avenger](https://github.com/jonmmease/avenger)), but both are heavy; the
owned-grammar start keeps the renderer a few hundred lines of SVG generation
against theme tokens, which is the same trade we made for expressions.

## What we are not doing (this phase)

- **Runtime extensibility.** Hosts add editors, commands, themes and fields
  at compile time. No plugin loading, no runtime schema registry. The old
  repo's plan reserved "live schemas and views" for its horizon phases with
  zero seed in code; same here, same reason.
- **Text editing.** Read-only viewers first. Revisit only with a deliberate
  decision on own-vs-wrap.
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
3. **Outliner** editor + filesystem feed in powderman.
4. **Code viewer** (`syntect`), then **diff viewer**, then the **PR viewer**
   composition in powderman.
5. **Chart editor** with the owned spec grammar; powderman's machine charts
   become its first consumer (charts-as-documents replace charts-as-code).
6. **Chrome backlog** interleaved: preferences editor, task progress,
   tab reorder, sweep-drag, area-scoped hints, collapsible panels.

The order encodes the priorities: make the existing claims true (1–2) before
widening the surface (3–5), and let the chrome polish ride along rather than
lead.
