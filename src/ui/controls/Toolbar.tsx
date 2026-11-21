import { useState } from "react";
import { clear, librarySave, setEdges, setSpeed as sendSpeed } from "../backend";
import { bumpLibrary, requestFit, setDensity, setLibraryOpen, setNewDialog, setNotice, setSpeed, SPEEDS, useUiState } from "../state";
import { SavePatternDialog } from "../library/SavePatternDialog";
import { ColorControls } from "../color/ColorControls";
import { NewUniverseDialog } from "./NewUniverseDialog";
import { RuleDialog } from "./RuleDialog";
import { copy, cut, deleteAction, endPlacing, flipH, flipV, newUniverse, paste, randomFill, redo, setPasteMode, setTool, stepOnce, togglePlay, undo } from "./actions";
import { toggleStyle } from "./toggleStyle";

const divider = <div role="separator" aria-orientation="vertical" style={{ alignSelf: "stretch", width: 1, background: "#3a414d" }} />;
const column = { display: "flex", flexDirection: "column", gap: 4 } as const;

export function Toolbar() {
  const { header, speed, density, edit, libraryOpen, newDialog } = useUiState();
  const [saveDialog, setSaveDialog] = useState(false);
  const [ruleDialog, setRuleDialog] = useState(false);
  const playing = header?.playing ?? false;
  const edges = header?.edges ?? "wrap";

  const hasSelection = edit.selection !== null;
  const canFlip = edit.placing !== null || hasSelection;
  const canSave = edit.placing !== null || hasSelection;

  return (
    <div role="toolbar" style={{ position: "relative", display: "flex", flexWrap: "wrap", gap: 12, alignItems: "flex-start", padding: 8, background: "#1b1f26", color: "#e8f1ff" }}>
      <div style={column}>
        <button onClick={() => void newUniverse()}>New universe…</button>
        <button onClick={() => setSaveDialog(true)} disabled={!canSave}>Save to library…</button>
      </div>
      {divider}
      <div style={column}>
        <button onClick={() => void togglePlay()} aria-pressed={playing} style={toggleStyle(playing)}>{playing ? "Pause" : "Play"}</button>
        <button onClick={stepOnce} disabled={playing}>Step</button>
      </div>
      <div style={column}>
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
        <button onClick={() => setEdges(edges === "wrap" ? "dead" : "wrap").catch((e) => setNotice(String(e)))}>
          Edges: {edges === "wrap" ? "Wrap" : "Dead"}
        </button>
        <button onClick={() => setRuleDialog(true)} disabled={!header}>Rule…</button>
      </div>
      {divider}
      <div style={column}>
        <div style={{ display: "flex", gap: 4, alignItems: "center" }}>
          <button onClick={() => void randomFill()}>Random fill</button>
          <input type="number" aria-label="Density" min={1} max={99} value={density}
            onChange={(e) => setDensity(e.target.value)} style={{ width: 56 }} />
          <span>%</span>
        </div>
        <button onClick={() => void clear()}>Clear</button>
      </div>
      {divider}
      <div style={column}>
        <button onClick={() => setTool("draw")} aria-pressed={!edit.placing && edit.tool === "draw"} style={toggleStyle(!edit.placing && edit.tool === "draw")}>Draw</button>
        <button onClick={() => setTool("select")} aria-pressed={!edit.placing && edit.tool === "select"} style={toggleStyle(!edit.placing && edit.tool === "select")}>Select</button>
      </div>
      <div style={column}>
        <button onClick={undo} disabled={playing || !header?.canUndo}>Undo</button>
        <button onClick={redo} disabled={playing || !header?.canRedo}>Redo</button>
      </div>
      {divider}
      <div style={column}>
        <button onClick={cut} disabled={!hasSelection}>Cut</button>
        <button onClick={copy} disabled={!hasSelection}>Copy</button>
      </div>
      <div style={column}>
        <button onClick={() => (edit.placing ? endPlacing() : void paste())} aria-pressed={edit.placing !== null} style={toggleStyle(edit.placing !== null)}>Paste</button>
        <button onClick={deleteAction} disabled={!hasSelection}>Delete</button>
      </div>
      <div style={column}>
        <button onClick={flipH} disabled={!canFlip}>Flip H</button>
        <button onClick={flipV} disabled={!canFlip}>Flip V</button>
      </div>
      {divider}
      <div style={column}>
        <div role="group" aria-label="Paste mode" style={{ display: "flex" }}>
          {(["or", "copy"] as const).map((m) => (
            <button key={m} aria-label={`Paste mode ${m === "or" ? "Or" : "Copy"}`} aria-pressed={edit.mode === m}
              disabled={!edit.placing} onClick={() => setPasteMode(m)}
              style={{ flex: 1, ...toggleStyle(edit.mode === m), ...(m === "or" ? { borderRadius: "4px 0 0 4px" } : { borderRadius: "0 4px 4px 0", marginLeft: -1 }) }}>
              {m === "or" ? "Or" : "Copy"}
            </button>
          ))}
        </div>
        <button onClick={() => setLibraryOpen(!libraryOpen)} aria-pressed={libraryOpen} style={toggleStyle(libraryOpen)}>Library</button>
      </div>
      {divider}
      <div style={column}>
        <button onClick={requestFit}>Fit</button>
      </div>
      {divider}
      <ColorControls />
      {ruleDialog && <RuleDialog onClose={() => setRuleDialog(false)} />}
      {newDialog && <NewUniverseDialog onClose={() => setNewDialog(false)} />}
      {saveDialog && (
        <SavePatternDialog title="Save to library" fields={{ name: true, comment: true }} onClose={() => setSaveDialog(false)}
          submit={async (name, comment, replace) => {
            const reply = await librarySave(edit.placing ? "pending" : { selection: edit.selection! }, name, comment || undefined, replace);
            if (!reply || !reply.nameTaken) bumpLibrary();
            return reply;
          }} />
      )}
    </div>
  );
}
