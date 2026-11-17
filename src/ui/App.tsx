import { Legend } from "./color/Legend";
import { Notice } from "./controls/Notice";
import { StatusBar } from "./controls/StatusBar";
import { Toolbar } from "./controls/Toolbar";
import { useShortcuts } from "./controls/shortcuts";
import { GridView } from "./view/GridView";

export function App() {
  useShortcuts();
  return (
    <div data-testid="app" style={{ width: "100%", height: "100%", display: "flex", flexDirection: "column" }}>
      <Toolbar />
      <Notice />
      <div style={{ flex: 1, minHeight: 0, position: "relative" }}>
        <GridView />
        <Legend />
      </div>
      <StatusBar />
    </div>
  );
}
