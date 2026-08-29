// The gesture shim: Blender's drags, client-side until the moment of truth.
//
// Liveview owns all state, but a drag is a 60Hz gesture and a websocket round
// trip per mousemove would feel like dragging through syrup. So the preview is
// local DOM mutation — styles the server is not changing this second — and
// exactly ONE message is sent, on pointerup, carrying the committed mutation.
//
// Ids are read from the DOM at gesture time, never cached: the server
// re-renders areas with fresh ids after every commit.

import { type GestureMsg, once, send } from "./types";

if (once("__imGestures")) {
  const SPLIT_THRESHOLD = 24;

  // One overlay for whatever preview the active gesture needs.
  let overlay: HTMLElement | null = null;
  const showOverlay = (css: string, extra?: string): void => {
    if (!overlay) {
      overlay = document.createElement("div");
      document.body.appendChild(overlay);
    }
    overlay.className = extra ? "im-overlay " + extra : "im-overlay";
    overlay.style.cssText = css;
  };
  const hideOverlay = () => {
    if (overlay) {
      overlay.remove();
      overlay = null;
    }
  };

  // --- seam drag: resize -------------------------------------------------
  // The handle sits over the split's border. Preview adjusts the two sibling
  // cells' flex-basis directly; commit sends the final ratio.

  let seam: any = null; // { el, splitId, dir, splitEl, a, b, rect }

  const onSeamDown = (e: PointerEvent, el: HTMLElement): void => {
    const splitEl = el.closest<HTMLElement>(".im-split");
    if (!splitEl) return;
    const cells = splitEl.querySelectorAll<HTMLElement>(":scope > .im-cell");
    const idx = Number(el.dataset.imSeamIndex ?? 0);
    // The seam moves the pair either side of it; the rest keep their sizes.
    const a = cells[idx];
    const b = cells[idx + 1];
    if (!a || !b) return;
    seam = {
      el,
      splitId: Number(el.dataset.imSeam),
      index: Number(el.dataset.imSeamIndex ?? 0),
      dir: el.dataset.imDir,
      splitEl,
      a,
      b,
      rect: splitEl.getBoundingClientRect(),
      aStart: 0,
      span: 1,
    };
    {
      // Where this pair sits within the split, as fractions of the whole.
      const r = splitEl.getBoundingClientRect();
      const ar = a.getBoundingClientRect();
      const br = b.getBoundingClientRect();
      const horiz = el.dataset.imDir === "row";
      const total = horiz ? r.width : r.height;
      seam.aStart = ((horiz ? ar.left - r.left : ar.top - r.top) || 0) / total;
      seam.span =
        ((horiz ? ar.width + br.width : ar.height + br.height) || 0) / total;
    }
    el.setPointerCapture(e.pointerId);
    e.preventDefault();
  };

  const seamRatio = (e: PointerEvent): number => {
    const r = seam.rect;
    const frac =
      seam.dir === "row"
        ? (e.clientX - r.left) / r.width
        : (e.clientY - r.top) / r.height;
    return Math.min(0.95, Math.max(0.05, frac));
  };

  const onSeamMove = (e: PointerEvent): void => {
    const frac = seamRatio(e);
    // Preview only the pair the seam divides; their combined span is fixed, so
    // the neighbours outside it do not move.
    const aStart = seam.aStart as number;
    const span = seam.span as number;
    const within = Math.min(
      Math.max(frac - aStart, 0.02 * span),
      span - 0.02 * span,
    );
    seam.a.style.flexBasis = within * 100 + "%";
    seam.b.style.flexBasis = (span - within) * 100 + "%";
    if (seam.dir === "row") seam.el.style.left = (aStart + within) * 100 + "%";
    else seam.el.style.top = (aStart + within) * 100 + "%";
  };

  const onSeamUp = (e: PointerEvent): void => {
    send({
      t: "ratio",
      id: seam.splitId,
      index: seam.index,
      ratio: seamRatio(e),
    } satisfies GestureMsg);
    seam = null;
  };

  // --- corner drag: split or join ----------------------------------------

  let grip: any = null; // { areaId, rect, corner, mode, dir, frac, target }

  const areaAt = (x: number, y: number): HTMLElement | null => {
    const el = document.elementFromPoint(x, y);
    return el ? el.closest<HTMLElement>(".im-area") : null;
  };

  const onGripDown = (e: PointerEvent, el: HTMLElement): void => {
    const areaEl = el.closest<HTMLElement>(".im-area");
    if (!areaEl) return;
    grip = {
      areaId: Number(areaEl.dataset.imArea),
      rect: areaEl.getBoundingClientRect(),
      corner: el.dataset.imGrip,
      startX: e.clientX,
      startY: e.clientY,
      mode: null,
      dir: null,
      frac: 0.5,
      target: null,
    };
    el.setPointerCapture(e.pointerId);
    e.preventDefault();
  };

  const onGripMove = (e: PointerEvent): void => {
    const g = grip;
    const r = g.rect;
    const inside =
      e.clientX >= r.left &&
      e.clientX <= r.right &&
      e.clientY >= r.top &&
      e.clientY <= r.bottom;

    if (inside) {
      g.mode = "split";
      g.target = null;
      const dx = e.clientX - g.startX;
      const dy = e.clientY - g.startY;
      if (Math.hypot(dx, dy) < SPLIT_THRESHOLD) {
        hideOverlay();
        g.mode = null;
        return;
      }
      // Dominant axis picks the divider: mostly-horizontal movement draws a
      // vertical seam (a Row split), and vice versa.
      if (Math.abs(dx) >= Math.abs(dy)) {
        g.dir = "row";
        g.frac = (e.clientX - r.left) / r.width;
        showOverlay(
          `left:${e.clientX}px;top:${r.top}px;width:2px;height:${r.height}px;`,
        );
      } else {
        g.dir = "col";
        g.frac = (e.clientY - r.top) / r.height;
        showOverlay(
          `left:${r.left}px;top:${e.clientY}px;width:${r.width}px;height:2px;`,
        );
      }
      g.frac = Math.min(0.95, Math.max(0.05, g.frac));
    } else {
      // Outside the source, over another area: a join — or a SWAP when the
      // command key is held (Ctrl on Linux/Windows, Cmd on macOS), Blender's
      // "trade these two panels" gesture. The mode picks the overlay colour.
      const swap = e.ctrlKey || e.metaKey;
      g.mode = swap ? "swap" : "join";
      const t = areaAt(e.clientX, e.clientY);
      if (t && Number(t.dataset.imArea) !== g.areaId) {
        g.target = Number(t.dataset.imArea);
        const tr = t.getBoundingClientRect();
        // Colour lives in .im-overlay.im-join / .im-swap (theme files); the
        // shim sets only geometry, so no literal leaks into JavaScript.
        showOverlay(
          `left:${tr.left}px;top:${tr.top}px;width:${tr.width}px;height:${tr.height}px;`,
          swap ? "im-swap" : "im-join",
        );
      } else {
        g.target = null;
        hideOverlay();
      }
    }
  };

  const onGripUp = () => {
    const g = grip;
    hideOverlay();
    grip = null;
    if (!g || !g.mode) return;
    if (g.mode === "split") {
      send({
        t: "split",
        id: g.areaId,
        dir: g.dir,
        frac: g.frac,
      } satisfies GestureMsg);
    } else if (g.mode === "join" && g.target !== null) {
      // The server validates siblinghood; an invalid join is silently a
      // no-op there, which the reverted overlay already communicated here.
      send({
        t: "join",
        survivor: g.areaId,
        victim: g.target,
      } satisfies GestureMsg);
    } else if (g.mode === "swap" && g.target !== null) {
      // Swap needs no siblinghood — any two areas can trade editors.
      send({ t: "swap", a: g.areaId, b: g.target } satisfies GestureMsg);
    }
  };

  // --- region resize: drag a toolbar/sidebar edge --------------------------
  // The toolbar grows to the right, the sidebar to the left. Preview the strip
  // width locally; commit one set_region_width on release.

  let region: any = null; // { strip, areaId, which, startW, startX }

  const onRegionDown = (e: PointerEvent, el: HTMLElement): void => {
    const areaEl = el.closest<HTMLElement>(".im-area");
    const strip = el.closest<HTMLElement>(".im-toolbar, .im-sidebar");
    if (!areaEl || !strip) return;
    region = {
      strip,
      areaId: Number(areaEl.dataset.imArea),
      which: el.dataset.imRegionHandle,
      startW: strip.getBoundingClientRect().width,
      startX: e.clientX,
    };
    el.setPointerCapture(e.pointerId);
    e.preventDefault();
  };

  const regionWidth = (e: PointerEvent): number => {
    const dx = e.clientX - region.startX;
    const w =
      region.which === "toolbar" ? region.startW + dx : region.startW - dx;
    return Math.min(500, Math.max(32, w));
  };

  const onRegionMove = (e: PointerEvent): void => {
    region.strip.style.width = regionWidth(e) + "px";
  };

  const onRegionUp = (e: PointerEvent): void => {
    send({
      t: "regionwidth",
      id: region.areaId,
      region: region.which,
      w: Math.round(regionWidth(e)),
    } satisfies GestureMsg);
    region = null;
  };

  // --- wiring, delegated so re-rendered areas need no re-binding ----------

  document.addEventListener("pointerdown", (e: PointerEvent) => {
    const seamEl = (e.target as Element | null)?.closest?.<HTMLElement>(
      ".im-seam-handle",
    );
    if (seamEl) return onSeamDown(e, seamEl);
    const gripEl = (e.target as Element | null)?.closest?.<HTMLElement>(
      ".im-grip",
    );
    if (gripEl) return onGripDown(e, gripEl);
    const regionEl = (e.target as Element | null)?.closest?.<HTMLElement>(
      ".im-region-handle",
    );
    if (regionEl) return onRegionDown(e, regionEl);
  });
  document.addEventListener("pointermove", (e: PointerEvent) => {
    if (seam) onSeamMove(e);
    else if (grip) onGripMove(e);
    else if (region) onRegionMove(e);
  });
  document.addEventListener("pointerup", (e: PointerEvent) => {
    if (seam) onSeamUp(e);
    else if (grip) onGripUp();
    else if (region) onRegionUp(e);
  });
}
