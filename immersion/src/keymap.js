// Generated from immersion/ts/keymap.ts — do not edit by hand.
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

  // immersion/ts/keymap.ts
  var isMac = /Mac|iPhone|iPad|iPod/.test(navigator.platform || "") || /Mac OS X/.test(navigator.userAgent || "");
  var mod = isMac ? "Meta" : "Ctrl";
  var chordsRef = null;
  var resolvedMap = new Map;
  var resolvedFor = () => {
    const list = window.__imChords ?? [];
    if (list !== chordsRef) {
      chordsRef = list;
      resolvedMap = new Map;
      for (const c of list)
        resolvedMap.set(c.replace(/\bMod\b/g, mod), c);
    }
    return resolvedMap;
  };
  var norm = (key) => {
    if (key === " ")
      return "Space";
    return key.length === 1 ? key.toUpperCase() : key;
  };
  var pressed = (e) => {
    const parts = [];
    if (e.ctrlKey)
      parts.push("Ctrl");
    if (e.metaKey)
      parts.push("Meta");
    if (e.altKey)
      parts.push("Alt");
    if (e.shiftKey)
      parts.push("Shift");
    if (["Control", "Meta", "Alt", "Shift"].includes(e.key))
      return null;
    parts.push(norm(e.key));
    return parts.join("+");
  };
  var capturing = false;
  window.__imCaptureChord = () => {
    capturing = true;
  };
  document.addEventListener("keydown", (e) => {
    const chord = pressed(e);
    if (!chord)
      return;
    if (capturing) {
      e.preventDefault();
      capturing = false;
      const grammar = chord.replace(new RegExp(`\\b${mod}\\b`), "Mod");
      send({ t: "capture", chord: grammar });
      return;
    }
    const original = resolvedFor().get(chord);
    if (!original)
      return;
    const t = e.target;
    const typing = !!t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.isContentEditable);
    const hasMod = e.ctrlKey || e.metaKey || e.altKey;
    if (typing && !hasMod)
      return;
    e.preventDefault();
    send({ t: "chord", chord: original });
  });
})();
