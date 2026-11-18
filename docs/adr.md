<!-- kit:core/docs/adr v1 -->
# Architecture decisions

What was decided, and what follows from it. What the system *is* today lives in
[architecture.md](architecture.md).

Entries are **append-only**. A reversed decision gets a new entry and the old
one is marked `Superseded by ADR-NNN` — never rewritten. Each entry states the
decision and its implications. `Guardrail` appears only where the bare
statement would read as an oversight someone might helpfully "fix". Extended
reasoning belongs in its own file, linked from the entry.

---

### ADR-001 — glife is a Tauri 2 desktop app for macOS, Windows and Linux

**Decision.** The app is built on Tauri 2: a Rust backend process and a UI
running in the operating system's webview. One codebase targets macOS, Windows
and Linux.

**Implications.**
- The UI is tested against three webviews: WKWebView (macOS), WebView2
  (Windows) and WebKitGTK (Linux).
- Webview features missing on any of the three, WebGPU among them, are not
  relied on.

### ADR-002 — The simulation engine is a Rust library running in the Tauri backend

**Decision.** The engine is a standalone Rust library with no dependency on
Tauri or the UI. The Tauri backend hosts it and runs it on threads of its own,
separate from the UI.

**Implications.**
- The UI never runs a generation step; it sends commands and receives cells.
- The engine may use native threads and as much memory as the machine has.
- Moving the engine into the webview as WASM later replaces only the glue
  between engine and UI, not the engine.
- A browser version of the app is not supported.

### ADR-003 — Engine-to-UI transfer uses raw binary messages, limited to the cells in view

**Decision.** Cell data travels from the backend to the UI as raw bytes over
Tauri 2 binary IPC, never as JSON. Only the cells the current view needs are
sent.

**Implications.**
- The UI decodes a binary layout shared with the backend; changing that layout
  means changing both sides.
- Large or unbounded universes send a viewport or a summary, never the whole
  universe.

**Guardrail.** Don't switch cell transfer to JSON `invoke` results; JSON is too
slow for per-frame grids.

### ADR-004 — The UI is React with TypeScript

**Decision.** Panels, toolbars and dialogs in the webview are written in React
with TypeScript.

**Implications.**
- The three.js scene is hosted inside the React UI.

### ADR-005 — Rendering is WebGL through three.js, one renderer for every geometry

**Decision.** Square, hexagonal, triangular, closed-surface and 3D universes
are all drawn with three.js on WebGL.

**Implications.**
- No separate Canvas 2D renderer exists for the square grid.
- Large grids rely on GPU instancing, or another GPU-side technique, rather than
  drawing cells one by one.

### ADR-006 — Alive cells are bit-packed; per-cell attributes live in separate layers

**Decision.** The engine stores alive cells as one bit each, 64 per `u64`, row
after row. Any per-cell attribute, such as age in Phase 2, is stored in a
separate layer and never enters the generation step.

**Implications.**
- Padding bits past a row's width are always zero; every writer keeps them so
  ([grid.rs:1-3](../src/engine/src/grid.rs)).
- The step works on whole words, not single cells
  ([step.rs:1-12](../src/engine/src/step.rs)).
- The frame carries each layer as its own kind, and readers skip kinds they do
  not know ([frame.ts:62-72](../src/ui/frame.ts)); a new attribute adds a kind
  without changing the alive-bits layer.

### ADR-007 — One simulation thread owns the universe; every change is a command

Guardrail superseded by ADR-011.

**Decision.** The backend runs one thread that owns the `Universe`. Play,
pause, step, speed, edits, fills, edge changes and new universes arrive on its
command queue and are applied between generations, in arrival order. The
thread publishes an immutable snapshot after each command, and at most every
8 ms while running.

**Implications.**
- Nothing else mutates the universe
  ([sim.rs:153-235](../src-tauri/src/sim.rs)).
- At a set speed, the thread waits for each generation's deadline and, when
  more than one interval behind, resets the deadline instead of catching up
  ([sim.rs:313-346](../src-tauri/src/sim.rs)). "As fast as possible" steps
  back to back.
- `step` is ignored while playing, and `new_universe` pauses
  ([sim.rs:153-168](../src-tauri/src/sim.rs)).
- Achieved gen/s is counted over a one-second window
  ([sim.rs:17-18](../src-tauri/src/sim.rs)).
- The random seed comes from the OS in the backend; the engine takes it as an
  argument and stays deterministic
  ([sim.rs:393-395](../src-tauri/src/sim.rs),
  [lib.rs:211](../src/engine/src/lib.rs)).

**Guardrail.** `frame` is the one command that bypasses the queue; it reads the
latest snapshot ([commands.rs:90-94](../src-tauri/src/commands.rs)). Don't route
it through the queue.

### ADR-008 — The UI pulls the visible region once per animation frame; the GPU shades zoomed-out pixels

Amended by ADR-009 (layers) and ADR-011 (commands may return values).

**Decision.** On every animation frame the UI requests the cells in view with
`frame(x0, y0, w, h, since)`, with at most one request in flight. The backend
clips the region to the grid and aligns it to 32 columns. The UI uploads the
bits as an `R32UI` texture, and one fragment shader draws cells, grid lines and
the boundary. When a cell is smaller than a pixel, the shader shades the pixel
by the share of live cells beneath it.

**Implications.**
- A frame is at most the whole grid's bits: 512 KiB at 2048 × 2048.
- Zoomed-out shading is done on the GPU; the backend never downsamples
  ([gridShader.ts:96-119](../src/ui/view/gridShader.ts)).
- Beyond 16 cells per pixel the shade samples a 16 × 16 block, not every cell
  ([gridShader.ts:97](../src/ui/view/gridShader.ts)).
- The UI sends `since` only while the requested region equals the uploaded one,
  and a sentinel that matches no version otherwise
  ([GridView.tsx:136-137](../src/ui/view/GridView.tsx),
  [backend.ts:14-15](../src/ui/backend.ts)).
- Commands other than `frame` return no data
  ([backend.ts:17-154](../src/ui/backend.ts)); the UI learns state only from
  frame headers.

### ADR-009 — Cells carry an age and a trail counter in layers outside the step

**Decision.** The engine keeps one `u16` age and one `u8` trail counter per
cell, always, whatever the UI shows. Ages saturate at 65 535 and travel as
`min(age, 255)` bytes; the trail counts generations since the rule killed the
cell, saturating at 255, and 0 means none. Neither layer enters the B3/S23
step. → ADR-006

**Implications.**
- A rule birth or an edit birth gives age 0; a survivor gets age + 1; a dead
  cell keeps its age.
- A rule death sets the trail to 1. Any birth, `clear`, `random_fill` and
  restore set it to 0, as do kill and Copy placing across their whole
  rectangle ([region.rs:144-186](../src/engine/src/region.rs)). Erasing one
  already-dead cell keeps its trail ([lib.rs:178-195](../src/engine/src/lib.rs)).
- Age 0 is white in every palette ([palettes.ts:6](../src/ui/color/palettes.ts)).
- `frame` sends the age and trail layers only when asked for
  ([frame.rs:38-80](../src-tauri/src/frame.rs)); a frame grows with each layer.
- The UI sends `NO_VERSION` when the requested layers change
  ([GridView.tsx:136-137](../src/ui/view/GridView.tsx)).
- Flip moves ages and trail with their cells; restoring a capture sets the
  trail to 0.

### ADR-010 — Undo history lives on the simulation thread and ends at the first generation

**Decision.** The simulation thread holds the undo history, up to 50 steps,
each a before and after capture with its generation and revision. Any
generation, `new_universe` and `open` clear it. A change made while playing is
applied, not recorded, and clears it. Undo and redo are unavailable while
playing. → ADR-007

**Implications.**
- A stroke is every `edit` with the same `stroke` id and no other command
  between ([sim.rs:153-158](../src-tauri/src/sim.rs)).
- A capture holds alive bits and the ages of live cells only; restored dead
  cells get age 0 and trail 0 ([region.rs:246-266](../src/engine/src/region.rs)).
- Every change gives the universe a new revision; undo and redo restore the
  entry's revision. The universe is unsaved when its revision differs from the
  saved one, and the flag travels in frame word 1 bit 3
  ([sim.rs:354-391](../src-tauri/src/sim.rs)).
- `set_edges` is not recorded; it marks the universe unsaved
  ([sim.rs:177-184](../src-tauri/src/sim.rs)).

### ADR-011 — Read-only commands read the latest snapshot or the pending slot

**Decision.** Commands that change the universe go through the simulation
thread's queue. Commands that only read take the latest snapshot or the pending
pattern slot directly: `frame`, `pending_cells`, `copy_selection`, `save`,
`save_as`, `export_rle`, and `library_save` from a selection. A save sends its
revision back to the thread as `MarkSaved`. Supersedes the Guardrail of ADR-007.

**Implications.**
- A read sees the universe as of the latest publish, not as of the latest
  queued command.
- Commands may return values; the UI still learns universe state from frame
  headers only.
- The pending pattern lives in `Host`, outside the queue
  ([commands.rs:45](../src-tauri/src/commands.rs)); `place` sends the thread an
  `Arc` of it.

### ADR-012 — The glife file and the pattern file are versioned, and readers tolerate additions

**Decision.** `.glife` universe files and pattern TOML files carry `format = 1`.
A reader ignores unknown header keys, skips unknown sections, and refuses a
higher `format` with a message to update glife
([glife_file.rs:121-137](../src-tauri/src/glife_file.rs),
[pattern_file.rs:66-79](../src-tauri/src/pattern_file.rs)).

**Implications.**
- New data is a new section or key; `format` rises only for an incompatible
  change.
- Files written by an earlier format keep opening
  ([CLAUDE.md](../CLAUDE.md) § Cross-cutting rules).
- A glife file stores the ages of live cells; trail is not saved.
- Writes are a temporary file renamed over the target, so a failed write keeps
  the old file ([glife_file.rs:56-66](../src-tauri/src/glife_file.rs)).

### ADR-013 — A pattern is one TOML file; the built-in library is a folder checked on load

**Decision.** Both libraries hold one pattern file per pattern. The built-in
library is the `patterns/` folder shipped as a resource and read at start; each
file is checked against its own stated facts, and a file that fails is listed
as a problem and not offered
([library/builtin.rs:77-130](../src-tauri/src/library/builtin.rs)). The personal
library is the `patterns/` folder in the app data directory.

**Implications.**
- A built-in pattern is added by dropping a file in the folder and restarting.
- A test runs the check over every shipped file
  ([builtin.rs:280](../src-tauri/src/library/builtin.rs)).
- The name inside a personal file is the identity; names are unique ignoring
  case.

### ADR-014 — Dialogs, clipboard, file access and menus live in the backend

**Decision.** The backend opens dialogs, reads and writes the clipboard and
files, and builds the menu bar, through the Tauri dialog, clipboard-manager and
opener plugins ([main.rs:47-49](../src-tauri/src/main.rs)). The UI sees file
names, never paths. Unsaved-changes prompts are native dialogs run on quit,
window close and `ExitRequested`
([main.rs:59-64](../src-tauri/src/main.rs), [menu.rs:134-146](../src-tauri/src/menu.rs)).

**Implications.**
- While a text field has focus the UI sends `set_edit_target`, and the backend
  swaps the Edit menu for the platform's text items
  ([menu.rs:148-205](../src-tauri/src/menu.rs)).
- On macOS the web view takes Cmd+Shift+Z before the Edit menu, so the UI
  handles it as a fallback ([shortcuts.ts:11-12](../src/ui/controls/shortcuts.ts)).

**Guardrail.** Don't replace the Edit-menu swap with fixed items; text fields
lose their native shortcuts.
