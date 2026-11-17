import { paletteColors } from "./palettes";
import { useUiState } from "../state";

/** The palette strip with age labels, shown in age coloring. */
export function Legend() {
  const { settings } = useUiState();
  if (settings.coloring !== "age") return null;
  const n = settings.paletteSize;
  const bytes = paletteColors(settings.palette, settings.paletteSize);
  const stops = Array.from({ length: n }, (_, i) => {
    const [r, g, b] = bytes.subarray(i * 4, i * 4 + 3);
    return `rgb(${r},${g},${b}) ${(i / n) * 100}% ${((i + 1) / n) * 100}%`;
  });
  return (
    <div data-testid="legend" style={{ position: "absolute", right: 8, bottom: 8, width: 180, padding: 6, font: "11px monospace", color: "#e8f1ff", background: "rgba(0,0,0,0.6)", pointerEvents: "none" }}>
      <div style={{ height: 12, background: `linear-gradient(to right, ${stops.join(", ")})` }} />
      <div style={{ display: "flex", justifyContent: "space-between" }}>
        <span>0</span>
        <span>{Math.floor(n / 2)}</span>
        <span>{n - 1}+</span>
      </div>
    </div>
  );
}
