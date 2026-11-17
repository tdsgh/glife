export type PaletteId = "viridis" | "heat" | "ice";
export type PaletteSize = 16 | 32 | 256;

export const PALETTE_SIZES: PaletteSize[] = [16, 32, 256];

/** Stops are sRGB hex; each palette runs from the youngest to the oldest cell. */
export const PALETTES: Record<PaletteId, { label: string; stops: string[] }> = {
  // Safe for common color-vision deficiencies; the darkest end of viridis is trimmed.
  viridis: { label: "Viridis", stops: ["#472d7b", "#3b528b", "#21918c", "#5ec962", "#fde725"] },
  heat: { label: "Heat", stops: ["#ffffff", "#ffe600", "#ff8c00", "#e8281c"] },
  ice: { label: "Ice", stops: ["#ffffff", "#00e5ff", "#1e6bff", "#8a3ffc"] },
};

type Lab = [number, number, number];

const toLinear = (c: number) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
const toSrgb = (c: number) => (c <= 0.0031308 ? c * 12.92 : 1.055 * c ** (1 / 2.4) - 0.055);

function hexToLab(hex: string): Lab {
  const [r, g, b] = [1, 3, 5].map((i) => toLinear(parseInt(hex.slice(i, i + 2), 16) / 255));
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  return [
    0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  ];
}

function labToBytes([L, a, b]: Lab): [number, number, number] {
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
  const lin = [
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
  ];
  return lin.map((c) => Math.round(255 * Math.min(1, Math.max(0, toSrgb(c))))) as [number, number, number];
}

/** RGBA bytes, `size` entries, the stops interpolated in OKLab. */
export function paletteColors(id: PaletteId, size: PaletteSize): Uint8Array {
  const stops = PALETTES[id].stops.map(hexToLab);
  const out = new Uint8Array(size * 4);
  for (let i = 0; i < size; i++) {
    const t = (i / (size - 1)) * (stops.length - 1);
    const k = Math.min(Math.floor(t), stops.length - 2);
    const f = t - k;
    const lab = stops[k].map((v, j) => v + (stops[k + 1][j] - v) * f) as Lab;
    out.set([...labToBytes(lab), 255], i * 4);
  }
  return out;
}
