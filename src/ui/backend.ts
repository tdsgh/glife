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
export const edit = (cells: [x: number, y: number][], alive: boolean) =>
  invoke<void>("edit", { cells, alive });
export const clear = () => invoke<void>("clear");
export const randomFill = () => invoke<void>("random_fill");

export const fetchFrame = (region: Region, since: number) =>
  invoke<ArrayBuffer>("frame", { ...region, since });
