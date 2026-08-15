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
    document.addEventListener("click", (e) => {
      const target = e.target;
      const exportBtn = target?.closest?.("[data-im-export]");
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
      }
    });
  }
})();
