import { act, cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import type { FrameHeader } from "../frame";
import { resetUiState, setFileName, setHeader } from "../state";

const setTitle = vi.hoisted(() => vi.fn(() => Promise.resolve()));
vi.mock("../backend", async (orig) => ({ ...(await orig<object>()), setTitle }));

import { useWindowTitle } from "./menu";

const header = (dirty: boolean): FrameHeader => ({
  version: 1, playing: false, snapshotVersion: 1, generation: 0, liveCount: 0, gensPerSec: 0,
  gridWidth: 10, gridHeight: 10, edges: "wrap", x0: 0, y0: 0, w: 0, h: 0, layerCount: 0,
  canUndo: false, canRedo: false, dirty,
});

function Title() {
  useWindowTitle();
  return null;
}

beforeEach(() => {
  resetUiState();
  setTitle.mockClear();
});
afterEach(cleanup);

test("the title follows the file name and the dirty flag", () => {
  render(<Title />);
  expect(setTitle).toHaveBeenLastCalledWith("Untitled — glife");
  act(() => setFileName("a.glife"));
  expect(setTitle).toHaveBeenLastCalledWith("a.glife — glife");
  act(() => setHeader(header(true)));
  expect(setTitle).toHaveBeenLastCalledWith("• a.glife — glife");
  act(() => setHeader(header(false)));
  expect(setTitle).toHaveBeenLastCalledWith("a.glife — glife");
});
