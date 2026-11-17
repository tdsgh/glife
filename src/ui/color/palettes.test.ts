import { expect, test } from "vitest";
import { PALETTE_SIZES, PALETTES, paletteColors, type PaletteId } from "./palettes";

const ids = Object.keys(PALETTES) as PaletteId[];
const rgb = (hex: string) => [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));

test("every palette at every size has exactly that many entries", () => {
  for (const id of ids) for (const size of PALETTE_SIZES) expect(paletteColors(id, size).length).toBe(size * 4);
});

test("first and last entries equal the first and last stops, fully opaque", () => {
  for (const id of ids) {
    for (const size of PALETTE_SIZES) {
      const c = paletteColors(id, size);
      const { stops } = PALETTES[id];
      expect(Array.from(c.subarray(0, 4))).toEqual([...rgb(stops[0]), 255]);
      expect(Array.from(c.subarray(c.length - 4))).toEqual([...rgb(stops[stops.length - 1]), 255]);
    }
  }
});
