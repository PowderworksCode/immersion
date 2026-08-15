// Generated from immersion/ts/gestures.ts — do not edit by hand.
// Run `bun run build` after changing the TypeScript source.
(() => {
  // immersion/ts/types.ts
  function send(msg) {
    try {
      dioxus.send(typeof msg === "string" ? msg : JSON.stringify(msg));
    } catch {}
  }
  function once(flag) {
    const w = window;
    if (w[flag])
      return false;
    w[flag] = true;
    return true;
  }

  // immersion/ts/gestures.ts
  if (once("__imGestures")) {
    const SPLIT_THRESHOLD = 24;
    let overlay = null;
    const showOverlay = (css, extra) => {
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
    let seam = null;
    const onSeamDown = (e, el) => {
      const splitEl = el.closest(".im-split");
      if (!splitEl)
        return;
      const cells = splitEl.querySelectorAll(":scope > .im-cell");
      const idx = Number(el.dataset.imSeamIndex ?? 0);
      const a = cells[idx];
      const b = cells[idx + 1];
      if (!a || !b)
        return;
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
        span: 1
      };
      {
        const r = splitEl.getBoundingClientRect();
        const ar = a.getBoundingClientRect();
        const br = b.getBoundingClientRect();
        const horiz = el.dataset.imDir === "row";
        const total = horiz ? r.width : r.height;
        seam.aStart = ((horiz ? ar.left - r.left : ar.top - r.top) || 0) / total;
        seam.span = ((horiz ? ar.width + br.width : ar.height + br.height) || 0) / total;
      }
      el.setPointerCapture(e.pointerId);
      e.preventDefault();
    };
    const seamRatio = (e) => {
      const r = seam.rect;
      const frac = seam.dir === "row" ? (e.clientX - r.left) / r.width : (e.clientY - r.top) / r.height;
      return Math.min(0.95, Math.max(0.05, frac));
    };
    const onSeamMove = (e) => {
      const frac = seamRatio(e);
      const aStart = seam.aStart;
      const span = seam.span;
      const within = Math.min(Math.max(frac - aStart, 0.02 * span), span - 0.02 * span);
      seam.a.style.flexBasis = within * 100 + "%";
      seam.b.style.flexBasis = (span - within) * 100 + "%";
      if (seam.dir === "row")
        seam.el.style.left = (aStart + within) * 100 + "%";
      else
        seam.el.style.top = (aStart + within) * 100 + "%";
    };
    const onSeamUp = (e) => {
      send({ t: "ratio", id: seam.splitId, index: seam.index, ratio: seamRatio(e) });
      seam = null;
    };
    let grip = null;
    const areaAt = (x, y) => {
      const el = document.elementFromPoint(x, y);
      return el ? el.closest(".im-area") : null;
    };
    const onGripDown = (e, el) => {
      const areaEl = el.closest(".im-area");
      if (!areaEl)
        return;
      grip = {
        areaId: Number(areaEl.dataset.imArea),
        rect: areaEl.getBoundingClientRect(),
        corner: el.dataset.imGrip,
        startX: e.clientX,
        startY: e.clientY,
        mode: null,
        dir: null,
        frac: 0.5,
        target: null
      };
      el.setPointerCapture(e.pointerId);
      e.preventDefault();
    };
    const onGripMove = (e) => {
      const g = grip;
      const r = g.rect;
      const inside = e.clientX >= r.left && e.clientX <= r.right && e.clientY >= r.top && e.clientY <= r.bottom;
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
        if (Math.abs(dx) >= Math.abs(dy)) {
          g.dir = "row";
          g.frac = (e.clientX - r.left) / r.width;
          showOverlay(`left:${e.clientX}px;top:${r.top}px;width:2px;height:${r.height}px;`);
        } else {
          g.dir = "col";
          g.frac = (e.clientY - r.top) / r.height;
          showOverlay(`left:${r.left}px;top:${e.clientY}px;width:${r.width}px;height:2px;`);
        }
        g.frac = Math.min(0.95, Math.max(0.05, g.frac));
      } else {
        const swap = e.ctrlKey || e.metaKey;
        g.mode = swap ? "swap" : "join";
        const t = areaAt(e.clientX, e.clientY);
        if (t && Number(t.dataset.imArea) !== g.areaId) {
          g.target = Number(t.dataset.imArea);
          const tr = t.getBoundingClientRect();
          showOverlay(`left:${tr.left}px;top:${tr.top}px;width:${tr.width}px;height:${tr.height}px;`, swap ? "im-swap" : "im-join");
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
      if (!g || !g.mode)
        return;
      if (g.mode === "split") {
        send({ t: "split", id: g.areaId, dir: g.dir, frac: g.frac });
      } else if (g.mode === "join" && g.target !== null) {
        send({ t: "join", survivor: g.areaId, victim: g.target });
      } else if (g.mode === "swap" && g.target !== null) {
        send({ t: "swap", a: g.areaId, b: g.target });
      }
    };
    let region = null;
    const onRegionDown = (e, el) => {
      const areaEl = el.closest(".im-area");
      const strip = el.closest(".im-toolbar, .im-sidebar");
      if (!areaEl || !strip)
        return;
      region = {
        strip,
        areaId: Number(areaEl.dataset.imArea),
        which: el.dataset.imRegionHandle,
        startW: strip.getBoundingClientRect().width,
        startX: e.clientX
      };
      el.setPointerCapture(e.pointerId);
      e.preventDefault();
    };
    const regionWidth = (e) => {
      const dx = e.clientX - region.startX;
      const w = region.which === "toolbar" ? region.startW + dx : region.startW - dx;
      return Math.min(500, Math.max(32, w));
    };
    const onRegionMove = (e) => {
      region.strip.style.width = regionWidth(e) + "px";
    };
    const onRegionUp = (e) => {
      send({
        t: "regionwidth",
        id: region.areaId,
        region: region.which,
        w: Math.round(regionWidth(e))
      });
      region = null;
    };
    document.addEventListener("pointerdown", (e) => {
      const seamEl = e.target?.closest?.(".im-seam-handle");
      if (seamEl)
        return onSeamDown(e, seamEl);
      const gripEl = e.target?.closest?.(".im-grip");
      if (gripEl)
        return onGripDown(e, gripEl);
      const regionEl = e.target?.closest?.(".im-region-handle");
      if (regionEl)
        return onRegionDown(e, regionEl);
    });
    document.addEventListener("pointermove", (e) => {
      if (seam)
        onSeamMove(e);
      else if (grip)
        onGripMove(e);
      else if (region)
        onRegionMove(e);
    });
    document.addEventListener("pointerup", (e) => {
      if (seam)
        onSeamUp(e);
      else if (grip)
        onGripUp();
      else if (region)
        onRegionUp(e);
    });
  }
})();
