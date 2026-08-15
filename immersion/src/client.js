// The client bundle: every purely-local behaviour that needs no server round
// trip, installed once. These used to be three separate shims (tooltip, slider,
// status-bar chord prettify) evaluated by three different components; they share
// nothing with the wire and everything with the DOM, so they live together here
// behind one install and one set of helpers.
//
// The channel shims — gestures, keymap, palette, context menu, layout files —
// stay separate: each owns a dioxus.send channel a component reads. This bundle
// is only the no-channel half.

(() => {
  // window.__imTooltipsEnabled is set by the Chrome component just before this
  // script on every eval; a re-eval to flip it hits the guard below and returns.
  if (window.__imClient) return;
  window.__imClient = true;

  // --- shared helpers -------------------------------------------------------
  const isMac = () =>
    /Mac|iPhone|iPad|iPod/.test(navigator.platform || "") ||
    /Mac OS X/.test(navigator.userAgent || "");
  const prettifyChord = (text) => {
    const sym = { Mod: "⌘", Ctrl: "⌃", Alt: "⌥", Shift: "⇧" };
    return isMac()
      ? text.split("+").map((t) => sym[t] || t).join("")
      : text.split("+").map((t) => (t === "Mod" ? "Ctrl" : t)).join("+");
  };

  // --- status-bar chord prettify -------------------------------------------
  // Rewrite `Mod+Shift+Z` to the platform glyphs on any .im-hint-key; re-run
  // after a re-render recreates them (the data-pretty mark keeps it idempotent).
  const pretty = () => {
    // Status-bar hints and menu chords both show a chord; prettify either.
    for (const el of document.querySelectorAll(
      ".im-hint-key:not([data-pretty]), .im-ctx-chord:not([data-pretty])",
    )) {
      el.dataset.pretty = "1";
      el.textContent = prettifyChord(el.textContent);
    }
  };
  const runPretty = () => requestAnimationFrame(pretty);
  runPretty();
  new MutationObserver(runPretty).observe(document.body, { childList: true, subtree: true });

  // --- slider live preview --------------------------------------------------
  // The slider commits on release; update the fill and readout locally as it
  // drags so it does not feel frozen. No server message until change.
  document.addEventListener(
    "input",
    (e) => {
      const input = e.target;
      if (!input.classList || !input.classList.contains("im-slider-input")) return;
      const min = parseFloat(input.min) || 0;
      const max = parseFloat(input.max);
      const val = parseFloat(input.value);
      if (!isFinite(max) || max <= min) return;
      const bar = input.closest(".im-slider");
      if (!bar) return;
      const pct = Math.min(100, Math.max(0, ((val - min) / (max - min)) * 100));
      bar.style.setProperty("--im-fill", pct + "%");
      const label = bar.querySelector(".im-slider-val");
      if (label) label.textContent = input.value;
    },
    true,
  );

  // --- tooltip --------------------------------------------------------------
  // A styled hover tooltip from a control's native `title` (or data-tip*),
  // 500ms delay (0ms on an instant hop between neighbours). Never talks to the
  // server. Honours the global enable flag.
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
      if (n.nodeType === 1 && (n.hasAttribute("data-tip") || n.hasAttribute("title"))) return n;
      n = n.parentElement;
    }
    return null;
  };
  const content = (node) => {
    const label = node.getAttribute("data-tip") || node.getAttribute("title");
    if (!label) return null;
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
    if (x + w > window.innerWidth - 4) x = window.innerWidth - w - 4;
    if (y + h > window.innerHeight - 4) y = rect.top - h - 6;
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
      release();
      const c = content(node);
      if (!c) return;
      active = node;
      if (node.hasAttribute("title")) {
        stashedTitle = node.getAttribute("title");
        node.removeAttribute("title");
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
      if (e.relatedTarget && active.contains(e.relatedTarget)) return;
      hide();
    },
    true,
  );
  document.addEventListener("mousedown", hide, true);
  window.addEventListener("scroll", hide, true);
})();
