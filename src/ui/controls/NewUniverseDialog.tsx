import { useEffect, useState } from "react";
import { newUniverse, type Edges, type TilingId } from "../backend";
import { resetEdit } from "./actions";
import { RuleField } from "./RuleField";
import { requestFit, setFileName, useUiState } from "../state";

const MIN = 3;
const MAX = 2048;

const parseSide = (s: string) => (/^\d+$/.test(s.trim()) && +s >= MIN && +s <= MAX ? +s : null);

export function NewUniverseDialog({ onClose }: { onClose: () => void }) {
  const { tilings } = useUiState();
  const [tilingId, setTilingId] = useState<TilingId>("square");
  const tiling = tilings.find((t) => t.id === tilingId);
  const [width, setWidth] = useState("256");
  const [height, setHeight] = useState("256");
  const [edges, setEdges] = useState<Edges>("wrap");
  const [rule, setRule] = useState(tiling?.defaultRule.text ?? "");
  const [error, setError] = useState<string | null>(null);
  // The grids arrive after the dialog opened: start at the default then.
  useEffect(() => {
    if (tiling && rule === "") setRule(tiling.defaultRule.text);
  }, [tiling]);
  const w = parseSide(width);
  const h = parseSide(height);
  // Wrapping edges need an even side where the grid's rows or triangles repeat.
  const evenW = edges === "wrap" && !!tiling?.wrapEvenWidth;
  const evenH = edges === "wrap" && !!tiling?.wrapEvenHeight;
  const oddW = evenW && w !== null && w % 2 === 1;
  const oddH = evenH && h !== null && h % 2 === 1;
  const valid = w !== null && h !== null && !oddW && !oddH;

  const pickGrid = (id: TilingId) => {
    setTilingId(id);
    setRule(tilings.find((t) => t.id === id)?.defaultRule.text ?? "");
    setError(null);
  };

  const confirm = async () => {
    if (!valid) return;
    try {
      await newUniverse(w, h, edges, tilingId, rule.trim() || undefined);
    } catch (e) {
      setError(String(e));
      return;
    }
    resetEdit();
    setFileName(null);
    requestFit();
    onClose();
  };

  const side = (label: string, value: string, set: (v: string) => void, ok: boolean, even: boolean) => (
    <label style={{ display: "block", margin: "6px 0" }}>
      {label}{" "}
      <input type="text" inputMode="numeric" value={value} aria-label={label} aria-invalid={!ok}
        onChange={(e) => set(e.target.value)} style={{ width: 70 }} />
      {!ok && <span role="alert" style={{ color: "#ff8a80", marginLeft: 6 }}>{MIN}–{MAX}</span>}
      {ok && even && <span style={{ marginLeft: 6, color: "#ff8a80" }}>wrapping needs an even {label.toLowerCase()}</span>}
    </label>
  );

  return (
    <div role="dialog" aria-label="New universe"
      style={{ position: "absolute", top: 48, left: 8, zIndex: 10, padding: 12, background: "#20242c", color: "#e8f1ff", border: "1px solid #4a5260" }}
      onKeyDown={(e) => {
        if (e.key === "Escape") onClose();
        if (e.key === "Enter") void confirm();
      }}>
      <label style={{ display: "block", margin: "6px 0" }}>
        Grid{" "}
        <select aria-label="Grid" value={tilingId} onChange={(e) => pickGrid(e.target.value as TilingId)}>
          {tilings.length === 0 && <option value="square">Square</option>}
          {tilings.map((t) => <option key={t.id} value={t.id}>{t.name}</option>)}
        </select>
      </label>
      {side("Width", width, setWidth, w !== null, oddW)}
      {side("Height", height, setHeight, h !== null, oddH)}
      {(evenW || evenH) && (
        <div style={{ fontSize: 12, color: "#9aa4b5" }}>
          Wrapping needs an even {evenW && evenH ? "width and height" : evenW ? "width" : "height"} on this grid.
        </div>
      )}
      <label style={{ display: "block", margin: "6px 0" }}>
        Edges{" "}
        <select aria-label="Dialog edges" value={edges} onChange={(e) => setEdges(e.target.value as Edges)}>
          <option value="wrap">Wrap</option>
          <option value="dead">Dead</option>
        </select>
      </label>
      <RuleField tiling={tiling} value={rule} onChange={(r) => { setRule(r); setError(null); }} error={error} />
      <button onClick={() => void confirm()} disabled={!valid}>Create</button>{" "}
      <button onClick={onClose}>Cancel</button>
    </div>
  );
}
