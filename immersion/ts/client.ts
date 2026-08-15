// The client bundle: every purely-local behaviour that needs no server round
// trip, installed once — the list filter, the slider live preview, and the
// tooltip. They share nothing with the wire and everything with the DOM, so
// they live together behind one install.
//
// The channel shims — gestures, keymap, palette, context menu, layout files —
// stay separate: each owns a dioxus.send channel a component reads.

import { once } from "./types";

interface TipContent {
  label: string;
  key: string | null;
  desc: string | null;
}

if (once("__imClient")) {

  // --- shared helpers -------------------------------------------------------
  // (Chord prettifying used to live here, rewriting .im-hint-key text. That
  // detached the text nodes the renderer patches, so a chord that changed —
  // a rebind — never visibly updated. Chords are now written server-side from
  // a platform flag the client reports once; see keymap.rs.)

  // --- slider live preview --------------------------------------------------
  // The slider commits on release; update the fill and readout locally as it
  // drags so it does not feel frozen. No server message until change.
  document.addEventListener(
    "input",
    (e) => {
      const input = e.target as HTMLInputElement;
      if (!input.classList || !input.classList.contains("im-slider-input")) return;
      const min = parseFloat(input.min) || 0;
      const max = parseFloat(input.max);
      const val = parseFloat(input.value);
      if (!isFinite(max) || max <= min) return;
      const bar = input.closest<HTMLElement>(".im-slider");
      if (!bar) return;
      const pct = Math.min(100, Math.max(0, ((val - min) / (max - min)) * 100));
      bar.style.setProperty("--im-fill", pct + "%");
      const label = bar.querySelector<HTMLElement>(".im-slider-val");
      if (label) label.textContent = input.value;
    },
    true,
  );

  // --- list filter ----------------------------------------------------------
  // Type in an [data-im-filter] box to hide non-matching rows in its scope.
  // Subsequence matching, like the palette: "tbf" reaches "treebank_fix". Pure
  // DOM work — the server is never told, so it costs nothing per keystroke.
  const subseq = (q: string, text: string): boolean => {
    if (!q) return true;
    let i = 0;
    for (const ch of text) {
      if (ch === q[i]) i++;
      if (i === q.length) return true;
    }
    return false;
  };
  // Typing in a filter box must not reach the server at all. Dioxus delegates
  // events at the document, so a keydown anywhere is forwarded once ANY
  // component registers a keydown listener — including keystrokes in a purely
  // local field. Stopping propagation in the capture phase (before the
  // delegated bubble listener) keeps the filter's typing entirely client-side.
  document.addEventListener(
    "keydown",
    (e) => {
      const t = e.target as HTMLElement | null;
      if (t?.dataset && t.dataset.imFilter !== undefined) {
        e.stopPropagation();
      }
    },
    true,
  );

  document.addEventListener("input", (e) => {
    const box = e.target as HTMLInputElement;
    if (!box.dataset || box.dataset.imFilter === undefined) return;
    const scope = box.closest(".im-filter-scope") || document;
    const q = box.value.toLowerCase().trim();
    for (const row of scope.querySelectorAll<HTMLElement>("[data-filter-text]")) {
      row.style.display = subseq(q, (row.dataset.filterText || "").toLowerCase()) ? "" : "none";
    }
  });

  // --- tooltip --------------------------------------------------------------
  // A styled hover tooltip from a control's native `title` (or data-tip*),
  // 500ms delay (0ms on an instant hop between neighbours). Never talks to the
  // server. Honours the global enable flag.
  const DELAY = 250;
  const RESHOW = 250;
  let tip: HTMLElement | null = null;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let active: HTMLElement | null = null;
  let stashedTitle: string | null = null;
  let lastHidden = 0;
  const enabled = () => window.__imTooltipsEnabled !== false;

  const tipEl = (): HTMLElement => {
    if (!tip) {
      tip = document.createElement("div");
      tip.className = "im-tooltip";
      tip.style.display = "none";
      document.body.appendChild(tip);
    }
    return tip;
  };
  const owner = (node: EventTarget | null): HTMLElement | null => {
    let n = node as HTMLElement | null;
    while (n && n !== document.body) {
      if (n.nodeType === 1 && (n.hasAttribute("data-tip") || n.hasAttribute("title"))) return n;
      n = n.parentElement;
    }
    return null;
  };
  const content = (node: HTMLElement): TipContent | null => {
    const label = node.getAttribute("data-tip") || node.getAttribute("title");
    if (!label) return null;
    return { label, key: node.getAttribute("data-tip-key"), desc: node.getAttribute("data-tip-desc") };
  };
  const render = (c: TipContent): void => {
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
  const place = (rect: DOMRect): void => {
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
      if (e.relatedTarget && active.contains(e.relatedTarget as Node)) return;
      hide();
    },
    true,
  );
  document.addEventListener("mousedown", hide, true);
  window.addEventListener("scroll", hide, true);
}
