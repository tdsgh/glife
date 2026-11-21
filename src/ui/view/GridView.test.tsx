import { cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { act } from "react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { NO_VERSION } from "../backend";
import { resetUiState, setSettings } from "../state";

// jsdom has no WebGL: the renderer and textures are stand-ins.
vi.mock("three", () => {
  class Obj {
    image: { width: number; height: number; data: unknown };
    constructor(data?: unknown, w = 1, h = 1) {
      this.image = { data, width: w, height: h };
    }
    dispose() {}
    set() {}
    add() {}
  }
  class Renderer {
    domElement = document.createElement("canvas");
    debug: { onShaderError?: (...a: unknown[]) => void } = {};
    constructor() {
      (globalThis as { __renderer?: Renderer }).__renderer = this;
    }
    setPixelRatio() {}
    getPixelRatio = () => 1;
    setSize() {}
    render() {}
    dispose() {}
  }
  return {
    WebGLRenderer: Renderer,
    DataTexture: Obj,
    ShaderMaterial: class extends Obj {
      uniforms: unknown;
      constructor(o: { uniforms: unknown }) {
        super();
        this.uniforms = o.uniforms;
      }
    },
    Scene: Obj,
    Mesh: Obj,
    PlaneGeometry: Obj,
    OrthographicCamera: Obj,
    Vector2: Obj,
    Vector4: Obj,
    Color: Obj,
    NearestFilter: 0,
    RedIntegerFormat: 0,
    UnsignedIntType: 0,
    UnsignedByteType: 0,
    GLSL3: 0,
  };
});

import { getState } from "../state";
import { GridView } from "./GridView";

let calls: { layers: number; since: number }[];
let edits: { cmd: string; args: unknown }[];
let tilingCode = 0;
let reportMode = false;
beforeEach(() => {
  reportMode = false;
  calls = [];
  edits = [];
  tilingCode = 0;
  resetUiState();
  vi.stubGlobal("ResizeObserver", class { observe() {} disconnect() {} });
  mockIPC((cmd, args) => {
    if (cmd === "bench_report_mode") return reportMode;
    if (cmd !== "frame") {
      edits.push({ cmd, args });
      return;
    }
    const a = args as { x0: number; y0: number; w: number; h: number; since: number; layers: number };
    calls.push({ layers: a.layers, since: a.since });
    // Echo the requested region with its alive layer, and the byte layers asked for.
    const words = [1, tilingCode << 8, 3, 0, 0, 0, 0, 0, 64, 64, 0, a.x0, a.y0, a.w, a.h, 1 + (a.layers & 1) + ((a.layers >> 1) & 1)];
    words.push(0, (a.w / 32) * a.h, ...new Array((a.w / 32) * a.h).fill(0));
    if (a.layers & 1) words.push(1, (a.w / 4) * a.h, ...new Array((a.w / 4) * a.h).fill(0));
    if (a.layers & 2) words.push(2, (a.w / 4) * a.h, ...new Array((a.w / 4) * a.h).fill(0));
    return new Uint32Array(words).buffer;
  });
});
afterEach(() => {
  cleanup();
  clearMocks();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

const lastCall = () => calls[calls.length - 1];
const settled = async (layers: number) => {
  await waitFor(() => expect(lastCall().layers).toBe(layers));
  // Wait until a reply with this layer set has been taken: the next request may reuse its version.
  await waitFor(() => expect(lastCall().since).not.toBe(NO_VERSION));
};

test("asks for layers 0, 1 and 3 as the mode and trail change, and resets the version each time", async () => {
  render(<GridView />);
  await settled(0);

  act(() => setSettings({ coloring: "age" }));
  await waitFor(() => expect(calls.some((c) => c.layers === 1)).toBe(true));
  expect(calls.find((c) => c.layers === 1)!.since).toBe(NO_VERSION);
  await settled(1);

  act(() => setSettings({ trail: true }));
  await waitFor(() => expect(calls.some((c) => c.layers === 3)).toBe(true));
  expect(calls.find((c) => c.layers === 3)!.since).toBe(NO_VERSION);
  await settled(3);

  // The trail is drawn only in age coloring.
  const before = calls.length;
  act(() => setSettings({ coloring: "plain" }));
  await waitFor(() => expect(calls.slice(before).some((c) => c.layers === 0)).toBe(true));
  expect(calls.slice(before).find((c) => c.layers === 0)!.since).toBe(NO_VERSION);
});

// The host is 640 x 640 px over a 64 x 64 grid: 10 px per cell, so cell = px / 10.
function setupPointer() {
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(640);
  vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(640);
  HTMLElement.prototype.setPointerCapture = () => {};
}
const at = (cell: number) => ({ clientX: cell * 10 + 5, clientY: 5 });

test("a draw drag sends batches with one stroke id and the next press takes a new one", async () => {
  setupPointer();
  const { container } = render(<GridView />);
  const host = container.firstElementChild!;
  await waitFor(() => expect(calls.length).toBeGreaterThan(1));
  fireEvent.pointerDown(host, { button: 0, ...at(2) });
  fireEvent.pointerMove(host, at(4));
  fireEvent.pointerMove(host, at(6));
  fireEvent.pointerUp(host);
  fireEvent.pointerDown(host, { button: 0, ...at(10) });
  fireEvent.pointerUp(host);
  const strokes = edits.filter((e) => e.cmd === "edit").map((e) => (e.args as { stroke: number }).stroke);
  expect(strokes).toHaveLength(4);
  expect(strokes.slice(0, 3)).toEqual([1, 1, 1]);
  expect(strokes[3]).toBe(2);
});

test("a select drag makes a selection and shows its size", async () => {
  setupPointer();
  const { container, getByTestId } = render(<GridView />);
  const host = container.firstElementChild!;
  await waitFor(() => expect(calls.length).toBeGreaterThan(1));
  const { setTool } = await import("../controls/actions");
  act(() => setTool("select"));
  fireEvent.pointerDown(host, { button: 0, ...at(2) });
  fireEvent.pointerMove(host, { clientX: 85, clientY: 35 });
  fireEvent.pointerUp(host);
  expect(getState().edit.selection).toEqual({ x: 2, y: 0, w: 7, h: 4 });
  expect(edits.filter((e) => e.cmd === "edit")).toEqual([]);
  await waitFor(() => expect(getByTestId("selection-label").textContent).toBe("7 × 4"));
});

test("on a hexagonal grid a click draws the cell whose hexagon is under the pointer, near edges too", async () => {
  tilingCode = 1;
  setupPointer();
  const { container } = render(<GridView />);
  const host = container.firstElementChild!;
  await waitFor(() => expect(calls.length).toBeGreaterThan(1));
  // 640 px window over a 64 x 64 hexagonal grid: fitted view, so screen = (world - center) * ppc + 320.
  const { fit } = await import("./viewport");
  const { cellCenter, ROW } = await import("./geometry");
  const v = fit(640, 640, 64, 64, "hexagonal");
  const screenOf = (wx: number, wy: number) => ({ clientX: (wx - v.centerX) * v.pxPerCell + 320, clientY: (wy - v.centerY) * v.pxPerCell + 320 });
  const drawn = () => edits.filter((e) => e.cmd === "edit").map((e) => (e.args as { cells: number[][] }).cells[0]);
  // The center of an odd-row cell, then a point just inside its lower-right slanted edge.
  const [cx, cy] = cellCenter("hexagonal", 5, 3);
  fireEvent.pointerDown(host, { button: 0, ...screenOf(cx, cy) });
  fireEvent.pointerUp(host);
  fireEvent.pointerDown(host, { button: 0, ...screenOf(cx + 0.25 - 0.05, cy + ROW / 2 - 0.05) });
  fireEvent.pointerUp(host);
  // And one just past that edge, which is the cell below-right.
  fireEvent.pointerDown(host, { button: 0, ...screenOf(cx + 0.25 - 0.05, cy + ROW / 2 + 0.05) });
  fireEvent.pointerUp(host);
  expect(drawn()).toEqual([[5, 3], [5, 3], [6, 4]]);
});

test("outside report mode the view never reports", async () => {
  render(<GridView />);
  await waitFor(() => expect(calls.length).toBeGreaterThan(2));
  (globalThis as { __renderer?: { debug: { onShaderError?: (...a: unknown[]) => void } } }).__renderer!.debug.onShaderError!(
    { getProgramInfoLog: () => "boom", getShaderInfoLog: () => "" },
    {},
    {},
    {},
  );
  await new Promise((r) => setTimeout(r, 200));
  expect(edits.some((e) => e.cmd === "bench_report")).toBe(false);
  expect(getState().settings.coloring).toBe("plain");
});

test("in report mode a shader failure is reported as an error, not ok", async () => {
  reportMode = true;
  render(<GridView />);
  await waitFor(() => expect(calls.length).toBeGreaterThan(2));
  (globalThis as { __renderer?: { debug: { onShaderError?: (...a: unknown[]) => void } } }).__renderer!.debug.onShaderError!(
    { getProgramInfoLog: () => "link failed", getShaderInfoLog: () => "" },
    {},
    {},
    {},
  );
  await waitFor(() => expect(edits.some((e) => e.cmd === "bench_report")).toBe(true));
  const report = edits.find((e) => e.cmd === "bench_report")!.args as { line: string; ok: boolean };
  expect(report.ok).toBe(false);
  expect(JSON.parse(report.line)).toEqual({ error: "link failed" });
});
