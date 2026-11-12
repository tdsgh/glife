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

/** Draw (true) when the first cell is dead, erase (false) when it is alive. */
export const strokeMode = (aliveAtStart: boolean) => !aliveAtStart;

/**
 * Starts a stroke at `start`: sends it at once, then each `move` sends one batch
 * with the cells from the previous position through every point given (the
 * coalesced events of one pointer move), deduplicated.
 */
export function startStroke(start: Cell, alive: boolean, send: (cells: Cell[], alive: boolean) => void) {
  let last = start;
  send([start], alive);
  return (points: Cell[]) => {
    const seen = new Set<string>([`${last[0]},${last[1]}`]);
    const batch: Cell[] = [];
    for (const p of points) {
      for (const c of cellsBetween(last, p)) {
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
