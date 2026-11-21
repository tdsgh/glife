import { expect, test, vi } from "vitest";
import { worldToCell } from "../view/geometry";
import { cellsBetween, startStroke, strokeMode, worldWalk } from "./stroke";

test("horizontal line", () => {
  expect(cellsBetween([1, 2], [4, 2])).toEqual([[1, 2], [2, 2], [3, 2], [4, 2]]);
  expect(cellsBetween([4, 2], [2, 2])).toEqual([[4, 2], [3, 2], [2, 2]]);
});

test("vertical line", () => {
  expect(cellsBetween([3, 5], [3, 2])).toEqual([[3, 5], [3, 4], [3, 3], [3, 2]]);
});

test("diagonal line", () => {
  expect(cellsBetween([0, 0], [3, 3])).toEqual([[0, 0], [1, 1], [2, 2], [3, 3]]);
});

test("steep line has no gaps", () => {
  const cells = cellsBetween([0, 0], [2, 7]);
  expect(cells).toHaveLength(8);
  expect(cells[0]).toEqual([0, 0]);
  expect(cells[7]).toEqual([2, 7]);
  cells.slice(1).forEach((c, i) => {
    expect(Math.abs(c[0] - cells[i][0])).toBeLessThanOrEqual(1);
    expect(c[1] - cells[i][1]).toBe(1);
  });
});

test("single point", () => {
  expect(cellsBetween([5, 6], [5, 6])).toEqual([[5, 6]]);
});

test("strokeMode draws on dead, erases on alive", () => {
  expect(strokeMode(false)).toBe(true);
  expect(strokeMode(true)).toBe(false);
});

test("a simulated drag sends one deduplicated batch per move", () => {
  const send = vi.fn();
  const move = startStroke([0, 0], true, send);
  move([[0, 0]]);
  move([[3, 0]]);
  move([[3, 1], [3, 2], [2, 2]]);
  expect(send.mock.calls).toEqual([
    [[[0, 0]], true],
    [[[1, 0], [2, 0], [3, 0]], true],
    [[[3, 1], [3, 2], [2, 2]], true],
  ]);
});

test("a world stroke across a hexagonal grid sends a connected run of cells", () => {
  const send = vi.fn();
  const toCell = ([x, y]: [number, number]) => worldToCell("hexagonal", x, y);
  const move = startStroke([0.5, 0.6], true, send, worldWalk(toCell));
  move([[12.3, 9.1]]);
  const cells = send.mock.calls.flatMap(([c]) => c) as [number, number][];
  expect(cells[0]).toEqual([0, 0]);
  expect(cells[cells.length - 1]).toEqual(toCell([12.3, 9.1]));
  expect(new Set(cells.map((c) => c.join(","))).size).toBe(cells.length);
  const [first, ...rest] = cells;
  let prev = first;
  for (const c of rest) {
    // Neighbors on the hexagonal grid: same row, or the two cells in the adjacent row on each side.
    const dy = c[1] - prev[1];
    const shift = prev[1] % 2 === 0 ? [-1, 0] : [0, 1];
    const ok = dy === 0 ? Math.abs(c[0] - prev[0]) === 1 : Math.abs(dy) === 1 && shift.includes(c[0] - prev[0]);
    expect(ok, `${prev} -> ${c}`).toBe(true);
    prev = c;
  }
});
