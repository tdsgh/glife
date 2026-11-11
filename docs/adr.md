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
