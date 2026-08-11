// The keymap shim: turn key chords into actions, client-side.
//
// The Rust side hands us the binding chords (with "Mod" unresolved, because
// only the browser knows the platform) via window.__imChords. We resolve Mod
// to Ctrl or Cmd here, watch keydown, and when a pressed chord matches a
// binding we preventDefault and send the ORIGINAL chord back over the eval
// channel — the Rust side looks the binding up by that chord and fires its
// action. Typing in a field is never intercepted unless a binding opts in.

(() => {
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

  document.addEventListener("keydown", (e) => {
    const chord = pressed(e);
    if (!chord) return;
    const original = resolved.get(chord);
    if (!original) return;
    // Don't steal keys from a field the user is typing in, unless the chord
    // carries a real modifier (Ctrl/Cmd/Alt) — those are commands, not text.
    const t = e.target;
    const typing = t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.isContentEditable);
    const hasMod = e.ctrlKey || e.metaKey || e.altKey;
    if (typing && !hasMod) return;
    e.preventDefault();
    try { dioxus.send(original); } catch (err) { /* channel gone; a reload re-installs */ }
  });
})();
