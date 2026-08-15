// Compile the TypeScript shims to the JavaScript the Rust crate embeds.
//
// The browser receives these as strings via include_str!, so the crate must not
// need a JavaScript toolchain to build — that would break `cargo build` for
// anyone without bun, and the container image that ships powderman. So the
// generated .js files are committed, and CI re-runs this build and fails if the
// output has drifted from the .ts source.
//
// Each shim is an independent IIFE with no imports at runtime; bundling gives
// them shared helpers while still emitting one self-contained file each.

import { readdirSync } from "node:fs";
import { basename, join } from "node:path";

const TS_DIR = "immersion/ts";
const OUT_DIR = "immersion/src";

const entries = readdirSync(TS_DIR)
  .filter((f) => f.endsWith(".ts") && !f.endsWith(".d.ts") && f !== "types.ts")
  .map((f) => join(TS_DIR, f));

const result = await Bun.build({
  entrypoints: entries,
  outdir: OUT_DIR,
  target: "browser",
  format: "iife",
  minify: false,
  naming: "[dir]/[name].js",
});

if (!result.success) {
  for (const log of result.logs) console.error(log);
  process.exit(1);
}

const header = (name: string) =>
  `// Generated from immersion/ts/${name}.ts — do not edit by hand.\n` +
  `// Run \`bun run build\` after changing the TypeScript source.\n`;

for (const entry of entries) {
  const name = basename(entry, ".ts");
  const out = join(OUT_DIR, `${name}.js`);
  const code = await Bun.file(out).text();
  if (!code.startsWith("// Generated from")) {
    await Bun.write(out, header(name) + code);
  }
}

console.log(`built ${entries.length} shims`);
