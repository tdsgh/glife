# Roadmap

What glife will become, phase by phase. What exists today is in
[architecture.md](architecture.md); what was decided is in [adr.md](adr.md).

This file keeps the order of the phases, the scope of each at a glance, and
their status. When a phase starts it gets its own phase document, as described
in [CLAUDE.md](CLAUDE.md), and the detail moves there.

## Vision

A desktop app for personal exploration of cellular automata. It starts with
Conway's Game of Life on a square grid and grows into other cell shapes, three
dimensions, other rule families, and finally an unbounded universe.

Every phase ends with a working app: a universe can be created, run, edited and
saved at each step of the way.

## Status

| # | Phase | Status | Phase document |
|---|---|---|---|
| 1 | Classic Life | Done | [glife-phase-1.md](glife-phase-1.md) |
| 2 | Patterns | Done | [glife-phase-2.md](glife-phase-2.md) |
| 3 | Non-square tilings | Planned | — |
| 4 | Closed surfaces | Planned | — |
| 5 | 3D universes | Planned | — |
| 6 | Other rulesets | Planned | — |
| 7 | Infinite universe | Planned | — |

Status values: `Planned`, `In progress`, `Done`.

## Technology baseline

| Layer | Choice | Decision |
|---|---|---|
| Desktop shell | Tauri 2; macOS, Windows, Linux | [ADR-001](adr.md#adr-001--glife-is-a-tauri-2-desktop-app-for-macos-windows-and-linux) |
| Simulation engine | Rust library in the Tauri backend | [ADR-002](adr.md#adr-002--the-simulation-engine-is-a-rust-library-running-in-the-tauri-backend) |
| Engine → UI transfer | Raw binary, cells in view only | [ADR-003](adr.md#adr-003--engine-to-ui-transfer-uses-raw-binary-messages-limited-to-the-cells-in-view) |
| UI | React + TypeScript | [ADR-004](adr.md#adr-004--the-ui-is-react-with-typescript) |
| Rendering | WebGL through three.js | [ADR-005](adr.md#adr-005--rendering-is-webgl-through-threejs-one-renderer-for-every-geometry) |

## Phases

### Phase 1 — Classic Life

Conway's Game of Life on a bounded square grid, fully interactive.

- **In.** The app window and its controls. A square grid of a chosen size.
  Edges either wrap around (torus) or are dead. The rule B3/S23. Play, pause,
  single step, speed control. Draw and erase cells with the mouse. Random fill
  and clear. Zoom and pan. Generation count and live-cell count.
- **Out.** Saving, pattern files, any rule other than B3/S23.
- **Done when.** Known patterns behave correctly (a blinker oscillates, a block
  stays still, a glider travels), and a large grid runs smoothly on macOS.

### Phase 2 — Patterns

Working with patterns rather than single cells.

- **In.** A built-in library of well-known patterns (still lifes, oscillators,
  spaceships, guns, methuselahs), placed by clicking. A personal pattern
  library next to it, kept on the operator's machine: save a selection as a
  named pattern, add and edit a comment on it, rename or delete it, and paste
  it into any universe like a built-in pattern. Select a region; copy, cut,
  paste and flip it. Undo and redo for edits made while paused. Save and open
  universes. Import and export RLE, the text format the Life community uses to
  share patterns, including pasting RLE from the clipboard. Coloring cells by
  an attribute, starting with age: how many generations a live cell has
  survived, with an optional fading trail after death. Color palettes of 16,
  32 or 256 colors.
- **Out.** Online pattern catalogs; pattern search and analysis. Palettes
  larger than 256 colors.
- **Note.** Larger palettes may be needed later; the palette size is not fixed
  for good.

### Phase 3 — Non-square tilings

The same experience on grids whose cells are not squares.

- **In.** Hexagonal and triangular grids, chosen when a universe is created.
  Each tiling has its own neighborhood and a default rule suited to it. Every
  tool from Phases 1–2 works on these grids: drawing, selection, the pattern
  library, saving and opening.
- **Out.** Aperiodic tilings such as Penrose (see Open questions).
- **Constraint.** RLE describes square grids only, so patterns on these tilings
  need a glife file format.

### Phase 4 — Closed surfaces

2D Life on surfaces that close on themselves, with no edge anywhere.

- **In.** Universes on the surface of a sphere and on the surface of a cube.
  The surface is shown as a 3D object, with a camera that orbits, zooms and
  pans. Cells are drawn and erased by clicking on the surface. Play, step,
  patterns from the library, saving and opening work as on flat grids.
- **Candidate.** Klein bottle and other surfaces that wrap with a flip,
  shown as a flat grid with marked edges.
- **Out.** Surfaces with handles beyond the torus; surfaces that change shape
  while running.
- **Constraint.** A sphere or a cube cannot be covered by identical cells that
  all have the same number of neighbors. At a cube's corners three squares
  meet instead of four; a sphere always needs a few cells of a different
  shape. The rule must say what happens at those cells, and patterns crossing
  them behave differently from patterns on a flat grid.

### Phase 5 — 3D universes

Life in a bounded cubic 3D grid.

- **In.** A cubic grid of a chosen size, with wrapping or dead edges. Life-like
  3D rules (birth and survival counts over the 26 neighbors), with a few
  well-known 3D rules as presets. The 3D camera from Phase 4. A slice
  view that shows one layer at a time; cells are edited in the slice. Play and
  step as in 2D. Saving and opening 3D universes.
- **Out.** Non-cubic 3D cells; unbounded 3D universes.

### Phase 6 — Other rulesets

Opening rule choice to the operator, and rule families beyond plain
birth/survival counts.

- **In.** A rule editor for Life-like birth/survival rules on every geometry,
  with presets (HighLife, Seeds, Day & Night and others). Multi-state
  "Generations" rules, where dying cells fade through extra states, each state
  with its own color. Changing the rule of a universe while it runs.
- **Candidate.** Larger than Life: neighborhoods with a radius greater than one,
  on the square grid.
- **Note.** Phases 3 to 5 already need a rule per geometry. This phase makes
  that rule editable and adds the new families.

### Phase 7 — Infinite universe

A square 2D universe with no edges.

- **In.** Patterns grow without ever reaching an edge. Jump far ahead (for
  example to generation one million) quickly, for patterns that allow it.
  Navigate to distant regions; zoom out to see the whole pattern.
- **Out.** Unbounded hexagonal, triangular and 3D universes.
- **Constraint.** The universe can hold far more cells than can be drawn or sent
  to the UI, so the view shows a summary when zoomed out.

## Cross-cutting requirements

- The same app runs on macOS, Windows and Linux. It works offline, needs no
  account and sends no telemetry.
- The UI stays responsive while the simulation runs, at any grid size.
- The simulation is deterministic: the same starting cells and rule always
  produce the same history.
- Each phase keeps every earlier capability working. Files saved in an earlier
  phase still open.

## Risks and known costs

- **Transfer speed on Windows.** Moving cells from the engine to the UI may be
  slower in the Windows webview than on macOS and Linux. Verified on macOS; not
  yet measured on Windows or Linux. Accepted debt, not a gate for any phase.
- **Phase 2 checks not run.** Verified on macOS; Windows and Linux are
  deferred, as in Phase 1. The frame-rate measurement covered the fit view
  only, and the copy and paste round trip with Golly was not run.
- **Large grids on screen.** A zoomed-out view of millions of cells needs a
  different way of drawing than a zoomed-in view of individual cells.
- **Irregular cells on closed surfaces.** Cells with an unusual number of
  neighbors change how rules behave locally; a rule tuned for flat grids may
  die out or explode around them.
- **Editing in 3D.** Pointing at one cell inside a 3D block is hard; the slice
  view is the planned answer.
- **File formats.** There is no common format for non-square or 3D patterns;
  glife defines its own.
- **Infinite universe.** The largest engine effort on the roadmap. It may need a
  second engine next to the bounded one.

## Open questions

- Grid size and speed targets for each phase: what "a large grid runs smoothly"
  means in numbers.
- Aperiodic tilings (Penrose and similar): part of Phase 3, a later phase, or
  never.
- How the sphere is divided into cells: mostly hexagons with a few pentagons,
  or a cube's squares bulged onto the sphere.
- Which tilings closed surfaces support: square only, or also hexagonal and
  triangular.
- Larger than Life: Phase 6 or later.
- Distribution: unsigned builds for personal use, or signed installers for each
  operating system.
- Whether the infinite universe later extends to other tilings and to 3D.

## Not planned

- Continuous automata (Lenia, SmoothLife).
- A browser version of the app; the engine runs in the desktop backend.
- Online pattern catalogs and multi-user features.

## Keeping this file current

- A phase moves to `In progress` when its phase document is created, and to
  `Done` when it is built. The Phase document column links that document.
- A phase is verified and marked `Done` on macOS. Windows and Linux checks are
  deferred debt, listed under Risks and known costs.
- Phases are appended, or reordered before they start. A phase that has started
  keeps its number.
- Each phase keeps a short summary here. Requirements live in its phase
  document.
