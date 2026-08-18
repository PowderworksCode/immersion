// Injected into the content webview, before its page loads.
//
// This is the whole answer to "does a real web pane still obey the workbench".
// The page is a top-level browsing context, so it receives every key and every
// right-click and the host never sees them. Running first, in the capture
// phase, is what takes them back — and only the ones the workbench claims:
// anything unrecognised is left to the page, which is the difference between
// embedding a page and breaking it.
//
// It is `keymap.js` and `contextmenu.js` with the transport swapped:
// `window.ipc.postMessage` instead of `dioxus.send`.
(() => {
  const post = (msg) => {
    try {
      window.ipc.postMessage(JSON.stringify(msg));
    } catch (_) {
      // No host: the page was opened in a normal browser. Do nothing rather
      // than throw into someone else's console.
    }
  };

  // The chords this shell claims. In the real thing these arrive from the
  // effective keymap; matching in the guest rather than posting every
  // keystroke is the same budget rule the rest of the workbench holds to —
  // one message per meaningful event, not per keypress.
  const CLAIMED = {
    F3: "palette",
    Escape: "dismiss",
    p: "passthrough",
  };

  document.addEventListener(
    "keydown",
    (e) => {
      // Never claim a key someone is typing into something.
      const el = document.activeElement;
      const typing =
        el &&
        (el.tagName === "INPUT" ||
          el.tagName === "TEXTAREA" ||
          el.isContentEditable);
      if (typing && e.key !== "Escape") return;
      const action = CLAIMED[e.key];
      if (!action) return;
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      e.preventDefault();
      e.stopPropagation();
      post({ kind: "chord", chord: e.key, action });
    },
    true,
  );

  document.addEventListener(
    "contextmenu",
    (e) => {
      e.preventDefault();
      e.stopPropagation();
      post({ kind: "menu", x: e.clientX, y: e.clientY });
    },
    true,
  );

  const hello = () =>
    post({ kind: "hello", url: location.href, title: document.title });
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", hello, { once: true });
  } else {
    hello();
  }

  // Proof that a click reached the page and not an invisible overlay above it.
  document.addEventListener("click", () => post({ kind: "click" }), true);
})();
