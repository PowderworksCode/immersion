// The context-menu shim: Blender's right-click menu, client-side.
//
// A right-clickable control carries its menu as a `data-im-menu` JSON array of
// {label, action, params} (or {sep:true}). On contextmenu the shim reads it,
// draws a floating menu at the cursor (flipped to stay on-screen), drives it by
// arrow/enter/escape and click, and commits ONE message — the picked
// {action, params} — over the eval channel, which the host routes through the
// same command bus a button uses. Opening and moving the menu never touch the
// wire; only the pick does.

(() => {
  if (window.__imCtxMenu) return;
  window.__imCtxMenu = true;

  let menu = null;
  let onKey = null;

  const close = () => {
    if (menu) {
      menu.remove();
      menu = null;
    }
    if (onKey) {
      document.removeEventListener("keydown", onKey, true);
      onKey = null;
    }
  };

  const send = (action, params) => {
    try {
      dioxus.send(JSON.stringify({ action, params: params ?? null }));
    } catch (err) {
      /* channel gone; a reload re-installs */
    }
  };

  document.addEventListener("contextmenu", (e) => {
    const host = e.target.closest?.("[data-im-menu]");
    if (!host) return;
    let items;
    try {
      items = JSON.parse(host.dataset.imMenu);
    } catch (_) {
      return;
    }
    if (!Array.isArray(items) || items.length === 0) return;
    e.preventDefault();
    close();

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
      row.textContent = it.label;
      row.dataset.action = it.action;
      row.dataset.params = JSON.stringify(it.params ?? null);
      row.addEventListener("click", () => {
        send(row.dataset.action, JSON.parse(row.dataset.params));
        close();
      });
      menu.appendChild(row);
    }
    document.body.appendChild(menu);

    const w = menu.offsetWidth;
    const h = menu.offsetHeight;
    let x = e.clientX;
    let y = e.clientY;
    if (x + w > window.innerWidth - 4) x = window.innerWidth - w - 4;
    if (y + h > window.innerHeight - 4) y = window.innerHeight - h - 4;
    menu.style.left = Math.max(4, x) + "px";
    menu.style.top = Math.max(4, y) + "px";

    const rows = () => Array.from(menu.querySelectorAll(".im-ctx-item"));
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
        if (r) send(r.dataset.action, JSON.parse(r.dataset.params));
        close();
      } else if (ev.key === "Escape") {
        ev.preventDefault();
        close();
      }
    };
    document.addEventListener("keydown", onKey, true);
  });

  // Dismiss on any outside press.
  document.addEventListener(
    "pointerdown",
    (e) => {
      if (menu && !menu.contains(e.target)) close();
    },
    true,
  );
})();
