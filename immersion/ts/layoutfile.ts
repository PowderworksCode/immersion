// Layout export / import, client-side.
//
// Export is pure browser work: an [data-im-export] button carries the current
// layout as data-layout, and a click turns it into a downloaded workbench.json.
//
// Import must NOT use a file input in the Dioxus tree: liveview attaches its own
// file handling to such an input (a fetch to `/__file_dialog`, which 404s with
// an `undefined` base), and it fights ours. So the import button creates a
// throwaway file input in plain JS — outside the tree liveview manages — clicks
// it to open the dialog, reads the chosen file locally, and sends its text ONCE.
//
// Both are also reachable as document events — `im:layout-save` and
// `im:layout-load` — so a menu item can trigger them without carrying the
// button's attributes. The shim stays the only place that knows how either is
// actually done; the caller only says which.

import { once, send } from "./types";

if (once("__imLayoutFile")) {
  const save = (json: string) => {
    const blob = new Blob([json], { type: "application/json" });
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = "workbench.json";
    document.body.appendChild(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(a.href);
  };

  const load = () => {
    // Built here, not in the Dioxus render, so liveview never touches it.
    const input = document.createElement("input");
    input.type = "file";
    input.accept = "application/json,.json";
    input.style.display = "none";
    input.addEventListener("change", () => {
      const file = input.files?.[0];
      if (file) {
        const reader = new FileReader();
        reader.onload = () => send(String(reader.result));
        reader.readAsText(file);
      }
      input.remove();
    });
    document.body.appendChild(input);
    input.click();
  };

  document.addEventListener("click", (e) => {
    const target = e.target as Element | null;

    const exportBtn = target?.closest?.<HTMLElement>("[data-im-export]");
    if (exportBtn) {
      save(exportBtn.dataset.layout ?? "{}");
      return;
    }

    if (target?.closest?.("[data-im-import-trigger]")) load();
  });

  // The menu path. The layout to save lives on the export button, which is the
  // one element that always has the current value on it.
  document.addEventListener("im:layout-save", () => {
    const btn = document.querySelector<HTMLElement>("[data-im-export]");
    save(btn?.dataset.layout ?? "{}");
  });
  document.addEventListener("im:layout-load", load);
}
