// The vendored chart renderer: Vega-Lite, wrapped in the same small API the
// code and diff viewers use.
//
// Vega-Lite rather than a grammar of our own, for one reason: models already
// know it. A chart in this workbench is a Vega-Lite spec — a serde document
// like the layout and the settings — so an agent writes one with the tools it
// already has, and a person edits it by pointer in the data editor.
//
// The division of labour matches the diff viewer's. The server owns the spec
// and the data; this module draws. It never fetches and holds no state the
// server needs.

import embed from "vega-embed";

interface ChartHost extends HTMLElement {
  dataset: DOMStringMap & { imChart?: string; imChartDone?: string };
}

/// What has been drawn, by stamp — the framework empties the host element on
/// its poll, so the drawing is kept and put back rather than rebuilt. Same
/// contention, same fix, as the code viewer.
const drawn = new Map<string, Node>();

const paint = async (): Promise<void> => {
  const hosts = Array.from(document.querySelectorAll<ChartHost>("[data-im-chart]"));
  for (const host of hosts) {
    const stamp = host.dataset.imChart ?? "";
    if (host.dataset.imChartDone === stamp && host.childElementCount > 0) continue;
    const kept = drawn.get(stamp);
    if (kept) {
      host.replaceChildren(kept);
      host.dataset.imChartDone = stamp;
      continue;
    }
    const payload = document.querySelector<HTMLElement>(`[data-im-chart-src="${stamp}"]`);
    if (!payload) continue;
    const own = document.createElement("div");
    own.className = "im-chart-drawn";
    host.replaceChildren(own);
    try {
      const spec = JSON.parse(payload.textContent ?? "{}") as Record<string, unknown>;
      // The workbench owns its own chrome, so no embed menu. Sizing is the
      // spec's business ("container" width plus a fitting autosize, set
      // server-side) — passing it here instead silently does nothing.
      await embed(own, spec as never, { actions: false, renderer: "svg" });
      drawn.set(stamp, own);
      if (drawn.size > 8) drawn.delete(drawn.keys().next().value as string);
      host.dataset.imChartDone = stamp;
    } catch (err) {
      // A spec the renderer refuses says so where the chart would be; an
      // empty panel would look like a workbench bug rather than a bad spec.
      const msg = document.createElement("div");
      msg.className = "im-chart-error";
      msg.textContent = err instanceof Error ? err.message : String(err);
      host.replaceChildren(msg);
      host.dataset.imChartDone = stamp;
    }
  }
};

let queued = false;
const schedule = (): void => {
  if (queued) return;
  queued = true;
  queueMicrotask(() => {
    queued = false;
    void paint();
  });
};

new MutationObserver(schedule).observe(document.body, { childList: true, subtree: true });
schedule();
