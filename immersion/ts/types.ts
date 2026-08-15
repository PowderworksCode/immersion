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

/** A gesture the deck commits on pointerup — see `Gesture` in ui.rs. */
export type GestureMsg =
  | { t: "ratio"; id: number; ratio: number }
  | { t: "split"; id: number; dir: "row" | "col"; frac: number }
  | { t: "join"; survivor: number; victim: number }
  | { t: "swap"; a: number; b: number }
  | { t: "regionwidth"; id: number; region: string; w: number };

/** A keypress or a captured rebind — see `Msg` in keymap.rs. */
export type KeymapMsg =
  | { t: "chord"; chord: string }
  | { t: "capture"; chord: string };

/** One row of a menu — see the `*_menu_json` builders. */
export interface MenuItem {
  label?: string;
  action?: string;
  params?: unknown;
  chord?: string;
  sep?: boolean;
}

/** A menu pick — see `Pick` in contextmenu.rs. */
export interface MenuPick {
  action: string;
  params: unknown;
}

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

export {};
