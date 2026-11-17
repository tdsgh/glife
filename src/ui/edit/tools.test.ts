import { expect, test } from "vitest";
import type { Rect } from "../backend";
import type { Cell } from "./stroke";
import * as t from "./tools";

const G: t.Grid = { w: 100, h: 50 };
const withSel = (r: Rect): t.EditState => ({ ...t.initialEdit(), selection: r });
const pattern = { width: 3, height: 3, bits: new Uint32Array(3) };

test("Escape ends placing first, then the selection", () => {
  let s = withSel({ x: 1, y: 1, w: 4, h: 4 });
  s = t.startPlacing(s, pattern).state;
  s = t.escape(s).state;
  expect(s.placing).toBeNull();
  expect(s.selection).not.toBeNull();
  s = t.escape(s).state;
  expect(s.selection).toBeNull();
});

test("placing returns to the previous tool", () => {
  for (const tool of ["draw", "select"] as const) {
    let s = t.setTool(t.initialEdit(), tool).state;
    s = t.startPlacing(s, pattern).state;
    expect(s.placing!.returnTo).toBe(tool);
    expect(t.escape(s).state.tool).toBe(tool);
  }
});

test("choosing a tool while placing ends placing and keeps that tool", () => {
  const s = t.startPlacing(t.initialEdit(), pattern).state;
  const r = t.setTool(s, "select").state;
  expect(r.placing).toBeNull();
  expect(r.tool).toBe("select");
});

test("a click outside the selection with the select tool ends it; a click inside keeps it", () => {
  let s = { ...t.setTool(t.initialEdit(), "select").state, selection: { x: 10, y: 10, w: 5, h: 5 } as Rect | null };
  expect(t.pointerDown(s, [12, 12], G).state.selection).toEqual({ x: 10, y: 10, w: 5, h: 5 });
  s = t.pointerDown(s, [30, 30], G).state;
  s = t.pointerUp(s).state;
  expect(s.selection).toBeNull();
});

test("drawing and undo leave the selection in place", () => {
  let s = withSel({ x: 10, y: 10, w: 5, h: 5 });
  s = t.pointerDown(s, [60, 20], G).state;
  s = t.pointerUp(s).state;
  expect(s.selection).toEqual({ x: 10, y: 10, w: 5, h: 5 });
});

test("a drag is normalized whatever its direction", () => {
  const drag = (a: Cell, b: Cell) => {
    let s = t.setTool(t.initialEdit(), "select").state;
    s = t.pointerDown(s, a, G).state;
    s = t.pointerMove(s, b, G).state;
    return t.pointerUp(s).state.selection;
  };
  const want = { x: 5, y: 7, w: 11, h: 4 };
  expect(drag([5, 7], [15, 10])).toEqual(want);
  expect(drag([15, 10], [5, 7])).toEqual(want);
  expect(drag([15, 7], [5, 10])).toEqual(want);
  expect(drag([5, 10], [15, 7])).toEqual(want);
});

test("a drag is clamped at every grid edge", () => {
  const s0 = t.setTool(t.initialEdit(), "select").state;
  const dragged = (a: Cell, b: Cell) => t.pointerMove(t.pointerDown(s0, a, G).state, b, G).state.selection;
  expect(dragged([5, 5], [-20, -20])).toEqual({ x: 0, y: 0, w: 6, h: 6 });
  expect(dragged([95, 45], [500, 500])).toEqual({ x: 95, y: 45, w: 5, h: 5 });
  expect(dragged([-9, -9], [200, 200])).toEqual({ x: 0, y: 0, w: 100, h: 50 });
});

test("a click with the select tool makes no selection", () => {
  let s = t.setTool(t.initialEdit(), "select").state;
  s = t.pointerDown(s, [5, 5], G).state;
  expect(t.pointerMove(s, [5, 5], G).state.selection).toBeNull();
  expect(t.pointerUp(s).state.selection).toBeNull();
});

test("each draw press takes a new stroke id", () => {
  let r = t.pointerDown(t.initialEdit(), [1, 1], G);
  const first = r.effects[0];
  r = t.pointerDown(t.pointerUp(r.state).state, [2, 2], G);
  expect(first).toEqual({ kind: "stroke", id: 1 });
  expect(r.effects[0]).toEqual({ kind: "stroke", id: 2 });
  expect(t.pointerDown(t.initialEdit(), [-1, 1], G).effects).toEqual([]);
});

test("placing: a click places with the flips and mode; the anchor stays", () => {
  let s = t.setMode(t.initialEdit(), "copy").state;
  s = t.startPlacing(s, pattern).state;
  s = t.flip(s, "h").state;
  const r = t.pointerDown(s, [7, 8], G);
  expect(r.effects).toEqual([{ kind: "place", x: 7, y: 8, flipH: true, flipV: false, mode: "copy" }]);
  expect(r.state.placing).not.toBeNull();
  expect(t.pointerDown(s, [200, 8], G).effects).toEqual([]);
});

test("flip acts on the placement while placing and on the selection otherwise", () => {
  const sel = { x: 1, y: 2, w: 3, h: 4 };
  let s = withSel(sel);
  expect(t.key(s, "h")!.effects).toEqual([{ kind: "flipSelection", rect: sel, axis: "h" }]);
  expect(t.key(s, "v")!.effects).toEqual([{ kind: "flipSelection", rect: sel, axis: "v" }]);
  s = t.startPlacing(s, pattern).state;
  const r = t.key(s, "v")!;
  expect(r.effects).toEqual([]);
  expect(r.state.placing!.flipV).toBe(true);
});

test("Delete kills the selection, or clears without one", () => {
  const sel = { x: 1, y: 2, w: 3, h: 4 };
  expect(t.key(withSel(sel), "Delete")!.effects).toEqual([{ kind: "deleteSelection", rect: sel }]);
  expect(t.key(t.initialEdit(), "Backspace")!.effects).toEqual([{ kind: "clear" }]);
});

test("New and Open end placing and the selection; select all takes the grid", () => {
  let s = withSel({ x: 1, y: 1, w: 2, h: 2 });
  s = t.startPlacing(s, pattern).state;
  const r = t.reset(s).state;
  expect(r.placing).toBeNull();
  expect(r.selection).toBeNull();
  expect(t.selectAll(t.initialEdit(), G).state.selection).toEqual({ x: 0, y: 0, w: 100, h: 50 });
});

test("unrelated keys are not handled", () => {
  expect(t.key(t.initialEdit(), "x")).toBeNull();
});
