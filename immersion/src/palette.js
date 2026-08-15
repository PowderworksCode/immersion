// Generated from immersion/ts/palette.ts — do not edit by hand.
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

  // immersion/ts/palette.ts
  var run = () => {
    const root = document.querySelector(".im-palette");
    if (!root)
      return;
    const input = root.querySelector(".im-palette-input");
    const rows = Array.from(root.querySelectorAll(".im-palette-row"));
    if (!input)
      return;
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
    let sel = 0;
    const visible = () => rows.filter((r) => r.style.display !== "none");
    const paint = () => {
      const vis = visible();
      if (sel >= vis.length)
        sel = vis.length - 1;
      if (sel < 0)
        sel = 0;
      for (const r of rows)
        r.classList.remove("is-sel");
      const cur = vis[sel];
      if (cur) {
        cur.classList.add("is-sel");
        cur.scrollIntoView({ block: "nearest" });
      }
    };
    const filter = () => {
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
    const scrim = document.querySelector(".im-palette-scrim");
    scrim?.addEventListener("mousedown", (e) => {
      if (e.target === scrim)
        send(-1);
    });
    paint();
    input.focus();
  };
  requestAnimationFrame(run);
})();
