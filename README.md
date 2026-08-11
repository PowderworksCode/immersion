# immersion

Blender-style areas for [Dioxus](https://dioxuslabs.com) liveview: a tiling tree
of editors you split, join, and rearrange in place, with the whole layout as
server-side state.

The screen is a binary tree of **areas**, each showing one **editor**. You split
an area to get another view, join to give the space back, and switch what an
area shows from its header dropdown — no tabs, no floating panels. The layout is
a value, every mutation is a named **command**, and rendering is a component over
it, so persistence, undo, and (later) agent control are properties of the value
rather than machinery bolted beside it.

Built as the interface for [powderman](https://github.com/PowderworksCode/powderman),
its first host, which lives in `powderman/` here until the library stands alone.

## Layout

- `immersion/` — the library. The area tree, command bus, workspaces, gesture
  shim, keymap, splash, and the flat Blender chrome. Knows nothing of a host's
  domain.
- `powderman/` — the host. Editors (machine, fleet, runs, timers, run-detail),
  persistence, and the daemon.

## Lineage

A reimplementation of [the React + dockview Immersion](https://github.com/zmaril/immersion)
on liveview, keeping its locked decisions — one editor per area, flat gap-free
seams, close-is-join, invisible corner grips — and its interface-map triage of
Blender 2.93, but trading the browser-owned dock and its MCP relay for a
server-authoritative tree where agent parity is one route, not a subsystem.
