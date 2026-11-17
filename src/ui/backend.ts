import { invoke } from "@tauri-apps/api/core";

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

export const LAYER_AGE = 1;
export const LAYER_TRAIL = 2;

/** `layers` is a bitmask of LAYER_AGE and LAYER_TRAIL; the alive bits are always sent. */
export const fetchFrame = (region: Region, since: number, layers: number) =>
  invoke<ArrayBuffer>("frame", { ...region, since, layers });
