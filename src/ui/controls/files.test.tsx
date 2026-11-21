import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { afterEach, beforeEach, expect, test } from "vitest";
import type { FrameHeader } from "../frame";
import { LibraryPanel } from "../library/LibraryPanel";
import { initialEdit } from "../edit/tools";
import { getState, resetUiState, setEdit, setHeader } from "../state";
import { Toolbar } from "./Toolbar";
import { handleMenu, MENU_ACTIONS, useMenu, windowTitle } from "./menu";

const header = (over: Partial<FrameHeader> = {}): FrameHeader => ({
  version: 1, playing: false, snapshotVersion: 1, generation: 0, liveCount: 0, gensPerSec: 0,
  gridWidth: 100, gridHeight: 50, edges: "wrap", x0: 0, y0: 0, w: 0, h: 0, layerCount: 0,
  canUndo: false, canRedo: false, dirty: false, tiling: "square", rule: "B3/S23", ...over,
});

let calls: { cmd: string; args: any }[];
let handlers: Record<string, (args: any) => unknown>;
beforeEach(() => {
  calls = [];
  handlers = { library_list: () => ({ builtin: [], personal: [], problems: [] }), confirm_discard: () => true };
  resetUiState();
  setHeader(header({ canUndo: true, canRedo: true }));
  mockIPC((cmd, args) => {
    calls.push({ cmd, args });
    return handlers[cmd]?.(args);
  }, { shouldMockEvents: true });
});
afterEach(() => {
  cleanup();
  clearMocks();
});

const sent = () => calls.map((c) => c.cmd);
const select = () => act(() => setEdit({ ...getState().edit, selection: { x: 2, y: 3, w: 4, h: 5 } }));

function Menu() {
  useMenu();
  return <input aria-label="field" />;
}

const EXPECTED: Record<string, string> = {
  new: "confirm_discard",
  open: "confirm_discard",
  save: "save",
  save_as: "save_as",
  import_rle: "import_rle",
  export_rle: "export_rle",
  undo: "undo",
  redo: "redo",
  cut: "cut_selection",
  copy: "copy_selection",
  paste: "paste",
  delete: "delete_selection",
  flip_h: "flip_selection",
  flip_v: "flip_selection",
};

test("each menu id calls its action", async () => {
  expect(Object.keys(MENU_ACTIONS).sort()).toEqual([...Object.keys(EXPECTED), "select_all"].sort());
  for (const [id, cmd] of Object.entries(EXPECTED)) {
    calls.length = 0;
    act(() => setEdit(initialEdit()));
    select();
    handleMenu(id);
    await waitFor(() => expect(sent()).toContain(cmd), { timeout: 500 });
  }
  select();
  act(() => getState().edit.selection && handleMenu("select_all"));
  expect(getState().edit.selection).toEqual({ x: 0, y: 0, w: 100, h: 50 });
});

test("a menu event from the backend reaches its action", async () => {
  render(<Menu />);
  await act(async () => {
    await new Promise((r) => setTimeout(r, 0));
    await emit("menu", "save");
  });
  await waitFor(() => expect(sent()).toContain("save"));
});

test("focus in a text field sends set_edit_target text, blur sends universe", async () => {
  render(<><Menu /><LibraryPanel /></>);
  const input = screen.getByLabelText("Filter patterns");
  act(() => input.focus());
  await waitFor(() => expect(calls.at(-1)).toEqual({ cmd: "set_edit_target", args: { target: "text" } }));
  act(() => input.blur());
  await waitFor(() => expect(calls.at(-1)).toEqual({ cmd: "set_edit_target", args: { target: "universe" } }));
});

test("menu state follows the flags and the selection", async () => {
  render(<Menu />);
  await waitFor(() => expect(calls.at(-1)!.cmd).toBe("set_menu_state"));
  expect(calls.at(-1)!.args).toEqual({ canUndo: true, canRedo: true, hasSelection: false, playing: false, tiling: "square" });
  select();
  await waitFor(() => expect(calls.at(-1)!.args).toMatchObject({ hasSelection: true }));
  act(() => setHeader(header({ playing: true })));
  await waitFor(() => expect(calls.at(-1)!.args).toMatchObject({ playing: true, canUndo: false }));
});

test("the window title follows the file name and the dirty flag", () => {
  expect(windowTitle(null, false)).toBe("Untitled — glife");
  expect(windowTitle("a.glife", false)).toBe("a.glife — glife");
  expect(windowTitle("a.glife", true)).toBe("• a.glife — glife");
});

test("cancelling the discard prompt keeps the New dialog closed", async () => {
  handlers.confirm_discard = () => false;
  render(<Toolbar />);
  fireEvent.click(screen.getByRole("button", { name: "New universe…" }));
  await waitFor(() => expect(sent()).toContain("confirm_discard"));
  expect(screen.queryByRole("dialog")).toBeNull();
});

test("confirming the discard prompt opens the New dialog", async () => {
  render(<Toolbar />);
  fireEvent.click(screen.getByRole("button", { name: "New universe…" }));
  expect(await screen.findByRole("dialog", { name: "New universe" })).toBeTruthy();
});

test("Open sets the file name, ends the selection and shows the notices; cancel changes nothing", async () => {
  select();
  handlers.open = () => ({ fileName: "a.glife", notices: ["Rule loaded as B3/S23."] });
  handleMenu("open");
  await waitFor(() => expect(getState().fileName).toBe("a.glife"));
  expect(getState().edit.selection).toBeNull();
  expect(getState().notice).toBe("Rule loaded as B3/S23.");
  handlers.open = () => null;
  select();
  handleMenu("open");
  await waitFor(() => expect(sent().filter((c) => c === "open").length).toBe(2));
  expect(getState().fileName).toBe("a.glife");
  expect(getState().edit.selection).not.toBeNull();
});

test("Open does not ask for a file when the discard prompt is cancelled", async () => {
  handlers.confirm_discard = () => false;
  handleMenu("open");
  await waitFor(() => expect(sent()).toEqual(["confirm_discard"]));
});

test("Save and Save As take the file name; a cancelled dialog keeps it", async () => {
  handlers.save = () => "x.glife";
  handleMenu("save");
  await waitFor(() => expect(getState().fileName).toBe("x.glife"));
  handlers.save_as = () => null;
  handleMenu("save_as");
  await waitFor(() => expect(sent()).toContain("save_as"));
  expect(getState().fileName).toBe("x.glife");
});

test("Import RLE starts placing and shows notices", async () => {
  handlers.import_rle = () => ({ width: 3, height: 3, name: "x", comment: "", notices: ["Rule differs."] });
  handlers.pending_cells = () => new Uint32Array([3, 3, 2, 4, 7]).buffer;
  handleMenu("import_rle");
  await waitFor(() => expect(getState().edit.placing).not.toBeNull());
  expect(getState().notice).toBe("Rule differs.");
});

test("Export RLE sends the selection if any, else the universe", async () => {
  handleMenu("export_rle");
  await waitFor(() => expect(calls.at(-1)).toEqual({ cmd: "export_rle", args: { source: "universe" } }));
  select();
  handleMenu("export_rle");
  await waitFor(() => expect(JSON.stringify(calls.at(-1)!.args)).toContain('"selection"'));
});
