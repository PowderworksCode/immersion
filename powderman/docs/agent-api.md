# Driving powderman as an agent

The workbench has one write path — a registry of named commands — and it is
reachable over HTTP, so a coding agent watching this box is a peer of the person
at the keyboard: the same commands, the same undo history, the same live view. A
split an agent POSTs shows up in an open browser within a poll tick, and a person
can undo it.

Three routes, under `/api`:

## `GET /api/commands`

The catalogue. Every command the bus knows, with its description, plus
`undo`/`redo`. This is the same registry the F3 palette lists — a command is
discoverable by being registered.

```json
{ "commands": [
  { "name": "split", "description": "Split an area in two", "navigational": false },
  { "name": "open_run", "description": "Open a run in a new area beside the list", "navigational": false },
  { "name": "workspace.switch", "description": "Show a workspace by index", "navigational": true },
  { "name": "undo", "description": "Revert the last layout change", "navigational": false }
] }
```

## `GET /api/state`

What to read before acting: the live workspace tree (area ids, editors, ratios —
everything a command addresses), the settings document, and the run/fleet
snapshot the UI draws.

```json
{ "workspaces": { "active": 0, "tabs": [ { "name": "Overview", "layout": { "root": { "kind": "split", "id": 4, "dir": "row",
  "a": { "kind": "leaf", "id": 1, "editor": "runs" }, "b": { "kind": "leaf", "id": 5, "editor": "fleet" } } } } ] },
  "settings": { "accent": "#5680c2", "poll_ms": 1000 },
  "state": { "runs": [ ... ], "fleet": [ ... ], "herdr": "herdr 0.8.0" } }
```

## `POST /api/command`

The write. Body is `{ "name": ..., "params": {...} }`; `params` is optional.

```sh
# Split area 1 vertically
curl -X POST localhost:PORT/api/command -H 'content-type: application/json' \
  -d '{"name":"split","params":{"id":1,"dir":"col","frac":0.5}}'

# Open run abc123 in a new area beside the list
curl -X POST localhost:PORT/api/command -H 'content-type: application/json' \
  -d '{"name":"open_run","params":{"area":3,"run":"abc123"}}'

# Undo it
curl -X POST localhost:PORT/api/command -d '{"name":"undo"}'
```

`200` returns `{ "ok": true, "workspaces": {...} }` — the new tree. A bad name or
bad params is `422` with `{ "ok": false, "error": "..." }`, and the live
workbench is left untouched — a command runs against a clone, so a rejected one
never half-applies. `undo`/`redo` are accepted alongside the bus commands;
`params` is ignored for them.

Workflow control (trigger a run, resume a parked one) is separate: see
`POST /trigger/{name}` and `POST /resume/{id}`.
