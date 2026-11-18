import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, beforeEach, expect, test } from "vitest";
import * as tools from "../edit/tools";
import type { FrameHeader } from "../frame";
import { getState, resetUiState, setEdit, setHeader } from "../state";
import { Notice } from "./Notice";
import { Toolbar } from "./Toolbar";
import { paste } from "./actions";
import { handleShortcut } from "./shortcuts";

const header = (over: Partial<FrameHeader> = {}): FrameHeader => ({
  version: 1, playing: false, snapshotVersion: 1, generation: 0, liveCount: 0, gensPerSec: 0,
  gridWidth: 100, gridHeight: 50, edges: "wrap", x0: 0, y0: 0, w: 0, h: 0, layerCount: 0,
  canUndo: false, canRedo: false, dirty: false, ...over,
});

let calls: { cmd: string; args: unknown }[];
let handlers: Record<string, () => unknown>;
beforeEach(() => {
  calls = [];
  handlers = {};
  resetUiState();
  mockIPC((cmd, args) => {
    calls.push({ cmd, args });
    return handlers[cmd]?.();
  });
});
afterEach(() => {
  cleanup();
  clearMocks();
});

const sent = () => calls.map((c) => c.cmd);
const press = (k: string) => handleShortcut(new KeyboardEvent("keydown", { key: k, cancelable: true }));
const withSelection = () => act(() => setEdit({ ...getState().edit, selection: { x: 2, y: 3, w: 4, h: 5 } }));
const withPlacing = () =>
  act(() => setEdit(tools.startPlacing(getState().edit, { width: 3, height: 3, bits: new Uint32Array(3) }).state));

test("Delete and Backspace send delete_selection with a selection and clear without one", async () => {
  press("Delete");
  await waitFor(() => expect(sent()).toEqual(["clear"]));
  withSelection();
  press("Backspace");
  await waitFor(() => expect(calls[1]).toEqual({ cmd: "delete_selection", args: { x: 2, y: 3, w: 4, h: 5 } }));
});

test("H and V flip the placement while placing and the selection otherwise", async () => {
  withSelection();
  press("h");
  await waitFor(() => expect(calls[0]).toEqual({ cmd: "flip_selection", args: { x: 2, y: 3, w: 4, h: 5, axis: "h" } }));
  withPlacing();
  press("v");
  expect(getState().edit.placing!.flipV).toBe(true);
  expect(sent()).toEqual(["flip_selection"]);
});

test("S and D switch tools, Escape ends placing then the selection", () => {
  press("s");
  expect(getState().edit.tool).toBe("select");
  withSelection();
  withPlacing();
  press("Escape");
  expect(getState().edit.placing).toBeNull();
  expect(getState().edit.selection).not.toBeNull();
  expect(getState().edit.tool).toBe("select");
  press("Escape");
  expect(getState().edit.selection).toBeNull();
  press("d");
  expect(getState().edit.tool).toBe("draw");
});

test("undo and redo buttons follow the header flags and are disabled while playing", async () => {
  render(<Toolbar />);
  const undoBtn = screen.getByRole("button", { name: "Undo" }) as HTMLButtonElement;
  const redoBtn = screen.getByRole("button", { name: "Redo" }) as HTMLButtonElement;
  expect([undoBtn.disabled, redoBtn.disabled]).toEqual([true, true]);
  act(() => setHeader(header({ canUndo: true })));
  expect([undoBtn.disabled, redoBtn.disabled]).toEqual([false, true]);
  act(() => setHeader(header({ canUndo: true, canRedo: true })));
  fireEvent.click(undoBtn);
  fireEvent.click(redoBtn);
  await waitFor(() => expect(sent()).toEqual(["undo", "redo"]));
  act(() => setHeader(header({ canUndo: true, canRedo: true, playing: true })));
  expect([undoBtn.disabled, redoBtn.disabled]).toEqual([true, true]);
});

test("the Or/Copy switch is always shown, enabled while placing, and keeps its choice", () => {
  render(<Toolbar />);
  const copyMode = () => screen.getByRole("button", { name: "Paste mode Copy" }) as HTMLButtonElement;
  expect(copyMode().disabled).toBe(true);
  withPlacing();
  expect(copyMode().disabled).toBe(false);
  fireEvent.click(copyMode());
  expect(copyMode().getAttribute("aria-pressed")).toBe("true");
  expect(getState().edit.mode).toBe("copy");
  act(() => setEdit(tools.escape(getState().edit).state));
  withPlacing();
  expect(getState().edit.mode).toBe("copy");
});

test("a paste error shows a notice and does not start placing", async () => {
  handlers.paste = () => {
    throw "no pattern in the clipboard";
  };
  render(<Notice />);
  await act(() => paste());
  expect(screen.getByRole("alert").textContent).toContain("no pattern in the clipboard");
  expect(getState().edit.placing).toBeNull();
  expect(sent()).toEqual(["paste"]);
  fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
  expect(screen.queryByRole("alert")).toBeNull();
});

test("a paste starts placing from the pending cells, with notices shown", async () => {
  handlers.paste = () => ({ width: 3, height: 2, name: "", comment: "", notices: ["rule differs"] });
  handlers.pending_cells = () => new Uint32Array([3, 2, 0b101, 0b010]).buffer;
  render(<Notice />);
  await act(() => paste());
  expect(sent()).toEqual(["paste", "pending_cells"]);
  expect(getState().edit.placing).toMatchObject({ width: 3, height: 2 });
  expect(screen.getByRole("alert").textContent).toContain("rule differs");
});

test("copy and cut send the selection rect", async () => {
  const { copy, cut } = await import("./actions");
  copy();
  expect(calls).toEqual([]);
  withSelection();
  copy();
  cut();
  await waitFor(() => expect(sent()).toEqual(["copy_selection", "cut_selection"]));
  expect(calls[0].args).toEqual({ x: 2, y: 3, w: 4, h: 5 });
});

test("clipboard buttons follow the Edit menu's enabling and send their commands", async () => {
  render(<Toolbar />);
  const btn = (name: string) => screen.getByRole("button", { name }) as HTMLButtonElement;
  expect(["Cut", "Copy", "Delete", "Flip H", "Flip V"].map((n) => btn(n).disabled)).toEqual([true, true, true, true, true]);
  expect(btn("Paste").disabled).toBe(false);
  withSelection();
  expect(["Cut", "Copy", "Delete", "Flip H", "Flip V"].map((n) => btn(n).disabled)).toEqual([false, false, false, false, false]);
  fireEvent.click(btn("Cut"));
  fireEvent.click(btn("Copy"));
  fireEvent.click(btn("Delete"));
  await waitFor(() => expect(sent()).toEqual(["cut_selection", "copy_selection", "delete_selection"]));
});

test("toolbar toggle buttons expose and show their state", () => {
  render(<Toolbar />);
  const pressed = (name: string) => screen.getByRole("button", { name }).getAttribute("aria-pressed");
  expect(pressed("Draw")).toBe("true");
  expect(pressed("Select")).toBe("false");
  expect(pressed("Library")).toBe("false");
  expect(pressed("Play")).toBe("false");
  fireEvent.click(screen.getByRole("button", { name: "Select" }));
  expect(pressed("Select")).toBe("true");
  expect(screen.getByRole("button", { name: "Select" }).style.background).not.toBe("");
  expect(screen.getByRole("button", { name: "Draw" }).style.background).not.toBe(screen.getByRole("button", { name: "Select" }).style.background);
  expect(screen.getByRole("button", { name: "Draw" }).style.borderRadius).toBe(screen.getByRole("button", { name: "Select" }).style.borderRadius);
  act(() => setHeader(header({ playing: true })));
  expect(pressed("Pause")).toBe("true");
});

test("the Paste button is active while placing and clicking it ends placing", () => {
  render(<Toolbar />);
  const paste = () => screen.getByRole("button", { name: "Paste" });
  expect(paste().getAttribute("aria-pressed")).toBe("false");
  act(() => setEdit(tools.startPlacing(getState().edit, { width: 3, height: 3, bits: new Uint32Array(3) }).state));
  expect(paste().getAttribute("aria-pressed")).toBe("true");
  fireEvent.click(paste());
  expect(getState().edit.placing).toBeNull();
  expect(paste().getAttribute("aria-pressed")).toBe("false");
});
