# powderman reference

Generated from the registries themselves — the command bus, the MCP
router, the keymap and the editor registry. Nothing here is written
prose, so nothing here can be out of date.

Regenerate with `UPDATE_DOCS=1 cargo test -p powderman`; a stale file
fails the test suite.

## Commands

The write path. Every layout change — a button, a chord, a gesture, an agent — arrives as one of these.

### `duplicate_area`

Split an area and show the same editor in the new half

### `join`

Close an area; its sibling takes the space

### `join_into`

Merge one area into a sibling

### `open_editor`

Point an area at a specific thing (editor + argument)

### `open_run`

Open a run in a new area beside the list

### `ratio`

Move a seam between two areas

### `select`

Point every unpinned area of a kind (file, run, chart) at one thing

### `set_editor`

Change what an area shows

### `set_pinned`

Freeze an area on what it is showing, or let it follow the selection

### `set_region_width`

Resize an area's toolbar or sidebar

### `set_target`

Point an area at something without changing its editor

### `split`

Split an area in two

### `swap`

Swap what two areas show

### `toggle_region`

Show or hide an area's toolbar or sidebar

### `workspace.add`

Add a workspace from a layout

### `workspace.close`

Close a workspace

### `workspace.cycle`

Show the next or previous workspace

### `workspace.duplicate`

Duplicate a workspace

### `workspace.move`

Move a workspace tab to another position

### `workspace.rename`

Rename a workspace

### `workspace.switch`

Show a workspace by index


## MCP tools

What an agent can do, which is everything a person can do to server truth. A command without a tool here is a test failure, not an omission.

### `duplicate_area`

Split an area and show the same editor in the new half

| parameter | type | required | what it is |
|---|---|---|---|
| `id` | integer | yes | The area id. |

### `favorite_add`

Add an entry to the Quick Favourites menu (the Q menu). Deduped by label; the list is capped at 12.

| parameter | type | required | what it is |
|---|---|---|---|
| `action` | string | yes | The command the favourite runs when picked. |
| `label` | string | yes | The label shown in the Quick Favourites menu. |
| `params` | any | no | Parameters for that command, if any. |

### `get_run`

Read one run in full: every step with its result and error. Use after get_state to drill into a run by id.

| parameter | type | required | what it is |
|---|---|---|---|
| `id` | string | yes | The run id, as listed by `get_state`. |

### `get_state`

Read the live workbench: the workspace tree (area ids, editors, ratios), settings, current box metrics, the fleet, and a run summary. Runs carry a step COUNT only — call get_run for one run's step detail.

### `join`

Close an area; its sibling takes the space

| parameter | type | required | what it is |
|---|---|---|---|
| `id` | integer | yes | The area id. |

### `join_into`

Merge one area into a sibling

| parameter | type | required | what it is |
|---|---|---|---|
| `survivor` | integer | yes | The area that stays and takes the space. |
| `victim` | integer | yes | The area that is closed. |

### `open_editor`

Point an area at a specific thing (editor + argument)

| parameter | type | required | what it is |
|---|---|---|---|
| `arg` | string | yes | The editor's argument (e.g. a run id for the `run` editor). |
| `editor` | string | yes | The editor id to show. |
| `id` | integer | yes | The area to repoint. |

### `open_run`

Open a run in a new area beside the list

| parameter | type | required | what it is |
|---|---|---|---|
| `area` | integer | yes | The area to split; the run opens in the new half beside it. |
| `run` | string | yes | The run id to open. |

### `ratio`

Move the seam between two areas

| parameter | type | required | what it is |
|---|---|---|---|
| `id` | integer | yes | The split (or either of its children) whose seam moves. |
| `ratio` | number | yes | The first child's fraction of the space, 0..1. |

### `redo`

Reapply an undone change

### `repeat_last`

Re-run the most recent layout-changing command (Blender's Repeat Last). Navigation and failed commands are skipped.

### `select`

Point every unpinned area of a kind at one thing — the file browser's click, as a command. Selecting a file moves both a code viewer and a diff viewer, since both point at a path. Pinned areas are left alone.

| parameter | type | required | what it is |
|---|---|---|---|
| `kind` | string | yes | What sort of thing is being selected: `file`, `folder`, `run`,
`chart` or `data`. Areas showing that sort follow it. |
| `value` | string | yes | The thing itself — a path for `file`, a run id for `run`, a chart
pointer for `chart`. |
| `mode` | string | no | `replace` (the default) selects only this; `extend` adds it to the
selection and makes it active; `toggle` removes it if it was already
selected. Many things can be selected; the last one is active, and
that is what unpinned areas follow. |

### `set_editor`

Change what an area shows

| parameter | type | required | what it is |
|---|---|---|---|
| `editor` | string | yes | The editor id to show (e.g. `runs`, `fleet`, `settings`). |
| `id` | integer | yes | The area to repoint. |

### `set_pinned`

Freeze an area on what it is showing so the selection stops moving it, or let it follow again. Blender's pin.

| parameter | type | required | what it is |
|---|---|---|---|
| `id` | integer | yes | The area id. |
| `pinned` | boolean | yes | True freezes it on what it is showing; false lets it follow again. |

### `set_region_width`

Resize an area's toolbar or sidebar

| parameter | type | required | what it is |
|---|---|---|---|
| `id` | integer | yes | The area whose region is resized. |
| `region` | string | yes | `toolbar` or `sidebar`. |
| `w` | integer | yes | The new width in pixels. |

### `set_setting`

Write one value into the settings document by JSON pointer — theme, ui_scale, keymap overrides, favourites. The same operation the Settings editor performs.

| parameter | type | required | what it is |
|---|---|---|---|
| `pointer` | string | yes | JSON pointer into the settings document (e.g. `/theme`, `/ui_scale`,
`/keymap/undo`). `get_state` shows the whole document. |
| `value` | any | yes | The value to write at that pointer. |

### `set_target`

Point an area at something without changing its editor: a JSON pointer, a path, or a run id. Empty clears it.

| parameter | type | required | what it is |
|---|---|---|---|
| `id` | integer | yes | The area to retarget. |
| `target` | string | yes | What it should look at — a JSON pointer into the workbench documents
(`/settings/favorites`), a path for a file browser, or a run id. The
empty string clears the target. |

### `split`

Split an area in two

| parameter | type | required | what it is |
|---|---|---|---|
| `dir` | row \| col | yes | Split direction: `row` puts the new area beside, `col` below. |
| `id` | integer | yes | The area (leaf) id to split. Read ids from `get_state`. |
| `frac` | number | no | The first child's fraction of the space, 0..1. Defaults to 0.5. |

### `swap`

Swap what two areas show

| parameter | type | required | what it is |
|---|---|---|---|
| `a` | integer | yes | One area id. |
| `b` | integer | yes | The other; the two exchange what they show. |

### `toggle_region`

Show or hide an area's toolbar, sidebar, or header

| parameter | type | required | what it is |
|---|---|---|---|
| `id` | integer | yes | The area whose region is toggled. |
| `region` | string | yes | `toolbar`, `sidebar`, `header`, or `header_flip`. |

### `undo`

Revert the last layout change

### `undo_to`

Step back several layout changes at once, to a point in the undo history. depth is how many steps to take; every one lands on the redo stack. Read the names with get_state's log.

| parameter | type | required | what it is |
|---|---|---|---|
| `depth` | integer | yes | How many undo steps to take. Past the end of the stack unwinds as far
as there is history rather than failing. |

### `workspace_add`

Add a workspace tab

| parameter | type | required | what it is |
|---|---|---|---|
| `name` | string | yes | The tab name. |
| `layout` | any | no | The starting layout tree. Omit for a single-area default; the shape is
the `layout` value `get_state` returns. |

### `workspace_close`

Close a workspace

| parameter | type | required | what it is |
|---|---|---|---|
| `index` | integer | yes | The workspace tab index (0-based). |

### `workspace_cycle`

Show the next or previous workspace

| parameter | type | required | what it is |
|---|---|---|---|
| `delta` | integer | no | +1 for the next workspace, -1 for the previous. Defaults to +1. |

### `workspace_duplicate`

Duplicate a workspace tab

| parameter | type | required | what it is |
|---|---|---|---|
| `index` | integer | yes | The workspace tab index (0-based). |

### `workspace_move`

Move a workspace tab to another position

| parameter | type | required | what it is |
|---|---|---|---|
| `from` | integer | yes | The tab's current position (0-based). |
| `to` | integer | yes | Where it should end up. |

### `workspace_rename`

Rename a workspace

| parameter | type | required | what it is |
|---|---|---|---|
| `index` | integer | yes | The workspace tab index (0-based). |
| `name` | string | yes | The new name. |

### `workspace_switch`

Show a workspace by index

| parameter | type | required | what it is |
|---|---|---|---|
| `index` | integer | yes | The workspace tab index (0-based). |


## Keys

The default bindings. Every one names an action from the lists above, or a per-client view action; Preferences rebinds them without touching this.

### `F9`

adjust_last

### `F1`

cheatsheet

### `Q`

favorites

### `Mod+Shift+F`

fullscreen

### `Mod+Shift+Space`

maximize

### `F3`

palette

### ```

pie

### `Mod+Shift+Z`

redo

### `Shift+R`

repeat_last

### `N`

toggle_sidebar

### `T`

toggle_toolbar

### `Mod+Z`

undo

### `Alt+PageDown`

workspace.cycle

### `Alt+PageUp`

workspace.cycle


## Editors

What an area can show. The hints are what each answers to, and they are what the status bar shows while it has focus.

### `Welcome`

`welcome`

- **Click** — Become that editor

### `Machine`

`machine`

### `Fleet`

`fleet`

### `Runs`

`runs`

- **Click** — Open run
- **Type** — Filter

### `Actions`

`actions`

- **Click** — Trigger

### `Timers`

`timers`

### `Run detail`

`run` — takes a target

- **Chip** — Pick run

### `Settings`

`settings`

- **Drag** — Scrub number
- **Type** — 3*2 works

### `Info log`

`info`

- **Type** — Filter

### `Help`

`help`

- **Type** — Filter the reference

### `Keymap`

`keymap`

- **Set** — Rebind
- **Type** — Filter

### `Data`

`data` — takes a target

- **Click** — Expand
- **Right-click** — Copy data path

### `Files`

`files` — takes a target

- **Click** — Expand
- **Chip** — Root here

### `Code`

`code` — takes a target

- **Chip** — Pick file

### `Diff`

`diff` — takes a target

- **Chip** — Pick changed file

### `Chart`

`chart` — takes a target

- **Chip** — Pick chart
- **N** — Edit spec

