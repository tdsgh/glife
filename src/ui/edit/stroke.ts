export type Cell = [x: number, y: number];

/** The 8-connected line of cells from a to b, both included. */
export function cellsBetween(a: Cell, b: Cell): Cell[] {
  const n = Math.max(Math.abs(b[0] - a[0]), Math.abs(b[1] - a[1]));
  const cells: Cell[] = [];
  for (let i = 0; i <= n; i++) {
    const t = n === 0 ? 0 : i / n;
    cells.push([Math.round(a[0] + (b[0] - a[0]) * t), Math.round(a[1] + (b[1] - a[1]) * t)]);
  }
  return cells;
}

/** How a stroke turns its points into cells: the cell at a point, and the cells of the line between two points. */
export interface Walk {
  cell(p: Cell): Cell;
  between(a: Cell, b: Cell): Cell[];
}

/** Points are cells of a square grid. */
const cellWalk: Walk = { cell: (p) => p, between: cellsBetween };

/** World step of a stroke between two pointer samples. */
export const WORLD_STEP = 0.25;

/** Points are world points: the line is sampled every quarter cell, and each cell it enters is sent. */
export function worldWalk(toCell: (p: Cell) => Cell): Walk {
  return {
    cell: toCell,
    between(a, b) {
      const n = Math.max(1, Math.ceil(Math.hypot(b[0] - a[0], b[1] - a[1]) / WORLD_STEP));
      const cells: Cell[] = [];
      for (let i = 0; i <= n; i++) {
        const c = toCell([a[0] + ((b[0] - a[0]) * i) / n, a[1] + ((b[1] - a[1]) * i) / n]);
        const prev = cells[cells.length - 1];
        if (!prev || prev[0] !== c[0] || prev[1] !== c[1]) cells.push(c);
      }
      return cells;
    },
  };
}

/** Draw (true) when the first cell is dead, erase (false) when it is alive. */
export const strokeMode = (aliveAtStart: boolean) => !aliveAtStart;

/**
 * Starts a stroke at `start` (a point, by `walk`): sends it at once, then each `move` sends one batch
 * with the cells from the previous position through every point given (the
 * coalesced events of one pointer move), deduplicated.
 */
export function startStroke(start: Cell, alive: boolean, send: (cells: Cell[], alive: boolean) => void, walk: Walk = cellWalk) {
  let last = start;
  send([walk.cell(start)], alive);
  return (points: Cell[]) => {
    const lastCell = walk.cell(last);
    const seen = new Set<string>([`${lastCell[0]},${lastCell[1]}`]);
    const batch: Cell[] = [];
    for (const p of points) {
      for (const c of walk.between(last, p)) {
        const key = `${c[0]},${c[1]}`;
        if (!seen.has(key)) {
          seen.add(key);
          batch.push(c);
        }
      }
      last = p;
    }
    if (batch.length) send(batch, alive);
  };
}
