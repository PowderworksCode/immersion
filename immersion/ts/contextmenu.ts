// The menu shim: Blender's right-click menus, its click-open dropdowns, and
// its pie menus, client-side.
//
// An element carries its menu as a JSON array of MenuItem in one of two
// attributes: data-im-menu opens on right-click at the cursor, and
// data-im-menu-click opens on left-click anchored under the element. A pie is
// raised by the host through __imOpenPie. Either way the shim draws and drives
// the menu, and commits ONE message — the picked {action, params}.

import { once, send, type MenuItem, type MenuPick } from "./types";

if (once("__imCtxMenu")) {

  let menu: HTMLElement | null = null;
  let onKey: ((e: KeyboardEvent) => void) | null = null;
  // Where the pointer last was — Blender's Q opens the favourites at the mouse.
  let lastX = 0;
  let lastY = 0;
  document.addEventListener("pointermove", (e) => { lastX = e.clientX; lastY = e.clientY; }, true);
  let openFor: HTMLElement | null = null; // the element whose dropdown is open, for toggle-off

  const close = () => {
    if (menu) {
      menu.remove();
      menu = null;
    }
    if (onKey) {
      document.removeEventListener("keydown", onKey, true);
      onKey = null;
    }
    openFor = null;
  };

  const commit = (action: string, params: unknown): void =>
    send({ action, params: params ?? null } satisfies MenuPick);

  // Copy / Paste value are clipboard operations — handled here, not sent to the
  // server (which has no clipboard). dioxus.send only works from a synchronous
  // handler, not a detached promise, so paste cannot read-then-send inside the
  // click; instead the clipboard is prefetched when the menu opens and the pick
  // sends the stashed value synchronously.
  let clipStash: string | null = null;
  const pick = (action: string, paramsStr: string | undefined): void => {
    const params = JSON.parse(paramsStr || "null") as Record<string, unknown> | null;
    if (action === "copy_value") {
      const v = params?.["value"];
      navigator.clipboard
        .writeText(typeof v === "string" ? v : JSON.stringify(v))
        .catch(() => {});
      return;
    }
    if (action === "paste_value") {
      if (clipStash == null) return;
      let v;
      try {
        v = JSON.parse(clipStash);
      } catch (_) {
        v = clipStash;
      }
      commit("set_setting", { pointer: params?.["pointer"], value: v });
      return;
    }
    commit(action, params);
  };

  // Build and place the menu. `at` is either {x, y} (cursor) or {rect} (anchor
  // the menu under an element's box, the dropdown case).
  const openMenu = (items: MenuItem[], at: { x?: number; y?: number; rect?: DOMRect }): void => {
    close();
    if (items.some((it) => it.action === "paste_value")) {
      navigator.clipboard.readText().then((t) => { clipStash = t; }).catch(() => {});
    }

    menu = document.createElement("div");
    menu.className = "im-ctxmenu";
    for (const it of items) {
      if (it.sep) {
        const s = document.createElement("div");
        s.className = "im-ctx-sep";
        menu.appendChild(s);
        continue;
      }
      const row = document.createElement("div");
      // A row whose command cannot run right now is dim and inert, not
      // missing: a row that disappears teaches nothing, and one that is there
      // but grey says "this exists, not now".
      row.className = it.disabled ? "im-ctx-item is-disabled" : "im-ctx-item";
      if (it.icon) {
        // The icon is the library's own sprite output — markup by
        // construction, never user text — so it is inserted as markup while
        // the label stays textContent.
        const glyph = document.createElement("span");
        glyph.className = "im-ctx-icon";
        glyph.innerHTML = it.icon;
        row.appendChild(glyph);
      }
      const label = document.createElement("span");
      label.textContent = it.label ?? "";
      row.appendChild(label);
      if (it.chord) {
        const c = document.createElement("span");
        c.className = "im-ctx-chord";
        c.textContent = it.chord;
        row.appendChild(c);
      }
      row.dataset.action = it.action ?? "";
      row.dataset.params = JSON.stringify(it.params ?? null);
      row.addEventListener("click", () => {
        if (it.disabled) return;
        pick(row.dataset.action ?? "", row.dataset.params);
        close();
      });
      // Blender's "Add to Quick Favourites": right-click a menu row.
      row.addEventListener("contextmenu", (ev) => {
        ev.preventDefault();
        ev.stopPropagation();
        commit("favorite_add", {
          label: it.label,
          action: it.action,
          params: it.params ?? null,
        });
        close();
      });
      menu.appendChild(row);
    }
    document.body.appendChild(menu);

    const w = menu.offsetWidth;
    const h = menu.offsetHeight;
    let x: number;
    let y: number;
    if (at.rect) {
      x = at.rect.left;
      y = at.rect.bottom + 2; // hang under the button
      if (y + h > window.innerHeight - 4) y = at.rect.top - h - 2; // flip above
    } else {
      x = at.x ?? 0;
      y = at.y ?? 0;
      if (y + h > window.innerHeight - 4) y = window.innerHeight - h - 4;
    }
    if (x + w > window.innerWidth - 4) x = window.innerWidth - w - 4;
    menu.style.left = Math.max(4, x) + "px";
    menu.style.top = Math.max(4, y) + "px";

    const el = menu;
    // Disabled rows are excluded here rather than only from the click
    // handler: this list is what the arrows walk, what Enter fires and what
    // the letter accelerators match, so leaving them in would let the
    // keyboard run a command the pointer refuses.
    const rows = (): HTMLElement[] =>
      Array.from(el.querySelectorAll<HTMLElement>(".im-ctx-item:not(.is-disabled)"));
    let sel = 0;
    const paint = () => rows().forEach((r, i) => r.classList.toggle("is-sel", i === sel));
    paint();

    onKey = (ev) => {
      const rs = rows();
      if (ev.key === "ArrowDown") {
        ev.preventDefault();
        sel = (sel + 1) % rs.length;
        paint();
      } else if (ev.key === "ArrowUp") {
        ev.preventDefault();
        sel = (sel - 1 + rs.length) % rs.length;
        paint();
      } else if (ev.key === "Enter") {
        ev.preventDefault();
        const r = rs[sel];
        if (r) pick(r.dataset.action ?? "", r.dataset.params);
        close();
      } else if (ev.key === "Escape") {
        ev.preventDefault();
        close();
      } else if (ev.key.length === 1 && /[a-z0-9]/i.test(ev.key)) {
        // Accelerators: a letter jumps to the first item starting with it, and
        // picks straight away when only one matches — the way a desktop menu
        // has always worked. Repeating the letter cycles the matches.
        const want = ev.key.toLowerCase();
        const hits = rs.filter((r) => (r.textContent ?? "").trim().toLowerCase().startsWith(want));
        if (hits.length === 0) return;
        ev.preventDefault();
        if (hits.length === 1) {
          const only = hits[0];
          if (only) pick(only.dataset.action ?? "", only.dataset.params);
          close();
          return;
        }
        const cur = rs[sel];
        const from = cur && hits.includes(cur) ? hits.indexOf(cur) + 1 : 0;
        const next = hits[from % hits.length];
        if (next) sel = rs.indexOf(next);
        paint();
      }
    };
    document.addEventListener("keydown", onKey, true);
  };

  const parse = (raw: string | undefined): MenuItem[] | null => {
    try {
      const items = JSON.parse(raw ?? "") as MenuItem[];
      return Array.isArray(items) && items.length > 0 ? items : null;
    } catch (_) {
      return null;
    }
  };

  // Right-click: a context menu at the cursor.
  document.addEventListener("contextmenu", (e) => {
    const host = (e.target as Element | null)?.closest?.<HTMLElement>("[data-im-menu]");
    if (!host) return;
    const items = parse(host.dataset.imMenu);
    if (!items) return;
    e.preventDefault();
    openMenu(items, { x: e.clientX, y: e.clientY });
  });

  // Left-click: a dropdown anchored under the button. Clicking the open
  // button again closes it, the way a menu bar behaves.
  document.addEventListener("click", (e) => {
    const host = (e.target as Element | null)?.closest?.<HTMLElement>("[data-im-menu-click]");
    if (!host) return;
    e.preventDefault();
    e.stopPropagation();
    if (openFor === host) {
      close();
      return;
    }
    const items = parse(host.dataset.imMenuClick);
    if (!items) return;
    openMenu(items, { rect: host.getBoundingClientRect() });
    openFor = host;
  });

  // A menu the host raises from a keypress rather than a click — Quick
  // Favourites (Q). The shim's own channel carries the pick, so the host only
  // has to hand over the items.
  window.__imOpenMenu = (itemsJson: string): void => {
    const items = parse(itemsJson);
    if (!items) return;
    openMenu(items, { x: lastX || 40, y: lastY || 40 });
  };


  // --- pie menu -------------------------------------------------------------
  // Blender's radial menu: slices arranged around the pointer, the one nearest
  // the cursor highlighted, picked by click (or by releasing a drag out onto
  // it). Up to eight slices, ordered the way Blender lays them out — west,
  // east, south, north, then the diagonals — so muscle memory has somewhere to
  // form. Params carrying the string "@area" are resolved to the id of the area
  // under the pointer, so one pie definition works for whichever area you are
  // over.
  const PIE_ANGLES = [180, 0, 90, 270, 135, 45, 225, 315];
  const PIE_R = 92;
  let pie: { el: HTMLElement; slices: { el: HTMLElement; x: number; y: number }[]; cx: number; cy: number } | null = null;

  const resolveArea = (params: unknown, areaId: number | null): unknown => {
    if (params == null || typeof params !== "object") return params;
    const out: Record<string, unknown> = Array.isArray(params) ? ([] as never) : {};
    for (const k of Object.keys(params as Record<string, unknown>)) {
      const v = (params as Record<string, unknown>)[k];
      out[k] = v === "@area" ? areaId : (typeof v === "object" ? resolveArea(v, areaId) : v);
    }
    return out;
  };

  const closePie = () => {
    if (pie) {
      pie.el.remove();
      pie = null;
    }
    if (onKey) {
      document.removeEventListener("keydown", onKey, true);
      onKey = null;
    }
  };

  window.__imOpenPie = (itemsJson: string): void => {
    const items = parse(itemsJson);
    if (!items) return;
    close();
    closePie();
    const cx = lastX || window.innerWidth / 2;
    const cy = lastY || window.innerHeight / 2;
    const under = document.elementFromPoint(cx, cy);
    const areaEl = under?.closest ? under.closest<HTMLElement>(".im-area") : null;
    const areaId = areaEl ? Number((areaEl as HTMLElement).dataset.imArea) : null;

    const el = document.createElement("div");
    el.className = "im-pie";
    const slices: { el: HTMLElement; x: number; y: number }[] = [];
    items.slice(0, 8).forEach((it, i) => {
      const ang = ((PIE_ANGLES[i] ?? 0) * Math.PI) / 180;
      const x = cx + Math.cos(ang) * PIE_R;
      const y = cy + Math.sin(ang) * PIE_R;
      const s = document.createElement("div");
      s.className = "im-pie-slice";
      s.textContent = it.label ?? "";
      s.style.left = x + "px";
      s.style.top = y + "px";
      s.addEventListener("click", () => {
        pick(it.action ?? "", JSON.stringify(resolveArea(it.params ?? null, areaId)));
        closePie();
      });
      el.appendChild(s);
      slices.push({ el: s, x, y });
    });
    const dot = document.createElement("div");
    dot.className = "im-pie-center";
    dot.style.left = cx + "px";
    dot.style.top = cy + "px";
    el.appendChild(dot);
    document.body.appendChild(el);
    pie = { el, slices, cx, cy };

    // Highlight whichever slice the pointer is nearest.
    const track = (e: PointerEvent): void => {
      if (!pie) return;
      let best: { el: HTMLElement; x: number; y: number } | null = null;
      let bestD = Infinity;
      for (const s of pie.slices) {
        const d = Math.hypot(e.clientX - s.x, e.clientY - s.y);
        if (d < bestD) {
          bestD = d;
          best = s;
        }
      }
      for (const s of pie.slices) s.el.classList.toggle("is-sel", s === best && bestD < 140);
    };
    el.addEventListener("pointermove", track);
    document.addEventListener("pointermove", track, true);

    onKey = (ev) => {
      if (ev.key === "Escape") {
        ev.preventDefault();
        closePie();
      }
    };
    document.addEventListener("keydown", onKey, true);
    el.addEventListener("pointerdown", (e) => {
      // A press on the backdrop (not a slice) dismisses.
      if (e.target === el) closePie();
    });
  };

  // Dismiss on any outside press.
  document.addEventListener(
    "pointerdown",
    (e) => {
      const t = e.target as Element | null;
      if (menu && t && !menu.contains(t) && !t.closest?.("[data-im-menu-click]")) {
        close();
      }
    },
    true,
  );
}
