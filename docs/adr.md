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
  not know ([frame.ts:49-55](../src/ui/frame.ts)); a new attribute adds a kind
  without changing the alive-bits layer.

### ADR-007 — One simulation thread owns the universe; every change is a command

**Decision.** The backend runs one thread that owns the `Universe`. Play,
pause, step, speed, edits, fills, edge changes and new universes arrive on its
command queue and are applied between generations, in arrival order. The
thread publishes an immutable snapshot after each command, and at most every
8 ms while running.

**Implications.**
- Nothing else mutates the universe
  ([sim.rs:108-138](../src-tauri/src/sim.rs)).
- At a set speed, the thread waits for each generation's deadline and, when
  more than one interval behind, resets the deadline instead of catching up
  ([sim.rs:142-168](../src-tauri/src/sim.rs)). "As fast as possible" steps
  back to back.
- `step` is ignored while playing, and `new_universe` pauses
  ([sim.rs:110-123](../src-tauri/src/sim.rs)).
- Achieved gen/s is counted over a one-second window
  ([sim.rs:15-16](../src-tauri/src/sim.rs)).
- The random seed comes from the OS in the backend; the engine takes it as an
  argument and stays deterministic
  ([sim.rs:199-202](../src-tauri/src/sim.rs),
  [lib.rs:137-142](../src/engine/src/lib.rs)).

**Guardrail.** `frame` is the one command that bypasses the queue; it reads the
latest snapshot ([commands.rs:90-94](../src-tauri/src/commands.rs)). Don't route
it through the queue.

### ADR-008 — The UI pulls the visible region once per animation frame; the GPU shades zoomed-out pixels

**Decision.** On every animation frame the UI requests the cells in view with
`frame(x0, y0, w, h, since)`, with at most one request in flight. The backend
clips the region to the grid and aligns it to 32 columns. The UI uploads the
bits as an `R32UI` texture, and one fragment shader draws cells, grid lines and
the boundary. When a cell is smaller than a pixel, the shader shades the pixel
by the share of live cells beneath it.

**Implications.**
- A frame is at most the whole grid's bits: 512 KiB at 2048 × 2048.
- Zoomed-out shading is done on the GPU; the backend never downsamples
  ([gridShader.ts:50-64](../src/ui/view/gridShader.ts)).
- Beyond 16 cells per pixel the shade samples a 16 × 16 block, not every cell
  ([gridShader.ts:51](../src/ui/view/gridShader.ts)).
- The UI sends `since` only while the requested region equals the uploaded one,
  and a sentinel that matches no version otherwise
  ([GridView.tsx:83-86](../src/ui/view/GridView.tsx),
  [backend.ts:12-13](../src/ui/backend.ts)).
- Commands other than `frame` return no data
  ([backend.ts:15-27](../src/ui/backend.ts)); the UI learns state only from
  frame headers.
