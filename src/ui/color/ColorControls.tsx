import { PALETTE_SIZES, PALETTES, type PaletteId, type PaletteSize } from "./palettes";
import { toggleStyle } from "../controls/toggleStyle";
import { setSettings, useUiState } from "../state";

export function ColorControls() {
  const { settings } = useUiState();
  const age = settings.coloring === "age";
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
      <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
      <label>
        Coloring{" "}
        <select aria-label="Coloring" value={settings.coloring}
          onChange={(e) => setSettings({ coloring: e.target.value as "plain" | "age" })}>
          <option value="plain">Plain</option>
          <option value="age">Age</option>
        </select>
      </label>
      <select aria-label="Palette" value={settings.palette}
        onChange={(e) => setSettings({ palette: e.target.value as PaletteId })}>
        {(Object.keys(PALETTES) as PaletteId[]).map((id) => (
          <option key={id} value={id}>{PALETTES[id].label}</option>
        ))}
      </select>
      <select aria-label="Palette size" value={settings.paletteSize}
        onChange={(e) => setSettings({ paletteSize: Number(e.target.value) as PaletteSize })}>
        {PALETTE_SIZES.map((n) => (
          <option key={n} value={n}>{n}</option>
        ))}
      </select>
      </div>
      <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
      <button aria-pressed={settings.trail} style={toggleStyle(settings.trail)} disabled={!age} onClick={() => setSettings({ trail: !settings.trail })}>
        Trail: {settings.trail ? "On" : "Off"}
      </button>
      <input type="number" aria-label="Trail length" min={1} max={100} value={settings.trailLength}
        disabled={!age || !settings.trail} style={{ width: 56 }}
        onChange={(e) => {
          const n = Math.round(Number(e.target.value));
          if (!Number.isNaN(n)) setSettings({ trailLength: Math.min(100, Math.max(1, n)) });
        }} />
      </div>
    </div>
  );
}
