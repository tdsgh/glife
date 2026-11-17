import { cleanup, render, waitFor } from "@testing-library/react";
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

import { GridView } from "./GridView";

let calls: { layers: number; since: number }[];
beforeEach(() => {
  calls = [];
  resetUiState();
  vi.stubGlobal("ResizeObserver", class { observe() {} disconnect() {} });
  mockIPC((cmd, args) => {
    if (cmd !== "frame") return;
    const a = args as { x0: number; y0: number; w: number; h: number; since: number; layers: number };
    calls.push({ layers: a.layers, since: a.since });
    // Echo the requested region with its alive layer, and the byte layers asked for.
    const words = [1, 0, 3, 0, 0, 0, 0, 0, 64, 64, 0, a.x0, a.y0, a.w, a.h, 1 + (a.layers & 1) + ((a.layers >> 1) & 1)];
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
