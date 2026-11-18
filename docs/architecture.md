<!-- kit:core/docs/architecture v1 -->
# Architecture

What glife is, today. Decisions behind it are in [adr.md](adr.md).

## Purpose and data flow

glife runs Conway's Game of Life (B3/S23) on a bounded square grid and lets the
operator draw, run, step, zoom and pan it.

1. The operator acts in the UI, or in a native menu. Each action is a JSON
   `invoke` command ([src/ui/backend.ts:17-154](../src/ui/backend.ts) ↔ the
   handlers registered at [src-tauri/src/main.rs:67-104](../src-tauri/src/main.rs)).
   Menu clicks reach the UI as a `menu` event
   ([menu.rs:125-130](../src-tauri/src/menu.rs)).
2. Commands that change the universe are queued to the simulation thread, which
   applies them between generations and publishes an immutable snapshot
   ([src-tauri/src/sim.rs:153-235](../src-tauri/src/sim.rs),
   [sim.rs:354-391](../src-tauri/src/sim.rs)). → ADR-007
3. On every animation frame the UI pulls the cells in view and the layers it
   needs with `frame` and uploads them to the GPU, which draws them
   ([src/ui/view/GridView.tsx:130-207](../src/ui/view/GridView.tsx),
   [gridShader.ts](../src/ui/view/gridShader.ts)). → ADR-008

## Requirements

What the first phase must do and its targets are in
[glife-phase-1.md](glife-phase-1.md); repo-wide requirements are in
[roadmap.md](roadmap.md#cross-cutting-requirements). What stage 2 builds is in
[glife-phase-2.md](glife-phase-2.md).

- A universe side is 3 to 2048 cells
  ([lib.rs:23-25](../src/engine/src/lib.rs)).

## Topology

One Tauri process (ADR-001): the webview runs the UI; the Rust backend runs
the `simulation` thread, which owns the only `Universe`, and the dialog,
clipboard and opener plugins
([main.rs:38-49](../src-tauri/src/main.rs)). `Host` is built in the app's
`setup` and holds the queue, the latest snapshot, the pending pattern slot, the
file state and both libraries
([main.rs:50-56](../src-tauri/src/main.rs),
[commands.rs:41-55](../src-tauri/src/commands.rs)). The menu bar is built in
`setup` ([menu.rs:36](../src-tauri/src/menu.rs)). Commands that change the
universe go to the thread's queue; commands that only read take the latest
snapshot or the pending slot. → ADR-011

## Data model

**Universe.** Alive cells are bit-packed, 64 per `u64`, row after row; padding
bits past the grid's width are always zero
([grid.rs:1-3](../src/engine/src/grid.rs)). Each cell also has an age (`u16`)
and a trail counter (`u8`), kept in separate layers, plus a bit layer marking
the cells whose counter is 1 to 254. A universe holds its edge mode (`Wrap` or
`Dead`), generation and live-cell count
([lib.rs:56-72](../src/engine/src/lib.rs)). → ADR-006, ADR-009

**Snapshot.** Version, a copy of the universe, playing flag, achieved gen/s,
revision, and the dirty, can-undo and can-redo flags
([sim.rs:21-32](../src-tauri/src/sim.rs)).

**Frame layout v1.** The reply to `frame`. Little-endian 32-bit words, so the UI
reads it as a `Uint32Array`. Encoder:
[frame.rs:20-80](../src-tauri/src/frame.rs). Decoder:
[frame.ts:36-72](../src/ui/frame.ts). Fixtures:
[frame-v1.bin](../fixtures/frame-v1.bin) (alive bits only),
[frame-v1-layers.bin](../fixtures/frame-v1-layers.bin) (both layers).

| Word | Field |
|---|---|
| 0 | format version = 1 |
| 1 | flags: bit 0 = playing, bit 1 = undo available, bit 2 = redo available, bit 3 = unsaved changes |
| 2–3 | snapshot version, u64 (lo, hi) |
| 4–5 | generation, u64 (lo, hi) |
| 6 | live-cell count |
| 7 | achieved gen/s, f32 |
| 8–9 | grid width, grid height |
| 10 | edges: 0 = wrap, 1 = dead |
| 11–14 | region x0, y0, w, h (x0 and w multiples of 32) |
| 15 | layer count |
| then, per layer | kind (0 = alive bits, 1 = age, 2 = trail), word count, data |

Alive-bits data is `h` rows of `w / 32` words; bit `i` of word `k` in row `r` is
cell `(x0 + 32k + i, y0 + r)`; bits past the grid's width are zero
([lib.rs:243](../src/engine/src/lib.rs)). Age and trail data are `h` rows of `w`
bytes, four cells per word, cell `x0 + 4j + b` in byte `b` of word `j`, with
the age clamped to 255 ([lib.rs:262-279](../src/engine/src/lib.rs)). `frame`
takes a `layers` bitmask (bit 0 = age, bit 1 = trail); alive bits are always
sent ([frame.rs:12-13](../src-tauri/src/frame.rs)). The reply's region is the
request clipped to the grid and aligned to 32 columns. When `since` equals the
snapshot version the reply has zero layers; any other `since` gets a full
reply ([frame.rs:68-80](../src-tauri/src/frame.rs)). Readers skip layer kinds
they do not know by word count ([frame.ts:62-72](../src/ui/frame.ts)).

**Pattern buffer.** The layout of a pattern's cells going to the UI: `u32` LE
words `[width, height]`, then `height` rows of `ceil(width / 32)` words, bit `i`
of word `k` in row `r` being cell `(32k + i, r)`
([frame.ts:83-90](../src/ui/frame.ts),
[commands.rs:222-227](../src-tauri/src/commands.rs)). It carries placements,
pastes, imports and library previews.

**Pending pattern.** One slot in `Host`, filled by paste, import or choosing a
library entry, and read by `pending_cells` and `place`
([commands.rs:45](../src-tauri/src/commands.rs),
[commands.rs:78-99](../src-tauri/src/commands.rs)). A pattern larger than the
grid is refused when the slot is filled.

**Glife file.** `.glife`: the 8-byte magic `GLIFE\0\r\n`, a `u32` LE header
length, a JSON header (`format`, `tiling`, `width`, `height`, `edges`, `rule`,
`generation`, `sections`), then deflated `alive` and `ages` sections. `ages`
holds the live cells' ages only; no trail is saved
([glife_file.rs:74-113](../src-tauri/src/glife_file.rs)). Fixture:
[universe-v1.glife](../fixtures/universe-v1.glife). → ADR-012

**Pattern file.** One TOML file per pattern: `format`, `tiling`, `name`,
`comment`, `created`, `changed`, `rle`; built-in files add category,
description and facts about the pattern
([pattern_file.rs:9-98](../src-tauri/src/pattern_file.rs),
[library/builtin.rs:77-130](../src-tauri/src/library/builtin.rs)). Fixture:
[pattern-v1.toml](../fixtures/pattern-v1.toml). → ADR-012, ADR-013

**Libraries.** Built-in: the `patterns/` folder, shipped as a Tauri resource
([tauri.conf.json:28](../src-tauri/tauri.conf.json)), found by
[main.rs:22-35](../src-tauri/src/main.rs), source in
[src-tauri/patterns/](../src-tauri/patterns/). Personal: a `patterns/` folder in
the app data directory ([main.rs:53](../src-tauri/src/main.rs),
[library/personal.rs](../src-tauri/src/library/personal.rs)). → ADR-013

**Undo history.** Up to 50 steps on the simulation thread, each holding before
and after captures, generation and revision
([history.rs:9-37](../src-tauri/src/history.rs)). → ADR-010

## Components

| Component | What it is | Where |
|---|---|---|
| `glife-engine` | Pure library: grid storage, B3/S23 step, age and trail layers, patterns and regions, captures, seeded random fill | [src/engine/](../src/engine/) |
| `glife-app` | Tauri backend: simulation thread, commands, frame encoder | [src-tauri/](../src-tauri/) |
| History | Undo and redo steps on the simulation thread | [history.rs](../src-tauri/src/history.rs) |
| Formats | RLE codec, glife file, pattern file, RLE-as-universe | [rle.rs](../src-tauri/src/rle.rs), [glife_file.rs](../src-tauri/src/glife_file.rs), [pattern_file.rs](../src-tauri/src/pattern_file.rs), [open_rle.rs](../src-tauri/src/open_rle.rs) |
| Libraries | Built-in and personal pattern libraries | [src-tauri/src/library/](../src-tauri/src/library/), [library_cmds.rs](../src-tauri/src/library_cmds.rs) |
| Files, clipboard, menus | Dialogs, file access, clipboard, menu bar, unsaved-changes prompt | [files.rs](../src-tauri/src/files.rs), [clipboard.rs](../src-tauri/src/clipboard.rs), [menu.rs](../src-tauri/src/menu.rs) |
| UI | React app: toolbar, status bar, shortcuts, drawing strokes, selection and placing, color controls | [src/ui/](../src/ui/) |
| Library panel | Built-in and personal lists, SVG previews, save dialog | [src/ui/library/](../src/ui/library/) |
| Grid view | three.js quad with a fragment shader that draws cells, ages, trails, grid lines, the placement preview, the selection outline and the zoomed-out shading | [src/ui/view/](../src/ui/view/) |
| Command contract | `invoke` wrappers ↔ Tauri commands | [backend.ts](../src/ui/backend.ts) ↔ [main.rs:67-104](../src-tauri/src/main.rs) |

## Not built yet

Phases 3–7 in [roadmap.md](roadmap.md): non-square tilings, closed surfaces, 3D,
other rules and the infinite universe.

## Known issues / accepted debt

- Phase 1 and Phase 2 Verification have run on macOS only; Windows and Linux
  are unchecked.
- Phase 2's frame-rate measurement ran on the fit view only, and the Golly RLE
  round-trip checks were not run because Golly is not installed.
- In bench mode (`GLIFE_BENCH=1`) the toolbar shows Speed 10 while the backend
  runs as fast as possible, because frames do not carry the speed
  ([sim.rs:97-108](../src-tauri/src/sim.rs), [state.ts:54](../src/ui/state.ts)).
- Cells are drawn only through the shader; no automated test covers rendering.
- Release builds run as an unbundled binary; there are no installers or signing.
