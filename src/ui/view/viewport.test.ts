import { expect, test } from "vitest";
import { fit, panBy, resize, screenToCell, visibleRegion, zoomAt } from "./viewport";

test("fit on a wide window is limited by height", () => {
  const v = fit(1000, 400, 100, 100);
  expect(v.pxPerCell).toBe(4);
  expect(v).toMatchObject({ centerX: 50, centerY: 50 });
});

test("fit on a tall window is limited by width", () => {
  expect(fit(300, 900, 100, 100).pxPerCell).toBe(3);
});

test("zoomAt keeps the cell under the pointer fixed", () => {
  const v = zoomAt(fit(800, 600, 1000, 1000), 600, 200, 1);
  const z = zoomAt({ ...v, pxPerCell: 10, centerX: 500, centerY: 500 }, 600, 200, 2);
  const before = screenToCell({ ...v, pxPerCell: 10, centerX: 500, centerY: 500 }, 600, 200);
  const after = screenToCell(z, 600, 200);
  expect(z.pxPerCell).toBe(20);
  expect(after.x).toBeCloseTo(before.x, 9);
  expect(after.y).toBeCloseTo(before.y, 9);
});

test("zoom is clamped between fit and 64 px per cell", () => {
  const v = fit(800, 800, 100, 100);
  expect(zoomAt(v, 400, 400, 0.01).pxPerCell).toBe(v.pxPerCell);
  expect(zoomAt(v, 400, 400, 1000).pxPerCell).toBe(64);
});

test("pan is clamped so the centre stays inside the grid", () => {
  const v = { ...fit(800, 800, 100, 100), pxPerCell: 20 };
  expect(panBy(v, -1e6, -1e6)).toMatchObject({ centerX: 0, centerY: 0 });
  expect(panBy(v, 1e6, 1e6)).toMatchObject({ centerX: 100, centerY: 100 });
  const p = panBy(v, 40, -20);
  expect(p.centerX).toBeCloseTo(52);
  expect(p.centerY).toBeCloseTo(49);
});

test("resize keeps the centre", () => {
  const v = { ...fit(800, 800, 100, 100), pxPerCell: 20, centerX: 30, centerY: 70 };
  const r = resize(v, 500, 300);
  expect(r).toMatchObject({ centerX: 30, centerY: 70, width: 500, height: 300 });
});

test("resize re-clamps zoom to the new fit", () => {
  const v = fit(400, 400, 100, 100);
  expect(resize(v, 1000, 1000).pxPerCell).toBe(10);
});

test("visibleRegion at fit covers the whole grid, aligned to 32", () => {
  expect(visibleRegion(fit(500, 500, 100, 100))).toEqual({ x0: 0, y0: 0, w: 128, h: 100 });
  expect(visibleRegion(fit(512, 512, 64, 64))).toEqual({ x0: 0, y0: 0, w: 64, h: 64 });
});

test("visibleRegion clips and aligns at the top-left edge", () => {
  const v = { ...fit(800, 800, 1000, 1000), pxPerCell: 10, centerX: 0, centerY: 0 };
  expect(visibleRegion(v)).toEqual({ x0: 0, y0: 0, w: 64, h: 40 });
});

test("visibleRegion clips and aligns at the bottom-right edge", () => {
  const v = { ...fit(800, 800, 1000, 1000), pxPerCell: 10, centerX: 1000, centerY: 1000 };
  expect(visibleRegion(v)).toEqual({ x0: 960, y0: 960, w: 64, h: 40 });
});

test("visibleRegion in the middle aligns x0 down and the right edge up", () => {
  const v = { ...fit(800, 800, 1000, 1000), pxPerCell: 10, centerX: 500, centerY: 500 };
  // cells 460..540 wide -> 448..544
  expect(visibleRegion(v)).toEqual({ x0: 448, y0: 460, w: 96, h: 80 });
});
