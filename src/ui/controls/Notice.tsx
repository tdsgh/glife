import { setNotice, useUiState } from "../state";

/** One line for an error or a notice, dismissible. */
export function Notice() {
  const { notice } = useUiState();
  if (notice === null) return null;
  return (
    <div role="alert" style={{ display: "flex", gap: 8, padding: "4px 8px", background: "#3a2f14", color: "#ffe9a8" }}>
      <span style={{ flex: 1 }}>{notice}</span>
      <button aria-label="Dismiss" onClick={() => setNotice(null)}>×</button>
    </div>
  );
}
