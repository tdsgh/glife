import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { decodeFrame, type FrameHeader } from "../frame";
import type { TilingInfo } from "../backend";
import { getState, onFit, resetUiState, setHeader, setTilings } from "../state";
import { RuleDialog } from "./RuleDialog";
import { NewUniverseDialog } from "./NewUniverseDialog";
import { StatusBar } from "./StatusBar";
import { Toolbar } from "./Toolbar";
import { handleShortcut, useShortcuts } from "./shortcuts";

// Dynamic specifier: the repo has no Node type definitions.
const nodeFs = "node:fs";
async function fixture(): Promise<ArrayBuffer> {
  const { readFileSync } = await import(/* @vite-ignore */ nodeFs);
  const b = readFileSync("fixtures/frame-v1.bin");
  return b.buffer.slice(b.byteOffset, b.byteOffset + b.byteLength);
}

const header = (over: Partial<FrameHeader> = {}): FrameHeader => ({
  version: 1, playing: false, snapshotVersion: 1, generation: 12, liveCount: 34, gensPerSec: 9.6,
  gridWidth: 256, gridHeight: 128, edges: "wrap", x0: 0, y0: 0, w: 0, h: 0, layerCount: 0,
  canUndo: false, canRedo: false, dirty: false, tiling: "square", rule: "B3/S23", ...over,
});

const tiling = (id: TilingInfo["id"], name: string, rule: string, presets: [string, string][], even = false): TilingInfo => ({
  id, name, neighbors: 8, defaultRule: { text: rule, birth: 0, survival: 0 }, presets: presets.map(([n, r]) => ({ name: n, rule: r })),
  wrapEvenWidth: even && id !== "hexagonal", wrapEvenHeight: even,
});
const TILINGS = [
  tiling("square", "Square", "B3/S23", [["Life", "B3/S23"], ["HighLife", "B36/S23"]]),
  tiling("hexagonal", "Hexagonal", "B2/S35", [["Bays", "B2/S35"], ["Hex Life", "B3/S23"]], true),
  tiling("triangular-12", "Triangular, 12 neighbors", "B456/S45", [["Bays", "B456/S45"]], true),
];

let calls: { cmd: string; args: unknown }[];
beforeEach(() => {
  calls = [];
  resetUiState();
  setTilings(TILINGS);
  mockIPC((cmd, args) => {
    calls.push({ cmd, args });
    if (cmd === "set_rule" && String((args as { rule: string }).rule).includes("9")) throw "a cell on this grid has 8 neighbors, so a rule cannot count 9";
    return cmd === "confirm_discard" ? true : undefined;
  });
});
afterEach(() => {
  cleanup();
  clearMocks();
});

const sent = () => calls.map((c) => c.cmd);
const last = () => calls[calls.length - 1];
const click = (name: string) => fireEvent.click(screen.getByRole("button", { name }));

test("toolbar buttons send their commands", async () => {
  render(<Toolbar />);
  click("Play");
  click("Step");
  click("Clear");
  click("Random fill");
  await waitFor(() => expect(sent()).toEqual(["play", "step", "clear", "random_fill"]));
  expect(last().args).toEqual({ density: 25 });
});

test("pause and disabled step while playing", async () => {
  render(<Toolbar />);
  act(() => setHeader(header({ playing: true })));
  expect((screen.getByRole("button", { name: "Step" }) as HTMLButtonElement).disabled).toBe(true);
  click("Pause");
  await waitFor(() => expect(sent()).toEqual(["pause"]));
});

test("speed select starts at 10 and sends set_speed, Max as null", async () => {
  render(<Toolbar />);
  const select = screen.getByLabelText("Speed") as HTMLSelectElement;
  expect(select.value).toBe("10");
  expect(Array.from(select.options).map((o) => o.text)).toEqual(
    ["1", "2", "5", "10", "15", "30", "60", "120", "240", "Max"]);
  fireEvent.change(select, { target: { value: "60" } });
  await waitFor(() => expect(last()).toEqual({ cmd: "set_speed", args: { gensPerSec: 60 } }));
  fireEvent.change(select, { target: { value: "max" } });
  await waitFor(() => expect(last()).toEqual({ cmd: "set_speed", args: { gensPerSec: null } }));
});

test("edges toggle flips the current mode", async () => {
  render(<Toolbar />);
  click("Edges: Wrap");
  await waitFor(() => expect(last()).toEqual({ cmd: "set_edges", args: { edges: "dead" } }));
  act(() => setHeader(header({ edges: "dead" })));
  click("Edges: Dead");
  await waitFor(() => expect(last()).toEqual({ cmd: "set_edges", args: { edges: "wrap" } }));
});

test("random fill sends the chosen density, clamped", async () => {
  render(<Toolbar />);
  const input = screen.getByLabelText("Density");
  fireEvent.change(input, { target: { value: "40" } });
  click("Random fill");
  await waitFor(() => expect(last().args).toEqual({ density: 40 }));
  fireEvent.change(input, { target: { value: "500" } });
  click("Random fill");
  await waitFor(() => expect(last().args).toEqual({ density: 99 }));
});

test("fit button requests a fit", () => {
  const fit = vi.fn();
  const off = onFit(fit);
  render(<Toolbar />);
  click("Fit");
  expect(fit).toHaveBeenCalledOnce();
  off();
});

test("new universe dialog creates, then fits", async () => {
  const fit = vi.fn();
  const off = onFit(fit);
  render(<Toolbar />);
  click("New universe…");
  await screen.findByLabelText("Width");
  fireEvent.change(screen.getByLabelText("Width"), { target: { value: "300" } });
  fireEvent.change(screen.getByLabelText("Height"), { target: { value: "40" } });
  fireEvent.change(screen.getByLabelText("Dialog edges"), { target: { value: "dead" } });
  click("Create");
  await waitFor(() => expect(fit).toHaveBeenCalled());
  expect(last()).toEqual({ cmd: "new_universe", args: { width: 300, height: 40, edges: "dead", tiling: "square", rule: "B3/S23" } });
  expect(screen.queryByRole("dialog")).toBeNull();
  off();
});

test("dialog rejects 2 and 2049 and accepts the bounds", () => {
  render(<NewUniverseDialog onClose={() => {}} />);
  const create = screen.getByRole("button", { name: "Create" });
  for (const bad of ["2", "2049", "", "abc"]) {
    fireEvent.change(screen.getByLabelText("Width"), { target: { value: bad } });
    expect((create as HTMLButtonElement).disabled).toBe(true);
  }
  fireEvent.change(screen.getByLabelText("Width"), { target: { value: "3" } });
  fireEvent.change(screen.getByLabelText("Height"), { target: { value: "2048" } });
  expect((create as HTMLButtonElement).disabled).toBe(false);
  fireEvent.change(screen.getByLabelText("Height"), { target: { value: "2049" } });
  expect((create as HTMLButtonElement).disabled).toBe(true);
  expect(sent()).toEqual([]);
});

const grid = (id: string) => fireEvent.change(screen.getByLabelText("Grid"), { target: { value: id } });
const create = () => screen.getByRole("button", { name: "Create" }) as HTMLButtonElement;

test("the dialog lists the grids, square first, and sends the chosen grid and rule", async () => {
  render(<NewUniverseDialog onClose={() => {}} />);
  expect(Array.from((screen.getByLabelText("Grid") as HTMLSelectElement).options).map((o) => o.value)).toEqual(["square", "hexagonal", "triangular-12"]);
  expect((screen.getByLabelText("Grid") as HTMLSelectElement).value).toBe("square");
  grid("hexagonal");
  fireEvent.change(screen.getByLabelText("Height"), { target: { value: "40" } });
  fireEvent.change(screen.getByLabelText("Rule"), { target: { value: "B34/S3" } });
  click("Create");
  await waitFor(() => expect(sent()).toEqual(["new_universe"]));
  expect(last().args).toEqual({ width: 256, height: 40, edges: "wrap", tiling: "hexagonal", rule: "B34/S3" });
});

test("wrapping hexagonal and triangular grids refuse an odd side; dead edges allow it", () => {
  render(<NewUniverseDialog onClose={() => {}} />);
  grid("hexagonal");
  expect(screen.getByRole("dialog").textContent).toContain("Wrapping needs an even height");
  // Hexagonal rows repeat in height only.
  fireEvent.change(screen.getByLabelText("Width"), { target: { value: "31" } });
  expect(create().disabled).toBe(false);
  fireEvent.change(screen.getByLabelText("Height"), { target: { value: "31" } });
  expect(create().disabled).toBe(true);
  fireEvent.change(screen.getByLabelText("Dialog edges"), { target: { value: "dead" } });
  expect(create().disabled).toBe(false);
  fireEvent.change(screen.getByLabelText("Dialog edges"), { target: { value: "wrap" } });
  grid("triangular-12");
  expect(screen.getByRole("dialog").textContent).toContain("Wrapping needs an even width and height");
  expect(create().disabled).toBe(true);
  fireEvent.change(screen.getByLabelText("Height"), { target: { value: "32" } });
  expect(create().disabled).toBe(true);
  fireEvent.change(screen.getByLabelText("Width"), { target: { value: "32" } });
  expect(create().disabled).toBe(false);
  grid("square");
  fireEvent.change(screen.getByLabelText("Width"), { target: { value: "31" } });
  expect(create().disabled).toBe(false);
});

test("changing the grid resets the rule to that grid's default; a preset fills the text box", () => {
  render(<NewUniverseDialog onClose={() => {}} />);
  const rule = () => (screen.getByLabelText("Rule") as HTMLInputElement).value;
  expect(rule()).toBe("B3/S23");
  fireEvent.change(screen.getByLabelText("Rule preset"), { target: { value: "HighLife" } });
  expect(rule()).toBe("B36/S23");
  fireEvent.change(screen.getByLabelText("Rule"), { target: { value: "B1/S1" } });
  expect((screen.getByLabelText("Rule preset") as HTMLSelectElement).value).toBe("");
  grid("hexagonal");
  expect(rule()).toBe("B2/S35");
  grid("triangular-12");
  expect(rule()).toBe("B456/S45");
});

test("a refused new universe shows the message and stays open", async () => {
  mockIPC((cmd) => {
    if (cmd === "new_universe") throw "birth on 0 neighbors is not supported";
  });
  const onClose = vi.fn();
  render(<NewUniverseDialog onClose={onClose} />);
  click("Create");
  await screen.findByText("birth on 0 neighbors is not supported");
  expect(onClose).not.toHaveBeenCalled();
});

test("the Rule… control sends set_rule and closes", async () => {
  render(<Toolbar />);
  act(() => setHeader(header({ tiling: "hexagonal", rule: "B2/S35" })));
  click("Rule…");
  expect((screen.getByLabelText("Rule") as HTMLInputElement).value).toBe("B2/S35");
  fireEvent.change(screen.getByLabelText("Rule preset"), { target: { value: "Hex Life" } });
  click("Apply");
  await waitFor(() => expect(sent()).toEqual(["set_rule"]));
  expect(last().args).toEqual({ rule: "B3/S23" });
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
});

test("a refused rule shows the backend's message without closing", async () => {
  const onClose = vi.fn();
  render(<RuleDialog onClose={onClose} />);
  act(() => setHeader(header()));
  fireEvent.change(screen.getByLabelText("Rule"), { target: { value: "B9/S1" } });
  click("Apply");
  await screen.findByText("a cell on this grid has 8 neighbors, so a rule cannot count 9");
  expect(onClose).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("Rule"), { target: { value: "B3/S1" } });
  expect(screen.queryByRole("alert")).toBeNull();
});

test("an edge change refused by the backend shows its message as a notice", async () => {
  mockIPC((cmd) => {
    if (cmd === "set_edges") throw "a wrapping hexagonal universe needs an even height";
  });
  render(<Toolbar />);
  click("Edges: Wrap");
  await waitFor(() => expect(getState().notice).toBe("a wrapping hexagonal universe needs an even height"));
});

function Keys() {
  useShortcuts();
  return <input aria-label="text" />;
}
const key = (k: string, target: Element | Window = window) => fireEvent.keyDown(target, { key: k });

test("shortcuts send their commands", async () => {
  const fit = vi.fn();
  const off = onFit(fit);
  render(<Keys />);
  key("Enter");
  key("n");
  key("r");
  key("Backspace");
  key("Delete");
  key("f");
  await waitFor(() => expect(sent()).toEqual(["play", "step", "random_fill", "clear", "clear"]));
  expect(fit).toHaveBeenCalledOnce();
  act(() => setHeader(header({ playing: true })));
  key("Enter");
  key("n");
  await waitFor(() => expect(sent()).toContain("pause"));
  expect(sent().filter((c) => c === "step")).toHaveLength(1);
  off();
});

test("shortcuts are ignored in text fields and with modifiers", () => {
  render(<Keys />);
  key("Enter", screen.getByLabelText("text"));
  key("Backspace", screen.getByLabelText("text"));
  fireEvent.keyDown(window, { key: "r", ctrlKey: true });
  expect(calls).toEqual([]);
  expect(handleShortcut(new KeyboardEvent("keydown", { key: "x" }))).toBe(false);
});

test("Cmd+Shift+Z redoes outside text fields", async () => {
  render(<Keys />);
  fireEvent.keyDown(screen.getByLabelText("text"), { key: "Z", code: "KeyZ", metaKey: true, shiftKey: true });
  expect(calls).toEqual([]);
  fireEvent.keyDown(window, { key: "Z", code: "KeyZ", metaKey: true, shiftKey: true });
  await waitFor(() => expect(sent()).toEqual(["redo"]));
});

test("status bar renders a decoded header", async () => {
  const f = decodeFrame(await fixture(), [{ ...TILINGS[0], defaultRule: { text: "B3/S23", birth: 0b1000, survival: 0b1100 } }]);
  expect(f.header).toMatchObject({ playing: true, generation: 0, liveCount: 7, gridWidth: 64, gridHeight: 3, edges: "dead", tiling: "square", rule: "B3/S23" });
  render(<StatusBar />);
  act(() => setHeader(f.header));
  const text = screen.getByRole("status").textContent!;
  for (const part of ["Generation 0", "Live 7", "13 gen/s"]) expect(text).toContain(part);
  expect(text).toContain("Square · 64 × 3 · Dead · B3/S23");
});

test("status bar hides gen/s while paused", () => {
  render(<StatusBar />);
  act(() => setHeader(header()));
  const text = screen.getByRole("status").textContent!;
  expect(text).not.toContain("gen/s");
  expect(text).toContain("Square · 256 × 128 · Wrap · B3/S23");
});

test("status bar shows the grid's name and the rule from the frame", () => {
  render(<StatusBar />);
  act(() => setHeader(header({ tiling: "triangular-12", rule: "B4/S456", gridWidth: 64, gridHeight: 32 })));
  expect(screen.getByRole("status").textContent).toContain("Triangular, 12 neighbors · 64 × 32 · Wrap · B4/S456");
});

test("fixture frame-v1.bin decodes to its known pattern", async () => {
  const f = decodeFrame(await fixture());
  expect(f.header.gensPerSec).toBe(12.5);
  expect(f.header).toMatchObject({ x0: 0, y0: 0, w: 64, h: 2 });
  expect(Array.from(f.alive!)).toEqual([0x80000009, 0x80000001, 2, 256]);
});
