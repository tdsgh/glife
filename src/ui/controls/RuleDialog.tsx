import { useState } from "react";
import { setRule } from "../backend";
import { useUiState } from "../state";
import { RuleField } from "./RuleField";

/** Changes the rule of the open universe. A refused rule shows the backend's message and leaves the dialog open. */
export function RuleDialog({ onClose }: { onClose: () => void }) {
  const { header, tilings } = useUiState();
  const [rule, setText] = useState(header?.rule ?? "");
  const [error, setError] = useState<string | null>(null);

  const apply = async () => {
    try {
      await setRule(rule.trim());
    } catch (e) {
      setError(String(e));
      return;
    }
    onClose();
  };

  return (
    <div role="dialog" aria-label="Change rule"
      style={{ position: "absolute", top: 48, left: 8, zIndex: 10, padding: 12, background: "#20242c", color: "#e8f1ff", border: "1px solid #4a5260" }}
      onKeyDown={(e) => {
        if (e.key === "Escape") onClose();
        if (e.key === "Enter") void apply();
      }}>
      <RuleField tiling={tilings.find((t) => t.id === header?.tiling)} value={rule} onChange={(r) => { setText(r); setError(null); }} error={error} />
      <button onClick={() => void apply()}>Apply</button>{" "}
      <button onClick={onClose}>Cancel</button>
    </div>
  );
}
