// The command palette shim: fuzzy-filter and keyboard-drive a rendered list,
// client-side, and commit exactly one message — the chosen index — on Enter or
// click (or -1 on Escape / scrim). Same budget as the keymap and gesture shims:
// the typing and the arrowing never touch the wire; only the pick does.
//
// The Rust side renders every row with data-index and data-text (the label and
// action, lowercased, for matching); this filters by subsequence — "stpr"
// reaches "set property" — tracks a selection among the visible rows, and hands
// the index back over the eval channel dioxus.send opens.

(() => {
  const run = () => {
    const root = document.querySelector(".im-palette");
    if (!root) return;
    const input = root.querySelector(".im-palette-input");
    const rows = Array.from(root.querySelectorAll(".im-palette-row"));
    if (!input) return;

    // Show chords the way the platform does: ⌘ on a Mac, Ctrl elsewhere. The
    // same platform sniff the keymap uses (platform AND userAgent — platform
    // alone is unreliable and deprecated).
    const isMac =
      /Mac|iPhone|iPad|iPod/.test(navigator.platform || "") ||
      /Mac OS X/.test(navigator.userAgent || "");
    const macSym = { Mod: "⌘", Ctrl: "⌃", Alt: "⌥", Shift: "⇧" };
    for (const el of root.querySelectorAll(".im-palette-chord")) {
      if (isMac) {
        // ⌘⇧Z — symbols, no separators, the Mac convention.
        el.textContent = el.textContent
          .split("+")
          .map((t) => macSym[t] || t)
          .join("");
      } else {
        el.textContent = el.textContent.replace(/\bMod\b/g, "Ctrl");
      }
    }

    const subseq = (q, text) => {
      if (!q) return true;
      let i = 0;
      for (const ch of text) {
        if (ch === q[i]) i++;
        if (i === q.length) return true;
      }
      return false;
    };

    let sel = 0;
    const visible = () => rows.filter((r) => r.style.display !== "none");
    const paint = () => {
      const vis = visible();
      if (sel >= vis.length) sel = vis.length - 1;
      if (sel < 0) sel = 0;
      for (const r of rows) r.classList.remove("is-sel");
      if (vis[sel]) {
        vis[sel].classList.add("is-sel");
        vis[sel].scrollIntoView({ block: "nearest" });
      }
    };
    const filter = () => {
      const q = input.value.toLowerCase().trim();
      for (const r of rows) {
        r.style.display = subseq(q, r.dataset.text || "") ? "" : "none";
      }
      sel = 0;
      paint();
    };

    const send = (idx) => {
      try {
        dioxus.send(idx);
      } catch (err) {
        /* channel gone; the overlay is unmounting */
      }
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
    if (scrim) {
      scrim.addEventListener("mousedown", (e) => {
        if (e.target === scrim) send(-1);
      });
    }

    paint();
    input.focus();
  };

  // The list is rendered in the same cycle the future that evals this runs in;
  // wait one frame so the rows are in the DOM before we wire them.
  requestAnimationFrame(run);
})();
