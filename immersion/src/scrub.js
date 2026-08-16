// Generated from immersion/ts/scrub.ts — do not edit by hand.
// Run `bun run build` after changing the TypeScript source.
(() => {
  var __defProp = Object.defineProperty;
  var __getOwnPropNames = Object.getOwnPropertyNames;
  var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
  var __hasOwnProp = Object.prototype.hasOwnProperty;
  function __accessProp(key) {
    return this[key];
  }
  var __toCommonJS = (from) => {
    var entry = (__moduleCache ??= new WeakMap).get(from), desc;
    if (entry)
      return entry;
    entry = __defProp({}, "__esModule", { value: true });
    if (from && typeof from === "object" || typeof from === "function") {
      for (var key of __getOwnPropNames(from))
        if (!__hasOwnProp.call(entry, key))
          __defProp(entry, key, {
            get: __accessProp.bind(from, key),
            enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable
          });
    }
    __moduleCache.set(from, entry);
    return entry;
  };
  var __moduleCache;
  var __returnValue = (v) => v;
  function __exportSetter(name, newValue) {
    this[name] = __returnValue.bind(null, newValue);
  }
  var __export = (target, all) => {
    for (var name in all)
      __defProp(target, name, {
        get: all[name],
        enumerable: true,
        configurable: true,
        set: __exportSetter.bind(all, name)
      });
  };

  // immersion/ts/scrub.ts
  var exports_scrub = {};
  __export(exports_scrub, {
    evalExpr: () => evalExpr
  });

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
  var CONSTS = { pi: Math.PI, e: Math.E, tau: Math.PI * 2 };
  var evalExpr = (raw) => {
    const src = raw.trim().toLowerCase();
    if (src === "")
      return null;
    const plain = Number(src);
    if (Number.isFinite(plain))
      return plain;
    const named = src.replace(/\b(pi|tau|e)\b/g, "0").replace(/\d+(\.\d*)?(e[-+]?\d+)?/g, "0");
    if (!/^[-+*/()0.\s]+$/.test(named))
      return null;
    const expr = src.replace(/\b(pi|tau|e)\b/g, (m) => String(CONSTS[m] ?? 0));
    try {
      const v = Function(`"use strict";return (${expr})`)();
      return typeof v === "number" && Number.isFinite(v) ? v : null;
    } catch {
      return null;
    }
  };
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
    document.addEventListener("change", (e) => {
      const input = e.target;
      if (!input?.dataset || input.dataset.imScrub === undefined)
        return;
      const v = evalExpr(input.value);
      if (v !== null && String(v) !== input.value.trim())
        input.value = String(v);
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
