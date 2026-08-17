// Generated from immersion/ts/layoutfile.ts — do not edit by hand.
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

  // immersion/ts/layoutfile.ts
  if (once("__imLayoutFile")) {
    const save = (json) => {
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
      const input = document.createElement("input");
      input.type = "file";
      input.accept = "application/json,.json";
      input.style.display = "none";
      input.addEventListener("change", () => {
        const file = input.files?.[0];
        if (file) {
          const reader = new FileReader;
          reader.onload = () => send(String(reader.result));
          reader.readAsText(file);
        }
        input.remove();
      });
      document.body.appendChild(input);
      input.click();
    };
    document.addEventListener("click", (e) => {
      const target = e.target;
      const exportBtn = target?.closest?.("[data-im-export]");
      if (exportBtn) {
        save(exportBtn.dataset.layout ?? "{}");
        return;
      }
      if (target?.closest?.("[data-im-import-trigger]"))
        load();
    });
    document.addEventListener("im:layout-save", () => {
      const btn = document.querySelector("[data-im-export]");
      save(btn?.dataset.layout ?? "{}");
    });
    document.addEventListener("im:layout-load", load);
  }
})();
