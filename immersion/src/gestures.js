// The gesture shim: Blender's drags, client-side until the moment of truth.
//
// Liveview owns all state, but a drag is a 60Hz gesture and a websocket
// round trip per mousemove would feel like dragging through syrup. So the
// preview is local DOM mutation — styles the server is not changing this
// second, which liveview's diff therefore never patches over — and exactly
// ONE message is sent, on pointerup, carrying the committed mutation. The
// server validates and re-renders; the preview is then simply confirmed.
//
// The state machine is ported from the React Immersion's design notes:
// pointer capture on grip-down; dominant axis (|dx| >= |dy|) picks the
// divider; a 24px threshold before a split commits; inside the source rect
// means split, outside over a neighbour means join; the drop position sets
// the new ratio ("drop-ratio split").
//
// Ids are read from the DOM at gesture time, never cached — the server
// re-renders areas with fresh ids after every commit, and a cached id would
// go stale in the hand that holds it.

(() => {
  if (window.__imGestures) return;
  window.__imGestures = true;

  const SPLIT_THRESHOLD = 24;

  const send = (msg) => {
    try { dioxus.send(JSON.stringify(msg)); } catch (e) { /* channel gone; a reload re-installs */ }
  };

  // One overlay for whatever preview the active gesture needs.
  let overlay = null;
  const showOverlay = (css, extra) => {
    if (!overlay) {
      overlay = document.createElement("div");
      document.body.appendChild(overlay);
    }
    overlay.className = extra ? "im-overlay " + extra : "im-overlay";
    overlay.style.cssText = css;
  };
  const hideOverlay = () => { if (overlay) { overlay.remove(); overlay = null; } };

  // --- seam drag: resize -------------------------------------------------
  // The handle sits over the split's border. Preview adjusts the two sibling
  // cells' flex-basis directly; commit sends the final ratio.

  let seam = null; // { el, splitId, dir, splitEl, a, b, rect }

  const onSeamDown = (e, el) => {
    const splitEl = el.closest(".im-split");
    if (!splitEl) return;
    const cells = splitEl.querySelectorAll(":scope > .im-cell");
    if (cells.length !== 2) return;
    seam = {
      el,
      splitId: Number(el.dataset.imSeam),
      dir: el.dataset.imDir,
      splitEl,
      a: cells[0],
      b: cells[1],
      rect: splitEl.getBoundingClientRect(),
    };
    el.setPointerCapture(e.pointerId);
    e.preventDefault();
  };

  const seamRatio = (e) => {
    const r = seam.rect;
    const frac = seam.dir === "row"
      ? (e.clientX - r.left) / r.width
      : (e.clientY - r.top) / r.height;
    return Math.min(0.95, Math.max(0.05, frac));
  };

  const onSeamMove = (e) => {
    const frac = seamRatio(e);
    seam.a.style.flexBasis = (frac * 100) + "%";
    seam.b.style.flexBasis = ((1 - frac) * 100) + "%";
    if (seam.dir === "row") seam.el.style.left = (frac * 100) + "%";
    else seam.el.style.top = (frac * 100) + "%";
  };

  const onSeamUp = (e) => {
    send({ t: "ratio", id: seam.splitId, ratio: seamRatio(e) });
    seam = null;
  };

  // --- corner drag: split or join ----------------------------------------

  let grip = null; // { areaId, rect, corner, mode, dir, frac, target }

  const areaAt = (x, y) => {
    const el = document.elementFromPoint(x, y);
    return el ? el.closest(".im-area") : null;
  };

  const onGripDown = (e, el) => {
    const areaEl = el.closest(".im-area");
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

  const onGripMove = (e) => {
    const g = grip;
    const r = g.rect;
    const inside =
      e.clientX >= r.left && e.clientX <= r.right &&
      e.clientY >= r.top && e.clientY <= r.bottom;

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
          `left:${e.clientX}px;top:${r.top}px;width:2px;height:${r.height}px;`
        );
      } else {
        g.dir = "col";
        g.frac = (e.clientY - r.top) / r.height;
        showOverlay(
          `left:${r.left}px;top:${e.clientY}px;width:${r.width}px;height:2px;`
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
          swap ? "im-swap" : "im-join"
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
      send({ t: "split", id: g.areaId, dir: g.dir, frac: g.frac });
    } else if (g.mode === "join" && g.target !== null) {
      // The server validates siblinghood; an invalid join is silently a
      // no-op there, which the reverted overlay already communicated here.
      send({ t: "join", survivor: g.areaId, victim: g.target });
    } else if (g.mode === "swap" && g.target !== null) {
      // Swap needs no siblinghood — any two areas can trade editors.
      send({ t: "swap", a: g.areaId, b: g.target });
    }
  };

  // --- region resize: drag a toolbar/sidebar edge --------------------------
  // The toolbar grows to the right, the sidebar to the left. Preview the strip
  // width locally; commit one set_region_width on release.

  let region = null; // { strip, areaId, which, startW, startX }

  const onRegionDown = (e, el) => {
    const areaEl = el.closest(".im-area");
    const strip = el.closest(".im-toolbar, .im-sidebar");
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
      w: Math.round(regionWidth(e)),
    });
    region = null;
  };

  // --- wiring, delegated so re-rendered areas need no re-binding ----------

  document.addEventListener("pointerdown", (e) => {
    const seamEl = e.target.closest?.(".im-seam-handle");
    if (seamEl) return onSeamDown(e, seamEl);
    const gripEl = e.target.closest?.(".im-grip");
    if (gripEl) return onGripDown(e, gripEl);
    const regionEl = e.target.closest?.(".im-region-handle");
    if (regionEl) return onRegionDown(e, regionEl);
  });
  document.addEventListener("pointermove", (e) => {
    if (seam) onSeamMove(e);
    else if (grip) onGripMove(e);
    else if (region) onRegionMove(e);
  });
  document.addEventListener("pointerup", (e) => {
    if (seam) onSeamUp(e);
    else if (grip) onGripUp(e);
    else if (region) onRegionUp(e);
  });
})();
