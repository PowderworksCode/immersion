# Driving powderman as an agent

The workbench has one write path — a registry of named commands — and an agent
is a peer of the person at the keyboard: the same commands, one undo history,
one live view. A layout change an agent makes shows up in an open browser within
a poll tick, and a person can undo it.

That write path is exposed as an **MCP server** — the commands are native, typed
tools, not an HTTP API an agent must be taught.

## The MCP server

An rmcp streamable-HTTP server is mounted in-process on the daemon's own axum
router at **`/mcp`**. It shares the daemon's state, so an MCP tool call and a
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

Read `get_state` for area ids before acting; the ids are what every command
addresses.

## Connecting

Point an MCP client at the server — for Claude Code:

```sh
claude mcp add --transport http powderman http://HOST:PORT/mcp
```

### The Host allowlist

rmcp's streamable-HTTP transport carries a DNS-rebinding guard that, by default,
allows only loopback Host headers (`localhost`, `127.0.0.1`, `::1`). powderman is
typically reached through a proxy under its own hostname, where every request
would then be `Forbidden: Host header is not allowed`. Set:

```sh
POWDERMAN_MCP_ALLOWED_HOSTS=powderworks-dev.exe.xyz   # adds to the loopback set
```

A host given without a port matches any port, so the above covers `:7778`.

The server runs in **stateless JSON mode** (each request a self-contained POST,
answered with one JSON body — no session id, no held-open SSE stream), which is
what survives a reverse proxy. A plain browser `GET /mcp` therefore returns
`Not Acceptable: Client must accept text/event-stream` — that is expected; an
MCP client POSTs.
Multiple hosts are comma-separated. The value `*` disables the check entirely
(any Host allowed) — reasonable when a front proxy already authenticates, as the
exe.dev proxy does.

Workflow control (trigger a run, resume a parked one) is separate from the
workbench, over plain routes: `POST /trigger/{name}` and `POST /resume/{id}`.
