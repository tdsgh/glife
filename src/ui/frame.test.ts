import { expect, test } from "vitest";
import { decodeFrame } from "./frame";

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
