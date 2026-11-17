import { useState } from "react";
import { clear, setEdges, setSpeed as sendSpeed } from "../backend";
import { requestFit, setDensity, setSpeed, SPEEDS, useUiState } from "../state";
import { ColorControls } from "../color/ColorControls";
import { NewUniverseDialog } from "./NewUniverseDialog";
import { randomFill, stepOnce, togglePlay } from "./actions";

export function Toolbar() {
  const { header, speed, density } = useUiState();
  const [dialog, setDialog] = useState(false);
  const playing = header?.playing ?? false;
  const edges = header?.edges ?? "wrap";

  return (
    <div role="toolbar" style={{ position: "relative", display: "flex", gap: 8, alignItems: "center", padding: 8, background: "#1b1f26", color: "#e8f1ff" }}>
      <button onClick={() => setDialog(true)}>New universe…</button>
      <button onClick={() => void togglePlay()}>{playing ? "Pause" : "Play"}</button>
      <button onClick={stepOnce} disabled={playing}>Step</button>
      <label>
        Speed{" "}
        <select aria-label="Speed" value={speed ?? "max"}
          onChange={(e) => {
            const v = e.target.value === "max" ? null : Number(e.target.value);
            setSpeed(v);
            void sendSpeed(v);
          }}>
          {SPEEDS.map((s) => (
            <option key={s ?? "max"} value={s ?? "max"}>{s ?? "Max"}</option>
          ))}
        </select>
      </label>
      <button onClick={() => void setEdges(edges === "wrap" ? "dead" : "wrap")}>
        Edges: {edges === "wrap" ? "Wrap" : "Dead"}
      </button>
      <button onClick={() => void randomFill()}>Random fill</button>
      <input type="number" aria-label="Density" min={1} max={99} value={density}
        onChange={(e) => setDensity(e.target.value)} style={{ width: 56 }} />
      <span>%</span>
      <button onClick={() => void clear()}>Clear</button>
      <button onClick={requestFit}>Fit</button>
      <ColorControls />
      {dialog && <NewUniverseDialog onClose={() => setDialog(false)} />}
    </div>
  );
}
