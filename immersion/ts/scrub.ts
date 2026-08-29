// The number-scrub shim: drag a number field horizontally to change it.
//
// Blender's number drag, on the library's budget. Pointer-drag is continuous,
// so the scrub previews locally — the shim writes the input's value as you drag
// — and commits exactly once, on release, by dispatching the native `change`
// event the widget's own onchange already listens for. No new message type: a
// scrub reuses the same commit path a typed value takes.
//
// Press-and-drag past a threshold scrubs (blurring the field so no caret
// shows); press-and-release without moving falls through to a normal click,
// focusing the field to type. Ctrl/Cmd snaps to the step; Shift scrubs fine.

import { once } from "./types";

// Expression entry (`3*2`, `pi*100`) is NOT here: it runs once, on commit, on
// a message the server already receives, so it lives in Rust (`eval_number`).
// Only frame-path work — the drag preview below — has to be in the browser.

interface Scrub {
  input: HTMLInputElement;
  startX: number;
  startVal: number;
  step: number;
  min: number;
  max: number;
  pid: number;
  scrubbing: boolean;
}

if (once("__imScrub")) {
  const THRESHOLD = 3;
  let s: Scrub | null = null;

  const num = (v: string | undefined, d: number): number => {
    const n = parseFloat(v ?? "");
    return Number.isFinite(n) ? n : d;
  };

  document.addEventListener("pointerdown", (e) => {
    const input = (e.target as Element | null)?.closest?.<HTMLInputElement>(
      "[data-im-scrub]",
    );
    if (!input) return;
    s = {
      input,
      startX: e.clientX,
      startVal: num(input.value, 0),
      step: num(input.dataset.scrubStep, 1),
      min: input.dataset.scrubMin
        ? num(input.dataset.scrubMin, -Infinity)
        : -Infinity,
      max: input.dataset.scrubMax
        ? num(input.dataset.scrubMax, Infinity)
        : Infinity,
      pid: e.pointerId,
      scrubbing: false,
    };
  });

  document.addEventListener(
    "pointermove",
    (e) => {
      if (!s) return;
      const dx = e.clientX - s.startX;
      if (!s.scrubbing) {
        if (Math.abs(dx) < THRESHOLD) return;
        s.scrubbing = true;
        s.input.blur();
        try {
          s.input.setPointerCapture(s.pid);
        } catch {
          /* capture is best-effort */
        }
        document.body.style.cursor = "ew-resize";
      }
      e.preventDefault();
      const speed = e.shiftKey ? 0.05 : 0.25;
      let v = s.startVal + dx * s.step * speed;
      if (e.ctrlKey || e.metaKey) v = Math.round(v / s.step) * s.step;
      v = Math.min(s.max, Math.max(s.min, v));
      const dec = (String(s.step).split(".")[1] ?? "").length;
      s.input.value = v.toFixed(dec);
    },
    true,
  );

  document.addEventListener("pointerup", () => {
    if (!s) return;
    if (s.scrubbing) {
      document.body.style.cursor = "";
      // Commit through the widget's own onchange.
      s.input.dispatchEvent(new Event("change", { bubbles: true }));
    } else {
      // A plain click: focus for typing.
      s.input.focus();
      s.input.select?.();
    }
    s = null;
  });
}
