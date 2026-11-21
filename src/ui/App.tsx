import { Legend } from "./color/Legend";
import { Notice } from "./controls/Notice";
import { StatusBar } from "./controls/StatusBar";
import { Toolbar } from "./controls/Toolbar";
import { useMenu, useWindowTitle } from "./controls/menu";
import { LibraryPanel } from "./library/LibraryPanel";
import { useEffect } from "react";
import { tilings } from "./backend";
import { setTilings, useUiState } from "./state";
import { useShortcuts } from "./controls/shortcuts";
import { GridView } from "./view/GridView";

export function App() {
  useShortcuts();
  useMenu();
  useWindowTitle();
  useEffect(() => {
    tilings().then(setTilings).catch(() => {});
  }, []);
  const { libraryOpen } = useUiState();
  return (
    <div data-testid="app" style={{ width: "100%", height: "100%", display: "flex", flexDirection: "column" }}>
      <Toolbar />
      <Notice />
      <div style={{ flex: 1, minHeight: 0, display: "flex" }}>
        <div style={{ flex: 1, minWidth: 0, position: "relative" }}>
          <GridView />
          <Legend />
        </div>
        {libraryOpen && <LibraryPanel />}
      </div>
      <StatusBar />
    </div>
  );
}
