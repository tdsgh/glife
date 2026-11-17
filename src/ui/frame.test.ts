import { expect, test } from "vitest";
import { decodeFrame, decodePattern } from "./frame";

function build(layers: number[][], over: Partial<Record<number, number>> = {}): ArrayBuffer {
  const head = new Uint32Array(16);
  head[0] = 1;
  head[1] = 1;
  head[2] = 7;
  head[3] = 1; // version = 1 * 2^32 + 7
  head[4] = 42;
  head[6] = 5;
  head[8] = 100;
  head[9] = 50;
  head[10] = 1;
  head[11] = 32;
  head[12] = 4;
  head[13] = 64;
  head[14] = 2;
  head[15] = layers.length;
  for (const [k, v] of Object.entries(over)) head[Number(k)] = v!;
  const body = layers.flatMap((l) => l);
  const all = new Uint32Array(16 + body.length);
  all.set(head);
  all.set(body, 16);
  new Float32Array(all.buffer)[7] = 12.5;
  return all.buffer;
}

test("decodes header fields and the alive layer as a view", () => {
  const buf = build([[0, 4, 1, 2, 3, 4]]);
  const { header, alive } = decodeFrame(buf);
  expect(header).toMatchObject({
    version: 1,
    playing: true,
    snapshotVersion: 2 ** 32 + 7,
    generation: 42,
    liveCount: 5,
    gensPerSec: 12.5,
    gridWidth: 100,
    gridHeight: 50,
    edges: "dead",
    x0: 32,
    y0: 4,
    w: 64,
    h: 2,
    layerCount: 1,
  });
  expect(Array.from(alive!)).toEqual([1, 2, 3, 4]);
  expect(alive!.buffer).toBe(buf);
});

test("zero-layer reply has no alive data", () => {
  const { header, alive } = decodeFrame(build([]));
  expect(header.layerCount).toBe(0);
  expect(alive).toBeNull();
});

test("skips an unknown layer kind by word count", () => {
  const { alive } = decodeFrame(build([[9, 3, 7, 7, 7], [0, 2, 5, 6]]));
  expect(Array.from(alive!)).toEqual([5, 6]);
});

test("wrap edges and a stopped flag", () => {
  const { header } = decodeFrame(build([], { 1: 0, 10: 0 }));
  expect(header.playing).toBe(false);
  expect(header.edges).toBe("wrap");
});

test("flag bits 1-3 decode as undo, redo and unsaved changes", () => {
  expect(decodeFrame(build([], { 1: 0 })).header).toMatchObject({ canUndo: false, canRedo: false, dirty: false });
  expect(decodeFrame(build([], { 1: 0b0010 })).header).toMatchObject({ canUndo: true, canRedo: false, dirty: false });
  expect(decodeFrame(build([], { 1: 0b0101 })).header).toMatchObject({ playing: true, canUndo: false, canRedo: true });
  expect(decodeFrame(build([], { 1: 0b1000 })).header.dirty).toBe(true);
});

test("age and trail layers decode as byte views, four cells per word", () => {
  const buf = build([[1, 1, 0x04030201], [2, 1, 0x08070605]]);
  const { alive, ages, trail } = decodeFrame(buf);
  expect(alive).toBeNull();
  expect(Array.from(ages!)).toEqual([1, 2, 3, 4]);
  expect(Array.from(trail!)).toEqual([5, 6, 7, 8]);
  expect(ages!.buffer).toBe(buf);
});

test("a frame without byte layers has null ages and trail", () => {
  const { ages, trail } = decodeFrame(build([[0, 1, 1]]));
  expect(ages).toBeNull();
  expect(trail).toBeNull();
});

// Dynamic specifier: the repo has no Node type definitions.
const nodeFs = "node:fs";

test("fixtures/frame-v1-layers.bin decodes to its known ages and trail", async () => {
  const { readFileSync } = await import(/* @vite-ignore */ nodeFs);
  const b = readFileSync("fixtures/frame-v1-layers.bin");
  const { header, alive, ages, trail } = decodeFrame(b.buffer.slice(b.byteOffset, b.byteOffset + b.byteLength));
  expect(header).toMatchObject({ w: 64, h: 2, layerCount: 3 });
  expect(alive!.length).toBe(4);
  const at = (layer: Uint8Array, x: number, y: number) => layer[y * 64 + x];
  expect(ages!.length).toBe(128);
  expect(trail!.length).toBe(128);
  // Ages saturate at 255; the cell at (0, 0) is 308 generations old.
  expect([at(ages!, 0, 0), at(ages!, 1, 0), at(ages!, 0, 1), at(ages!, 1, 1)]).toEqual([255, 255, 255, 255]);
  expect([50, 51, 52].map((x) => at(ages!, x, 0))).toEqual([6, 5, 1]);
  expect([50, 52].map((x) => at(ages!, x, 1))).toEqual([7, 2]);
  expect([50, 51, 52].map((x) => at(trail!, x, 0))).toEqual([2, 3, 2]);
  expect([50, 52].map((x) => at(trail!, x, 1))).toEqual([1, 1]);
  expect(trail!.filter((v) => v > 0).length).toBe(5);
});

test("decodePattern reads the size and the rows of a hand-built buffer", () => {
  // 33 x 2: two words per row.
  const buf = new Uint32Array([33, 2, 0b101, 1, 0x80000000, 0]).buffer;
  const p = decodePattern(buf);
  expect(p.width).toBe(33);
  expect(p.height).toBe(2);
  expect(Array.from(p.bits)).toEqual([0b101, 1, 0x80000000, 0]);
});
