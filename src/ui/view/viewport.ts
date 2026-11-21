import type { TilingId } from "../backend";
import { cellRange, worldSize } from "./geometry";

/** The camera, in world units (see geometry.ts): a square or hexagon is 1 wide. */
export interface Viewport {
  centerX: number;
  centerY: number;
  /** Pixels per world unit. */
  pxPerCell: number;
  /** Window size in CSS pixels. */
  width: number;
  height: number;
  /** Grid size in cells. */
  gridW: number;
  gridH: number;
  tiling: TilingId;
}

export const MAX_PX_PER_CELL = 64;

const world = (v: Viewport) => worldSize(v.tiling, v.gridW, v.gridH);

const fitScale = (v: Viewport) => {
  const [ww, wh] = world(v);
  return Math.min(v.width / ww, v.height / wh);
};

/** Keeps zoom in [fit, 64] and the centre inside the grid, so no grid edge is more than half a window past the view's edge. */
function clamp(v: Viewport): Viewport {
  const fitPx = fitScale(v);
  const [ww, wh] = world(v);
  const pxPerCell = Math.min(Math.max(v.pxPerCell, fitPx), Math.max(MAX_PX_PER_CELL, fitPx));
  return {
    ...v,
    pxPerCell,
    centerX: Math.min(Math.max(v.centerX, 0), ww),
    centerY: Math.min(Math.max(v.centerY, 0), wh),
  };
}

export function fit(width: number, height: number, gridW: number, gridH: number, tiling: TilingId = "square"): Viewport {
  const [ww, wh] = worldSize(tiling, gridW, gridH);
  const v = { centerX: ww / 2, centerY: wh / 2, pxPerCell: 1, width, height, gridW, gridH, tiling };
  return { ...v, pxPerCell: fitScale(v) };
}

/** The world point under a screen point. */
export function screenToWorld(v: Viewport, sx: number, sy: number): { x: number; y: number } {
  return {
    x: v.centerX + (sx - v.width / 2) / v.pxPerCell,
    y: v.centerY + (sy - v.height / 2) / v.pxPerCell,
  };
}

/** Zooms by `factor`, keeping the cell under the screen point fixed. */
export function zoomAt(v: Viewport, sx: number, sy: number, factor: number): Viewport {
  const at = screenToWorld(v, sx, sy);
  const next = clamp({ ...v, pxPerCell: v.pxPerCell * factor });
  return clamp({
    ...next,
    centerX: at.x - (sx - v.width / 2) / next.pxPerCell,
    centerY: at.y - (sy - v.height / 2) / next.pxPerCell,
  });
}

/** Moves the camera by a screen-pixel delta. */
export function panBy(v: Viewport, dxPx: number, dyPx: number): Viewport {
  return clamp({ ...v, centerX: v.centerX + dxPx / v.pxPerCell, centerY: v.centerY + dyPx / v.pxPerCell });
}

export function resize(v: Viewport, width: number, height: number): Viewport {
  return clamp({ ...v, width, height });
}

/** Cells in view, clipped to the grid; x0 and w aligned to 32 columns (w may reach the padded row width). */
export function visibleRegion(v: Viewport): { x0: number; y0: number; w: number; h: number } {
  const halfW = v.width / 2 / v.pxPerCell;
  const halfH = v.height / 2 / v.pxPerCell;
  const { left, right, top, bottom } = cellRange(v.tiling, v.gridW, v.gridH, v.centerX - halfW, v.centerY - halfH, v.centerX + halfW, v.centerY + halfH);
  const x0 = Math.floor(left / 32) * 32;
  const x1 = Math.ceil(right / 32) * 32;
  return { x0, y0: top, w: x1 - x0, h: Math.max(0, bottom - top) };
}
