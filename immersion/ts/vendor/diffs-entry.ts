// The vendored renderer's entry point: @pierre/diffs, wrapped in the small
// API the workbench actually uses.
//
// This is the one place a third-party renderer is allowed to draw, and it is
// here rather than in a shim because it is *large* — the bundle is code-split
// and its language grammars load on demand, which only works if the library
// keeps its own module graph. Everything else in ts/ is a shim that commits
// one message; this draws.
//
// The division of labour with the server: file text and diffs are computed in
// Rust and arrive as ordinary DOM content. This module turns them into
// highlighted rows. It never fetches, never talks to the server, and holds no
// state the server needs.

import {
  File,
  FileDiff,
  parsePatchFiles,
  preloadHighlighter,
} from "@pierre/diffs";
// The grammar for a *specific* file resolves asynchronously; render() answers
// false until it has. preloadFile/preloadFileDiff do that resolution, so the
// render that follows draws on its first attempt instead of silently
// producing an empty container.
import { preloadFile, preloadFileDiff } from "@pierre/diffs/ssr";

/// The grammars we ship. A language outside this list is rendered as plain
/// text rather than requested, because its chunk is pruned from the vendored
/// build and asking for it would 404 at the worst moment.
const LANGS = [
  "rust",
  "typescript",
  "javascript",
  "tsx",
  "jsx",
  "json",
  "toml",
  "yaml",
  "markdown",
  "python",
  "shellscript",
  "html",
  "css",
  "sql",
  "go",
  "diff",
] as const;

const THEMES = ["pierre-dark", "pierre-light"] as const;

/// The javascript regex engine, not the WASM one: it needs no separate asset
/// fetch, and the components have to be told the same thing the preload was.
const HIGHLIGHTER = "shiki-js" as const;

const EXT_LANG: Record<string, string> = {
  rs: "rust",
  ts: "typescript",
  tsx: "tsx",
  js: "javascript",
  jsx: "jsx",
  mjs: "javascript",
  cjs: "javascript",
  json: "json",
  toml: "toml",
  yaml: "yaml",
  yml: "yaml",
  md: "markdown",
  py: "python",
  sh: "shellscript",
  bash: "shellscript",
  html: "html",
  css: "css",
  sql: "sql",
  go: "go",
  diff: "diff",
  patch: "diff",
};

const langOf = (path: string): string => {
  const ext = path.split(".").pop()?.toLowerCase() ?? "";
  return EXT_LANG[ext] ?? "text";
};

let ready: Promise<void> | null = null;
const boot = (): Promise<void> => {
  // The JavaScript regex engine, not the WASM one: it needs no separate
  // asset fetch and the difference is imperceptible on files this size.
  ready ??= preloadHighlighter({
    themes: [...THEMES],
    langs: [...LANGS] as unknown as Parameters<
      typeof preloadHighlighter
    >[0]["langs"],
    preferredHighlighter: HIGHLIGHTER,
  });
  return ready;
};

/// What a preload hands back: the markup, plus what hydrate needs.
type Ready = Record<string, unknown> & { prerenderedHTML?: string };

interface CodeHost extends HTMLElement {
  dataset: DOMStringMap & {
    imCode?: string;
    imCodeDone?: string;
    imCodePath?: string;
    imCodeKind?: string;
    imCodeLayout?: string;
  };
}

/// What has been drawn, by stamp. The framework owns the host element and
/// re-renders it on its poll, which empties it — so the drawn node is kept
/// here and put back rather than rebuilt, and "already done" means the stamp
/// matches *and* the node is still in place.
const drawn = new Map<string, Node>();

/// Render every code host whose content the server has changed, and restore
/// any whose content the framework has just wiped.
const paint = async (): Promise<void> => {
  const hosts = Array.from(
    document.querySelectorAll<CodeHost>("[data-im-code]"),
  );
  if (!hosts.length) return;
  await boot();
  for (const host of hosts) {
    const stamp = host.dataset.imCode ?? "";
    if (host.dataset.imCodeDone === stamp && host.childElementCount > 0)
      continue;
    // Wiped by a re-render: the drawing is still good, it just needs putting
    // back. Rebuilding here would highlight the same file on every poll.
    const kept = drawn.get(stamp);
    if (kept) {
      host.replaceChildren(kept);
      host.dataset.imCodeDone = stamp;
      continue;
    }
    // The server puts the payload in a sibling the renderer owns nothing of,
    // so the framework patching that node and this module drawing never
    // fight over the same children.
    const src = document.querySelector<HTMLElement>(
      `[data-im-code-src="${stamp}"]`,
    );
    if (!src) continue;
    const text = src.textContent ?? "";
    const path = host.dataset.imCodePath ?? "";
    const dark = !document.documentElement.classList.contains("im-light");
    const theme = dark ? "pierre-dark" : "pierre-light";
    host.replaceChildren();
    try {
      // Draw into a container this module owns, so what the framework
      // discards is one node we can re-attach. Attached before rendering:
      // the renderer measures its container, and a detached one measures
      // zero, which draws nothing at all.
      const own = document.createElement("div");
      own.className = "im-code-drawn";
      host.replaceChildren(own);
      if (host.dataset.imCodeKind === "diff") {
        // The payload is a unified diff; the renderer parses it into the two
        // sides itself.
        const options = {
          theme,
          diffStyle: (host.dataset.imCodeLayout === "split"
            ? "split"
            : "unified") as "split" | "unified",
          preferredHighlighter: HIGHLIGHTER,
          disableFileHeader: true,
        };
        // A patch holds many files; a diff area shows one, so the first is
        // the one asked for — the server sends a single-file patch.
        const [patch] = parsePatchFiles(text);
        const fileDiff = patch?.files?.[0];
        if (!fileDiff) throw new Error("no file in patch");
        const ready = (await preloadFileDiff({
          fileDiff,
          options,
        })) as unknown as Ready;
        own.innerHTML = ready.prerenderedHTML ?? "";
        new FileDiff(options).hydrate({
          ...ready,
          fileContainer: own,
        } as never);
      } else {
        // The area header already names the file; the renderer's own header
        // would say it a second time.
        const options = {
          theme,
          preferredHighlighter: HIGHLIGHTER,
          disableFileHeader: true,
        };
        const file = {
          name: path,
          contents: text,
          lang: langOf(path) as never,
        };
        const ready = (await preloadFile({
          file,
          options,
        })) as unknown as Ready;
        // The preload *is* the render: it returns the markup. Injecting it and
        // then hydrating is the library's own server-rendering path, and it is
        // the one that draws — calling render() with a preloaded result
        // answers true and produces nothing.
        own.innerHTML = ready.prerenderedHTML ?? "";
        new File(options).hydrate({ ...ready, fileContainer: own } as never);
      }
      drawn.set(stamp, own);
      // One file at a time per host; holding every file ever opened would
      // grow without bound in a long session.
      if (drawn.size > 8) drawn.delete(drawn.keys().next().value as string);
      host.dataset.imCodeDone = stamp;
    } catch {
      // A renderer that throws must not leave an empty panel with no
      // explanation; the text is still worth showing.
      const pre = document.createElement("pre");
      pre.className = "im-code-fallback";
      pre.textContent = text;
      host.replaceChildren(pre);
      host.dataset.imCodeDone = stamp;
    }
  }
};

// The server re-renders on its own schedule, so watch the tree rather than
// asking it to tell us. One microtask-batched pass per mutation burst.
let queued = false;
const schedule = (): void => {
  if (queued) return;
  queued = true;
  queueMicrotask(() => {
    queued = false;
    void paint();
  });
};

new MutationObserver(schedule).observe(document.body, {
  childList: true,
  subtree: true,
});
schedule();
