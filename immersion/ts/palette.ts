// The command palette shim: fuzzy-filter and keyboard-drive a rendered list,
// client-side, and commit exactly one message — the chosen index — on Enter or
// click (or -1 on Escape / scrim). Same budget as the keymap and gesture shims:
// the typing and the arrowing never touch the wire; only the pick does.

import { send } from "./types";

const run = (): void => {
  const root = document.querySelector<HTMLElement>(".im-palette");
  if (!root) return;
  const input = root.querySelector<HTMLInputElement>(".im-palette-input");
  const rows = Array.from(
    root.querySelectorAll<HTMLElement>(".im-palette-row"),
  );
  if (!input) return;

  const subseq = (q: string, text: string): boolean => {
    if (!q) return true;
    let i = 0;
    for (const ch of text) {
      if (ch === q[i]) i++;
      if (i === q.length) return true;
    }
    return false;
  };

  let sel = 0;
  const visible = (): HTMLElement[] =>
    rows.filter((r) => r.style.display !== "none");
  const paint = (): void => {
    const vis = visible();
    if (sel >= vis.length) sel = vis.length - 1;
    if (sel < 0) sel = 0;
    for (const r of rows) r.classList.remove("is-sel");
    const cur = vis[sel];
    if (cur) {
      cur.classList.add("is-sel");
      cur.scrollIntoView({ block: "nearest" });
    }
  };
  const filter = (): void => {
    const q = input.value.toLowerCase().trim();
    for (const r of rows) {
      r.style.display = subseq(q, r.dataset.text ?? "") ? "" : "none";
    }
    sel = 0;
    paint();
  };

  input.addEventListener("input", filter);
  input.addEventListener("keydown", (e) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      sel++;
      paint();
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      sel--;
      paint();
    } else if (e.key === "Enter") {
      e.preventDefault();
      const v = visible()[sel];
      send(v ? Number(v.dataset.index) : -1);
    } else if (e.key === "Escape") {
      e.preventDefault();
      send(-1);
    }
  });
  for (const r of rows) {
    r.addEventListener("click", () => send(Number(r.dataset.index)));
  }
  const scrim = document.querySelector<HTMLElement>(".im-palette-scrim");
  scrim?.addEventListener("mousedown", (e) => {
    if (e.target === scrim) send(-1);
  });

  paint();
  input.focus();
};

// The list is rendered in the same cycle the future that evals this runs in;
// wait one frame so the rows are in the DOM before we wire them.
requestAnimationFrame(run);
