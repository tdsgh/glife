import { expect, test } from "vitest";
import { PALETTE_SIZES, PALETTES, paletteColors, type PaletteId } from "./palettes";

const ids = Object.keys(PALETTES) as PaletteId[];
const rgb = (hex: string) => [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));

test("every palette at every size has exactly that many entries", () => {
  for (const id of ids) for (const size of PALETTE_SIZES) expect(paletteColors(id, size).length).toBe(size * 4);
});

test("entry 0 is white and entry 1 equals the first stop, at every size", () => {
  for (const id of ids) {
    for (const size of PALETTE_SIZES) {
      const c = paletteColors(id, size);
      expect(Array.from(c.subarray(0, 4))).toEqual([255, 255, 255, 255]);
      expect(Array.from(c.subarray(4, 8))).toEqual([...rgb(PALETTES[id].stops[0]), 255]);
    }
  }
});

test("the last entry equals the last stop, fully opaque", () => {
  for (const id of ids) {
    for (const size of PALETTE_SIZES) {
      const c = paletteColors(id, size);
      const { stops } = PALETTES[id];
      expect(Array.from(c.subarray(c.length - 4))).toEqual([...rgb(stops[stops.length - 1]), 255]);
    }
  }
});
