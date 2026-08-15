# A Blender-like interface on top of serde

This is the design the code has been converging on. It is written down here
because the next round of features is much cheaper if it is stated plainly
first — and because the two obvious generalisations (a document registry, and
schemas as data) are worth doing deliberately rather than by accident.

## What is already true

Four things in the workbench are, today, a serde value that only ever changes
through a named command, and that fact is what makes everything else work:

| Value | Shape | Edited by | Persisted as |
| --- | --- | --- | --- |
| The layout | `Workspaces` → `Layout` → an `Area` tree | `split`, `join`, `swap`, `ratio`, `set_editor`, `toggle_region`, `set_region_width`, … | one JSON row |
| Settings | a free `serde_json::Value` | `set_setting(pointer, value)` | one JSON row |
| The keymap | defaults + an overrides map | `set_setting("/keymap/<action>", chord)` | inside settings |
| Favourites | a list of `(label, action, params)` | `favorite_add` | inside settings |

Everything the workbench can do falls out of that shape, and none of it was
built separately:

- **Undo** is a snapshot of one value. There is no diff engine because there is
  nothing to diff — the layout *is* the document.
- **Persistence** is `serde_json::to_string`. A deploy does not lose the
  workbench because the workbench was never in memory in the first place.
- **Export / import** is the same serialisation with a file dialog attached.
- **Agent parity** is not a feature; the MCP tools are the command bus, and the
  bus takes `(name, params: Value)`. An agent and a keypress reach the same
  function with the same argument type.
- **The widget kit** edits a document by JSON pointer and reports
  `(pointer, value)`. It does not know which document it is editing.
- **Previews** work because a Fly machine with no herdr still has all four
  values; only the runtime is missing.

So the workbench is already a set of serde documents with Blender-shaped
editors over them. The rest of this note is about admitting that.

## The pattern, stated once

> A **document** is a serde value with a **schema**. It is read whole, edited
> by **pointer**, mutated only through **commands**, and rendered by **editors**
> that know the schema but not the document.

Three of the four documents above have an implicit schema — `settings_fields()`
is one, written by hand in the host; the layout's is the `Area` enum; the
keymap's is `Vec<Binding>`. Only the settings schema is data today, and only
the settings document gets the widget kit for free.

## The generalisation

Give the workbench a **document registry**, and let everything that is a value
join it:

```rust
/// A named serde document the workbench can show, edit, undo and export.
pub struct Doc {
    pub id: &'static str,          // "layout", "settings", "keymap", "run:0f6a"
    pub title: &'static str,
    pub schema: Vec<Field>,        // the widget kit's schema — may be empty
    pub value: Value,
    pub revision: u64,             // what the deck memoises on
}

pub trait Docs {
    fn list(&self) -> Vec<DocInfo>;
    fn get(&self, id: &str) -> Option<&Doc>;
    fn set(&mut self, id: &str, pointer: &str, value: Value) -> Result<()>;
    fn reset(&mut self, id: &str, pointer: &str) -> Result<()>;
}
```

Two commands then cover every document that is not the layout:

```
doc.set    { doc, pointer, value }
doc.reset  { doc, pointer }
```

and the things that are currently special cases stop being special:

- `set_setting` becomes `doc.set { doc: "settings", … }`.
- A keymap rebind becomes `doc.set { doc: "keymap", pointer: "/favorites", … }`
  instead of reaching into the settings document by string.
- Undo covers preferences, not just layout, because the undo stack records
  `(doc id, before)` rather than a `Workspaces`.
- The MCP server gains `list_docs`, `get_doc`, `set_doc` — and an agent can read
  and edit **any** of them without a new tool per document. Right now an agent
  can reshape the layout but cannot change a preference except through a tool
  written by hand.
- The property editor becomes a **generic editor kind**: point an area at a
  document id and it renders that document's schema. "Settings" stops being a
  bespoke editor and becomes `props(doc = "settings")`, the way Blender's
  Properties editor is one editor showing whatever is selected.

## Schemas as data

`Field` is already the right shape (`path`, `label`, `kind`, `hint`, `default`).
Two changes make it carry its weight:

1. **Serialise it.** A schema that is a serde value can come from the host at
   runtime — a workflow's parameters, a run's inputs — instead of being compiled
   in. `FieldKind` is a closed enum, so this is a `Serialize`/`Deserialize`
   derive and a version tag, not a redesign.
2. **Derive it.** For host types that already have serde derives, a
   `#[derive(Schema)]` that emits `Vec<Field>` removes the hand-written
   `settings_fields()` and the drift that comes with it. This is the same
   information `schemars` gives the MCP tools; the two should come from one
   place.

Once a schema is data and a document is data, three features stop being
features:

- **Adjust Last (F9)** already builds a form from a command's params by guessing
  types. With param schemas it renders the real form, with ranges and labels.
- **The command palette** can prompt for arguments the same way, which is the
  one thing it cannot do today.
- **Presets** are a named `Value` at a pointer — "save these params", "apply
  them" — with no new machinery at all.

## What this does not solve

- **The layout is not a `Field` document.** An area tree is not a property form,
  and pretending otherwise would make both worse. It stays its own document with
  its own commands and its own editor (the deck). The registry holds it so that
  undo, export and the agent see one list, not so that it renders as fields.
- **Live external state is not a document.** Runs, the fleet, the machine
  metrics are a *snapshot*, not something you edit by pointer. They stay a
  read-only projection; conflating them with documents would invite an editor
  that writes into a value the next poll overwrites.
- **Multi-client editing is still last-writer-wins.** One process owns the
  documents, which is why this is coherent at all, but two people editing the
  same pointer is a race today and stays one. Fixing it means revisions per
  pointer, and nothing here needs that yet.

## Migration, in the order it should happen

Each step is useful alone, and nothing below requires the step after it.

1. **`Doc` + registry in the host, with the four existing documents.** No UI
   change; `set_setting` delegates. Proves the shape.
2. **`doc.set` / `doc.reset` commands**, with `set_setting` kept as an alias.
   Undo starts covering settings.
3. **A generic `props` editor kind** taking a document id. The Settings editor
   becomes one instance of it; the keymap editor stays bespoke (it is a table,
   not a form).
4. **MCP `list_docs` / `get_doc` / `set_doc`.** An agent can now change a
   preference, which today it cannot.
5. **`Serialize` on `Field`/`FieldKind`**, and a host-supplied schema for one
   real thing — a workflow's input parameters is the obvious first.
6. **Param schemas on `Command`**, feeding Adjust Last and a palette that can
   ask for arguments.
7. **`#[derive(Schema)]`**, last, once there are enough hand-written schemas to
   make the derive obviously right.

## The shape of the thing

Blender is a set of editors over one document (the .blend). This workbench is a
set of editors over several — layout, preferences, keymap, and whatever the host
brings. The interface is Blender's because that is the best answer anyone has
for "many editors, one screen, all keyboard-reachable". The substrate is serde
because a value that can be serialised can be persisted, undone, exported, sent
to an agent, and rendered by a widget kit that never learns what it is editing.

Neither half is the interesting part. The interesting part is that every feature
above — undo, previews, MCP, export, presets — is a consequence of holding both
at once.
