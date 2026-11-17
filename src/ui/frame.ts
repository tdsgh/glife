export interface FrameHeader {
  version: number;
  playing: boolean;
  snapshotVersion: number;
  generation: number;
  liveCount: number;
  gensPerSec: number;
  gridWidth: number;
  gridHeight: number;
  edges: "wrap" | "dead";
  x0: number;
  y0: number;
  w: number;
  h: number;
  layerCount: number;
  canUndo: boolean;
  canRedo: boolean;
  dirty: boolean;
}

export interface Frame {
  header: FrameHeader;
  /** Alive bits: `h` rows of `w / 32` words. Null when the reply carries no alive layer. */
  alive: Uint32Array | null;
  /** Ages, `min(age, 255)`: `h` rows of `w` bytes. Null when not in the reply. */
  ages: Uint8Array | null;
  /** Trail counters, shaped like `ages`. Null when not in the reply. */
  trail: Uint8Array | null;
}

const HEADER_WORDS = 16;
const KIND_ALIVE = 0;
const KIND_AGE = 1;
const KIND_TRAIL = 2;

export function decodeFrame(buffer: ArrayBuffer): Frame {
  const words = new Uint32Array(buffer);
  const f32 = new Float32Array(buffer);
  const u64 = (lo: number, hi: number) => lo + hi * 2 ** 32;
  const header: FrameHeader = {
    version: words[0],
    playing: (words[1] & 1) === 1,
    snapshotVersion: u64(words[2], words[3]),
    generation: u64(words[4], words[5]),
    liveCount: words[6],
    gensPerSec: f32[7],
    gridWidth: words[8],
    gridHeight: words[9],
    edges: words[10] === 0 ? "wrap" : "dead",
    x0: words[11],
    y0: words[12],
    w: words[13],
    h: words[14],
    layerCount: words[15],
    canUndo: (words[1] & 2) !== 0,
    canRedo: (words[1] & 4) !== 0,
    dirty: (words[1] & 8) !== 0,
  };
  let alive: Uint32Array | null = null;
  let ages: Uint8Array | null = null;
  let trail: Uint8Array | null = null;
  let pos = HEADER_WORDS;
  for (let i = 0; i < header.layerCount; i++) {
    const kind = words[pos];
    const count = words[pos + 1];
    pos += 2;
    if (kind === KIND_ALIVE) alive = new Uint32Array(buffer, pos * 4, count);
    if (kind === KIND_AGE) ages = new Uint8Array(buffer, pos * 4, count * 4);
    if (kind === KIND_TRAIL) trail = new Uint8Array(buffer, pos * 4, count * 4);
    pos += count;
  }
  return { header, alive, ages, trail };
}
