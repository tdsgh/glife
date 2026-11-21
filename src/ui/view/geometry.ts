import type { TilingId } from "../backend";

export type World = [x: number, y: number];
export type Cell = [x: number, y: number];

/** Height of a triangle of base 1, and the pitch of a hexagon row. */
export const ROW = Math.sqrt(3) / 2;
/** Half the height of a pointy-top hexagon of width 1. */
const HEX_HALF = 1 / Math.sqrt(3);

const triangular = (t: TilingId) => t === "triangular-12" || t === "triangular-3";

/** World size of a `w` x `h` grid. */
export function worldSize(t: TilingId, w: number, h: number): World {
  if (t === "hexagonal") return [w + (h > 1 ? 0.5 : 0), (h - 1) * ROW + 2 * HEX_HALF];
  if (triangular(t)) return [(w + 1) / 2, h * ROW];
  return [w, h];
}

/** World distance between neighboring columns and between neighboring rows. */
export function pitch(t: TilingId): World {
  if (t === "hexagonal") return [1, ROW];
  if (triangular(t)) return [0.5, ROW];
  return [1, 1];
}

/** True when triangle `(x, y)` points up. */
export const pointsUp = (x: number, y: number) => ((x + y) & 1) === 0;

export function cellCenter(t: TilingId, x: number, y: number): World {
  if (t === "hexagonal") return [x + 0.5 + (y & 1) * 0.5, y * ROW + HEX_HALF];
  if (triangular(t)) return [(x + 1) / 2, (y + 0.5) * ROW];
  return [x + 0.5, y + 0.5];
}

/** The cell under a world point. Not clamped: points outside the grid give cells outside it. */
export function worldToCell(t: TilingId, wx: number, wy: number): Cell {
  if (t === "hexagonal") {
    // Hexagons are the Voronoi cells of their centers: take the nearest of the three candidate rows.
    const r = Math.round((wy - HEX_HALF) / ROW);
    let best: Cell = [0, r];
    let bestD = Infinity;
    for (let y = r - 1; y <= r + 1; y++) {
      const x = Math.round(wx - 0.5 - (y & 1) * 0.5);
      const [cx, cy] = cellCenter(t, x, y);
      const d = (wx - cx) ** 2 + (wy - cy) ** 2;
      if (d < bestD) {
        bestD = d;
        best = [x, y];
      }
    }
    return best;
  }
  if (triangular(t)) {
    const y = Math.floor(wy / ROW);
    const fy = wy / ROW - y;
    const i = Math.floor(wx * 2);
    for (let x = i - 1; x <= i + 1; x++) {
      const half = (pointsUp(x, y) ? fy : 1 - fy) / 2;
      if (Math.abs(wx - (x + 1) / 2) <= half) return [x, y];
    }
    return [i, y];
  }
  return [Math.floor(wx), Math.floor(wy)];
}

/**
 * A range of columns and rows, `left`/`top` inclusive and `right`/`bottom` exclusive,
 * that holds every cell touching the world rectangle, clipped to the `w` x `h` grid.
 */
export function cellRange(t: TilingId, w: number, h: number, x0: number, y0: number, x1: number, y1: number) {
  let left: number, right: number, top: number, bottom: number;
  if (t === "hexagonal") {
    left = Math.floor(x0 - 0.5);
    right = Math.ceil(x1);
    top = Math.floor((y0 - 2 * HEX_HALF) / ROW) + 1;
    bottom = Math.ceil(y1 / ROW);
  } else if (triangular(t)) {
    left = Math.floor(x0 * 2) - 1;
    right = Math.ceil(x1 * 2);
    top = Math.floor(y0 / ROW);
    bottom = Math.ceil(y1 / ROW);
  } else {
    left = Math.floor(x0);
    right = Math.ceil(x1);
    top = Math.floor(y0);
    bottom = Math.ceil(y1);
  }
  return { left: Math.max(0, left), right: Math.min(w, right), top: Math.max(0, top), bottom: Math.min(h, bottom) };
}
