# Spike: does a stack of webviews behave like a workbench?

**Compiles and its tests pass; never run.** There is no display on the machine
this was written on, and `build_as_child` is macOS / Windows / X11 only anyway.
What follows is a thing to run on your Mac, built so that one command answers
the questions instead of you having to design the test.

## Run it

```sh
cd spike/webview-stack
cargo run
```

It opens one window with two child webviews stacked in it. The lower one loads
`github.com`, which sets `X-Frame-Options: DENY` — chosen on purpose, because
if it renders then this is definitively not an iframe. The upper one is
transparent and holds the chrome.

Then, **with the pointer inside the page**:

| | |
|---|---|
| `F3` | a modal over the web pane |
| right-click | a context menu at the pointer |
| click | proves the page is still reachable |
| `p` | keep the overlay up and empty — the passthrough test |
| `Escape` | dismiss |

## What it answers on its own

Printed as it happens, and again when you close the window:

```
  PASS  a page that refuses to be framed rendered, and our script ran inside it
  PASS  a chord pressed inside that page reached the host
  PASS  a right-click inside that page reached the host
  PASS  a click reached the page with the overlay idle
```

## What you have to look at

**Did the magenta frame and the menu draw above the page, and could you read
the page through the dimmed backdrop?**

That is z-order and transparency, and it is the pair that decides the whole
architecture. No program can check it for you. macOS is
[reported to handle transparent webview overlays well](https://github.com/tauri-apps/wry/issues/1266);
Windows is where wry's transparency bugs live
([#1540](https://github.com/tauri-apps/wry/issues/1540)), so if Windows matters,
run it there second and expect worse.

If the overlay renders as an opaque sheet, the next things to try are marking
the window itself transparent, and building the overlay before the content
rather than after.

## The question behind the `p` key

The overlay is **hidden between uses** rather than made click-through, because
hiding needs no platform code and click-through needs an `NSView` `hitTest:`
override on macOS and a window style on Windows.

That is fine for menus and modals, which are transient. It is not fine for
chrome that has to stay live — a tooltip that follows the pointer, a drag
preview. `p` holds the overlay up and empty so you can find out whether clicks
reach the page through a transparent webview anyway. If they do, persistent
chrome is free. If they do not, it costs interop, and it is better to know that
now than after building on it.

## What this does not test

- Positioning several content webviews to match area rects, and keeping them
  there through a corner-drag. The workbench already commits layout changes on
  release rather than per frame, so the expected answer is "fine", but it is
  untested here.
- Whether the chrome itself should be a webview (Dioxus, unchanged) or native
  (GPUI, rewritten). This spike is deliberately agnostic — it proves the
  substrate, not the choice.
- Anything about Servo. That is the other branch, and it only becomes worth
  costing if the answer here is that overlays are unacceptable.

## What is verified

`cargo check --all-targets` and `cargo clippy` are clean against wry 0.56 and
winit 0.30 on Linux, and both tests pass — they pin the seams a spike like this
rots at: the JSON kinds the scripts emit against the enum that receives them,
and the `imOverlay` functions the host calls by name against the ones the
overlay defines. Both injected scripts parse under `bun build`.

What remains unverified is everything that needs a screen: whether the overlay
is transparent, whether it stacks above, whether clicks pass through it, and
whether the injected shim actually takes the keys back from a live page.
