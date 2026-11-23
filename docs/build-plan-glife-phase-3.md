# glife stage 3 — build plan

Builds [glife-phase-3.md](glife-phase-3.md). Run with `/kit:run-build-plan`.

## Context

Phase 2 is built and verified on macOS. Everything below links to the code
as it stands at commit `0eef2e7`:

- The engine knows one grid. `Universe` holds bit-packed rows, ages, trail,
  edges and generation, and no tiling
  ([lib.rs:56-72](../src/engine/src/lib.rs)). The step is a B3/S23 bitwise
  adder over whole words ([step.rs:12](../src/engine/src/step.rs)). → ADR-006
- Ages and trail are updated by comparing the old and new alive bits, with no
  knowledge of the neighborhood ([ages.rs:10](../src/engine/src/ages.rs)).
- Selections, placements, flips and captures work on rectangles of cells
  ([region.rs:115-300](../src/engine/src/region.rs)); the UI flips the placement
  preview itself with flip uniforms
  ([gridShader.ts:24-28](../src/ui/view/gridShader.ts)).
- The frame header has no grid field. Word 10 is the edge mode, compared to 0
  by the decoder ([frame.rs:44-66](../src-tauri/src/frame.rs),
  [frame.ts:48](../src/ui/frame.ts)). → ADR-008
- The glife file and the pattern file carry `tiling = "square"` and refuse
  any other value ([glife_file.rs:140](../src-tauri/src/glife_file.rs),
  [pattern_file.rs:80](../src-tauri/src/pattern_file.rs)). → ADR-012
- The built-in check places a pattern on a dead-edged square universe and
  compares it after its period ([builtin.rs:115-160](../src-tauri/src/library/builtin.rs)).
- The viewport, pointer mapping and shader treat a cell as a unit square
  ([viewport.ts](../src/ui/view/viewport.ts),
  [GridView.tsx:288-316](../src/ui/view/GridView.tsx)).
- The status bar shows `B3/S23` as a fixed string
  ([StatusBar.tsx:12](../src/ui/controls/StatusBar.tsx)).

The stage's main risk is the 12-neighbor triangular step at 2048 × 2048.
WP2 builds every step and measures it before anything else depends on it.
WP1 chose each grid's default rule and its patterns (Rules table below).
Rules are set per universe in this stage, so any birth/survival rule runs on
every grid, square included.

## Decisions taken

### Grids and coordinates

- **Four grids**, one per universe, fixed at creation:

  | Id | Code | Cells | Neighbors |
  |---|---|---|---|
  | `square` | 0 | squares | 8 |
  | `hexagonal` | 1 | pointy-top hexagons | 6 |
  | `triangular-12` | 2 | triangles | 12 |
  | `triangular-3` | 3 | triangles | 3 |

  The id is the string in files and commands; the code is the number in the
  frame header.
- **Storage is unchanged for every grid.** Cell `(x, y)` is bit `x % 64` of
  word `x / 64` in row `y`, as today. A grid differs only in which cells are
  neighbors and in how the UI draws them.
- **Hexagonal: odd rows shifted right.** Row `y` runs across the window; when
  `y` is odd its cells sit half a cell right. Neighbors of `(x, y)`: `(x ± 1, y)`;
  in rows `y ± 1`, cells `x − 1` and `x` when `y` is even, `x` and `x + 1`
  when `y` is odd.
- **Triangular: cell `(x, y)` points up when `x + y` is even.** Width counts
  triangles along a row. Edge neighbors: `(x ± 1, y)`, and `(x, y + 1)` for an
  up triangle or `(x, y − 1)` for a down triangle. The 12-neighbor set of an
  up triangle is `x − 2 … x + 2` in its own row (4 cells), `x − 1 … x + 1` in
  the row above (3) and `x − 2 … x + 2` in the row below (5); a down
  triangle mirrors that, 5 above and 3 below.
- **Wrapping needs even sides where the pattern of rows or triangles
  repeats.** Hexagonal: even height. Triangular, both variants: even width and
  even height. `Universe::new` and `set_edges` refuse an odd side with
  wrapping edges; dead edges allow any side from 3 to 2048.

### Rules

- **`Rule { birth: u16, survival: u16 }`**, bit `n` set when a count of `n`
  neighbors gives birth or survival. It displays as `B…/S…` and parses from
  that text, case-insensitive. A list without a comma has one digit per
  count (`B3/S23`); a list with a comma has numbers separated by commas
  (`B4,10,11/S5,12`), and a lone count of 10 or more ends with a comma
  (`S12,`). Display uses commas only when some count is 10 or more.
  `Rule::parse(text, tiling)` refuses a count
  above the grid's neighbor count and birth on 0, with a message giving the
  reason.
- **A rule per universe.** `Universe` holds its rule. A new universe starts
  with `Tiling::default_rule()`; `set_rule` changes it, refusing a rule the
  grid cannot run. Changing the rule is not an undo step and marks the
  universe changed, as `set_edges` does.
- **Defaults and presets per grid.** `Tiling::presets()` lists
  `(name, Rule)`, the default first:

  | Grid | Default | Source | Other presets |
  |---|---|---|---|
  | `square` | B3/S23 Life | — | HighLife B36/S23, Seeds B2/S, Day & Night B3678/S34678 |
  | `hexagonal` | B2/S35 | Bays 2005, Complex Systems 15, 245–252 | B3/S23 |
  | `triangular-12` | B456/S45 | Bays 1994, Complex Systems 8, 127–150 ("4546") | B4/S456 (Bays "4644") |
  | `triangular-3` | B2/S013 | found by search in WP1 | B2/S23 |

  B2/S35 admits no still life, so the hexagonal built-in set has none.
- **Two steps.** The square grid under B3/S23 keeps its step unchanged. Every
  other case, the square grid under another rule included, goes through one
  bit-sliced step: per word, it adds the neighbor bits into a 4-bit count
  across bit planes and applies the rule's birth and survival masks. Ages and
  trail are updated by the existing pass, unchanged.

### Patterns on non-square grids

- **A `Pattern` carries its tiling and an offset bit.** On the hexagonal grid
  the offset says whether the pattern's row 0 is a shifted row; on the
  triangular grids, whether its cell `(0, 0)` points down. Square patterns
  always have offset 0.
- **`extract(rect)`** sets the offset from the rectangle's origin: `y & 1` on
  the hexagonal grid, `(x + y) & 1` on the triangular grids.
- **Placing aligns the pattern to the anchor.** The top-left cell is the
  anchor minus `(⌊w/2⌋, ⌊h/2⌋)`, as today. If the offset does not match the
  grid at that cell:
  - hexagonal: the pattern is realigned. Each row that changes from shifted
    to unshifted moves one cell right, and the pattern grows one column
    wider. On screen it lands half a cell right of where it was.
  - triangular: the top-left moves one cell right.
- **Flips are true mirror images:**

  | Grid | Horizontal | Vertical |
  |---|---|---|
  | square | `x → w − 1 − x` | `y → h − 1 − y` |
  | hexagonal | unshifted rows `x → w − x`, shifted rows `x → w − 1 − x`; one column wider | `y → h − 1 − y`; offset flips when `h` is even |
  | triangular | `x → w − 1 − x`; offset flips when `w` is even | `y → h − 1 − y`; offset flips when `h` is odd |

  "Unshifted" and "shifted" are relative to the pattern's offset.
- **Flipping a selection** kills the rectangle and writes the mirrored cells,
  with their ages and trail, at the rectangle's origin, aligned as a
  placement is. The cells may reach one column past the rectangle's right
  side. `flip_footprint(rect, axis)` gives every rectangle the flip touches,
  wrapped or clipped, so history captures them first.
- **A pattern is placed only into a universe of its own tiling.** The host
  refuses anything else with a message naming both grids.
- **A pattern's rule is host metadata.** The rule a pattern was made under
  travels with it in the pattern file, pattern text, RLE header, library
  entries and the pending pattern; the engine `Pattern` carries none. Placing
  a pattern under another rule of the same tiling proceeds, with a notice
  naming the pattern's rule, as RLE with another rule does in Phase 2.

### Transfer

- **The frame stays version 1.** Flags word 1, bits 8–11, carry the tiling
  code. A v1 reader already ignores unknown flag bits, and a square frame
  sets them to 0, so `fixtures/frame-v1.bin` and
  `fixtures/frame-v1-layers.bin` stay valid.
- **The rule rides in word 10.** Bit 0 is the edge mode (0 wrap, 1 dead).
  Bits 1–13 hold the birth mask and bits 14–26 the survival mask, each XORed
  with the grid's default rule, so a universe under its default rule sends
  the same word as today and the fixtures above decode as square B3/S23.
- **Placement buffer.** `pending_cells` returns the pending pattern exactly as
  `place` would stamp it at a given anchor with given flips: `u32` LE words
  `[width, height, left, top]`, where `left` and `top` are `i32` offsets of
  the top-left cell from the anchor, then the rows as in the pattern buffer.
  The UI caches it by `(flipH, flipV, x & 1, y & 1)` of the anchor and no
  longer flips the preview itself.
- **`tilings` command.** Returns, for each grid, its id, display name,
  neighbor count, default rule as text and masks, presets as name and text,
  and whether wrapping needs an even width and an even height. The New
  universe dialog, the rule control, the status bar and the library panels
  read it. The UI hard-codes no rule.

### Files and formats

- **glife file.** `tiling` takes the four ids; `rule` is the universe's rule
  text. A reader refuses an unknown tiling with a message saying the file
  needs a newer glife, and a rule the grid cannot run with a message naming
  it. `format` stays 1. A new fixture,
  `fixtures/universe-hex-v1.glife`, is kept loading from now on.
- **Pattern file.** `tiling` takes the four ids. A new key, `offset` (0 or 1,
  default 0), carries the offset bit. `rle` holds the cells as RLE whose
  header names the rule the pattern was made under. `format` stays 1.
- **Pattern text** is the pattern-file TOML without `created` and `changed`.
  On the clipboard: copying on a non-square grid writes pattern text; on the
  square grid it writes RLE, as today. Paste reads pattern text when the text
  parses as a pattern file, and RLE otherwise.
- **RLE stays square-only.** On a non-square universe, Import RLE and Export
  RLE are disabled, and pasting RLE is refused with a message.
- **Pattern files can be exported and imported** with `.toml` as the extension,
  the same files the personal library holds. File menu: Import Pattern… and
  Export Pattern…, enabled on every grid.
- **Built-in library.** One folder as today; each file's `tiling` says which
  grid it belongs to. The check runs on a dead-edged universe of that grid
  under the pattern's rule, and compares against the pattern placed with the
  alignment rules above.
  Guns on a non-square grid are shipped only if WP1 finds one and the gun
  check works on that grid.

### Interaction and display

- **Geometry module.** `src/ui/view/geometry.ts` holds, for each tiling, the
  world size of the grid, cell → world center, world → cell, and the cells a
  world rectangle covers. In world units a square and a hexagon are 1 wide, a
  hexagon row is √3/2 tall, and a triangle has a base of 1, a height of √3/2
  and a horizontal pitch of 1/2. The viewport works in world units; the shader
  has the same mapping in GLSL.
- **Drawing.** A stroke between two pointer samples walks the line in world
  units at quarter-cell steps and sends each cell it enters.
- **Zoomed out.** When a cell is smaller than a pixel, the shader samples the
  stored rows and columns under the pixel, as it does for squares. The blend
  is approximate at cell boundaries and exact in share.
- **Grid lines** follow cell edges once cells are large enough, as for
  squares. **The selection outline** is drawn where a selected cell meets an
  unselected one, so it is stepped on non-square grids.
- **Status bar** shows `<grid name> · W × H · <edges> · <rule>`, the rule
  read from the frame.
- **Rule field.** A preset select for the grid plus a text box. The New
  universe dialog has one, starting at the grid's default and reset to the new
  grid's default when the grid changes. A Rule… button on the toolbar opens
  the same field for the open universe; Apply sends `set_rule`. A refused rule
  shows the backend's message under the field.
- **No new keys.**

### Command contract

`backend.ts` and the handlers registered in `main.rs` change together.

| Command | Change | Returns | WP |
|---|---|---|---|
| `tilings` | new | `[{ id, name, neighbors, defaultRule: { text, birth, survival }, presets: [{ name, rule }], wrapEvenWidth, wrapEvenHeight }]` | 4 |
| `new_universe` | + `tiling`, `rule` | error text if the size does not allow wrapping or the grid cannot run the rule | 4 |
| `set_edges` | — | error text if the size does not allow wrapping | 4 |
| `set_rule` | new: `rule` text | error text if the grid cannot run the rule | 4 |
| `pending_cells` | + `flipH`, `flipV`, `x`, `y` | raw placement buffer | 4 |
| `place` | aligns per Decisions taken | — | 4 |
| `copy_selection`, `cut_selection` | pattern text on non-square grids | — | 5 |
| `paste` | reads pattern text or RLE; refuses another grid; notice for another rule | `PendingInfo` or error | 5 |
| `begin_place` | refuses a library entry of another grid; notice for another rule | `PendingInfo` or error | 5 |
| `library_list` | each entry gains `tiling` and `rule` | as before | 5 |
| `import_pattern` | new | `PendingInfo`, or null | 5 |
| `export_pattern` | new; same arguments as `export_rle` | — | 5 |
| `set_menu_state` | + `tiling` | — | 5 |

`PendingInfo` gains `tiling` and `rule`.

## Step routing at a glance

| Step | Agent | Model | Gate | Parallel | Status |
|---|---|---|---|---|---|
| WP1 Rules and patterns research | `general-purpose` | opus | human | — | Done `d13efc8` |
| WP2 Engine grids and steps | `engine` | opus | test | — | Done `fb07f27` |
| WP3 Engine patterns on grids | `engine` | opus | test | — | Done `6d5e65e` |
| WP4 Grid commands and frame | `host` | sonnet | test | yes, with WP6 | Done `0d11743` |
| WP5 Formats, clipboard and libraries | `host` | sonnet | test | yes, with WP6 | Done `d211e5b` |
| WP6 Grid view and drawing | `ui` | sonnet | human | yes, with WP4–WP5 | Done `10ea9a2` |
| WP7 Placing, selection and libraries in the UI | `ui` | sonnet | human | — | Next |
| WP8 Docs and rules | `kit:docs-writer` | sonnet | human | — | Not started |
| Verification | session | — | — | — | Not started |

Status is updated in the commit that finishes each step. A step's own commit
can't name its hash, so it records `Done`, and the next step's commit adds the
hash.

Test command for every `test` gate: `cargo test --workspace && npm test`.

## WP1 — Rules and patterns research
> `agent: general-purpose` · `model: opus` · `gate: human` · `reads: stage §5, §8, §13; Decisions taken › Grids and coordinates, Rules`

Chooses the rule for each new grid and collects the patterns the built-in
library ships for it. Writes no app code.

### Research
- For `hexagonal`, `triangular-12` and `triangular-3`, find published
  Life-like rules: Carter Bays' papers on the hexagonal and triangular
  tessellations, LifeWiki, and LifeViewer and Golly rule notes. Each rule's
  neighbor counts must match the neighborhoods in Decisions taken.
- Prefer, in order: a rule with a known glider; then one with known
  oscillators; then one whose random fills neither die out nor fill the grid
  within 1000 generations.
- For `triangular-3`, if no published rule has patterns, pick the rule whose
  random fills settle into the most still lifes and oscillators, and harvest
  them.

### Check
A throwaway simulator in the session's scratchpad, using exactly the
coordinates in Decisions taken, runs every candidate pattern and confirms its
category, period and displacement. Patterns found in sources drawn with
another layout, such as Golly's slanted hexagonal grid, are converted to
glife coordinates first, and the conversion is stated.

### Output
- `src-tauri/patterns/pending/*.toml`: one pattern file per candidate, in the
  built-in format with `tiling`, `offset`, `category`, `description`,
  `period`, `dx`/`dy`. At least three patterns per grid, at least one still
  life where the rule allows one and one oscillator each, and a travelling
  pattern where one is known.
  The loader does not read subfolders, so these files are inert until WP5
  moves them.
- The step report: the three rules with sources, what each rule does from a
  random fill, every pattern with its source or "found by search", and the
  simulator's results.

### Human gate
The person approves the three rules and the pattern list. On approval, the
session fills the Rules table in Decisions taken; this plan lists that edit,
so the approval covers it.

## WP2 — Engine grids and steps
> `agent: engine` · `model: opus` · `gate: test` · `reads: stage §4, §5, §11; Decisions taken › Grids and coordinates, Rules`

### `src/engine/src/lib.rs` (new private `tiling.rs`, `step_generic.rs`)
- Public `Tiling` with the four variants, `ALL`, `id()`, `parse(id)`,
  `name()`, `neighbors()`, `default_rule()`, `presets()`,
  `wrap_needs_even() -> (bool, bool)`.
- Public `Rule` per Decisions taken, with `Display` and `parse`.
- `Universe::new(width, height, edges, tiling)`, starting under the default
  rule, and `tiling()`. A new `SizeError` case for an odd side with wrapping
  edges.
- `rule()` and `set_rule(rule) -> Result`.
- `set_edges` returns `Result`, refusing wrapping on an odd side.
- `step` dispatches: square under B3/S23 to the existing `step.rs`,
  everything else to the generic step. The ages pass is unchanged.
- `from_parts` takes the tiling and the rule.

### `src/engine/src/step_generic.rs`
The bit-sliced step from Decisions taken. Per row it builds the shifted
neighbor words the grid needs (square: the 8 neighbors; hexagonal: by row
parity; triangular: by the alternating up/down mask of the row), adds them
into four bit planes, and applies the rule. Wrapping follows the existing `row_sums` approach for
columns and rows.

### Call sites
Every `Universe::new` and `from_parts` in `src-tauri/` passes
`Tiling::Square` (and B3/S23 to `from_parts`), with no other change in
behavior. The engine agent makes
this mechanical change so the workspace builds.

### Tests
- A naive reference stepper for each grid: for every cell, it lists its
  neighbors from the coordinate rules and counts. Random universes of sizes
  3…70 in both dimensions, wrapping (even sides) and dead, 50 generations
  each, under the grid's presets and random rules the grid can run (the
  square grid included), match the bitwise step cell for cell, ages and trail
  included.
- `Rule` parses and displays every preset; `parse` and `set_rule` refuse a
  count above the grid's neighbor count and birth on 0, and `set_rule`
  changes nothing when it refuses.
- Neighbor lists by hand: the 6, 12 and 3 neighbors of a cell in an even and
  an odd row, an up and a down triangle, and at each wrapping edge and
  corner.
- `Universe::new` refuses odd sides with wrapping edges per grid, and accepts
  them with dead edges. `set_edges(Wrap)` on an odd side errors and changes
  nothing.
- Each WP1 still life is still after one generation, and each oscillator
  returns after its period, on its own grid.
- Phase 1 and Phase 2 tests still pass for the square grid.
- The `#[ignore]` timing test reports ms per generation at 2048² for each
  grid under its default rule, and for the square grid under B36/S23, after a
  25 % fill and after 1000 generations.

The target is under 20 ms per generation in every row on the dev machine.
A timing over target is reported as a deviation, which stops the run before
WP3.

## WP3 — Engine patterns on grids
> `agent: engine` · `model: opus` · `gate: test` · `reads: stage §7, §8; Decisions taken › Patterns on non-square grids`

### `src/engine/src/pattern.rs`
- `Pattern` gains `tiling()` and `offset()`. `Pattern::new(w, h, tiling,
  offset)`.
- `flipped(axis)` per the flip table.
- `aligned(offset) -> Pattern`: the same shape with the other offset
  (hexagonal: realigned rows, one column wider; triangular: one empty column
  added on the left). Identity when the offset already matches.
- `placement(anchor_x, anchor_y) -> (Pattern, i64, i64)` returns the pattern
  as it will be stamped and its top-left, and is what `place` and the host's
  `pending_cells` both use.

### `src/engine/src/region.rs`
- `extract` sets tiling and offset.
- `place` refuses a pattern of another tiling, and aligns as Decisions taken
  says.
- `flip_rect` per Decisions taken, keeping ages and trail.
  `flip_footprint(rect, axis) -> Vec<Rect>`.

### Tests
- Flipping twice on each axis gives back the same cells and offset on every
  grid, except a hexagonal horizontal flip: twice gives the cells one column
  right in a pattern two columns wider.
- On each grid, a flipped pattern, run for a few generations of an
  asymmetric oscillator or glider, equals the flipped run of the original:
  the mirror is a true mirror under the grid's rule.
- An extracted pattern placed back at the same origin is unchanged; placed
  at an anchor of the other parity, it runs to the same shape after a period
  (glider or oscillator from WP1).
- `flip_rect` moves ages and trail with their cells; `flip_footprint` agrees
  with the cells `flip_rect` actually writes, including across a wrapping
  edge.
- Placing a pattern of another tiling is an error.
- Square behavior is unchanged: the Phase 2 pattern and region tests pass.

## WP4 — Grid commands and frame
> `agent: host` · `model: sonnet` · `gate: test` · `parallel: yes` · `reads: stage §4, §5, §7, §9; Decisions taken › Transfer, Files and formats (glife file), Command contract`

### `src-tauri/src/frame.rs`
Flags bits 8–11 carry the tiling code; word 10 carries the rule per Decisions
taken › Transfer.

### `src-tauri/src/sim.rs`
- `NewUniverse` gains `tiling` and `rule`. `SetEdges` and the new `SetRule`
  are validated in the command before they are queued; `SetRule` behaves as
  `SetEdges` does for history and the saved state.
- `FlipRect` captures `flip_footprint` instead of the rectangle.
- Bench mode: `GLIFE_BENCH=1` keeps the square universe;
  `GLIFE_BENCH=<tiling id>[:<rule>]` opens a 2048 × 2048 wrapping universe of
  that grid, under the rule if given, with a 25 % fill.

### `src-tauri/src/commands.rs`, `main.rs`
The WP4 rows of the command contract. `place` and `pending_cells` share the
engine's `placement`.

### `src-tauri/src/glife_file.rs`
Reads and writes the tiling and the universe's rule per Decisions taken.

### Tests
- `fixtures/frame-v1.bin` and `fixtures/frame-v1-layers.bin` still match
  byte for byte. A hexagonal frame sets code 1 in bits 8–11. A square frame
  under B36/S23 carries the rule in word 10, and a frame under each grid's
  default rule has 0 in bits 1–26.
- A new universe of each grid, `set_edges` on an odd side, and `set_rule`
  with a valid and a refused rule, through `Sim`.
- An undo after a hexagonal horizontal flip that reaches past the selection
  restores every cell it touched.
- `pending_cells` at anchors of both parities on each grid returns the
  buffer `place` then stamps.
- Save and open of a universe of each grid gives the same cells, ages,
  tiling and rule. `fixtures/universe-hex-v1.glife`, committed by this WP
  under B3/S23 rather than the default, loads under that rule.
  `fixtures/universe-v1.glife` still opens as square. An unknown tiling is
  refused with the newer-glife message.

## WP5 — Formats, clipboard and libraries
> `agent: host` · `model: sonnet` · `gate: test` · `parallel: yes` · `reads: stage §8, §10; Decisions taken › Files and formats, Command contract`

### `src-tauri/src/pattern_file.rs`
`tiling` with four ids and `offset`. Parsing the cells keeps the RLE rule
and refuses one the grid cannot run. `pattern_text(&Pattern, rule, name,
comment)` and `parse_pattern_text`.

### `src-tauri/src/clipboard.rs`
Copy and cut write pattern text on non-square grids. Paste tries pattern
text, then RLE, and refuses a pattern of another grid, and RLE on a
non-square universe. A pattern under another rule of the same grid pastes
with a notice.

### `src-tauri/src/files.rs`, `menu.rs`
`import_pattern` and `export_pattern`, with a `.toml` filter. File menu gains
Import Pattern… and Export Pattern…. `set_menu_state` takes `tiling` and
disables the RLE items off the square grid.

### `src-tauri/src/library/builtin.rs`, `personal.rs`, `library_cmds.rs`
- Entries carry `tiling` and `rule`; `begin_place` refuses another grid
  and gives a notice for another rule.
- The check builds its universe with the pattern's tiling and rule, and
  compares using `placement`.
- Personal patterns save with the selection's tiling and offset and the
  universe's rule. Names stay unique across grids.

### `src-tauri/patterns/`
Move the WP1 files from `pending/` into the folder; remove `pending/`.

### Tests
- Pattern text round trip on each grid keeps cells, offset, rule, name and
  comment.
- A hexagonal pattern made under B3/S23, pasted into a B2/S35 hexagonal
  universe, starts placing with a notice naming B3/S23.
- Paste of hexagonal text into a triangular universe is refused naming both
  grids; RLE into a hexagonal universe is refused.
- Every file in `src-tauri/patterns/` loads and passes its check, the new
  grids included. A hexagonal oscillator file with a wrong period is a
  problem.
- `fixtures/pattern-v1.toml` loads as a square pattern with offset 0.
- Export then import of a triangular pattern file keeps everything.

## WP6 — Grid view and drawing
> `agent: ui` · `model: sonnet` · `gate: human` · `parallel: yes` · `reads: stage §4, §5, §6, §11; Decisions taken › Interaction and display, Transfer, Command contract`

### `src/ui/view/geometry.ts`
Per Decisions taken › Interaction and display.

### `src/ui/view/viewport.ts`, `GridView.tsx`
- The viewport keeps its center and zoom in world units; `fit`, zoom and pan
  use the grid's world size. `visibleRegion` asks geometry for the rows and
  columns a world rectangle covers.
- Pointer → cell through geometry; strokes walk the line per Decisions taken.

### `src/ui/view/gridShader.ts`
- A tiling uniform. The fragment shader maps a pixel to its cell for each
  grid, draws cells, grid lines along cell edges and the boundary, with plain
  and age coloring and the trail.
- Zoomed-out shading per Decisions taken.

### `src/ui/frame.ts`, `backend.ts`, `state.ts`
`decodeFrame` returns `tiling` from flags bits 8–11, the edge mode from bit
0 of word 10, and the rule from bits 1–26 XORed with the grid's default from
`tilings()`, formatted as `B…/S…`. `tilings()` is fetched once at start into
state.

### `src/ui/controls/NewUniverseDialog.tsx`, `StatusBar.tsx`, `Toolbar.tsx`
- The dialog's Grid select lists `tilings()`, square by default. With
  wrapping edges it shows the even-size requirement next to the sizes and
  disables Create on an odd side.
- The rule field in the dialog and the toolbar's Rule… control, per
  Decisions taken › Interaction and display.
- The status bar per Decisions taken.
- An edge-mode change refused by the backend shows its message as a notice.

### Tests
- `geometry.ts`: for each grid, world → cell of a cell's center gives the
  cell; points just inside each edge of a hexagon and of an up and a down
  triangle give the right cell; the cells covered by a world rectangle
  include every cell whose center lies inside it.
- A stroke across a hexagonal grid sends a connected run of cells.
- `decodeFrame` reads the tiling code and the rule; the existing fixtures
  decode as square B3/S23.
- The dialog refuses odd sides with wrapping for hexagonal and triangular
  grids, resets the rule to the grid's default when the grid changes, and
  sends `new_universe` with the chosen tiling and rule.
- The Rule… control sends `set_rule`, and a refused rule shows the backend's
  message without closing.

### Human gate — look and speed on macOS
Needs WP2 and WP4. The session builds with `npm run tauri build --
--no-bundle`. For each of `1`, `square:B36/S23`, `hexagonal`, `triangular-12`
and `triangular-3`, the session runs `GLIFE_BENCH=<value> GLIFE_BENCH_REPORT=1
target/release/glife-app`, which fits, measures fps and gen/s in Plain and in
Age with the trail at 100, prints one line and quits.

Then by eye, yes or no, on each new grid at a zoom of 20+ px per cell:
- Cells have the right shape, and grid lines follow their edges.
- Clicking draws the cell under the pointer, near edges and corners too.
- A known oscillator from WP1, drawn by hand, oscillates.
- Zooming out to fit shows dense and sparse regions as on the square grid.
- Changing the rule from the toolbar while the universe runs shows the new
  rule in the status bar, and the cells keep running under it.

The gate passes at 60 fps and at least 30 gen/s in every row and a yes on
every check.

## WP7 — Placing, selection and libraries in the UI
> `agent: ui` · `model: sonnet` · `gate: human` · `reads: stage §7, §8, §10; Decisions taken › Patterns on non-square grids, Interaction and display, Command contract`

### `src/ui/edit/tools.ts`, `src/ui/view/GridView.tsx`, `gridShader.ts`
- The preview draws the placement buffer at the anchor plus `(left, top)`,
  re-fetched when the anchor's parity or the flips change; the flip uniforms
  go.
- The selection outline per Decisions taken.

### `src/ui/library/`
- Both lists show the current universe's grid. A line under each list gives
  the count for other grids; a switch shows them, marked with their grid
  name, with Place disabled.
- Each entry shows its rule; one under a rule other than the universe's is
  marked and can be placed.
- Previews draw hexagons and triangles as SVG paths through `geometry.ts`.

### `src/ui/controls/`
- File actions Import Pattern… and Export Pattern…; Export uses the
  selection if any, else the universe, as Export RLE does.
- `menu.ts` routes the new items.

### Tests
- The preview requests `pending_cells` once per parity and flip state, and
  draws at the returned offset.
- Library entries of another grid are hidden until the switch is on, and then
  cannot be placed.
- An entry under another rule of the same grid is marked and starts placing
  with the backend's notice.
- A paste refused for another grid shows the message and does not start
  placing.

### Human gate — editing on each grid
On a hexagonal and a 12-neighbor triangular universe, yes or no:
- A selection's outline is stepped along cell edges, and copy then paste in
  *Copy* mode reproduces it.
- Flip H and V on a WP1 glider: the copy is the mirror image and travels the
  mirrored way.
- The placement preview follows the pointer, half a cell or one triangle
  aside where needed, and lands where shown.
- Undo after each of these restores the universe.
- Copy on a hexagonal universe and paste into a text editor shows readable
  pattern text; pasting it into a triangular universe is refused with a
  message naming both grids.
- A personal pattern saved on each grid appears only on its own grid until
  the switch is on.
- With the hexagonal universe switched to B3/S23, the built-in B2/S35
  patterns are marked with their rule and still place.

## WP8 — Docs and rules
> `agent: kit:docs-writer` · `model: sonnet` · `gate: human`

Writes the edits in place, uncommitted, for review in the diff.

- `docs/architecture.md`: purpose (four grids, a rule per universe), the
  tiling and rule in the data model and snapshot, frame flags bits 8–11 and
  the rule in word 10, the placement buffer, the glife file and pattern file
  `tiling` values, `rule` and `offset`, pattern text, the `tilings` and
  `set_rule` commands, the geometry module, Not built yet.
- `docs/adr.md`: new entries for grid coordinates and even-side wrapping; a
  rule per universe with a default and presets per grid, the square B3/S23
  step kept and a generic step for everything else; the rule in frame word 10
  relative to the grid's default;
  pattern offset, alignment and true-mirror flips, including that a
  horizontal flip of a hexagonal or triangular selection may write one column
  past it and a second flip does not undo that; pattern text as the
  exchange format off the square grid.
- `docs/roadmap.md`: Phase 3 to Done; Risks and known costs as measured.
- Root `CLAUDE.md` § Cross-cutting rules: add `fixtures/universe-hex-v1.glife`
  to the files kept loading. Command index: the benchmark row gains
  `GLIFE_BENCH=<tiling id>[:<rule>]` and its `main.rs` link is corrected; a
  row for `GLIFE_BENCH_REPORT=1`, which prints fps and gen/s and quits.
- `docs/architecture.md` also: the bench report mode.
- `.claude/agents/engine.md`: the rule "Keep both layers out of the B3/S23
  step" names every grid's step.
- Move `glife-phase-3.md` and this plan to `docs/hist/` once Phase 3 is Done,
  updating links.

## Verification

- The test command passes.
- The WP6 measurement table: 60 fps and at least 30 gen/s on every grid, with
  plain and age coloring.
- The WP6 and WP7 visual checks pass on macOS.
- A universe of each new grid, saved, closed, reopened and run 100
  generations, matches a run of the original, under its default rule and
  under another preset.
- Phase 2 universes and personal patterns open unchanged.
- Windows and Linux stay unchecked, as in Phases 1 and 2.

## Deliberately out of scope

- Golly and LifeViewer exchange for non-square grids (`H` and `L` rules).
- Rule families beyond birth/survival counts (Phase 6); birth on 0.
- Rotation; hexagonal orientation other than pointy-top; rhombus-shaped
  universes.
- Converting a universe or pattern between grids.
- Exact zoomed-out area weighting at cell boundaries.
