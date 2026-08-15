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

import { once, send } from "./types";

if (once("__imLayoutFile")) {
  document.addEventListener("click", (e) => {
    const target = e.target as Element | null;

    const exportBtn = target?.closest?.<HTMLElement>("[data-im-export]");
    if (exportBtn) {
      const json = exportBtn.dataset.layout ?? "{}";
      const blob = new Blob([json], { type: "application/json" });
      const a = document.createElement("a");
      a.href = URL.createObjectURL(blob);
      a.download = "workbench.json";
      document.body.appendChild(a);
      a.click();
      a.remove();
      URL.revokeObjectURL(a.href);
      return;
    }

    const importBtn = target?.closest?.("[data-im-import-trigger]");
    if (importBtn) {
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
    }
  });
}
