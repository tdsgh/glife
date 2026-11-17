import type { FlipAxis, PasteMode, Rect } from "../backend";
import type { Cell } from "./stroke";

export type Tool = "draw" | "select";

/** A pattern following the pointer. `bits` is a pattern buffer's rows: `ceil(width / 32)` words each. */
export interface Placing {
  width: number;
  height: number;
  bits: Uint32Array;
  flipH: boolean;
  flipV: boolean;
  /** The tool that comes back when placing ends. */
  returnTo: Tool;
}

export interface EditState {
  tool: Tool;
  /** Kept between placements. */
  mode: PasteMode;
  placing: Placing | null;
  selection: Rect | null;
  /** The cell a selection drag started on, while one is under way. */
  dragging: { anchor: Cell; moved: boolean } | null;
  /** The id of the draw stroke under way. */
  stroke: number | null;
  nextStroke: number;
}

export interface Grid {
  w: number;
  h: number;
}

/** What the caller must do after a transition. */
export type Effect =
  | { kind: "stroke"; id: number }
  | { kind: "place"; x: number; y: number; flipH: boolean; flipV: boolean; mode: PasteMode }
  | { kind: "deleteSelection"; rect: Rect }
  | { kind: "flipSelection"; rect: Rect; axis: FlipAxis }
  | { kind: "clear" };

export interface Result {
  state: EditState;
  effects: Effect[];
}

export const initialEdit = (): EditState => ({
  tool: "draw",
  mode: "or",
  placing: null,
  selection: null,
  dragging: null,
  stroke: null,
  nextStroke: 1,
});

const done = (state: EditState, ...effects: Effect[]): Result => ({ state, effects });

const inGrid = ([x, y]: Cell, g: Grid) => x >= 0 && y >= 0 && x < g.w && y < g.h;
const clampCell = ([x, y]: Cell, g: Grid): Cell => [Math.min(Math.max(x, 0), g.w - 1), Math.min(Math.max(y, 0), g.h - 1)];
const inside = (r: Rect, [x, y]: Cell) => x >= r.x && y >= r.y && x < r.x + r.w && y < r.y + r.h;

/** The rectangle between two cells, whatever their order, clamped to the grid. */
export function rectBetween(a: Cell, b: Cell, g: Grid): Rect {
  const [ax, ay] = clampCell(a, g);
  const [bx, by] = clampCell(b, g);
  const x = Math.min(ax, bx);
  const y = Math.min(ay, by);
  return { x, y, w: Math.abs(ax - bx) + 1, h: Math.abs(ay - by) + 1 };
}

export function setTool(s: EditState, tool: Tool): Result {
  return done({ ...s, tool, placing: null, dragging: null });
}

export function setMode(s: EditState, mode: PasteMode): Result {
  return done({ ...s, mode });
}

export function startPlacing(s: EditState, pattern: { width: number; height: number; bits: Uint32Array }): Result {
  const placing: Placing = { ...pattern, flipH: false, flipV: false, returnTo: s.placing?.returnTo ?? s.tool };
  return done({ ...s, placing, dragging: null });
}

export function selectAll(s: EditState, g: Grid): Result {
  return done({ ...s, ...endPlacing(s), selection: { x: 0, y: 0, w: g.w, h: g.h } });
}

/** Placing ends and the earlier tool returns. */
const endPlacing = (s: EditState): Partial<EditState> =>
  s.placing ? { placing: null, tool: s.placing.returnTo } : {};

/** New universe, Open, or a grid of another size: no placement, no selection. */
export function reset(s: EditState): Result {
  return done({ ...s, ...endPlacing(s), selection: null, dragging: null, stroke: null });
}

export function pointerDown(s: EditState, cell: Cell, g: Grid): Result {
  if (s.placing) {
    if (!inGrid(cell, g)) return done(s);
    const { flipH, flipV } = s.placing;
    return done(s, { kind: "place", x: cell[0], y: cell[1], flipH, flipV, mode: s.mode });
  }
  if (s.tool === "draw") {
    if (!inGrid(cell, g)) return done(s);
    return done({ ...s, stroke: s.nextStroke, nextStroke: s.nextStroke + 1 }, { kind: "stroke", id: s.nextStroke });
  }
  if (s.selection && inside(s.selection, cell)) return done(s);
  return done({ ...s, selection: null, dragging: { anchor: clampCell(cell, g), moved: false } });
}

export function pointerMove(s: EditState, cell: Cell, g: Grid): Result {
  if (!s.dragging) return done(s);
  const { anchor, moved } = s.dragging;
  const [cx, cy] = clampCell(cell, g);
  if (!moved && cx === anchor[0] && cy === anchor[1]) return done(s);
  return done({ ...s, dragging: { anchor, moved: true }, selection: rectBetween(anchor, cell, g) });
}

export function pointerUp(s: EditState): Result {
  return done({ ...s, dragging: null, stroke: null });
}

export function flip(s: EditState, axis: FlipAxis): Result {
  if (s.placing) {
    const p = s.placing;
    return done({ ...s, placing: axis === "h" ? { ...p, flipH: !p.flipH } : { ...p, flipV: !p.flipV } });
  }
  return s.selection ? done(s, { kind: "flipSelection", rect: s.selection, axis }) : done(s);
}

/** Delete and Backspace: cancel placing, else kill the selection, else clear the universe. */
export function deleteKey(s: EditState): Result {
  if (s.placing) return done({ ...s, ...endPlacing(s) });
  return s.selection ? done(s, { kind: "deleteSelection", rect: s.selection }) : done(s, { kind: "clear" });
}

/** Ends placing first, then the selection. */
export function escape(s: EditState): Result {
  if (s.placing) return done({ ...s, ...endPlacing(s) });
  return done({ ...s, selection: null, dragging: null });
}

/** The unmodified keys of the editing tools; null when the key is not one of them. */
export function key(s: EditState, k: string): Result | null {
  switch (k) {
    case "s":
    case "S":
      return setTool(s, "select");
    case "d":
    case "D":
      return setTool(s, "draw");
    case "h":
    case "H":
      return flip(s, "h");
    case "v":
    case "V":
      return flip(s, "v");
    case "Escape":
      return escape(s);
    case "Delete":
    case "Backspace":
      return deleteKey(s);
    default:
      return null;
  }
}
