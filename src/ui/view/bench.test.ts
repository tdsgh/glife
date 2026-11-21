import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, beforeEach, expect, test } from "vitest";
import { runBench, type BenchEnv } from "./bench";

let reports: { line: string; ok: boolean }[];
beforeEach(() => {
  reports = [];
  mockIPC((cmd, args) => {
    if (cmd === "bench_report") reports.push(args as { line: string; ok: boolean });
  });
});
afterEach(clearMocks);

// A clock that only advances in sleep; frames and generations follow it at fixed rates per look.
function fakeEnv(over: Partial<BenchEnv> = {}) {
  let t = 0;
  let look: "plain" | "age" = "plain";
  const log: string[] = [];
  const fpsOf = () => (look === "plain" ? 120 : 60);
  const gensOf = () => (look === "plain" ? 50 : 25);
  let frames = 0;
  let gens = 100;
  let playing = false;
  const env: BenchEnv = {
    now: () => t,
    sleep: async (ms) => {
      t += ms;
      frames += (fpsOf() * ms) / 1000;
      gens += (gensOf() * ms) / 1000;
    },
    frames: () => frames,
    generation: () => Math.floor(gens),
    playing: () => playing,
    ready: () => true,
    failure: () => null,
    fit: () => log.push("fit"),
    play: async () => {
      playing = true;
      log.push("play");
    },
    setLook: (l) => {
      look = l;
      log.push(l);
    },
    info: () => ({ tiling: "hexagonal", rule: "B2/S34", width: 256, height: 256, dpr: 2 }),
    ...over,
  };
  return { env, log };
}

test("fits, plays, measures plain then age, and reports the JSON shape", async () => {
  const { env, log } = fakeEnv();
  await runBench(env);
  expect(log).toEqual(["fit", "play", "plain", "age"]);
  expect(reports).toHaveLength(1);
  expect(reports[0].ok).toBe(true);
  expect(JSON.parse(reports[0].line)).toEqual({
    tiling: "hexagonal",
    rule: "B2/S34",
    width: 256,
    height: 256,
    dpr: 2,
    plain: { fps: 120, gensPerSec: 50 },
    age: { fps: 60, gensPerSec: 25 },
  });
});

test("rounds to one decimal and does not replay when already playing", async () => {
  const { env, log } = fakeEnv({ playing: () => true });
  let n = 0;
  env.frames = () => (n += 333);
  await runBench(env);
  expect(log).not.toContain("play");
  const r = JSON.parse(reports[0].line);
  expect(r.plain.fps).toBe(Math.round(r.plain.fps * 10) / 10);
});

test("a shader failure reports ok false with the message", async () => {
  let bad: string | null = null;
  const { env } = fakeEnv({ failure: () => bad });
  const sleep = env.sleep;
  env.sleep = async (ms) => {
    await sleep(ms);
    if (env.now() > 3000) bad = "compile error";
  };
  await runBench(env);
  expect(reports).toHaveLength(1);
  expect(reports[0].ok).toBe(false);
  expect(JSON.parse(reports[0].line)).toEqual({ error: "compile error" });
});

test("any exception in the sequence reports ok false", async () => {
  const { env } = fakeEnv({ play: async () => Promise.reject(new Error("no host")) });
  await runBench(env);
  expect(reports[0].ok).toBe(false);
  expect(JSON.parse(reports[0].line)).toEqual({ error: "no host" });
});
