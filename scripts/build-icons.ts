// straitjacket-allow-file:inline-svg — this script reads SVG files and slices
// their markup; the <svg> tags it names are the input format, not a component
// drawing an icon inline.
//
// Vendor the Tabler outline set as path data.
//
// The workbench used a dozen hand-picked glyphs, which is fine until a host
// wants an icon for an editor of its own and has to come here to add one. The
// whole outline set is 1.2 MB of path data — cheaper than the diff renderer,
// and it means an editor names any Tabler icon and gets it.
//
// Only the outline set: every glyph here is stroked in currentColor so it
// takes the surrounding text colour, and the filled set would double the size
// to supply a second look nothing in the workbench asks for.

import { mkdir, readdir, writeFile } from "node:fs/promises";
import { join } from "node:path";

const SRC = "node_modules/@tabler/icons/icons/outline";
const OUT = "immersion/vendor/icons";

const icons: Record<string, string> = {};
for (const file of (await readdir(SRC)).sort()) {
  if (!file.endsWith(".svg")) continue;
  const svg = await Bun.file(join(SRC, file)).text();
  // Keep the drawing elements; drop the outer <svg> and the invisible
  // sizing path every Tabler icon starts with.
  const body = svg
    .slice(svg.indexOf(">") + 1, svg.lastIndexOf("</svg>"))
    .replace(/<path stroke="none"[^/]*\/>/g, "")
    .replace(/\s+/g, " ")
    .trim();
  if (body) icons[file.slice(0, -4)] = body;
}

await mkdir(OUT, { recursive: true });
const json = JSON.stringify(icons);
await writeFile(join(OUT, "tabler-outline.json"), json);
console.log(
  `vendored ${Object.keys(icons).length} icons, ${(json.length / 1048576).toFixed(2)} MB`,
);
