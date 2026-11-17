import { afterEach, beforeEach, expect, test, vi } from "vitest";

beforeEach(() => {
  localStorage.clear();
  vi.resetModules();
});
afterEach(() => vi.restoreAllMocks());

test("settings survive a reload of the state module", async () => {
  const first = await import("./state");
  first.setSettings({ coloring: "age", palette: "ice", paletteSize: 256, trail: true, trailLength: 42 });
  vi.resetModules();
  const second = await import("./state");
  expect(second.getState().settings).toEqual({ coloring: "age", palette: "ice", paletteSize: 256, trail: true, trailLength: 42 });
});

test("a throwing localStorage falls back to defaults", async () => {
  vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
    throw new Error("blocked");
  });
  vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
    throw new Error("blocked");
  });
  const s = await import("./state");
  expect(s.getState().settings).toEqual({ coloring: "plain", palette: "viridis", paletteSize: 32, trail: false, trailLength: 10 });
  expect(() => s.setSettings({ trail: true })).not.toThrow();
  expect(s.getState().settings.trail).toBe(true);
});

test("invalid stored values fall back per field", async () => {
  localStorage.setItem("glife.viewSettings", JSON.stringify({ coloring: "x", palette: "ice", paletteSize: 7, trailLength: 500 }));
  const s = await import("./state");
  expect(s.getState().settings).toMatchObject({ coloring: "plain", palette: "ice", paletteSize: 32, trailLength: 10 });
});

test("wantedLayers follows the mode and the trail", async () => {
  const s = await import("./state");
  expect(s.wantedLayers()).toBe(0);
  s.setSettings({ trail: true });
  expect(s.wantedLayers()).toBe(0);
  s.setSettings({ coloring: "age" });
  expect(s.wantedLayers()).toBe(3);
  s.setSettings({ trail: false });
  expect(s.wantedLayers()).toBe(1);
});
