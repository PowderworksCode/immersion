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

// Blender lets you type arithmetic into a number field: `3*2`, `1920/2`,
// `pi*2`. Evaluate it here, on commit, and hand the widget a plain number —
// the server never sees the expression, and an unparseable one is left alone
// for the field's own validation to reject.
const CONSTS: Record<string, number> = { pi: Math.PI, e: Math.E, tau: Math.PI * 2 };

export const evalExpr = (raw: string): number | null => {
  const src = raw.trim().toLowerCase();
  if (src === "") return null;
  // A plain number is the common case and needs no parsing.
  const plain = Number(src);
  if (Number.isFinite(plain)) return plain;
  // Only digits, the four operators, parens, dots and known constant names —
  // anything else is not arithmetic and must not be evaluated.
  // Word boundaries, or the `e` in `1e3` is read as Euler's number and
  // `2*1e3` quietly evaluates to 25.4 instead of 2000. Numeric literals —
  // exponent and all — collapse to 0 before the check, so an exponent is
  // arithmetic while a stray letter still is not.
  const named = src
    .replace(/\b(pi|tau|e)\b/g, "0")
    .replace(/\d+(\.\d*)?(e[-+]?\d+)?/g, "0");
  if (!/^[-+*/()0.\s]+$/.test(named)) return null;
  const expr = src.replace(/\b(pi|tau|e)\b/g, (m) => String(CONSTS[m] ?? 0));
  try {
    // eslint-disable-next-line no-new-func -- the input is restricted to
    // arithmetic by the test above; this evaluates numbers, not code.
    const v = Function(`"use strict";return (${expr})`)() as unknown;
    return typeof v === "number" && Number.isFinite(v) ? v : null;
  } catch {
    return null;
  }
};

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
    const input = (e.target as Element | null)?.closest?.<HTMLInputElement>("[data-im-scrub]");
    if (!input) return;
    s = {
      input,
      startX: e.clientX,
      startVal: num(input.value, 0),
      step: num(input.dataset.scrubStep, 1),
      min: input.dataset.scrubMin ? num(input.dataset.scrubMin, -Infinity) : -Infinity,
      max: input.dataset.scrubMax ? num(input.dataset.scrubMax, Infinity) : Infinity,
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

  // Typing commits through `change`; evaluate an expression before the widget
  // reads the value, so `3*2` reaches the document as 6.
  document.addEventListener(
    "change",
    (e) => {
      const input = e.target as HTMLInputElement | null;
      if (!input?.dataset || input.dataset.imScrub === undefined) return;
      const v = evalExpr(input.value);
      if (v !== null && String(v) !== input.value.trim()) input.value = String(v);
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
