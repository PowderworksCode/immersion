// Build the vendored renderer.
//
// Separate from build-shims.ts because the output is different in kind: the
// shims are small single files inlined into the binary with include_str!,
// while this is a code-split bundle whose grammar chunks load on demand and
// are served as static assets. Committing the output keeps the Rust build
// free of a javascript toolchain, the same bargain the shims make.
//
// Pruning: shiki ships ~200 grammars and we want sixteen. The entry names the
// languages it will ask for and maps everything else to plain text, so a
// pruned chunk can never be requested. This trims ~10 MB to ~4.

import { rm, readdir, stat, writeFile } from "node:fs/promises";
import { join } from "node:path";

const OUT = "immersion/vendor/diffs";
// Themes are chunks too, and they load dynamically — so pruning them passes
// every static check and then 404s at first paint. They are named here for
// the same reason the grammars are.
const THEMES = ["pierre-dark", "pierre-light"];
const KEEP = new Set([
  ...THEMES,
  "rust", "typescript", "javascript", "tsx", "jsx", "json", "toml", "yaml",
  "markdown", "python", "shellscript", "html", "css", "sql", "go", "diff",
  // Grammars the ones above embed; dropping these breaks their neighbours.
  "cpp", "c", "java", "regexp", "xml", "scss", "less", "graphql", "wasm",
  "sass", "stylus", "pug", "handlebars", "php", "ruby", "lua", "cmake",
]);

await rm(OUT, { recursive: true, force: true });

const built = await Bun.build({
  entrypoints: ["immersion/ts/vendor/diffs-entry.ts"],
  outdir: OUT,
  target: "browser",
  format: "esm",
  splitting: true,
  minify: true,
  naming: { entry: "entry.js", chunk: "[name]-[hash].js" },
});
if (!built.success) {
  console.error(built.logs.join("\n"));
  process.exit(1);
}

// Drop the grammars we will never ask for. Bun names shared runtime chunks
// after the entry ("diffs-entry-<hash>.js") and language chunks after the
// language ("rust-<hash>.js"), so the entry prefix is the thing that must
// never be pruned — deleting those breaks every surviving grammar.
let dropped = 0;
let droppedBytes = 0;
for (const name of await readdir(OUT)) {
  if (name === "entry.js" || !name.endsWith(".js")) continue;
  const base = name.replace(/-[a-z0-9]+\.js$/, "");
  if (base === "diffs-entry" || KEEP.has(base)) continue;
  droppedBytes += (await stat(join(OUT, name))).size;
  await rm(join(OUT, name));
  dropped++;
}

// A static import of a file we pruned would fail at load. A *dynamic* import
// of one is fine — that is shiki's language map, whose entries we never call
// because the entry maps unknown extensions to plain text — so the two are
// checked separately and only the first is fatal.
const present = new Set(await readdir(OUT));
const brokenStatic: string[] = [];
for (const name of present) {
  if (!name.endsWith(".js")) continue;
  const text = await Bun.file(join(OUT, name)).text();
  for (const m of text.matchAll(/(?<!import\()["\']\.\/([A-Za-z0-9+#._-]+\.js)["\']/g)) {
    const ref = m[1] ?? "";
    if (!present.has(ref) && !text.includes(`import("./${ref}")`)) {
      brokenStatic.push(`${name} -> ${ref}`);
    }
  }
}
if (brokenStatic.length) {
  console.error("pruning broke static imports:\n  " + brokenStatic.slice(0, 10).join("\n  "));
  process.exit(1);
}

// The entry asks for these by name at boot; if pruning ever takes one the
// page renders nothing, which a static import check cannot see.
for (const t of THEMES) {
  if (![...present].some((f) => f.startsWith(`${t}-`))) {
    console.error(`pruning removed the ${t} theme`);
    process.exit(1);
  }
}

const files = await readdir(OUT);
let total = 0;
for (const f of files) total += (await stat(join(OUT, f))).size;

// A manifest the Rust side asserts against, so a rebuild that silently loses
// the entry or balloons is a failing test rather than a broken page.
await writeFile(
  join(OUT, "manifest.json"),
  `${JSON.stringify({ files: files.length, bytes: total }, null, 2)}\n`,
);
console.log(
  `vendored ${files.length} files, ${(total / 1048576).toFixed(1)} MB ` +
    `(pruned ${dropped} grammars, ${(droppedBytes / 1048576).toFixed(1)} MB)`,
);
