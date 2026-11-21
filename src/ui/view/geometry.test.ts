import { expect, test } from "vitest";
import type { TilingId } from "../backend";
import { cellCenter, cellRange, pitch, pointsUp, ROW, worldSize, worldToCell } from "./geometry";

const GRIDS: TilingId[] = ["square", "hexagonal", "triangular-12", "triangular-3"];

test("world size follows the cell shapes", () => {
  expect(worldSize("square", 10, 6)).toEqual([10, 6]);
  const [hw, hh] = worldSize("hexagonal", 10, 6);
  expect(hw).toBe(10.5);
  expect(hh).toBeCloseTo(5 * ROW + 2 / Math.sqrt(3), 12);
  expect(worldSize("triangular-12", 10, 6)).toEqual([5.5, 6 * ROW]);
  expect(worldSize("triangular-3", 10, 6)).toEqual([5.5, 6 * ROW]);
});

test.each(GRIDS)("%s: world -> cell of a cell's center gives the cell", (t) => {
  for (let y = -2; y < 9; y++) {
    for (let x = -2; x < 9; x++) {
      const [wx, wy] = cellCenter(t, x, y);
      expect(worldToCell(t, wx, wy)).toEqual([x, y]);
    }
  }
});

test("hexagon: points just inside each edge and corner give the cell", () => {
  const [cx, cy] = cellCenter("hexagonal", 3, 2);
  const e = 0.02;
  // Left and right edges (flat sides, 0.5 from the center), then the four slanted edges.
  const inside: [number, number][] = [
    [-0.5 + e, 0], [0.5 - e, 0],
    [0.25, -(ROW / 2) + e], [-0.25, -(ROW / 2) + e], [0.25, ROW / 2 - e], [-0.25, ROW / 2 - e],
    // near the top and bottom corners
    [0, -1 / Math.sqrt(3) + e], [0, 1 / Math.sqrt(3) - e],
  ];
  for (const [dx, dy] of inside) expect(worldToCell("hexagonal", cx + dx, cy + dy)).toEqual([3, 2]);
  // Just outside the right edge is the next cell of the row; just outside the lower right edge, the row below.
  expect(worldToCell("hexagonal", cx + 0.5 + e, cy)).toEqual([4, 2]);
  expect(worldToCell("hexagonal", cx + 0.25 + e, cy + ROW / 2 + e)).toEqual([3, 3]);
  expect(worldToCell("hexagonal", cx - 0.25 - e, cy + ROW / 2 + e)).toEqual([2, 3]);
});

test.each(["triangular-12", "triangular-3"] as const)("%s: points just inside an up and a down triangle give the cell", (t) => {
  const e = 0.02;
  for (const [x, y] of [[2, 2], [3, 2]] as const) {
    const up = pointsUp(x, y);
    const [cx] = cellCenter(t, x, y);
    const top = y * ROW;
    const bottom = top + ROW;
    // Just inside the base, and just inside each slanted edge at mid height.
    const base = up ? bottom - e : top + e;
    expect(worldToCell(t, cx, base)).toEqual([x, y]);
    expect(worldToCell(t, cx - 0.5 + 2 * e, base)).toEqual([x, y]);
    expect(worldToCell(t, cx + 0.5 - 2 * e, base)).toEqual([x, y]);
    const mid = top + ROW / 2;
    expect(worldToCell(t, cx - 0.25 + e, mid)).toEqual([x, y]);
    expect(worldToCell(t, cx + 0.25 - e, mid)).toEqual([x, y]);
    // The apex side: the cell above an up triangle's base is the row above; below a down triangle's top, the row below.
    expect(worldToCell(t, cx, up ? top + e * 2 : bottom - e * 2)).toEqual([x, y]);
    expect(worldToCell(t, cx, up ? bottom + e : top - e)).toEqual([x, up ? y + 1 : y - 1]);
    // Just outside a slanted edge is the next triangle in the row.
    expect(worldToCell(t, cx + 0.25 + e, mid)).toEqual([x + 1, y]);
    expect(worldToCell(t, cx - 0.25 - e, mid)).toEqual([x - 1, y]);
  }
});

test.each(GRIDS)("%s: the cells covered by a world rectangle include every cell whose center lies inside it", (t) => {
  const w = 40;
  const h = 30;
  const rects: [number, number, number, number][] = [
    [0, 0, 5, 5], [3.3, 2.1, 9.7, 7.9], [10, 10, 10.2, 10.2], [-5, -5, 3, 3], [30, 20, 100, 100], [0, 0, 1000, 1000],
  ];
  for (const [x0, y0, x1, y1] of rects) {
    const r = cellRange(t, w, h, x0, y0, x1, y1);
    for (let y = 0; y < h; y++) {
      for (let x = 0; x < w; x++) {
        const [cx, cy] = cellCenter(t, x, y);
        if (cx >= x0 && cx <= x1 && cy >= y0 && cy <= y1) {
          expect(x >= r.left && x < r.right && y >= r.top && y < r.bottom, `${t} ${x},${y} in ${x0},${y0},${x1},${y1}`).toBe(true);
        }
      }
    }
    expect(r.left).toBeGreaterThanOrEqual(0);
    expect(r.right).toBeLessThanOrEqual(w);
    expect(r.bottom).toBeLessThanOrEqual(h);
  }
});

test("pitch is the world distance between neighboring columns and rows", () => {
  expect(pitch("square")).toEqual([1, 1]);
  expect(pitch("hexagonal")).toEqual([1, ROW]);
  expect(pitch("triangular-3")).toEqual([0.5, ROW]);
});
