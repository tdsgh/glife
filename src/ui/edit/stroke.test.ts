import { expect, test, vi } from "vitest";
import { cellsBetween, startStroke, strokeMode } from "./stroke";

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
