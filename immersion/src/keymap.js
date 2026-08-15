// The keymap shim: turn key chords into actions, client-side.
//
// The Rust side hands us the binding chords (with "Mod" unresolved, because
// only the browser knows the platform) via window.__imChords. We resolve Mod
// to Ctrl or Cmd here, watch keydown, and when a pressed chord matches a
// binding we preventDefault and send the ORIGINAL chord back over the eval
// channel — the Rust side looks the binding up by that chord and fires its
// action. Typing in a field is never intercepted unless a binding opts in.

(() => {
  // Read the chord list fresh each time it changes: a rebind rewrites
  // window.__imChords, and the shim must honour it without a reload.
  let chordsRef = null;
  let resolvedMap = new Map();
  const resolvedFor = () => {
    const list = window.__imChords || [];
    if (list !== chordsRef) {
      chordsRef = list;
      resolvedMap = new Map();
      for (const c of list) resolvedMap.set(c.replace(/\bMod\b/g, mod), c);
    }
    return resolvedMap;
  };
  const chords = window.__imChords || [];
  // navigator.platform alone is unreliable (deprecated, and empty under some
  // privacy settings — which then resolves Mod to Ctrl and kills every Cmd
  // chord on a Mac); check the userAgent too.
  const isMac =
    /Mac|iPhone|iPad|iPod/.test(navigator.platform || "") ||
    /Mac OS X/.test(navigator.userAgent || "");
  const mod = isMac ? "Meta" : "Ctrl";

  // Expand "Mod" in each binding to the platform key, keeping a map from the
  // resolved chord back to the original the Rust side knows.
  const resolved = new Map();
  for (const c of chords) {
    resolved.set(c.replace(/\bMod\b/g, mod), c);
  }

  // e.key for the spacebar is " "; the binding writes "Space". Map named keys
  // that report as a character so chords stay readable.
  const norm = (key) => {
    if (key === " ") return "Space";
    return key.length === 1 ? key.toUpperCase() : key;
  };

  const pressed = (e) => {
    const parts = [];
    if (e.ctrlKey) parts.push("Ctrl");
    if (e.metaKey) parts.push("Meta");
    if (e.altKey) parts.push("Alt");
    if (e.shiftKey) parts.push("Shift");
    // A bare modifier keydown has key === "Control" etc; skip those.
    if (["Control", "Meta", "Alt", "Shift"].includes(e.key)) return null;
    parts.push(norm(e.key));
    return parts.join("+");
  };

  // Rebind capture: the host turns this on, the next real chord is reported
  // back as a binding string (with the platform key written as "Mod", the way
  // bindings are authored) instead of firing an action.
  let capturing = false;
  window.__imCaptureChord = () => { capturing = true; };

  document.addEventListener("keydown", (e) => {
    const chord = pressed(e);
    if (!chord) return;
    if (capturing) {
      e.preventDefault();
      capturing = false;
      // Write the platform's primary modifier back as "Mod" so the stored
      // binding means the same thing on every platform.
      const grammar = chord.replace(new RegExp("\\b" + mod + "\\b"), "Mod");
      try { dioxus.send(JSON.stringify({ t: "capture", chord: grammar })); } catch (err) { /* gone */ }
      return;
    }
    const original = resolvedFor().get(chord);
    if (!original) return;
    // Don't steal keys from a field the user is typing in, unless the chord
    // carries a real modifier (Ctrl/Cmd/Alt) — those are commands, not text.
    const t = e.target;
    const typing = t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.isContentEditable);
    const hasMod = e.ctrlKey || e.metaKey || e.altKey;
    if (typing && !hasMod) return;
    e.preventDefault();
    try { dioxus.send(JSON.stringify({ t: "chord", chord: original })); } catch (err) { /* channel gone */ }
  });
})();
