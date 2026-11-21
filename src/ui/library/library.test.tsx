import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, beforeEach, expect, test } from "vitest";
import { Toolbar } from "../controls/Toolbar";
import type { FrameHeader } from "../frame";
import { getState, resetUiState, setEdit, setHeader } from "../state";
import { LibraryPanel } from "./LibraryPanel";
import { previewPath } from "./PatternPreview";

const header = (over: Partial<FrameHeader> = {}): FrameHeader => ({
  version: 1, playing: false, snapshotVersion: 1, generation: 0, liveCount: 0, gensPerSec: 0,
  gridWidth: 100, gridHeight: 50, edges: "wrap", x0: 0, y0: 0, w: 0, h: 0, layerCount: 0,
  canUndo: false, canRedo: false, dirty: false, tiling: "square", rule: "B3/S23", ...over,
});

const stats = { live: 5, created: "2026-01-02T03:04:05Z", changed: "2026-02-03T04:05:06Z" };
const library = {
  builtin: [
    { id: "glider", name: "Glider", category: "spaceship", description: "Moves diagonally.", width: 3, height: 3, live: 5, period: 4, speed: "c/4 diagonal" },
    { id: "block", name: "Block", category: "still-life", description: "Four cells.", width: 2, height: 2, live: 4, period: null, speed: null },
    { id: "huge", name: "Huge gun", category: "gun", description: "Large.", width: 300, height: 20, live: 90, period: 30 },
  ],
  personal: [
    { name: "My glider", comment: "line one\nline two", width: 3, height: 3, ...stats },
    { name: "Other", comment: "", width: 4, height: 4, ...stats },
  ],
  problems: [{ file: "bad.toml", reason: "no cells" }],
};

/** A 1 × 1 pattern buffer with its one cell alive. */
const previewBuffer = () => new Uint32Array([1, 1, 1]).buffer;

let calls: { cmd: string; args: any }[];
let handlers: Record<string, (args: any) => unknown>;
beforeEach(() => {
  calls = [];
  handlers = {
    library_list: () => library,
    pattern_preview: previewBuffer,
  };
  resetUiState();
  setHeader(header());
  mockIPC((cmd, args) => {
    calls.push({ cmd, args });
    return handlers[cmd]?.(args);
  });
});
afterEach(() => {
  cleanup();
  clearMocks();
});

const sent = () => calls.map((c) => c.cmd);
const click = (name: string) => fireEvent.click(screen.getByRole("button", { name }));
const open = async () => {
  render(<LibraryPanel />);
  await screen.findByText("Glider");
};

test("previewPath draws one run per row of live cells", () => {
  // 3 × 2: row 0 = cells 0 and 1, row 1 = cell 2.
  expect(previewPath({ width: 3, height: 2, bits: new Uint32Array([0b011, 0b100]) })).toBe("M0 0h2v1h-2zM2 1h1v1h-1z");
});

test("lists built-in entries by category and personal entries, with problems", async () => {
  await open();
  expect(screen.getByRole("region", { name: "Spaceships" })).toBeTruthy();
  expect(screen.getByRole("region", { name: "Still lifes" })).toBeTruthy();
  expect(screen.getByText("My glider")).toBeTruthy();
  expect(screen.getByText(/bad\.toml: no cells/)).toBeTruthy();
  expect(screen.getByText("3 × 3 · 5 live · period 4 · c/4 diagonal")).toBeTruthy();
  expect(screen.getByText("2 × 2 · 4 live")).toBeTruthy();
});

test("filtering narrows both lists by name", async () => {
  await open();
  fireEvent.change(screen.getByLabelText("Filter patterns"), { target: { value: "glider" } });
  expect(screen.getByText("Glider")).toBeTruthy();
  expect(screen.getByText("My glider")).toBeTruthy();
  expect(screen.queryByText("Block")).toBeNull();
  expect(screen.queryByText("Other")).toBeNull();
});

test("an empty personal library shows the hint", async () => {
  handlers.library_list = () => ({ ...library, personal: [] });
  await open();
  expect(screen.getByText(/No personal patterns yet/)).toBeTruthy();
});

test("the selected entry shows its description, and personal entries their dates", async () => {
  await open();
  fireEvent.click(screen.getByText("Glider"));
  expect(screen.getByText("Moves diagonally.")).toBeTruthy();
  fireEvent.click(screen.getByText("My glider"));
  expect(screen.getByText(/line one/)).toBeTruthy();
  expect(screen.getByText(/Saved .* changed /)).toBeTruthy();
});

test("an oversized row is not clickable and says why; a fitting row places", async () => {
  handlers.begin_place = () => ({ width: 3, height: 3, name: "Glider", comment: "", notices: [] });
  handlers.pending_cells = () => new Uint32Array([3, 3, 2, 4, 7]).buffer;
  await open();
  expect(screen.getByText(/Larger than the universe \(100 × 50\)/)).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Place" })).toBeNull();
  fireEvent.click(screen.getByText("Huge gun"));
  await new Promise((r) => setTimeout(r, 20));
  expect(sent()).not.toContain("begin_place");
  fireEvent.click(screen.getByText("Glider"));
  await waitFor(() => expect(getState().edit.placing).not.toBeNull());
  expect(JSON.stringify(calls.find((c) => c.cmd === "begin_place")!.args)).toContain('"builtin":"glider"');
  expect(getState().edit.placing!.width).toBe(3);
});

test("a begin_place error shows a notice and does not start placing", async () => {
  handlers.begin_place = () => {
    throw "too big";
  };
  await open();
  fireEvent.click(screen.getByText("Glider"));
  await waitFor(() => expect(getState().notice).toBe("too big"));
  expect(getState().edit.placing).toBeNull();
});

test("personal actions send their commands", async () => {
  handlers.library_delete = () => true;
  await open();
  fireEvent.click(screen.getByText("My glider"));
  click("Export");
  click("Delete");
  await waitFor(() => expect(sent()).toContain("library_delete"));
  expect(JSON.stringify(calls.find((c) => c.cmd === "export_rle")!.args)).toContain('"personal":"My glider"');
  expect(calls.find((c) => c.cmd === "library_delete")!.args).toEqual({ name: "My glider" });
  // The deletion reloads the list.
  await waitFor(() => expect(sent().filter((c) => c === "library_list").length).toBe(2));
});

test("Copy to personal edits the name in the row and sends the built-in id", async () => {
  await open();
  fireEvent.click(within(screen.getByText("Block").closest("li")!).getByRole("button", { name: "Copy to personal" }));
  const field = screen.getByLabelText("Name");
  expect(screen.queryByRole("dialog")).toBeNull();
  fireEvent.change(field, { target: { value: "My block" } });
  fireEvent.keyDown(field, { key: "Enter" });
  await waitFor(() => expect(sent()).toContain("library_copy_builtin"));
  expect(calls.find((c) => c.cmd === "library_copy_builtin")!.args).toEqual({ id: "block", name: "My block", replace: false });
});

test("in-place rename: Enter saves, Escape cancels", async () => {
  await open();
  fireEvent.click(screen.getByText("Other"));
  click("Rename");
  let field = screen.getByLabelText("Name");
  fireEvent.change(field, { target: { value: "Nope" } });
  fireEvent.keyDown(field, { key: "Escape" });
  expect(screen.queryByLabelText("Name")).toBeNull();
  expect(sent()).not.toContain("library_rename");
  click("Rename");
  field = screen.getByLabelText("Name");
  fireEvent.change(field, { target: { value: "Renamed" } });
  fireEvent.keyDown(field, { key: "Enter" });
  await waitFor(() => expect(sent()).toContain("library_rename"));
  expect(calls.find((c) => c.cmd === "library_rename")!.args).toEqual({ name: "Other", newName: "Renamed", replace: false });
});

test("in-place comment edit saves when the field loses focus", async () => {
  await open();
  fireEvent.click(screen.getByText("Other"));
  click("Edit comment");
  const field = screen.getByLabelText("Comment");
  fireEvent.change(field, { target: { value: "hello" } });
  fireEvent.blur(field);
  await waitFor(() => expect(sent()).toContain("library_set_comment"));
  expect(calls.find((c) => c.cmd === "library_set_comment")!.args).toEqual({ name: "Other", comment: "hello" });
});

test("a taken name in place offers Replace, which resends with replace: true", async () => {
  let n = 0;
  handlers.library_rename = () => (n++ === 0 ? { nameTaken: true } : null);
  await open();
  fireEvent.click(screen.getByText("Other"));
  click("Rename");
  const field = screen.getByLabelText("Name");
  fireEvent.change(field, { target: { value: "My glider" } });
  fireEvent.keyDown(field, { key: "Enter" });
  await screen.findByText(/already exists/);
  expect(screen.getByRole("button", { name: "Choose another name" })).toBeTruthy();
  click("Replace");
  await waitFor(() => expect(screen.queryByLabelText("Name")).toBeNull());
  expect(calls.filter((c) => c.cmd === "library_rename").map((c) => c.args.replace)).toEqual([false, true]);
});

test("a taken name offers Replace, and Replace resends with replace: true", async () => {
  let n = 0;
  handlers.library_save = () => (n++ === 0 ? { nameTaken: true } : null);
  render(<Toolbar />);
  act(() => setEdit({ ...getState().edit, selection: { x: 1, y: 2, w: 3, h: 4 } }));
  click("Save to library…");
  fireEvent.change(screen.getByLabelText("Name"), { target: { value: "Glider" } });
  fireEvent.change(screen.getByLabelText("Comment"), { target: { value: "mine" } });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await screen.findByText(/already exists/);
  expect(screen.getByRole("button", { name: "Choose another name" })).toBeTruthy();
  click("Replace");
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  const saves = calls.filter((c) => c.cmd === "library_save");
  expect(saves.map((c) => c.args.replace)).toEqual([false, true]);
  expect(saves[1].args).toMatchObject({ name: "Glider", comment: "mine" });
  expect(JSON.stringify(saves[1].args)).toContain('"selection":{"x":1,"y":2,"w":3,"h":4}');
});

test("Choose another name returns to the name field", async () => {
  handlers.library_save = () => ({ nameTaken: true });
  render(<Toolbar />);
  act(() => setEdit({ ...getState().edit, selection: { x: 0, y: 0, w: 1, h: 1 } }));
  click("Save to library…");
  fireEvent.change(screen.getByLabelText("Name"), { target: { value: "A" } });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await screen.findByText(/already exists/);
  click("Choose another name");
  expect(screen.queryByText(/already exists/)).toBeNull();
  expect(screen.getByRole("button", { name: "Save" })).toBeTruthy();
});
