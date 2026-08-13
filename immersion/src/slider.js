// The slider live-preview shim.
//
// The slider commits once, on release (onchange) — the liveview budget. But the
// fill and the value are server-rendered, so without a local preview the bar
// sits still through the whole drag and only jumps when you let go, which reads
// as unresponsive. This updates the fill width and the value readout LOCALLY on
// each input event (during the drag), touching only DOM the server is not
// changing this instant; the server still hears just the final value on
// release, then re-renders and confirms the preview.

(() => {
  if (window.__imSlider) return;
  window.__imSlider = true;

  document.addEventListener(
    "input",
    (e) => {
      const input = e.target;
      if (!input.classList || !input.classList.contains("im-slider-input")) return;
      const min = parseFloat(input.min) || 0;
      const max = parseFloat(input.max);
      const val = parseFloat(input.value);
      if (!isFinite(max) || max <= min) return;
      const bar = input.closest(".im-slider");
      if (!bar) return;
      const pct = Math.min(100, Math.max(0, ((val - min) / (max - min)) * 100));
      bar.style.setProperty("--im-fill", pct + "%");
      const label = bar.querySelector(".im-slider-val");
      if (label) label.textContent = input.value;
    },
    true,
  );
})();
