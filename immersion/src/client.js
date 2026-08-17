// Generated from immersion/ts/client.ts — do not edit by hand.
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

  // immersion/ts/client.ts
  if (once("__imClient")) {
    document.addEventListener("input", (e) => {
      const input = e.target;
      if (!input.classList || !input.classList.contains("im-slider-input"))
        return;
      const min = parseFloat(input.min) || 0;
      const max = parseFloat(input.max);
      const val = parseFloat(input.value);
      if (!isFinite(max) || max <= min)
        return;
      const bar = input.closest(".im-slider");
      if (!bar)
        return;
      const pct = Math.min(100, Math.max(0, (val - min) / (max - min) * 100));
      bar.style.setProperty("--im-fill", pct + "%");
      const label = bar.querySelector(".im-slider-val");
      if (label)
        label.textContent = input.value;
    }, true);
    document.addEventListener("click", (e) => {
      const target = e.target;
      const btn = target?.closest?.("[data-im-copy]");
      if (!btn)
        return;
      const text = btn.getAttribute("data-im-copy") ?? "";
      if (!text)
        return;
      e.preventDefault();
      e.stopPropagation();
      navigator.clipboard?.writeText(text).then(() => {
        btn.classList.add("is-copied");
        setTimeout(() => btn.classList.remove("is-copied"), 1200);
      }).catch(() => {});
    }, true);
    const subseq = (q, text) => {
      if (!q)
        return true;
      let i = 0;
      for (const ch of text) {
        if (ch === q[i])
          i++;
        if (i === q.length)
          return true;
      }
      return false;
    };
    document.addEventListener("keydown", (e) => {
      const t = e.target;
      if (t?.dataset && t.dataset.imFilter !== undefined) {
        e.stopPropagation();
      }
    }, true);
    document.addEventListener("input", (e) => {
      const box = e.target;
      if (!box.dataset || box.dataset.imFilter === undefined)
        return;
      const scope = box.closest(".im-filter-scope") || document;
      const q = box.value.toLowerCase().trim();
      for (const row of scope.querySelectorAll("[data-filter-text]")) {
        row.style.display = subseq(q, (row.dataset.filterText || "").toLowerCase()) ? "" : "none";
      }
    });
    const DELAY = 250;
    const RESHOW = 250;
    let tip = null;
    let timer = null;
    let active = null;
    let stashedTitle = null;
    let lastHidden = 0;
    const enabled = () => window.__imTooltipsEnabled !== false;
    const tipEl = () => {
      if (!tip) {
        tip = document.createElement("div");
        tip.className = "im-tooltip";
        tip.style.display = "none";
        document.body.appendChild(tip);
      }
      return tip;
    };
    const owner = (node) => {
      let n = node;
      while (n && n !== document.body) {
        if (n.nodeType === 1 && (n.hasAttribute("data-tip") || n.hasAttribute("title")))
          return n;
        n = n.parentElement;
      }
      return null;
    };
    const content = (node) => {
      const label = node.getAttribute("data-tip") || node.getAttribute("title");
      if (!label)
        return null;
      return { label, key: node.getAttribute("data-tip-key"), desc: node.getAttribute("data-tip-desc") };
    };
    const render = (c) => {
      const t = tipEl();
      t.textContent = "";
      const name = document.createElement("div");
      name.className = "im-tooltip-name";
      name.textContent = c.label;
      t.appendChild(name);
      if (c.desc) {
        const d = document.createElement("div");
        d.className = "im-tooltip-desc";
        d.textContent = c.desc;
        t.appendChild(d);
      }
      if (c.key) {
        const k = document.createElement("div");
        k.className = "im-tooltip-key";
        k.textContent = c.key;
        t.appendChild(k);
      }
    };
    const place = (rect) => {
      const t = tipEl();
      t.style.display = "block";
      const w = t.offsetWidth;
      const h = t.offsetHeight;
      let x = rect.left;
      let y = rect.bottom + 6;
      if (x + w > window.innerWidth - 4)
        x = window.innerWidth - w - 4;
      if (y + h > window.innerHeight - 4)
        y = rect.top - h - 6;
      t.style.left = Math.max(4, x) + "px";
      t.style.top = Math.max(4, y) + "px";
    };
    const release = () => {
      if (active && stashedTitle != null)
        active.setAttribute("title", stashedTitle);
      active = null;
      stashedTitle = null;
    };
    const hide = () => {
      if (timer) {
        clearTimeout(timer);
        timer = null;
      }
      if (tip && tip.style.display !== "none") {
        tip.style.display = "none";
        lastHidden = performance.now();
      }
      release();
    };
    document.addEventListener("mouseover", (e) => {
      if (!enabled())
        return;
      const node = owner(e.target);
      if (!node || node === active)
        return;
      release();
      const c = content(node);
      if (!c)
        return;
      active = node;
      if (node.hasAttribute("title")) {
        stashedTitle = node.getAttribute("title");
        node.removeAttribute("title");
      }
      if (timer)
        clearTimeout(timer);
      const delay = performance.now() - lastHidden < RESHOW ? 0 : DELAY;
      timer = setTimeout(() => {
        render(c);
        place(node.getBoundingClientRect());
      }, delay);
    }, true);
    document.addEventListener("mouseout", (e) => {
      if (!active)
        return;
      if (e.relatedTarget && active.contains(e.relatedTarget))
        return;
      hide();
    }, true);
    document.addEventListener("mousedown", hide, true);
    window.addEventListener("scroll", hide, true);
  }
})();
