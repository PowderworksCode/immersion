// The status-bar shim: rewrite the hint chords to the platform's glyphs.
//
// The bar renders chords in grammar form (`Mod+Shift+Z`) because the server
// cannot know the platform. Here we turn `Mod` into ⌘ on a Mac (⌃⌥⇧ for the
// others, concatenated the Mac way) and leave the word forms elsewhere — the
// same rule the palette uses. A MutationObserver re-runs it after a liveview
// re-render recreates the spans; the `data-pretty` mark keeps it idempotent.

(() => {
  if (window.__imStatusInstalled) return;
  window.__imStatusInstalled = true;

  const isMac =
    /Mac|iPhone|iPad|iPod/.test(navigator.platform || "") ||
    /Mac OS X/.test(navigator.userAgent || "");
  const sym = { Mod: "⌘", Ctrl: "⌃", Alt: "⌥", Shift: "⇧" };

  const pretty = () => {
    for (const el of document.querySelectorAll(".im-hint-key:not([data-pretty])")) {
      el.dataset.pretty = "1";
      const parts = el.textContent.split("+");
      el.textContent = isMac
        ? parts.map((t) => sym[t] || t).join("")
        : parts.map((t) => (t === "Mod" ? "Ctrl" : t)).join("+");
    }
  };

  const run = () => requestAnimationFrame(pretty);
  run();
  // Re-prettify when a re-render recreates the hint spans (their text reverts
  // to grammar form). Scoped cheaply by the :not([data-pretty]) filter.
  new MutationObserver(run).observe(document.body, { childList: true, subtree: true });
})();
