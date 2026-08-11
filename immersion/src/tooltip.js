// The tooltip shim: a Blender hover-tooltip, entirely client-side.
//
// Hover is continuous and a per-hover round trip is exactly the liveview cost
// the rest of the library refuses, so this never talks to the server. It reads
// what is already in the DOM — a control's native `title` (or a richer
// `data-tip` / `data-tip-key` / `data-tip-desc`) — and renders one shared,
// styled tooltip after a short delay. The native `title` is stashed and removed
// while we own the control, so the browser's own tooltip does not also appear,
// and restored when the pointer leaves (so it survives with the shim disabled).
//
// One shared element, one timer: when a tooltip hides and the pointer lands on
// a neighbour within the re-show window, the next one appears with no delay —
// Blender's instant hop between adjacent controls.

(() => {
  // The flag (window.__imTooltipsEnabled) is set by the Rust side just before
  // this script on every eval; the listeners install once. A re-eval to flip
  // the flag hits this guard and returns without registering a second set.
  if (window.__imTooltipsInstalled) return;
  window.__imTooltipsInstalled = true;

  const DELAY = 500;
  const RESHOW = 250;
  let tip = null;
  let timer = null;
  let active = null; // the control we are currently showing/claiming
  let stashedTitle = null;
  let lastHidden = 0;

  const enabled = () => window.__imTooltipsEnabled !== false;

  const el = () => {
    if (!tip) {
      tip = document.createElement("div");
      tip.className = "im-tooltip";
      tip.style.display = "none";
      document.body.appendChild(tip);
    }
    return tip;
  };

  // The nearest ancestor of `node` that carries tooltip content.
  const owner = (node) => {
    let n = node;
    while (n && n !== document.body) {
      if (
        n.nodeType === 1 &&
        (n.hasAttribute("data-tip") || n.hasAttribute("title"))
      ) {
        return n;
      }
      n = n.parentElement;
    }
    return null;
  };

  const content = (node) => {
    const label = node.getAttribute("data-tip") || node.getAttribute("title");
    if (!label) return null;
    return {
      label,
      key: node.getAttribute("data-tip-key"),
      desc: node.getAttribute("data-tip-desc"),
    };
  };

  const render = (c) => {
    const t = el();
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
    const t = el();
    t.style.display = "block";
    const w = t.offsetWidth;
    const h = t.offsetHeight;
    let x = rect.left;
    let y = rect.bottom + 6;
    if (x + w > window.innerWidth - 4) x = window.innerWidth - w - 4;
    if (y + h > window.innerHeight - 4) y = rect.top - h - 6; // flip above
    t.style.left = Math.max(4, x) + "px";
    t.style.top = Math.max(4, y) + "px";
  };

  const release = () => {
    if (active && stashedTitle != null) active.setAttribute("title", stashedTitle);
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

  document.addEventListener(
    "mouseover",
    (e) => {
      if (!enabled()) return;
      const node = owner(e.target);
      if (!node || node === active) return;
      // switching controls: drop the old one first
      release();
      const c = content(node);
      if (!c) return;
      active = node;
      if (node.hasAttribute("title")) {
        stashedTitle = node.getAttribute("title");
        node.removeAttribute("title"); // suppress the browser's own tooltip
      }
      if (timer) clearTimeout(timer);
      const delay = performance.now() - lastHidden < RESHOW ? 0 : DELAY;
      timer = setTimeout(() => {
        render(c);
        place(node.getBoundingClientRect());
      }, delay);
    },
    true,
  );

  document.addEventListener(
    "mouseout",
    (e) => {
      if (!active) return;
      // only hide when the pointer actually leaves the active control
      if (e.relatedTarget && active.contains(e.relatedTarget)) return;
      hide();
    },
    true,
  );
  document.addEventListener("mousedown", hide, true);
  window.addEventListener("scroll", hide, true);
})();
