export interface Viewport {
  centerX: number;
  centerY: number;
  pxPerCell: number;
  /** Window size in CSS pixels. */
  width: number;
  height: number;
  gridW: number;
  gridH: number;
}

export const MAX_PX_PER_CELL = 64;

const fitScale = (v: Viewport) => Math.min(v.width / v.gridW, v.height / v.gridH);

/** Keeps zoom in [fit, 64] and the centre inside the grid, so no grid edge is more than half a window past the view's edge. */
function clamp(v: Viewport): Viewport {
  const fitPx = fitScale(v);
  const pxPerCell = Math.min(Math.max(v.pxPerCell, fitPx), Math.max(MAX_PX_PER_CELL, fitPx));
  return {
    ...v,
    pxPerCell,
    centerX: Math.min(Math.max(v.centerX, 0), v.gridW),
    centerY: Math.min(Math.max(v.centerY, 0), v.gridH),
  };
}

export function fit(width: number, height: number, gridW: number, gridH: number): Viewport {
  const v = { centerX: gridW / 2, centerY: gridH / 2, pxPerCell: 1, width, height, gridW, gridH };
  return { ...v, pxPerCell: fitScale(v) };
}

export function screenToCell(v: Viewport, sx: number, sy: number): { x: number; y: number } {
  return {
    x: v.centerX + (sx - v.width / 2) / v.pxPerCell,
    y: v.centerY + (sy - v.height / 2) / v.pxPerCell,
  };
}

/** Zooms by `factor`, keeping the cell under the screen point fixed. */
export function zoomAt(v: Viewport, sx: number, sy: number, factor: number): Viewport {
  const at = screenToCell(v, sx, sy);
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
  const left = Math.max(0, Math.floor(v.centerX - halfW));
  const right = Math.min(v.gridW, Math.ceil(v.centerX + halfW));
  const top = Math.max(0, Math.floor(v.centerY - halfH));
  const bottom = Math.min(v.gridH, Math.ceil(v.centerY + halfH));
  const x0 = Math.floor(left / 32) * 32;
  const x1 = Math.ceil(right / 32) * 32;
  return { x0, y0: top, w: x1 - x0, h: Math.max(0, bottom - top) };
}
