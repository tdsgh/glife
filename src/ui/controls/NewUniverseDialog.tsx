import { useState } from "react";
import { newUniverse, type Edges } from "../backend";
import { resetEdit } from "./actions";
import { requestFit } from "../state";

const MIN = 3;
const MAX = 2048;

const parseSide = (s: string) => (/^\d+$/.test(s.trim()) && +s >= MIN && +s <= MAX ? +s : null);

export function NewUniverseDialog({ onClose }: { onClose: () => void }) {
  const [width, setWidth] = useState("256");
  const [height, setHeight] = useState("256");
  const [edges, setEdges] = useState<Edges>("wrap");
  const w = parseSide(width);
  const h = parseSide(height);

  const confirm = async () => {
    if (w === null || h === null) return;
    await newUniverse(w, h, edges);
    resetEdit();
    requestFit();
    onClose();
  };

  const side = (label: string, value: string, set: (v: string) => void, ok: boolean) => (
    <label style={{ display: "block", margin: "6px 0" }}>
      {label}{" "}
      <input type="text" inputMode="numeric" value={value} aria-label={label} aria-invalid={!ok}
        onChange={(e) => set(e.target.value)} style={{ width: 70 }} />
      {!ok && <span role="alert" style={{ color: "#ff8a80", marginLeft: 6 }}>{MIN}–{MAX}</span>}
    </label>
  );

  return (
    <div role="dialog" aria-label="New universe"
      style={{ position: "absolute", top: 48, left: 8, zIndex: 10, padding: 12, background: "#20242c", color: "#e8f1ff", border: "1px solid #4a5260" }}
      onKeyDown={(e) => {
        if (e.key === "Escape") onClose();
        if (e.key === "Enter") void confirm();
      }}>
      {side("Width", width, setWidth, w !== null)}
      {side("Height", height, setHeight, h !== null)}
      <label style={{ display: "block", margin: "6px 0" }}>
        Edges{" "}
        <select aria-label="Dialog edges" value={edges} onChange={(e) => setEdges(e.target.value as Edges)}>
          <option value="wrap">Wrap</option>
          <option value="dead">Dead</option>
        </select>
      </label>
      <button onClick={() => void confirm()} disabled={w === null || h === null}>Create</button>{" "}
      <button onClick={onClose}>Cancel</button>
    </div>
  );
}
