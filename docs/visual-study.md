# Three workbenches: Blender, the old immersion, and this one

*A visual comparison, written from the old repository's own screenshots, the
current build running on demo data, and Blender's interface conventions. It
is a survey of what each one looks like and why, not a wish list — the
recommendations at the end are what the comparison actually argues for, and
several of them argue for leaving things alone.*

## What the three are

**Blender** is the source. A window is a gapless tiling of areas; each area
has a header whose leftmost control is an icon-only editor-type selector, and
each editor's identity is carried by that icon rather than by a word.

**The old immersion** (TypeScript, dockview, React) reproduced that on the web
with a rich chrome: Tabler icons throughout, uppercase panel section headers,
per-pane help lines, a topbar with search and a room indicator.

**This immersion** (Rust, Dioxus liveview) reproduces the same model with the
server owning state. It arrived at similar chrome by a different route, and
the differences are mostly consequences of that route.

## Element by element

| | Blender | old immersion | this one |
|---|---|---|---|
| Editor selector | icon + chevron, no text | icon + name + chevron | **icon + name + chevron** |
| Editor menu | icons per row | icons per row | **icons per row** |
| Area header | menus, then editor controls | View / Edit menus, editor controls | View menu, region toggles, split/join buttons |
| Region toggles | `T` / `N` keys, no buttons | keys | keys **and** header buttons |
| Workspace tabs | text, active highlighted | text, ✕ on active | text, ✕ on hover, drag to reorder |
| Panel sections | uppercase, collapsible | uppercase, collapsible | title case, collapsible |
| Outliner rows | per-type icons | per-type icons | **per-type icons** |
| Status bar | keymap hints, progress, version | mouse hints, version | area hints, progress, counts, badge |
| Per-pane help | — | a help line at the pane's foot | — (the status bar carries it) |
| Colour picker | wheel + value bar | wheel + value bar | SV square + hue bar |
| List rows | visibility toggles, reorder arrows | visibility toggles, +/−/… column | — |

Bold marks what this PR series brought level.

## Where this one is now equivalent

**Icons on editors.** The gap that prompted the study. Every editor declares a
glyph, the header shows it before the name, and the editor menu shows one per
row — the same information architecture as both references. Fourteen glyphs,
vendored as path data rather than as a dependency.

**Icons on tree rows.** Blender's Outliner marks every row by what it is;
ours now does, per serde shape, which is what makes a data tree skimmable
rather than readable.

**Area-scoped status hints.** Blender's status bar changes with the active
area. Ours follows the focused area and, unlike either reference, says the
non-chord things too — *Drag → Scrub number*, *Type → `3*2` works*.

## Where this one deliberately differs

**The selector keeps its name.** Blender's is icon-only because a Blender user
learns twenty icons once and then wants the pixels back. This workbench has
fourteen editors and a newcomer every time someone opens the demo; the name
earns its width. If the editor count doubles, revisit — that is the condition,
not a matter of taste.

**Region toggles are buttons as well as keys.** Blender expects `T` and `N` to
be muscle memory. A workbench reached by URL cannot expect that, so the header
shows them.

**No per-pane help line.** The old immersion put a sentence at the foot of
each pane. It is genuinely useful and costs a row of every pane forever; the
area-scoped status bar carries the same content in space that already exists.

**A square-and-strip colour picker, not a wheel.** Both references use a wheel.
Ours matches Blender's *other* picker, the SV square, which is easier to hit
precisely with a mouse and cheaper to draw. No plan to change it.

## Where this one is genuinely behind

These are the findings, in the order the comparison makes them obvious.

1. **Panel section headers are title case, not uppercase.** Both references
   use small uppercase labels for sections — `OPERATOR BUTTONS`, `LIVE STATE`,
   `MODIFIERS`. It reads as structure rather than as prose, and at a glance
   the difference is larger than it sounds. One CSS rule.

2. **List rows have no per-row controls.** Blender's modifier stack and the
   old immersion's list view carry visibility toggles and reorder affordances
   on the row itself. Our runs and fleet lists carry an open button and
   nothing else. This is the biggest remaining gap in *feel*, and the least
   trivial: it needs a row-actions convention rather than one control.

3. **The topbar has no search.** Both references put one in the topbar; ours
   has the command palette on F3 and no visible affordance for it. A palette
   nobody can see is a palette newcomers do not use.

4. **Icons stop at editors and trees.** Menu rows outside the editor menu, the
   header's own buttons (`⬒ ◧ ✕`), and the workspace `+` are still glyph
   characters. They are legible but they are not the same font, weight, or
   grid as the sprite, which is visible when they sit next to each other.

5. **No editor-owned header controls.** Blender's headers carry the editor's
   own tools; the old immersion's code pane had Wrap / Comment / Undo / Redo
   in its header. Ours are identical across every editor. The registry would
   have to grow a way for an editor to contribute header items.

## What the comparison does not recommend

**Do not adopt the room indicator or presence chrome.** The old immersion was
multi-user by design and its topbar said so. This one is accidentally
multi-user — every visitor sees the same server truth — and chrome implying
presence tracking would be a promise the daemon does not keep.

**Do not chase Blender's density.** Blender is a professional tool used all
day by people who have paid its learning cost. This workbench is opened by
people who have not. The current spacing is looser than Blender's and that is
the right trade for now; the Density setting already exists for anyone who
disagrees.

## Order of work, if these are taken up

1. Uppercase panel section headers — one rule, immediate.
2. Search affordance in the topbar, opening the existing palette.
3. Header buttons onto the sprite, so one icon system covers the chrome.
4. Row actions as a convention, then the runs and fleet lists adopt it.
5. Editor-contributed header items, which is a registry change and should
   wait until something concrete needs it.
