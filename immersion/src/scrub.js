// Generated from immersion/ts/scrub.ts — do not edit by hand.
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

  // immersion/ts/scrub.ts
  if (once("__imScrub")) {
    const THRESHOLD = 3;
    let s = null;
    const num = (v, d) => {
      const n = parseFloat(v ?? "");
      return Number.isFinite(n) ? n : d;
    };
    document.addEventListener("pointerdown", (e) => {
      const input = e.target?.closest?.("[data-im-scrub]");
      if (!input)
        return;
      s = {
        input,
        startX: e.clientX,
        startVal: num(input.value, 0),
        step: num(input.dataset.scrubStep, 1),
        min: input.dataset.scrubMin ? num(input.dataset.scrubMin, -Infinity) : -Infinity,
        max: input.dataset.scrubMax ? num(input.dataset.scrubMax, Infinity) : Infinity,
        pid: e.pointerId,
        scrubbing: false
      };
    });
    document.addEventListener("pointermove", (e) => {
      if (!s)
        return;
      const dx = e.clientX - s.startX;
      if (!s.scrubbing) {
        if (Math.abs(dx) < THRESHOLD)
          return;
        s.scrubbing = true;
        s.input.blur();
        try {
          s.input.setPointerCapture(s.pid);
        } catch {}
        document.body.style.cursor = "ew-resize";
      }
      e.preventDefault();
      const speed = e.shiftKey ? 0.05 : 0.25;
      let v = s.startVal + dx * s.step * speed;
      if (e.ctrlKey || e.metaKey)
        v = Math.round(v / s.step) * s.step;
      v = Math.min(s.max, Math.max(s.min, v));
      const dec = (String(s.step).split(".")[1] ?? "").length;
      s.input.value = v.toFixed(dec);
    }, true);
    document.addEventListener("pointerup", () => {
      if (!s)
        return;
      if (s.scrubbing) {
        document.body.style.cursor = "";
        s.input.dispatchEvent(new Event("change", { bubbles: true }));
      } else {
        s.input.focus();
        s.input.select?.();
      }
      s = null;
    });
  }
})();
