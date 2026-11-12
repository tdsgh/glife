# glife stage 1 — build plan

Builds [glife-phase-1.md](glife-phase-1.md). Run with `/kit:run-build-plan`.

## Context

The repository holds documentation, kit rules and the `engine` and `ui` agent
definitions. There is no code and no test command. The stack is fixed by
ADR-001 to ADR-005: Tauri 2, a pure Rust engine in the backend, raw binary
transfer of the cells in view, React with TypeScript, three.js on WebGL.

## Decisions taken

- **Layout.** A Cargo workspace with two crates: `src/engine/` (`glife-engine`,
  pure library) and `src-tauri/` (`glife-app`, the Tauri backend). The UI lives
  in `src/ui/` and is built by Vite from that directory. `package.json` and the
  Vite and TypeScript configs sit at the repository root.
- **Agents.** `engine` owns `src/engine/`, `ui` owns `src/ui/`, and a new
  `host` agent owns `src-tauri/` and the root build files.
- **Engine storage.** Alive cells are bit-packed, 64 per `u64` word, one row
  after another; the padding bits past the grid's width are always zero. A
  generation is computed with bitwise adders over whole words. Attributes such
  as age (Phase 2) go in a separate per-cell layer and never change the step.
- **One simulation thread.** The backend runs one thread that owns the
  `Universe`. Every change (play, pause, step, speed, edits, fills, new
  universe) arrives on its command queue and is applied between generations,
  in arrival order. The thread publishes an immutable snapshot after each
  command, and at most every 8 ms while running.
- **Pacing.** At a chosen speed, the thread waits until each generation's
  deadline and handles commands while it waits. When it falls more than one
  interval behind, it resets the deadline to now instead of catching up in a
  burst. "As fast as possible" steps back to back and checks the queue between
  generations. Achieved gen/s is counted over a sliding one-second window.
- **Transfer.** On every animation frame, the UI pulls the cells in view with
  `frame(x0, y0, w, h, since)` and keeps at most one request in flight. The
  region is clipped to the grid and aligned to 32 columns. The reply is raw
  bytes (`tauri::ipc::Response`) in the layout below. When the snapshot version
  equals `since`, the reply is the header with zero layers.
- **Rendering.** One full-window quad with a GLSL3 `ShaderMaterial` reads the
  region as an `R32UI` integer texture. When a cell is at least one pixel, the
  shader draws cells, grid lines (fading in between 6 and 12 px per cell) and
  the grid boundary. When a cell is smaller than a pixel, it counts the live
  cells under the pixel (a k×k loop, k capped at 16) and shades by their share,
  with a minimum shade for any live cell.
- **Commands from the UI** are ordinary JSON `invoke` calls: `new_universe`,
  `play`, `pause`, `step`, `set_speed`, `set_edges`, `edit`, `clear`,
  `random_fill`. Only `frame` returns raw bytes.
- **Randomness.** The backend draws a seed from the OS; the engine fills from
  that seed with SplitMix64, so the engine stays pure and deterministic.
- **Shortcuts.** Enter: play/pause. N: step. R: random fill. Backspace/Delete:
  clear. F: fit to window. Space is only the pan modifier.
- **Open question §13 (discard confirmation)** is built without confirmations.
  Adding one later changes only WP5.
- **Three-OS checks.** Gates check macOS only. Windows and Linux are checked
  once, in Verification, before the roadmap marks Phase 1 `Done`.

### Frame layout v1

Little-endian, 4-byte aligned, so the UI reads the buffer as a `Uint32Array`.

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

Alive-bits data: `h` rows of `w / 32` words; bit `i` of word `k` in row `r` is
cell `(x0 + 32k + i, y0 + r)`. Bits past the grid's width are zero. Phase 2
adds an age layer as a new kind; readers skip unknown kinds by word count.

## Step routing at a glance

| Step | Agent | Model | Gate | Parallel |
|---|---|---|---|---|
| WP1 Workspace skeleton | `host` | sonnet | test | — |
| WP2 Engine | `engine` | opus | test | yes, with WP3 |
| WP3 Grid view | `ui` | sonnet | test | yes, with WP2 |
| WP4 Simulation host and measurement | `host` | sonnet | human | — |
| WP5 Controls and status | `ui` | sonnet | test | — |
| WP6 Drawing | `ui` | sonnet | human | — |
| WP7 Docs and rules | `kit:docs-writer` | sonnet | human | — |

Test command for every `test` gate: `cargo test --workspace && npm test`.

## WP1 — Workspace skeleton
> `agent: host` · `model: sonnet` · `gate: test` · `reads: Decisions taken`

### Root
- `Cargo.toml`: workspace with members `src/engine` and `src-tauri`, plus
  `[profile.dev.package.glife-engine] opt-level = 3`, so dev runs stay usable.
- `package.json`: React, React DOM, three, `@tauri-apps/api`; dev dependencies
  `@tauri-apps/cli`, Vite with the React plugin, TypeScript, `@types/three`,
  Vitest, jsdom, Testing Library. Scripts `dev`, `build`, `test`
  (`vitest run`) and `tauri`. All UI dependencies come in here, so WP3 and
  WP5 leave `package.json` alone.
- `vite.config.ts` (root `src/ui`, output `dist/`, Vitest with jsdom) and
  `tsconfig.json` (strict).
- `.gitignore`: `target/`, `node_modules/`, `dist/`.

### `src/engine/`
`Cargo.toml` for `glife-engine` with no dependencies, and `src/lib.rs` with one
placeholder test.

### `src-tauri/`
`glife-app` created with `tauri init` (Tauri 2): `tauri.conf.json` (product
name glife, 1280×800 window, `frontendDist: ../dist`, dev URL from Vite),
default capabilities, icons, and `src/main.rs` opening the window. It depends on
`glife-engine` by path.

### `src/ui/`
`index.html`, `main.tsx` and an `App.tsx` that renders an empty full-window
layout, with one placeholder test.

### Tests
Both suites run green with their placeholder tests. `npm run build` succeeds.

## WP2 — Engine
> `agent: engine` · `model: opus` · `gate: test` · `parallel: yes` · `reads: §4, §5, §6, §8, §11, Decisions taken`

### `src/engine/src/lib.rs`
Public API, and nothing else is public:
- `MIN_SIDE = 3`, `MAX_SIDE = 2048`; `enum Edges { Wrap, Dead }`;
  `SizeError`.
- `Universe::new(width, height, edges) -> Result<Universe, SizeError>`.
- Readers: `width`, `height`, `edges`, `generation`, `live_count`,
  `get(x, y)`.
- `set_edges(edges)`; `set(x, y, alive)`, which keeps `live_count` exact.
- `clear()` and `random_fill(density, seed)`, both resetting the generation to 0.
- `step()`: one generation; increments the generation and recounts live
  cells with popcount.
- `copy_region(x0, y0, w, h, out: &mut Vec<u32>)`: the alive bits of a region
  aligned to 32 columns, in frame layout v1 order.

### `src/engine/src/grid.rs`, `step.rs`, `rng.rs`
- `grid.rs`: the row-major `u64` storage and padding mask.
- `step.rs`: the bitwise step. It handles neighbours across word boundaries,
  across the row end for wrap at any width (including widths that are not
  multiples of 64), and zero rows and columns for dead edges.
- `rng.rs`: SplitMix64; a cell is alive when its draw is below `density`.

### Tests
- One test per rule: underpopulation, survival with 2 and 3, overpopulation,
  birth.
- Patterns: blinker period 2; block still; glider moves one cell diagonally
  every 4 generations; a glider on wrapping N×N grids (N = 8, 64, 65)
  returns to its start after 4N generations.
- Dead edges: a glider driven into a corner never sets a padding bit or a cell
  outside the grid, and `live_count` always equals a full popcount.
- Reference check: a naive cell-by-cell stepper inside the test file must agree
  with `step()` for 20 generations of random grids with widths
  3, 5, 31, 63, 64, 65, 127, 128, 129, 200 and heights 3, 7, 64, in both
  edge modes.
- `random_fill`: the same seed gives the same grid; density 0.25 on 2048²
  lands within ±1 %.
- `copy_region`: partial last word, region at the grid's right edge, and a
  single row.
- `new`: sizes 2 and 2049 are rejected; 3 and 2048 are accepted.
- An `#[ignore]` timing test: 100 steps of a 2048² random grid in release mode,
  reporting ms per step. WP2 reports the number. Target: under 5 ms on the
  dev machine.

## WP3 — Grid view
> `agent: ui` · `model: sonnet` · `gate: test` · `parallel: yes` · `reads: §9, §10, Decisions taken, Frame layout v1`

### `src/ui/backend.ts`
Typed wrappers over `invoke` for every command in Decisions taken, and
`fetchFrame(region, since)` returning an `ArrayBuffer`.

### `src/ui/frame.ts`
`decodeFrame(buffer)` turns frame layout v1 into a header object and the
alive-bits `Uint32Array` (a view, not a copy). Unknown layer kinds are skipped.

### `src/ui/view/viewport.ts`
Pure camera math over `{ centerX, centerY, pxPerCell }`:
- `fit`;
- `zoomAt(screenPoint, factor)`, which keeps the cell under the pointer fixed;
- zoom clamped between fit and 64 px per cell;
- `panBy`, clamped so the grid's edge never goes more than half a window
  past the view's edge;
- `resize`, which keeps the centre;
- `screenToCell`;
- `visibleRegion`, clipped to the grid and aligned to 32 columns.

### `src/ui/view/gridShader.ts`
The vertex and fragment shaders from Decisions taken. Uniforms: cell texture,
region origin and size, grid size, view transform, pixels per cell, colours.

### `src/ui/view/GridView.tsx`
- Hosts the three.js renderer, orthographic camera and quad in a canvas that
  fills its parent.
- Runs the animation-frame loop: request a frame unless one is in flight,
  upload a changed region into the `R32UI` `DataTexture` (reallocating only
  when its size changes), and render every frame.
- Handles zoom and pan input: a wheel event with `ctrlKey` (trackpad pinch)
  zooms; any other wheel event pans. Right or middle
  drag pans, and so does Space with a left drag.
- Shows a hidden perf overlay toggled with the backquote key: fps, frame
  request time in ms, bytes per frame, gen/s.

### `src/ui/App.tsx`
Mounts `GridView` full-window.

### Tests
- `frame.test.ts`: decodes buffers built per frame layout v1, including the
  zero-layer reply and an unknown layer kind.
- `viewport.test.ts`: fit on wide and tall windows; zoom-at-point invariance;
  both zoom clamps; pan clamp; resize keeps the centre; region alignment and
  clipping at every grid edge.

## WP4 — Simulation host and measurement
> `agent: host` · `model: sonnet` · `gate: human` · `reads: §7, §11, Decisions taken, Frame layout v1`

### `src-tauri/src/sim.rs`
- A `Sim` struct holding the `Universe`, playing state, speed, gen/s window
  and snapshot version.
- `Sim::handle(command)` and `Sim::tick(now) -> next wake-up` contain all the
  logic. A thin thread loop around them uses `recv_timeout`.
- Snapshots are `Arc`s swapped under a mutex, published per Decisions taken.
- At launch: a 256 × 256 universe with wrapping edges, paused.
- With `GLIFE_BENCH=1` set, launch instead with 2048 × 2048, random fill
  25 %, playing as fast as possible.

### `src-tauri/src/frame.rs`
Encodes a snapshot region into frame layout v1.

### `src-tauri/src/commands.rs`, `main.rs`
- Tauri commands from Decisions taken, which forward to the sim thread's queue.
- `frame` reads the latest snapshot and returns `tauri::ipc::Response`.
- `main.rs` registers them and starts the thread.

### Tests
- `Sim` driven with a fake clock:
  - speed 10 gives 10 generations per simulated second;
  - falling behind does not burst;
  - edits between ticks apply in order;
  - step is ignored while playing;
  - fill and clear reset the generation;
  - `set_edges` applies from the next generation.
- `frame.rs`: byte-for-byte against `fixtures/frame-v1.bin`, a 64 × 2 region
  with a known pattern written by this test's first run and committed. Also
  the zero-layer reply.

### Human gate — measurement on macOS
Build with `npm run tauri build -- --no-bundle`, then run the binary with
`GLIFE_BENCH=1`. Open the overlay with backquote and record fps, request ms
and gen/s:
- fitted to the window;
- at about 4 px per cell;
- while panning.

The gate passes at 60 fps and at least 30 gen/s. If it fails, revisit the
transfer decision before WP5.

## WP5 — Controls and status
> `agent: ui` · `model: sonnet` · `gate: test` · `reads: §4, §5, §7, §8, §10`

### `src/ui/state.ts`
A small store (`useSyncExternalStore`) holding the latest frame header and the
UI-only settings (speed choice, fill density).

### `src/ui/controls/Toolbar.tsx`
- New universe…
- Play/Pause.
- Step, disabled while playing.
- Speed select: 1, 2, 5, 10, 15, 30, 60, 120, 240, Max.
- Edges toggle (Wrap/Dead).
- Random fill, with a density input (1–99, default 25).
- Clear.
- Fit.

### `src/ui/controls/NewUniverseDialog.tsx`
Width and height inputs (3–2048, validated) and the edge choice. Confirming
creates the universe and fits the view.

### `src/ui/controls/StatusBar.tsx`
Generation, live cells, gen/s (while playing), W × H, edges, B3/S23.

### `src/ui/controls/shortcuts.ts`
The keys from Decisions taken, ignored while focus is in a text field.

### Tests
- Every control and shortcut sends its command, with IPC mocked through
  `@tauri-apps/api/mocks`.
- The dialog rejects 2 and 2049.
- The status bar renders a decoded header.
- `fixtures/frame-v1.bin` from WP4 decodes to its known pattern.

## WP6 — Drawing
> `agent: ui` · `model: sonnet` · `gate: human` · `reads: §8`

### `src/ui/edit/stroke.ts`
- `cellsBetween(a, b)`: the 8-connected line of cells from a to b, both
  included.
- `strokeMode(aliveAtStart)`: draw when the first cell is dead, erase when it
  is alive.

### `src/ui/view/GridView.tsx`
- A left press without Space starts a stroke. The mode comes from the cell's
  bit in the last frame.
- Each pointer move sends one `edit` batch with the cells since the last
  position, deduplicated, using coalesced pointer events.

### Tests
- `cellsBetween`: horizontal, vertical, diagonal and steep lines, and a single
  point.
- `strokeMode`.
- A simulated drag sends the expected batches.

### Human gate — macOS click-through
Run every item in Verification on macOS with `npm run tauri dev`, except the
performance item, which needs the release build from WP4.

## WP7 — Docs and rules
> `agent: kit:docs-writer` · `gate: human`

Proposes, for approval:
- `docs/architecture.md`: purpose and data flow, components table, data model
  (storage and frame layout v1), and "Not built yet" pointing at Phases 2–7.
- `docs/adr.md`, three new entries:
  - ADR-006: bit-packed alive cells, attributes in separate layers;
  - ADR-007: one simulation thread owns the universe, and every change is a
    command;
  - ADR-008: the UI pulls the visible region once per animation frame, and the
    GPU shades zoomed-out pixels.
- Root `CLAUDE.md`:
  - Test command `cargo test --workspace && npm test`;
  - Subtrees: none;
  - Command index rows for running the app (`npm run tauri dev`), the release
    build, and the engine timing test;
  - Execution guardrails: replace the TODO with `git push`.
- `.claude/agents/engine.md`, `ui.md`, `host.md`: fill the `TODO(kit)` layer
  rules and the test commands.
- `docs/roadmap.md`: Phase 1 to `Done` once Verification has passed on all three
  operating systems.

## Verification

On macOS at WP6, then once on Windows and Linux before Phase 1 is `Done`:

- Launch opens an empty 256 × 256 wrapping universe, fitted.
- Draw and erase strokes while playing and while paused; a fast drag leaves no
  gaps.
- Known patterns:
  - a drawn blinker oscillates and a block stays still;
  - a glider crosses a wrapping edge and comes back;
  - with dead edges, a glider stops at the boundary.
- Speed 1 visibly steps once a second; Max shows the achieved gen/s.
- Switching edges during a run takes effect at once.
- New universe 2048 × 2048 → random fill → Max:
  - zoomed out, the shaded view is readable;
  - zoom, pan and Fit respond immediately;
  - the overlay shows 60 fps and at least 30 gen/s on a release build.
- Every shortcut works; none fires while typing in the dialog.

## Deliberately out of scope

- Everything stage 1 lists as Out.
- Signing and installers; the release build runs as an unbundled binary.
- Multithreading in the engine, unless WP2's timing misses its target.
- Optimistic local drawing before the backend confirms an edit.
- Automated end-to-end or WebGL rendering tests; the shader is checked by the
  human gates.
