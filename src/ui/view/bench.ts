import { benchReport } from "../backend";

/** What the report sequence needs from the running view; GridView supplies it, tests fake it. */
export interface BenchEnv {
  now(): number;
  sleep(ms: number): Promise<void>;
  /** Frames drawn so far. */
  frames(): number;
  /** Generation of the latest frame header; null before the first one. */
  generation(): number | null;
  playing(): boolean;
  /** True once the first frame is drawn. */
  ready(): boolean;
  /** Shader compile or link error text, once there is one. */
  failure(): string | null;
  fit(): void;
  play(): Promise<unknown>;
  /** Plain coloring with the trail off, or age coloring with the trail at 100. */
  setLook(look: "plain" | "age"): void;
  info(): { tiling: string; rule: string; width: number; height: number; dpr: number };
}

const TICK_MS = 50;
const round1 = (n: number) => Math.round(n * 10) / 10;

/** Warms up and measures both looks, then reports once: the JSON line, or `{"error"}` on any failure. */
export async function runBench(env: BenchEnv): Promise<void> {
  const wait = async (ms: number) => {
    const end = env.now() + ms;
    while (env.now() < end) {
      const failure = env.failure();
      if (failure) throw new Error(failure);
      await env.sleep(Math.min(TICK_MS, end - env.now()));
    }
    const failure = env.failure();
    if (failure) throw new Error(failure);
  };
  const measure = async (warmMs: number, look: "plain" | "age") => {
    env.setLook(look);
    await wait(warmMs);
    const f0 = env.frames();
    const g0 = env.generation() ?? 0;
    const t0 = env.now();
    await wait(5000);
    const secs = (env.now() - t0) / 1000;
    return {
      fps: round1((env.frames() - f0) / secs),
      gensPerSec: round1(((env.generation() ?? 0) - g0) / secs),
    };
  };
  try {
    while (!(env.ready() && env.generation() !== null)) await wait(TICK_MS);
    env.fit();
    if (!env.playing()) await env.play();
    const plain = await measure(2000, "plain");
    const age = await measure(1000, "age");
    const { tiling, rule, width, height, dpr } = env.info();
    await benchReport(JSON.stringify({ tiling, rule, width, height, dpr, plain, age }), true);
  } catch (e) {
    const error = e instanceof Error ? e.message : String(e);
    await benchReport(JSON.stringify({ error }), false).catch(() => {});
  }
}
