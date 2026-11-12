import { useUiState } from "../state";

export function StatusBar() {
  const { header: h } = useUiState();
  const items = h
    ? [
        `Generation ${h.generation}`,
        `Live ${h.liveCount}`,
        ...(h.playing ? [`${h.gensPerSec.toFixed(0)} gen/s`] : []),
        `${h.gridWidth} × ${h.gridHeight}`,
        h.edges === "wrap" ? "Wrap" : "Dead",
        "B3/S23",
      ]
    : ["B3/S23"];
  return (
    <div role="status" style={{ display: "flex", gap: 16, padding: "4px 8px", font: "12px monospace", background: "#1b1f26", color: "#e8f1ff" }}>
      {items.map((t) => <span key={t}>{t}</span>)}
    </div>
  );
}
