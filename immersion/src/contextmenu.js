// Generated from immersion/ts/contextmenu.ts — do not edit by hand.
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

  // immersion/ts/contextmenu.ts
  if (once("__imCtxMenu")) {
    let menu = null;
    let onKey = null;
    let lastX = 0;
    let lastY = 0;
    document.addEventListener("pointermove", (e) => {
      lastX = e.clientX;
      lastY = e.clientY;
    }, true);
    let openFor = null;
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
    const commit = (action, params) => send({ action, params: params ?? null });
    let clipStash = null;
    const pick = (action, paramsStr) => {
      const params = JSON.parse(paramsStr || "null");
      if (action === "copy_value") {
        const v = params?.["value"];
        navigator.clipboard.writeText(typeof v === "string" ? v : JSON.stringify(v)).catch(() => {});
        return;
      }
      if (action === "paste_value") {
        if (clipStash == null)
          return;
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
    const openMenu = (items, at) => {
      close();
      if (items.some((it) => it.action === "paste_value")) {
        navigator.clipboard.readText().then((t) => {
          clipStash = t;
        }).catch(() => {});
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
        row.className = "im-ctx-item";
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
          pick(row.dataset.action ?? "", row.dataset.params);
          close();
        });
        row.addEventListener("contextmenu", (ev) => {
          ev.preventDefault();
          ev.stopPropagation();
          commit("favorite_add", {
            label: it.label,
            action: it.action,
            params: it.params ?? null
          });
          close();
        });
        menu.appendChild(row);
      }
      document.body.appendChild(menu);
      const w = menu.offsetWidth;
      const h = menu.offsetHeight;
      let x;
      let y;
      if (at.rect) {
        x = at.rect.left;
        y = at.rect.bottom + 2;
        if (y + h > window.innerHeight - 4)
          y = at.rect.top - h - 2;
      } else {
        x = at.x ?? 0;
        y = at.y ?? 0;
        if (y + h > window.innerHeight - 4)
          y = window.innerHeight - h - 4;
      }
      if (x + w > window.innerWidth - 4)
        x = window.innerWidth - w - 4;
      menu.style.left = Math.max(4, x) + "px";
      menu.style.top = Math.max(4, y) + "px";
      const el = menu;
      const rows = () => Array.from(el.querySelectorAll(".im-ctx-item"));
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
          if (r)
            pick(r.dataset.action ?? "", r.dataset.params);
          close();
        } else if (ev.key === "Escape") {
          ev.preventDefault();
          close();
        }
      };
      document.addEventListener("keydown", onKey, true);
    };
    const parse = (raw) => {
      try {
        const items = JSON.parse(raw ?? "");
        return Array.isArray(items) && items.length > 0 ? items : null;
      } catch (_) {
        return null;
      }
    };
    document.addEventListener("contextmenu", (e) => {
      const host = e.target?.closest?.("[data-im-menu]");
      if (!host)
        return;
      const items = parse(host.dataset.imMenu);
      if (!items)
        return;
      e.preventDefault();
      openMenu(items, { x: e.clientX, y: e.clientY });
    });
    document.addEventListener("click", (e) => {
      const host = e.target?.closest?.("[data-im-menu-click]");
      if (!host)
        return;
      e.preventDefault();
      e.stopPropagation();
      if (openFor === host) {
        close();
        return;
      }
      const items = parse(host.dataset.imMenuClick);
      if (!items)
        return;
      openMenu(items, { rect: host.getBoundingClientRect() });
      openFor = host;
    });
    window.__imOpenMenu = (itemsJson) => {
      const items = parse(itemsJson);
      if (!items)
        return;
      openMenu(items, { x: lastX || 40, y: lastY || 40 });
    };
    const PIE_ANGLES = [180, 0, 90, 270, 135, 45, 225, 315];
    const PIE_R = 92;
    let pie = null;
    const resolveArea = (params, areaId) => {
      if (params == null || typeof params !== "object")
        return params;
      const out = Array.isArray(params) ? [] : {};
      for (const k of Object.keys(params)) {
        const v = params[k];
        out[k] = v === "@area" ? areaId : typeof v === "object" ? resolveArea(v, areaId) : v;
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
    window.__imOpenPie = (itemsJson) => {
      const items = parse(itemsJson);
      if (!items)
        return;
      close();
      closePie();
      const cx = lastX || window.innerWidth / 2;
      const cy = lastY || window.innerHeight / 2;
      const under = document.elementFromPoint(cx, cy);
      const areaEl = under?.closest ? under.closest(".im-area") : null;
      const areaId = areaEl ? Number(areaEl.dataset.imArea) : null;
      const el = document.createElement("div");
      el.className = "im-pie";
      const slices = [];
      items.slice(0, 8).forEach((it, i) => {
        const ang = (PIE_ANGLES[i] ?? 0) * Math.PI / 180;
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
      const track = (e) => {
        if (!pie)
          return;
        let best = null;
        let bestD = Infinity;
        for (const s of pie.slices) {
          const d = Math.hypot(e.clientX - s.x, e.clientY - s.y);
          if (d < bestD) {
            bestD = d;
            best = s;
          }
        }
        for (const s of pie.slices)
          s.el.classList.toggle("is-sel", s === best && bestD < 140);
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
        if (e.target === el)
          closePie();
      });
    };
    document.addEventListener("pointerdown", (e) => {
      const t = e.target;
      if (menu && t && !menu.contains(t) && !t.closest?.("[data-im-menu-click]")) {
        close();
      }
    }, true);
  }
})();
