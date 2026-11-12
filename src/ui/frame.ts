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
}

export interface Frame {
  header: FrameHeader;
  /** Alive bits: `h` rows of `w / 32` words. Null when the reply carries no alive layer. */
  alive: Uint32Array | null;
}

const HEADER_WORDS = 16;
const KIND_ALIVE = 0;

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
  };
  let alive: Uint32Array | null = null;
  let pos = HEADER_WORDS;
  for (let i = 0; i < header.layerCount; i++) {
    const kind = words[pos];
    const count = words[pos + 1];
    pos += 2;
    if (kind === KIND_ALIVE) alive = new Uint32Array(buffer, pos * 4, count);
    pos += count;
  }
  return { header, alive };
}
