import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { decodeFrame, type FrameHeader } from "../frame";
import { onFit, resetUiState, setHeader } from "../state";
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
  canUndo: false, canRedo: false, dirty: false, ...over,
});

let calls: { cmd: string; args: unknown }[];
beforeEach(() => {
  calls = [];
  resetUiState();
  mockIPC((cmd, args) => {
    calls.push({ cmd, args });
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
  expect(last()).toEqual({ cmd: "new_universe", args: { width: 300, height: 40, edges: "dead" } });
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
  const f = decodeFrame(await fixture());
  expect(f.header).toMatchObject({ playing: true, generation: 0, liveCount: 7, gridWidth: 64, gridHeight: 3, edges: "dead" });
  render(<StatusBar />);
  act(() => setHeader(f.header));
  const text = screen.getByRole("status").textContent!;
  for (const part of ["Generation 0", "Live 7", "13 gen/s", "64 × 3", "Dead", "B3/S23"]) expect(text).toContain(part);
});

test("status bar hides gen/s while paused", () => {
  render(<StatusBar />);
  act(() => setHeader(header()));
  const text = screen.getByRole("status").textContent!;
  expect(text).not.toContain("gen/s");
  expect(text).toContain("256 × 128");
});

test("fixture frame-v1.bin decodes to its known pattern", async () => {
  const f = decodeFrame(await fixture());
  expect(f.header.gensPerSec).toBe(12.5);
  expect(f.header).toMatchObject({ x0: 0, y0: 0, w: 64, h: 2 });
  expect(Array.from(f.alive!)).toEqual([0x80000009, 0x80000001, 2, 256]);
});
