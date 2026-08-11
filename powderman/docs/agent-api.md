# Driving powderman as an agent

The workbench has one write path — a registry of named commands — and an agent
is a peer of the person at the keyboard: the same commands, one undo history,
one live view. A layout change an agent makes shows up in an open browser within
a poll tick, and a person can undo it.

There are two faces on that one write path. **MCP is the one to use** — the
commands are native, typed tools. The HTTP/JSON routes underneath are kept for
curl and debugging.

## MCP (the interface)

An MCP server is mounted in-process on the daemon's own router, streamable-HTTP
transport, at **`/mcp`**. It shares the daemon's state, so an MCP tool call and a
click land on the same `dispatch_checked` with the same undo stack — there is no
second copy of the workbench behind the protocol.

Each layout command is a tool with a schema derived from its argument struct, so
a connected agent discovers them in `tools/list` and calls them the way it calls
any tool — no endpoints or param shapes to learn out of band:

| Tool | Args |
|---|---|
| `split` | `id`, `dir` (`row`\|`col`), `frac?` |
| `join` | `id` |
| `join_into` | `survivor`, `victim` |
| `ratio` | `id`, `ratio` |
| `set_editor` | `id`, `editor` |
| `open_editor` | `id`, `editor`, `arg` |
| `open_run` | `area`, `run` |
| `workspace_switch` | `index` |
| `workspace_cycle` | `delta?` |
| `workspace_rename` | `index`, `name` |
| `workspace_close` | `index` |
| `undo` / `redo` | — |
| `get_state` | — (the workspace tree, settings, run/fleet snapshot) |

`dir` is an enum, so a bad value is refused at the boundary
(`unknown variant 'diagonal', expected 'row' or 'col'`) rather than failing
deeper. A command that a valid-shaped call still can't apply comes back as a
tool error the agent reads, and never half-mutates the live tree.

Point an MCP client at it — for Claude Code:

```sh
claude mcp add --transport http powderman http://localhost:PORT/mcp
```

Note: the transport's default `allowed_hosts` is loopback-only (a DNS-rebinding
guard). Reaching `/mcp` through a non-loopback hostname (a proxy, a public
domain) needs that host added to the transport config.

## HTTP/JSON (curl & debugging)

The same operations as plain routes, handy from a shell.

- `GET /api/commands` — the command catalogue (name, description).
- `GET /api/state` — `{ workspaces, settings, state }`.
- `POST /api/command` — `{ "name": ..., "params": {...} }`; `200` with the new
  tree, or `422 { "ok": false, "error": ... }` on a bad name/params.

```sh
curl -X POST localhost:PORT/api/command -H 'content-type: application/json' \
  -d '{"name":"split","params":{"id":1,"dir":"col","frac":0.5}}'
curl -X POST localhost:PORT/api/command -d '{"name":"undo"}'
```

Workflow control (trigger a run, resume a parked one) is separate:
`POST /trigger/{name}` and `POST /resume/{id}`.
