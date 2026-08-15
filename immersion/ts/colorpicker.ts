// The colour picker popup — Blender's SV square and hue bar, client-side.
//
// The swatch button opens a popup holding a saturation/value square and a hue
// strip. Dragging either previews locally and commits exactly ONCE on release,
// by writing the hex into the field's text input and dispatching the `change`
// event the Rust widget already listens for. The drag never touches the wire.

import { once } from "./types";

if (once("__imColor")) {

  const clamp01 = (v: number): number => Math.min(1, Math.max(0, v));

  const hsv2rgb = (h: number, s: number, v: number): [number, number, number] => {
    const i = Math.floor(h * 6);
    const f = h * 6 - i;
    const p = v * (1 - s);
    const q = v * (1 - f * s);
    const t = v * (1 - (1 - f) * s);
    // The index is i % 6 into a six-element table, so a lookup cannot miss;
    // spelled out this way the compiler can see it too.
    const k = i % 6;
    const rs = [v, q, p, p, t, v] as const;
    const gs = [t, v, v, q, p, p] as const;
    const bs = [p, p, t, v, v, q] as const;
    return [
      Math.round((rs[k] ?? 0) * 255),
      Math.round((gs[k] ?? 0) * 255),
      Math.round((bs[k] ?? 0) * 255),
    ];
  };

  const rgb2hsv = (r: number, g: number, b: number): [number, number, number] => {
    r /= 255; g /= 255; b /= 255;
    const mx = Math.max(r, g, b);
    const mn = Math.min(r, g, b);
    const d = mx - mn;
    let h = 0;
    if (d) {
      if (mx === r) h = ((g - b) / d) % 6;
      else if (mx === g) h = (b - r) / d + 2;
      else h = (r - g) / d + 4;
      h /= 6;
      if (h < 0) h += 1;
    }
    return [h, mx ? d / mx : 0, mx];
  };

  const hex2rgb = (hex: string): [number, number, number] => {
    const m = /^#?([0-9a-f]{6})$/i.exec((hex || "").trim());
    if (!m) return [0, 0, 0];
    const n = parseInt(m[1] ?? "0", 16);
    return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
  };

  const rgb2hex = (r: number, g: number, b: number): string =>
    "#" + [r, g, b].map((x) => x.toString(16).padStart(2, "0")).join("");

  let pop: { el: HTMLElement; swatch: HTMLElement } | null = null;

  const closePop = () => {
    if (pop) {
      pop.el.remove();
      pop = null;
    }
  };

  document.addEventListener("click", (e) => {
    const swatch = (e.target as Element | null)?.closest?.<HTMLElement>("[data-im-color-open]");
    if (!swatch) {
      if (pop && !(e.target as Element | null)?.closest?.(".im-colorpop")) closePop();
      return;
    }
    e.preventDefault();
    if (pop && pop.swatch === swatch) {
      closePop();
      return;
    }
    closePop();

    const field = swatch.parentElement?.querySelector<HTMLInputElement>("[data-im-color-value]");
    if (!field) return;
    let [h, s, v] = rgb2hsv(...hex2rgb(field.value));

    const el = document.createElement("div");
    el.className = "im-colorpop";
    el.innerHTML =
      '<div class="im-sv"><span class="im-sv-dot"></span></div>' +
      '<div class="im-hue"><span class="im-hue-dot"></span></div>';
    document.body.appendChild(el);
    const r = swatch.getBoundingClientRect();
    el.style.left = Math.min(r.left, window.innerWidth - el.offsetWidth - 6) + "px";
    el.style.top =
      (r.bottom + el.offsetHeight > window.innerHeight - 6 ? r.top - el.offsetHeight - 4 : r.bottom + 4) + "px";

    const sv = el.querySelector<HTMLElement>(".im-sv")!;
    const svDot = el.querySelector<HTMLElement>(".im-sv-dot")!;
    const hue = el.querySelector<HTMLElement>(".im-hue")!;
    const hueDot = el.querySelector<HTMLElement>(".im-hue-dot")!;

    const paint = (): string => {
      const [r8, g8, b8] = hsv2rgb(h, s, v);
      const hex = rgb2hex(r8, g8, b8);
      // Only the hue travels from JS; the black/white ramps of an HSV square
      // are intrinsic to the control and live in the stylesheet.
      sv.style.setProperty("--im-sv-hue", rgb2hex(...hsv2rgb(h, 1, 1)));
      svDot.style.left = s * 100 + "%";
      svDot.style.top = (1 - v) * 100 + "%";
      hueDot.style.top = h * 100 + "%";
      swatch.style.background = hex;
      field.value = hex;
      return hex;
    };
    paint();

    // Commit once: write the hex and let the widget's own onchange fire.
    const commit = () => field.dispatchEvent(new Event("change", { bubbles: true }));

    const dragSv = (ev: PointerEvent): void => {
      const b = sv.getBoundingClientRect();
      s = clamp01((ev.clientX - b.left) / b.width);
      v = 1 - clamp01((ev.clientY - b.top) / b.height);
      paint();
    };
    const dragHue = (ev: PointerEvent): void => {
      const b = hue.getBoundingClientRect();
      h = clamp01((ev.clientY - b.top) / b.height);
      paint();
    };
    const wire = (target: HTMLElement, move: (e: PointerEvent) => void): void => {
      target.addEventListener("pointerdown", (ev: PointerEvent) => {
        ev.preventDefault();
        target.setPointerCapture(ev.pointerId);
        move(ev);
        const mv = (e2: PointerEvent) => move(e2);
        const up = () => {
          target.removeEventListener("pointermove", mv);
          target.removeEventListener("pointerup", up);
          commit();
        };
        target.addEventListener("pointermove", mv);
        target.addEventListener("pointerup", up);
      });
    };
    wire(sv, dragSv);
    wire(hue, dragHue);

    pop = { el, swatch };
  });

  document.addEventListener("keydown", (e: KeyboardEvent) => {
    if (e.key === "Escape" && pop) closePop();
  });
}
