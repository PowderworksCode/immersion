# Keymap web-safety

Immersion runs in a browser, so its keymap has a constraint Blender's does not:
some chords are already spoken for by the browser or the OS, and binding over
one gives a shortcut that looks broken — it does nothing (the browser ate the
key first) or fires two things at once. The worst case is invisible on Linux,
where the project's CI runs: a chord that resolves fine here can be dead on a
Mac, and the only symptom is a shortcut that does nothing.

The rule is **never bind over a native browser or OS shortcut.** The defaults in
`immersion/src/keymap.rs` are web-first: Blender's chords where they are free,
remapped where they collide.

## Kept (free in the browser, every platform)

| Action | Chord | Notes |
| --- | --- | --- |
| Undo / Redo | `Mod+Z` / `Mod+Shift+Z` | Free; the universal pair. `Mod` is Cmd on macOS, Ctrl elsewhere. |
| Command palette | `F3` | Blender's Menu Search; unclaimed. The shim `preventDefault`s it so find-again does not also fire. |

## Remapped (the browser or OS owns Blender's chord)

| Action | Blender | Collides with | Immersion uses |
| --- | --- | --- | --- |
| Maximize area | `Ctrl+Space` | macOS Input Sources; and `Cmd+Space` (the `Mod` resolution) is Spotlight | `Mod+Shift+Space` |
| Workspace cycle | `Ctrl+PageUp/PageDown` | Browser tab switching (all platforms) | `Alt+PageUp/PageDown` |

## The Mac trap specifically

Two things kill a modifier chord on a Mac while it works on Linux:

1. **`Mod` resolves to Cmd**, so any chord the OS reserves under Cmd (Spotlight
   is `Cmd+Space`) is gone. This is why maximize could not stay on `Mod+Space`.
2. **Platform detection.** `navigator.platform` is deprecated and can be empty;
   when it is, `isMac` reads false, `Mod` resolves to Ctrl, and *every* Cmd
   chord misses. The shim (`keymap.js`) checks `navigator.userAgent` as well.

## Never bind (reserved)

`Mod+F` (find), `Mod+L` (address bar), `Mod+R` (reload), `Mod+T/W/N` (tabs),
`Mod+PageUp/PageDown` (tab switch); on macOS `Ctrl+Space`, `Ctrl+Alt+Space`,
`Cmd+Space`, `Cmd+Q/H/M`; on Windows/Linux `Alt+F`, `F5`, `Alt+F4`.
