import { useState } from "react";
import { clear, librarySave, setEdges, setSpeed as sendSpeed } from "../backend";
import { bumpLibrary, requestFit, setDensity, setLibraryOpen, setNewDialog, setSpeed, SPEEDS, useUiState } from "../state";
import { SavePatternDialog } from "../library/SavePatternDialog";
import { ColorControls } from "../color/ColorControls";
import { NewUniverseDialog } from "./NewUniverseDialog";
import { flipH, flipV, newUniverse, randomFill, redo, setPasteMode, setTool, stepOnce, togglePlay, undo } from "./actions";

export function Toolbar() {
  const { header, speed, density, edit, libraryOpen, newDialog } = useUiState();
  const [saveDialog, setSaveDialog] = useState(false);
  const playing = header?.playing ?? false;
  const edges = header?.edges ?? "wrap";

  return (
    <div role="toolbar" style={{ position: "relative", display: "flex", gap: 8, alignItems: "center", padding: 8, background: "#1b1f26", color: "#e8f1ff" }}>
      <button onClick={() => void newUniverse()}>New universe…</button>
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
      <button onClick={() => setTool("draw")} aria-pressed={!edit.placing && edit.tool === "draw"}>Draw</button>
      <button onClick={() => setTool("select")} aria-pressed={!edit.placing && edit.tool === "select"}>Select</button>
      <button onClick={undo} disabled={playing || !header?.canUndo}>Undo</button>
      <button onClick={redo} disabled={playing || !header?.canRedo}>Redo</button>
      <button onClick={flipH} disabled={!edit.placing && !edit.selection}>Flip H</button>
      <button onClick={flipV} disabled={!edit.placing && !edit.selection}>Flip V</button>
      {edit.placing && (
        <>
          <button onClick={() => setPasteMode("or")} aria-pressed={edit.mode === "or"}>Or</button>
          <button onClick={() => setPasteMode("copy")} aria-pressed={edit.mode === "copy"}>Copy</button>
        </>
      )}
      <button onClick={() => setSaveDialog(true)} disabled={!edit.placing && !edit.selection}>Save to library…</button>
      <button onClick={() => setLibraryOpen(!libraryOpen)} aria-pressed={libraryOpen}>Library</button>
      <button onClick={requestFit}>Fit</button>
      <ColorControls />
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
