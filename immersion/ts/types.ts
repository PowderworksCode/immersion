// The wire between a shim and the server, typed on both ends.
//
// Every shim commits by calling `dioxus.send(...)` with one of these shapes,
// and each has a serde counterpart in the Rust module that owns the shim. They
// live together here so a change to one side is visibly a change to the other.

/** The eval channel dioxus opens for a shim. */
declare global {
  const dioxus: { send(payload: string): void };
  interface Window {
    __imChords?: string[];
    __imCaptureChord?: () => void;
    __imOpenMenu?: (itemsJson: string) => void;
    __imOpenPie?: (itemsJson: string) => void;
    __imTooltipsEnabled?: boolean;
    __imScrub?: boolean;
    __imColor?: boolean;
    __imCtxMenu?: boolean;
    __imGestures?: boolean;
    __imLayoutFile?: boolean;
    __imClient?: boolean;
    __imKeymapInstalled?: boolean;
  }
}

export type { Dir } from "./generated/Dir";
// The wire is declared in Rust and generated from it — see the `TS` derives on
// Gesture, Msg and Pick. A shape can no longer drift on one side only: change
// the Rust and `cargo test` rewrites these, and CI fails if the checked-in
// bindings are stale.
export type { Gesture as GestureMsg } from "./generated/Gesture";
export type { KeymapMsg } from "./generated/KeymapMsg";
export type { MenuItem } from "./generated/MenuItem";
export type { MenuPick } from "./generated/MenuPick";
export type { Region } from "./generated/Region";

/** Send a typed message over the shim's channel. The channel disappears when
 *  the view unmounts; a reload re-installs, so a failure here is not an error. */
export function send(msg: unknown): void {
  try {
    dioxus.send(typeof msg === "string" ? msg : JSON.stringify(msg));
  } catch {
    /* channel gone; a reload re-installs */
  }
}

/** Install-once guard: a shim may be evaluated again (a re-render, a flag
 *  change) and must not register its listeners twice. */
export function once(flag: keyof Window & string): boolean {
  const w = window as unknown as Record<string, unknown>;
  if (w[flag]) return false;
  w[flag] = true;
  return true;
}

/** Exhaustiveness: the compiler rejects an unhandled variant, and this throws
 *  if one slips through at runtime. Cheaper than a pattern-matching dependency
 *  for unions this small, and it needs no new syntax. */
export function assertNever(x: never): never {
  throw new Error(`unhandled variant: ${JSON.stringify(x)}`);
}
