<!-- kit:core/docs/architecture v1 -->
# Architecture

What glife is, today. Decisions behind it are in [adr.md](adr.md).

## Purpose and data flow

glife runs Conway's Game of Life (B3/S23) on a bounded square grid and lets the
operator draw, run, step, zoom and pan it.

1. The operator acts in the UI. Each action is a JSON `invoke` command
   ([src/ui/backend.ts:15-27](../src/ui/backend.ts) ↔
   [src-tauri/src/commands.rs:32-88](../src-tauri/src/commands.rs)).
2. Each command is queued to the simulation thread, which applies it between
   generations and publishes an immutable snapshot
   ([src-tauri/src/sim.rs:108-138](../src-tauri/src/sim.rs)). → ADR-007
3. On every animation frame the UI pulls the cells in view with `frame` and
   uploads them to the GPU, which draws them
   ([src/ui/view/GridView.tsx:83-126](../src/ui/view/GridView.tsx),
   [gridShader.ts](../src/ui/view/gridShader.ts)). → ADR-008

## Requirements

What the first phase must do and its targets are in
[glife-phase-1.md](glife-phase-1.md); repo-wide requirements are in
[roadmap.md](roadmap.md#cross-cutting-requirements).

- A universe side is 3 to 2048 cells
  ([lib.rs:14-17](../src/engine/src/lib.rs)).

## Topology

One Tauri process (ADR-001): the webview runs the UI; the Rust backend runs
the `simulation` thread, which owns the only `Universe`
([main.rs:9-16](../src-tauri/src/main.rs)). Tauri's command handlers read the
latest snapshot and forward every other command to the thread's queue
([commands.rs:13-22](../src-tauri/src/commands.rs)).

## Data model

**Universe.** Alive cells are bit-packed, 64 per `u64`, row after row; padding
bits past the grid's width are always zero
([grid.rs:1-3](../src/engine/src/grid.rs)). A universe also holds its edge mode
(`Wrap` or `Dead`), generation and live-cell count
([lib.rs:47-55](../src/engine/src/lib.rs)). → ADR-006

**Snapshot.** Version, a copy of the universe, playing flag and achieved gen/s
([sim.rs:18-24](../src-tauri/src/sim.rs)).

**Frame layout v1.** The reply to `frame`. Little-endian 32-bit words, so the UI
reads it as a `Uint32Array`. Encoder:
[frame.rs:14-54](../src-tauri/src/frame.rs). Decoder:
[frame.ts:27-57](../src/ui/frame.ts). Fixture:
[fixtures/frame-v1.bin](../fixtures/frame-v1.bin).

| Word | Field |
|---|---|
| 0 | format version = 1 |
| 1 | flags: bit 0 = playing |
| 2–3 | snapshot version, u64 (lo, hi) |
| 4–5 | generation, u64 (lo, hi) |
| 6 | live-cell count |
| 7 | achieved gen/s, f32 |
| 8–9 | grid width, grid height |
| 10 | edges: 0 = wrap, 1 = dead |
| 11–14 | region x0, y0, w, h (x0 and w multiples of 32) |
| 15 | layer count |
| then, per layer | kind (0 = alive bits), word count, data |

Alive-bits data is `h` rows of `w / 32` words; bit `i` of word `k` in row `r` is
cell `(x0 + 32k + i, y0 + r)`; bits past the grid's width are zero
([lib.rs:152-161](../src/engine/src/lib.rs)). The reply's region is the
request clipped to the grid and aligned to 32 columns. When `since` equals the
snapshot version the reply has zero layers; any other `since` gets a full
reply ([frame.rs:47-52](../src-tauri/src/frame.rs)). Readers skip layer kinds
they do not know by word count ([frame.ts:49-55](../src/ui/frame.ts)).

## Components

| Component | What it is | Where |
|---|---|---|
| `glife-engine` | Pure library: grid storage, B3/S23 step, seeded random fill, region copy | [src/engine/](../src/engine/) |
| `glife-app` | Tauri backend: simulation thread, commands, frame encoder | [src-tauri/](../src-tauri/) |
| UI | React app: toolbar, status bar, shortcuts, drawing strokes | [src/ui/](../src/ui/) |
| Grid view | three.js quad with a fragment shader that draws cells, grid lines and the zoomed-out shading | [src/ui/view/](../src/ui/view/) |
| Command contract | `invoke` wrappers ↔ Tauri commands | [backend.ts](../src/ui/backend.ts) ↔ [commands.rs](../src-tauri/src/commands.rs) |

## Not built yet

Phases 2–7 in [roadmap.md](roadmap.md): patterns, saving and opening, undo,
non-square tilings, closed surfaces, 3D, other rules and the infinite universe.

## Known issues / accepted debt

- Phase 1 Verification, including performance, has run on macOS only; Windows
  and Linux are unchecked.
- Cells are drawn only through the shader; no automated test covers rendering.
- Release builds run as an unbundled binary; there are no installers or signing.
