// Generated from immersion/ts/colorpicker.ts — do not edit by hand.
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

  // immersion/ts/colorpicker.ts
  if (once("__imColor")) {
    const clamp01 = (v) => Math.min(1, Math.max(0, v));
    const hsv2rgb = (h, s, v) => {
      const i = Math.floor(h * 6);
      const f = h * 6 - i;
      const p = v * (1 - s);
      const q = v * (1 - f * s);
      const t = v * (1 - (1 - f) * s);
      const k = i % 6;
      const rs = [v, q, p, p, t, v];
      const gs = [t, v, v, q, p, p];
      const bs = [p, p, t, v, v, q];
      return [
        Math.round((rs[k] ?? 0) * 255),
        Math.round((gs[k] ?? 0) * 255),
        Math.round((bs[k] ?? 0) * 255)
      ];
    };
    const rgb2hsv = (r, g, b) => {
      r /= 255;
      g /= 255;
      b /= 255;
      const mx = Math.max(r, g, b);
      const mn = Math.min(r, g, b);
      const d = mx - mn;
      let h = 0;
      if (d) {
        if (mx === r)
          h = (g - b) / d % 6;
        else if (mx === g)
          h = (b - r) / d + 2;
        else
          h = (r - g) / d + 4;
        h /= 6;
        if (h < 0)
          h += 1;
      }
      return [h, mx ? d / mx : 0, mx];
    };
    const hex2rgb = (hex) => {
      const m = /^#?([0-9a-f]{6})$/i.exec((hex || "").trim());
      if (!m)
        return [0, 0, 0];
      const n = parseInt(m[1] ?? "0", 16);
      return [n >> 16 & 255, n >> 8 & 255, n & 255];
    };
    const rgb2hex = (r, g, b) => "#" + [r, g, b].map((x) => x.toString(16).padStart(2, "0")).join("");
    let pop = null;
    const closePop = () => {
      if (pop) {
        pop.el.remove();
        pop = null;
      }
    };
    document.addEventListener("click", (e) => {
      const swatch = e.target?.closest?.("[data-im-color-open]");
      if (!swatch) {
        if (pop && !e.target?.closest?.(".im-colorpop"))
          closePop();
        return;
      }
      e.preventDefault();
      if (pop && pop.swatch === swatch) {
        closePop();
        return;
      }
      closePop();
      const field = swatch.parentElement?.querySelector("[data-im-color-value]");
      if (!field)
        return;
      let [h, s, v] = rgb2hsv(...hex2rgb(field.value));
      const el = document.createElement("div");
      el.className = "im-colorpop";
      el.innerHTML = '<div class="im-sv"><span class="im-sv-dot"></span></div>' + '<div class="im-hue"><span class="im-hue-dot"></span></div>';
      document.body.appendChild(el);
      const r = swatch.getBoundingClientRect();
      el.style.left = Math.min(r.left, window.innerWidth - el.offsetWidth - 6) + "px";
      el.style.top = (r.bottom + el.offsetHeight > window.innerHeight - 6 ? r.top - el.offsetHeight - 4 : r.bottom + 4) + "px";
      const sv = el.querySelector(".im-sv");
      const svDot = el.querySelector(".im-sv-dot");
      const hue = el.querySelector(".im-hue");
      const hueDot = el.querySelector(".im-hue-dot");
      const paint = () => {
        const [r8, g8, b8] = hsv2rgb(h, s, v);
        const hex = rgb2hex(r8, g8, b8);
        sv.style.setProperty("--im-sv-hue", rgb2hex(...hsv2rgb(h, 1, 1)));
        svDot.style.left = s * 100 + "%";
        svDot.style.top = (1 - v) * 100 + "%";
        hueDot.style.top = h * 100 + "%";
        swatch.style.background = hex;
        field.value = hex;
        return hex;
      };
      paint();
      const commit = () => field.dispatchEvent(new Event("change", { bubbles: true }));
      const dragSv = (ev) => {
        const b = sv.getBoundingClientRect();
        s = clamp01((ev.clientX - b.left) / b.width);
        v = 1 - clamp01((ev.clientY - b.top) / b.height);
        paint();
      };
      const dragHue = (ev) => {
        const b = hue.getBoundingClientRect();
        h = clamp01((ev.clientY - b.top) / b.height);
        paint();
      };
      const wire = (target, move) => {
        target.addEventListener("pointerdown", (ev) => {
          ev.preventDefault();
          target.setPointerCapture(ev.pointerId);
          move(ev);
          const mv = (e2) => move(e2);
          const up = () => {
            target.removeEventListener("pointermove", mv);
            target.removeEventListener("pointerup", up);
            commit();
          };
          target.addEventListener("pointermove", mv);
          target.addEventListener("pointerup", up);
        });
      };
      wire(sv, dragSv);
      wire(hue, dragHue);
      pop = { el, swatch };
    });
    document.addEventListener("keydown", (e) => {
      if (e.key === "Escape" && pop)
        closePop();
    });
  }
})();
