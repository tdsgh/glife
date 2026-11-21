import type { TilingInfo } from "../backend";

/** A preset select for the grid plus a text box for the rule; a refused rule's message shows underneath. */
export function RuleField({ tiling, value, onChange, error }: { tiling: TilingInfo | undefined; value: string; onChange: (rule: string) => void; error?: string | null }) {
  const presets = tiling?.presets ?? [];
  const preset = presets.find((p) => p.rule === value.trim());
  return (
    <div style={{ margin: "6px 0" }}>
      <span>
        Rule{" "}
        <select aria-label="Rule preset" value={preset?.name ?? ""} onChange={(e) => {
          const p = presets.find((q) => q.name === e.target.value);
          if (p) onChange(p.rule);
        }}>
          {!preset && <option value="">Custom</option>}
          {presets.map((p) => <option key={p.name} value={p.name}>{p.name} {p.rule}</option>)}
        </select>
      </span>{" "}
      <input type="text" aria-label="Rule" value={value} aria-invalid={!!error} spellCheck={false}
        onChange={(e) => onChange(e.target.value)} style={{ width: 120 }} />
      {error && <div role="alert" style={{ color: "#ff8a80" }}>{error}</div>}
    </div>
  );
}
