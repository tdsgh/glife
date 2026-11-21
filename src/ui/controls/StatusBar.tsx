import { useUiState } from "../state";

export function StatusBar() {
  const { header: h, tilings } = useUiState();
  const items = h
    ? [
        `Generation ${h.generation}`,
        `Live ${h.liveCount}`,
        ...(h.playing ? [`${h.gensPerSec.toFixed(0)} gen/s`] : []),
        [tilings.find((t) => t.id === h.tiling)?.name ?? h.tiling, `${h.gridWidth} × ${h.gridHeight}`, h.edges === "wrap" ? "Wrap" : "Dead", h.rule]
          .filter(Boolean)
          .join(" · "),
      ]
    : [];
  return (
    <div role="status" style={{ display: "flex", gap: 16, padding: "4px 8px", font: "12px monospace", background: "#1b1f26", color: "#e8f1ff" }}>
      {items.map((t) => <span key={t}>{t}</span>)}
    </div>
  );
}
