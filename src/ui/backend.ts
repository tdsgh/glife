import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

export type Edges = "wrap" | "dead";

export interface Region {
  x0: number;
  y0: number;
  w: number;
  h: number;
}

/** A `since` value that never equals a snapshot version: forces a full reply. */
export const NO_VERSION = Number.MAX_SAFE_INTEGER;

export const newUniverse = (width: number, height: number, edges: Edges) =>
  invoke<void>("new_universe", { width, height, edges });
export const play = () => invoke<void>("play");
export const pause = () => invoke<void>("pause");
export const step = () => invoke<void>("step");
/** `gensPerSec` null means as fast as possible. */
export const setSpeed = (gensPerSec: number | null) => invoke<void>("set_speed", { gensPerSec });
export const setEdges = (edges: Edges) => invoke<void>("set_edges", { edges });
/** Batches with the same `stroke` id and no other command between them undo as one step; without one, each batch is its own step. */
export const edit = (cells: [x: number, y: number][], alive: boolean, stroke?: number) =>
  invoke<void>("edit", { cells, alive, stroke });
export const undo = () => invoke<void>("undo");
export const redo = () => invoke<void>("redo");
export const clear = () => invoke<void>("clear");
/** `density` is a percent, 1-99; the backend defaults to 25. */
export const randomFill = (density?: number) => invoke<void>("random_fill", { density });

/** A rectangle of cells inside the grid, `(x, y)` being its top-left cell. */
export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export type FlipAxis = "h" | "v";
export type PasteMode = "or" | "copy";

export const deleteSelection = (rect: Rect) => invoke<void>("delete_selection", { ...rect });
export const flipSelection = (rect: Rect, axis: FlipAxis) => invoke<void>("flip_selection", { ...rect, axis });
/** Stamps the pending pattern, flipped as given, with its center cell `(⌊w/2⌋, ⌊h/2⌋)` at `(x, y)`. */
export const place = (x: number, y: number, flipH: boolean, flipV: boolean, mode: PasteMode) =>
  invoke<void>("place", { x, y, flipH, flipV, mode });
/** The pending pattern as a raw pattern buffer. */
export const pendingCells = () => invoke<ArrayBuffer>("pending_cells");

/** Reads the clipboard's RLE into the pending slot; rejects with a message when there is none. */
export const paste = () => invoke<PendingInfo>("paste");
export const copySelection = (rect: Rect) => invoke<void>("copy_selection", { ...rect });
export const cutSelection = (rect: Rect) => invoke<void>("cut_selection", { ...rect });

export interface PendingInfo {
  width: number;
  height: number;
  name: string;
  comment: string;
  notices: string[];
}

export const LAYER_AGE = 1;
export const LAYER_TRAIL = 2;

/** `layers` is a bitmask of LAYER_AGE and LAYER_TRAIL; the alive bits are always sent. */
export const fetchFrame = (region: Region, since: number, layers: number) =>
  invoke<ArrayBuffer>("frame", { ...region, since, layers });

/** Where a pattern comes from: a built-in id, a personal name, or the pending slot. */
export type Source = { builtin: string } | { personal: string } | "pending";

/** Fills the pending slot from a library entry or keeps the current one. */
export const beginPlace = (source: Source) => invoke<PendingInfo>("begin_place", { source });

export interface BuiltinEntry {
  id: string;
  name: string;
  category: "still-life" | "oscillator" | "spaceship" | "gun" | "methuselah";
  description: string;
  width: number;
  height: number;
  live: number;
  period: number | null;
  speed: string | null;
}

export interface PersonalEntry {
  name: string;
  comment: string;
  width: number;
  height: number;
  live: number;
  period?: null;
  speed?: null;
  /** RFC 3339; empty when the file has none. */
  created: string;
  changed: string;
}

export interface LibraryProblem {
  file: string;
  reason: string;
}

export interface LibraryList {
  builtin: BuiltinEntry[];
  personal: PersonalEntry[];
  problems: LibraryProblem[];
}

/** null when written, or `{ nameTaken: true }` when another pattern has the name (ignoring case) and `replace` was false. */
export type LibraryResult = { nameTaken: true } | null;

export const libraryList = () => invoke<LibraryList>("library_list");
/** A raw pattern buffer of at most 64 x 64 cells. */
export const patternPreview = (source: Source) => invoke<ArrayBuffer>("pattern_preview", { source });
export const librarySave = (source: { selection: Rect } | "pending", name: string, comment: string | undefined, replace: boolean) =>
  invoke<LibraryResult>("library_save", { source, name, comment, replace });
export const libraryCopyBuiltin = (id: string, name: string, replace: boolean) =>
  invoke<LibraryResult>("library_copy_builtin", { id, name, replace });
export const libraryRename = (name: string, newName: string, replace: boolean) =>
  invoke<LibraryResult>("library_rename", { name, newName, replace });
export const librarySetComment = (name: string, comment: string) => invoke<void>("library_set_comment", { name, comment });
/** Asks first in a native dialog; resolves to whether the pattern was deleted. */
export const libraryDelete = (name: string) => invoke<boolean>("library_delete", { name });
/** Opens the personal library's folder in the system file manager. */
export const libraryShowFolder = () => invoke<void>("library_show_folder");

/** The unsaved-changes prompt (Save / Don't Save / Cancel). Resolves to whether to go on. Run it before New and Open. */
export const confirmDiscard = () => invoke<boolean>("confirm_discard");
/** Resolves to the file name, or null if the dialog was cancelled. */
export const save = () => invoke<string | null>("save");
export const saveAs = () => invoke<string | null>("save_as");

export interface OpenInfo {
  fileName: string;
  notices: string[];
}

/** Replaces the universe with a glife or RLE file the operator picks; null if cancelled. Run `confirmDiscard` first. */
export const open = () => invoke<OpenInfo | null>("open");
/** Reads an RLE file into the pending slot; null if cancelled. */
export const importRle = () => invoke<PendingInfo | null>("import_rle");
export const exportRle = (source: { selection: Rect } | "universe" | Source) => invoke<void>("export_rle", { source });

export type EditTarget = "text" | "universe";
/** Swaps the Edit menu for the platform's text items while a text field has focus. */
export const setEditTarget = (target: EditTarget) => invoke<void>("set_edit_target", { target });
export const setMenuState = (state: { canUndo: boolean; canRedo: boolean; hasSelection: boolean; playing: boolean }) =>
  invoke<void>("set_menu_state", state);

/** Sets the window title, e.g. `• name.glife — glife`. Does nothing outside the Tauri window. */
export const setTitle = async (title: string) => {
  try {
    await getCurrentWindow().setTitle(title);
  } catch {
    // no window (tests, a plain browser)
  }
};
/** Menu clicks: the item id (`new`, `open`, `save`, `save_as`, `import_rle`, `export_rle`, `undo`, `redo`, `cut`, `copy`, `paste`, `delete`, `select_all`, `flip_h`, `flip_v`). Resolves to an unlisten function. */
export const onMenu = (handler: (id: string) => void) => listen<string>("menu", (e) => handler(e.payload));
