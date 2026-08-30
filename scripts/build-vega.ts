// Build the vendored chart renderer.
//
// Vega has no grammar chunks to prune — the whole runtime is the parser for
// the spec language — so this is a single bundle rather than a split one, and
// simply large. Committing it keeps the Rust build free of a javascript
// toolchain, the same bargain the shims and the diff renderer make.

import { readdir, rm, stat } from "node:fs/promises";
import { join } from "node:path";

const OUT = "immersion/vendor/vega";
await rm(OUT, { recursive: true, force: true });

const built = await Bun.build({
  entrypoints: ["immersion/ts/vendor/vega-entry.ts"],
  outdir: OUT,
  target: "browser",
  format: "esm",
  minify: true,
  naming: { entry: "entry.js" },
});
if (!built.success) {
  console.error(built.logs.join("\n"));
  process.exit(1);
}

let total = 0;
const files = await readdir(OUT);
for (const f of files) total += (await stat(join(OUT, f))).size;
console.log(
  `vendored ${files.length} file(s), ${(total / 1048576).toFixed(1)} MB`,
);
