# glife stage 2 — build plan

Builds [glife-phase-2.md](glife-phase-2.md). Run with `/kit:run-build-plan`.

## Context

Phase 1 is built and verified on macOS. Everything below links to the code
as it stands at commit `82f1334`:

- The engine stores alive cells bit-packed, and has no attribute layers yet
  ([lib.rs:48-55](../../src/engine/src/lib.rs)). → ADR-006
- One simulation thread owns the universe. It publishes a full `Universe` clone
  as a snapshot after each command, and at most every 8 ms while running
  ([sim.rs:182-196](../../src-tauri/src/sim.rs)). → ADR-007
- The UI pulls the region in view once per animation frame as frame layout v1.
  The frame has only the alive-bits layer kind, and readers skip unknown kinds
  ([frame.ts:49-55](../../src/ui/frame.ts)). → ADR-008
- Commands are JSON `invoke` calls that return nothing except `frame`
  ([backend.ts:15-30](../../src/ui/backend.ts) ↔
  [commands.rs](../../src-tauri/src/commands.rs)).
- The app has no plugins, menus, file access or clipboard. Its capabilities are
  `core:default` ([default.json](../../src-tauri/capabilities/default.json)).

The stage's main risk is age coloring at 2048 × 2048: the extra layers in each
snapshot copy, in each step and on the wire. WP1–WP3 build that path and measure
it before anything else is built on top of it.

## Decisions taken

### Engine

- **Age layer.** One `u16` per cell, row-major, saturating at 65 535. A cell
  born by the rule or set alive by an edit gets age 0. A cell that survives a
  generation gets age + 1. A cell that dies keeps its age, which the trail
  uses as its last color.
- **Trail layer.** One `u8` per cell holding the generations since the rule
  killed the cell, saturating at 255. 0 means no trail. A death by the rule
  sets it to 1. A death by an edit, any birth, `clear` and `random_fill` set
  it to 0. Cut, Delete and *Copy* placing set it to 0 across their whole
  rectangle; erasing a single already-dead cell leaves its trail. A bit layer marks the cells whose counter is between 1 and 254, so
  each generation's cost grows with the number of live and trailing cells, not
  with the grid.
- **Both layers are always kept**, whatever the UI shows, so switching to age
  coloring shows true ages at once. Neither layer enters the B3/S23 step.
- **Patterns.** A public `Pattern` (width, height, bit-packed cells, 1 to 2048
  per side, empty allowed) is the unit the engine exchanges for selections,
  placements, the clipboard, files and libraries.
- **Undo captures.** The engine offers compact before and after captures of
  rectangles and of single cells. A rectangle capture holds the alive bits and
  the ages of its live cells only, run-length encoded. Restoring a capture
  restores cells and ages exactly and sets the trail to 0 inside it.

### Simulation thread and undo

- **History lives on the simulation thread** (`src-tauri/src/history.rs`),
  next to the universe it changes. Every generation clears it, whether it
  comes from play or a single step, as do `new_universe` and `open`. An edit,
  fill, clear, placement, delete or flip while playing is applied, not
  recorded, and clears the history. While playing, undo and redo are ignored
  and reported as unavailable. `set_edges` neither records nor clears; it
  marks the universe unsaved, and undo cannot remove that mark.
- **One entry per undo step**, holding before and after captures, the
  generation before and after, and the revision before and after. A stroke is
  every `edit` carrying the same `stroke` id with no other command between
  them. The history keeps the newest 50 entries.
- **Revisions and the unsaved mark.** Every change, generations included, gives
  the universe a new revision number from a counter that only goes up. Undo and
  redo restore the entry's revision. Save and open record the saved revision;
  the universe is dirty when the two differ.
- **Trail on undo.** Undo does not bring back a trail. The trail fades only
  while the universe advances, and undo exists only while it does not.
- **Snapshot buffers are reused.** Publishing copies into the previous
  snapshot's buffers when no reader still holds it, instead of allocating
  about 12.5 MiB each time at 2048².

### Transfer

- **Frame layout stays version 1.** The additions are new flag bits and two
  new layer kinds. A v1 reader ignores unknown flag bits and skips unknown
  layers, so `fixtures/frame-v1.bin` stays valid.
- **`frame` takes a `layers` bitmask**: bit 0 = age, bit 1 = trail. The alive
  bits are always sent; the other layers only when asked for. In plain
  coloring the frame is exactly what Phase 1 sends.
- **The UI sends `NO_VERSION`** whenever the requested layers differ from
  those already uploaded, just as it does when the region changes.
- **Pattern buffer.** The one binary layout for a pattern's cells going to
  the UI: little-endian `u32` words `[width, height]`, then `height` rows of
  `ceil(width / 32)` words. Bit `i` of word `k` in row `r` is cell
  `(32k + i, r)`. It carries placements, pastes, imports and library previews.

#### Frame layout v1 additions

| Where | Addition |
|---|---|
| Word 1, flags | bit 1 = undo available, bit 2 = redo available, bit 3 = unsaved changes |
| Layer kind 1, age | `h` rows of `w` bytes, `min(age, 255)`, four cells per word, cell `x0 + 4j + b` in byte `b` of word `j` |
| Layer kind 2, trail | the same shape, holding the trail counter |

`w` is a multiple of 32, so every row of a byte layer starts on a word. At
2048² with the whole grid in view, a frame is 0.5 MiB in plain coloring,
4.5 MiB with age and 8.5 MiB with the trail as well.

### Rendering

- **Age coloring in the existing shader.** Ages and trail are uploaded as
  `R8UI` textures, and the palette as a 256 × 1 RGBA texture. A live cell takes
  `palette[min(age, N − 1)]`. A trailing cell with counter `d ≤ L` (trail
  length) takes `mix(palette[min(age, N − 1)], dead, d / (L + 1))`.
- **Zoomed-out blend.** The k × k loop from Phase 1 (k capped at 16) averages
  the colors of the live and trailing cells under the pixel. It then mixes
  that average over the dead color by their share of the k × k cells, with a
  minimum share of 0.25, as Phase 1 does.
- **Placement preview and selection outline are drawn by the same shader**:
  the pattern as an `R32UI` texture with an offset and flip uniforms, drawn
  translucent and wrapping on a wrapping grid; the selection as a rectangle
  uniform drawn as a 1 px outline. The size label is a DOM element.
- **Library previews are SVG**: one `<path>` built from a thumbnail of at most
  64 × 64 cells, which the backend makes by OR-downsampling. No Canvas 2D.
  → ADR-005

### Palettes and view settings

- Three palettes, each generated at 16, 32 and 256 colors by interpolating its
  color stops in OKLab. **Viridis** is safe for common color-vision
  deficiencies, with its darkest end trimmed. **Heat** runs white → yellow →
  orange → red. **Ice** runs white → cyan → blue → violet. Every color contrasts
  with the dead-cell color.
- **Age 0 is white in every palette.** Entry 0 is white; the palette's stops
  fill entries 1 to N − 1.
- Coloring mode, palette, palette size, trail on or off, and trail length
  (1–100, default 10) are kept in `localStorage`, read and written inside
  `try`/`catch`, and never stored in a universe file.

### Files and formats

- **RLE codec** (`src-tauri/src/rle.rs`):
  - Reading skips text before the first `x = …` line. It takes `#N` as the
    name and `#C`/`#c` lines as the comment. It accepts `B3/S23`, `b3/s23`,
    `23/3` and `Life` as Conway; any other rule loads with a notice.
  - It reads Golly's bounded-grid suffix (`:T w,h`, `:P w,h`, others) and
    refuses more than two states and sides over 2048.
  - Writing emits `#N`, `#C`, then `x = W, y = H, rule = B3/S23`, plus
    `:T W,H` or `:P W,H` for a whole universe. Lines are at most 70 columns,
    and the header keeps the full rectangle, dead margin included.
- **glife file** (`.glife`, `src-tauri/src/glife_file.rs`):
  - Layout: the 8-byte magic `GLIFE\0\r\n`, a `u32` LE header length, a UTF-8
    JSON header, then the sections in header order.
  - Header: `format` (1), `tiling` (`"square"`), `width`, `height`, `edges`,
    `rule`, `generation`, and `sections` as a list of
    `{ kind, codec: "deflate", bytes }`.
  - Sections: `alive` holds the rows of `ceil(width / 64)` `u64` LE words;
    `ages` holds the `u16` LE ages of the live cells in row-major order.
  - A reader ignores unknown header keys, skips unknown section kinds, and
    refuses `format` > 1 as written by a newer glife.
  - Writes go to a temporary file in the target folder and are renamed over
    the target, so a failed save leaves an existing file intact.
- **Opening an RLE as a universe** applies stage §10:
  - a declared bounded grid of 3–2048 per side opens at that size (torus →
    wrap; plane → dead; anything else → dead, with a notice);
  - any other pattern opens centered on a dead-edged grid with each side
    `clamp(3 × side, 256, 2048)`, at generation 0 with every age 0.
- **Pattern file** (`src-tauri/src/pattern_file.rs`): one TOML file per
  pattern, shared by both libraries. It holds `format = 1`,
  `tiling = "square"`, `name`, an optional multi-line `comment`, `created`
  and `changed` (RFC 3339), and `rle`, the pattern's RLE as a multi-line
  string.
- **Built-in library** is a folder of pattern files read when the app starts,
  so a pattern is added by dropping a file into it and restarting, without a
  rebuild:
  - The folder ships with the app as a Tauri resource, `patterns/`. The source
    copy is `src-tauri/patterns/`.
  - A built-in file adds `category` (`still-life`, `oscillator`, `spaceship`,
    `gun`, `methuselah`), `description` in place of `comment`, `period`,
    `dx`/`dy` for spaceships, and `gliders` per period for guns.
  - On loading, the app checks each pattern against its stated facts, using
    the checks in stage §13. A file that cannot be read or fails its check is
    not offered. The panel lists it with the reason.
  - The test suite runs the same check over every file in the source folder.
- **Personal library**: the `patterns/` folder in the app's data directory,
  one pattern file each.
  - File names are a filesystem-safe form of the pattern name, with a number
    added on collision. The name inside the file is the one that counts, and
    names are unique ignoring case.
  - Every write is a temporary file plus a rename. A file that fails to read
    is listed by file name and left alone.

### Platform integration

- **Dialogs, the clipboard and file access run in the backend**, through
  `tauri-plugin-dialog`, `tauri-plugin-clipboard-manager` and
  `tauri-plugin-opener` (for Show folder). The UI never sees a path; it gets
  file names for display.
- **Confirmations are native message dialogs** opened by the backend: unsaved
  changes (Save / Don't Save / Cancel) and deleting a personal pattern.
- **Menus are built in the backend** (`src-tauri/src/menu.rs`):
  - **File**: New…, Open…, Save, Save As…, Import RLE…, Export RLE….
  - **Edit**: Undo, Redo, Cut, Copy, Paste, Delete, Select All, Flip
    Horizontally, Flip Vertically.
  - Accelerators are CmdOrCtrl + N, O, S, Shift+S, Z, Shift+Z (and Ctrl+Y off
    macOS), X, C, V and A. Delete has no accelerator; the webview handles that
    key.
  - On macOS the web view takes Cmd+Shift+Z before the Edit menu; the UI
    handles it as a fallback (`src/ui/controls/shortcuts.ts`).
  - Menu clicks reach the UI as a `menu` event carrying the item id.
- **Edit menu swap.** While a text field has focus, the backend swaps the Edit
  menu for the platform's predefined text items (undo, redo, cut, copy, paste,
  select all), so the shortcuts edit the text. The UI reports focus changes
  with `set_edit_target`.
- **Close and quit.** A close request and the app's own Quit item (Cmd+Q)
  first run the unsaved-changes prompt. So does `ExitRequested` on macOS.
- **Window title** is `<file name> — glife`, prefixed with `• ` while there
  are unsaved changes, and `Untitled` before the first save. The UI sets it
  through `backend.ts`.

### Interaction

- **Keys** (no modifier, ignored in text fields): S = select tool, D = draw
  tool, H = flip horizontally, V = flip vertically, Escape = end placing, then
  end the selection. Delete/Backspace kill the selection if there is one, and
  clear the universe otherwise. Phase 1's keys are unchanged.
- **Placement anchor**: the cell under the pointer is the pattern's center,
  `(⌊w/2⌋, ⌊h/2⌋)`.
- **Paste modes**:
  - *Or* sets the pattern's live cells alive. Cells already alive keep their
    ages.
  - *Copy* sets the whole rectangle. Cells alive before and after keep their
    ages, newly live cells get age 0, and cells killed get no trail.
- **Flip** of a selection moves ages and trail counters with their cells.
- **The pending pattern** lives in the backend (`Host`), one slot.
  - Paste, import and choosing a library entry fill it and return its size,
    name and notices; the UI then fetches its cells with `pending_cells`.
  - `place` stamps it, flipped as the UI says.
  - A pattern wider or taller than the grid is refused when the slot is
    filled, with a message giving both sizes.
- **Reads that bypass the queue.** `copy_selection`, `save`, `save_as`,
  `export_rle` and `library_save` from a selection read the latest snapshot,
  as `frame` does. After a save, the saved revision goes back to the
  simulation thread as a command.

### Command contract

`backend.ts` and `commands.rs` change together. Rect = `{ x, y, w, h }`
inside the grid. Source = `{ builtin: id } | { personal: name } | "pending"`.

| Command | Arguments | Returns | WP |
|---|---|---|---|
| `frame` | + `layers` | raw frame | 2 |
| `edit` | + `stroke` | — | 5 |
| `undo`, `redo` | — | — | 5 |
| `delete_selection` | rect | — | 5 |
| `flip_selection` | rect, `axis` (`"h"`/`"v"`) | — | 5 |
| `place` | `x`, `y`, `flipH`, `flipV`, `mode` (`"or"`/`"copy"`) | — | 5 |
| `pending_cells` | — | raw pattern buffer | 5 |
| `copy_selection`, `cut_selection` | rect | — | 8 |
| `paste` | — | `PendingInfo` or error | 8 |
| `begin_place` | source | `PendingInfo` or error | 8 |
| `library_list` | — | `{ builtin, personal, problems }` | 8 |
| `pattern_preview` | source | raw pattern buffer, ≤ 64 × 64 | 8 |
| `library_save` | `{ selection: rect } \| "pending"`, `name`, `comment?`, `replace` | ok or `{ nameTaken: true }` | 8 |
| `library_copy_builtin` | `id`, `name`, `replace` | as `library_save` | 8 |
| `library_rename` | `name`, `newName`, `replace` | as `library_save` | 8 |
| `library_set_comment` | `name`, `comment` | — | 8 |
| `library_delete` | `name` | whether it was deleted | 8 |
| `library_show_folder` | — | — | 8 |
| `confirm_discard` | — | whether to go on | 8 |
| `save`, `save_as` | — | file name, or null if cancelled | 8 |
| `open` | — | `{ fileName, notices }`, or null | 8 |
| `import_rle` | — | `PendingInfo`, or null | 8 |
| `export_rle` | `{ selection: rect } \| "universe" \| source` | — | 8 |
| `set_edit_target` | `"text"` / `"universe"` | — | 8 |
| `set_menu_state` | `{ canUndo, canRedo, hasSelection, playing }` | — | 8 |

`PendingInfo` = `{ width, height, name, comment, notices }`. Library entry
metadata = `{ id or name, category?, description or comment, width, height,
live, period?, speed?, created?, changed? }`. Errors are strings shown as
notices.

## Step routing at a glance

| Step | Agent | Model | Gate | Parallel |
|---|---|---|---|---|
| WP1 Engine ages and trail | `engine` | opus | test | yes, with WP3 |
| WP2 Frame layers and snapshot reuse | `host` | sonnet | test | yes, with WP3 |
| WP3 Age coloring | `ui` | sonnet | human | yes, with WP1–WP2 |
| WP4 Engine patterns and regions | `engine` | sonnet | test | — |
| WP5 Editing commands and undo | `host` | opus | test | — |
| WP6 Formats and libraries | `host` | sonnet | test | yes, with WP7 |
| WP7 Selection, placing and undo in the UI | `ui` | sonnet | test | yes, with WP6 |
| WP8 Files, clipboard, menus | `host` | sonnet | test | yes, with WP9 |
| WP9 Library panels and file actions | `ui` | sonnet | human | yes, with WP8 |
| WP9b UI polish | `ui` | sonnet | human | — |
| WP10 Docs and rules | `kit:docs-writer` | sonnet | human | — |

Test command for every `test` gate: `cargo test --workspace && npm test`.

## WP1 — Engine ages and trail
> `agent: engine` · `model: opus` · `gate: test` · `parallel: yes` · `reads: §11, Decisions taken › Engine`

### `src/engine/src/lib.rs`
- `Universe` gains the age layer (`Vec<u16>`), the trail layer (`Vec<u8>`) and
  the trailing-cells bit layer.
- `age(x, y) -> u16` and `trail(x, y) -> u8`.
- `set`, `clear` and `random_fill` keep both layers per Decisions taken.
- `copy_ages(x0, y0, w, h, out: &mut Vec<u32>)` and `copy_trail(…)` write the
  byte layers in the frame layout v1 additions order, with the same alignment
  rules and panics as `copy_region`.
- `Clone` gets a `clone_from` that reuses the target's buffers.

### `src/engine/src/step.rs` (or a new private `ages.rs`)
After the bitwise step, a pass over the words updates ages for born and
surviving cells and trail counters for dying and trailing cells. It visits set
bits only and leaves the B3/S23 step itself unchanged.

### Tests
- Ages: a block's cells reach age n after n generations; a blinker's
  end cells stay at age 0 and its centre counts up; saturation at 65 535
  (driven by a test-only setter).
- Trail: a cell killed by the rule counts 1, 2, 3 … and stops at 255; a
  cell killed by `set` has trail 0; birth resets the trail; `clear` and
  `random_fill` zero both layers.
- The Phase 1 reference check extends to ages and trail: the naive stepper
  also tracks them, for the same widths, heights and edge modes.
- `copy_ages` and `copy_trail`: saturation to 255, byte order inside a word,
  partial last word, region at the right edge.
- `clone_from` gives an equal universe.
- The `#[ignore]` timing test reports ms per step at 2048² right after a 25 %
  fill and after 1000 generations. WP1 reports both. Target: under 10 ms on the
  dev machine.

## WP2 — Frame layers and snapshot reuse
> `agent: host` · `model: sonnet` · `gate: test` · `parallel: yes` · `reads: Decisions taken › Transfer, Frame layout v1 additions`

### `src-tauri/src/frame.rs`
`encode` takes `layers` and appends layer kinds 1 and 2 when asked for.

### `src-tauri/src/commands.rs`
`frame` takes `layers: u32`.

### `src-tauri/src/sim.rs`
`publish` reuses the previous snapshot's allocation through `Arc::get_mut` and
`clone_from` when no reader holds it, and allocates otherwise.

### Tests
- `fixtures/frame-v1.bin` still matches byte for byte with `layers = 0`.
- A new fixture `fixtures/frame-v1-layers.bin`: the same 64 × 2 region with
  known ages (one above 255) and trail counters, both layers requested,
  checked against hand-written words as in Phase 1.
- Requesting only the trail sends kinds 0 and 2.
- An `#[ignore]` timing test encodes a full 2048² frame with both layers and
  reports ms.

## WP3 — Age coloring
> `agent: ui` · `model: sonnet` · `gate: human` · `parallel: yes` · `reads: §11, §12, §13, Decisions taken › Transfer, Rendering, Palettes`

### `src/ui/backend.ts`, `src/ui/frame.ts`
- `fetchFrame(region, since, layers)`, with `LAYER_AGE = 1` and
  `LAYER_TRAIL = 2`.
- `decodeFrame` returns `ages` and `trail` as `Uint8Array` views (or null),
  plus the new flags as `canUndo`, `canRedo` and `dirty`.

### `src/ui/color/palettes.ts`
Palette stops and `paletteColors(id, size) -> Uint8Array` (RGBA, `size`
entries), in OKLab per Decisions taken.

### `src/ui/state.ts`
Coloring mode, palette, size, trail on and trail length, loaded from and
saved to `localStorage` as Decisions taken says.

### `src/ui/view/gridShader.ts`, `GridView.tsx`
- The age path from Decisions taken › Rendering, chosen by a mode uniform; the
  plain path is unchanged.
- `GridView` requests layers to match the mode and the trail setting. It
  uploads `R8UI` textures, reallocating them only when their size changes, and
  re-uploads the palette texture when the palette changes.

### `src/ui/color/ColorControls.tsx`, `Legend.tsx`
- Toolbar group: Plain/Age, palette, size (16/32/256), Trail on/off, trail
  length (1–100).
- The legend is shown in age coloring: the palette strip with age labels 0,
  ⌊N/2⌋ and `N−1+`.

### Tests
- `frame.test.ts`: `fixtures/frame-v1-layers.bin` decodes to its known ages
  and trail; the new flag bits decode.
- Every palette at every size has exactly that many entries; the first and
  last entries equal the first and last stops.
- Settings survive a reload of the state module, and a throwing
  `localStorage` falls back to defaults.
- `GridView` asks for layers 0, 1 and 3 as the mode and trail change, and
  sends `NO_VERSION` after each change.

### Human gate — measurement on macOS
Needs WP2. The session builds with `npm run tauri build -- --no-bundle`. The
person runs `GLIFE_BENCH=1 target/release/glife-app`, presses F to fit, opens
the overlay with backquote, and records one row per mode:

| Mode | Coloring | Trail | fps | req ms | B/frame | gen/s |
|---|---|---|---|---|---|---|
| A | Plain | — | | | | |
| B | Age | Off | | | | |
| C | Age | On, length 100 | | | | |

Fit is the heaviest view for transfer: it requests the whole grid.

Then by eye, yes or no:
- In Age, old and young regions can be told apart.
- In C at Speed 10, dead cells fade toward the background.
- The legend and toolbar lay out cleanly.
- The darkest palette colors stand out from dead cells.

The gate passes at 60 fps and at least 30 gen/s in every row and a yes on
every visual check. If it fails,
stop before WP4 and revisit transfer. The candidates: fetch the age and trail
layers on every other animation frame, or have the backend send one blended
byte per pixel when cells are smaller than a pixel. The second supersedes
ADR-008.

## WP4 — Engine patterns and regions
> `agent: engine` · `model: sonnet` · `gate: test` · `reads: §6, §7, §8, Decisions taken › Engine, Interaction`

### `src/engine/src/lib.rs` (new private `pattern.rs`, `region.rs`)
- `Pattern`: `new(w, h)`, `width`, `height`, `get`, `set`, `live_count`,
  `flipped(Axis)`, `thumbnail(max_side)` (OR-downsample), and
  `to_words(out: &mut Vec<u32>)` in the pattern-buffer row order.
- `Rect { x, y, w, h }`, `enum Axis { Horizontal, Vertical }`,
  `enum PasteMode { Or, Copy }`.
- `Universe::extract(Rect) -> Pattern`.
- `Universe::footprint(w, h, x: i64, y: i64) -> Vec<Rect>`: the grid
  rectangles a placement touches. There are up to four on a wrapping grid,
  and the clipped one on a dead-edged grid.
- `Universe::place(&Pattern, x, y, PasteMode)`, wrapping or clipping by edge
  mode. It returns an error when the pattern is wider or taller than the grid.
- `kill_rect(Rect)` and `flip_rect(Rect, Axis)`.
- `capture(&[Rect]) -> Capture` and `restore(&Capture)`.
  `capture_cell(x, y) -> CellState` and `restore_cell(CellState)`.
  `Capture::bytes()` reports its memory use.
- `set_generation(u64)`, and
  `from_parts(width, height, edges, generation, &Pattern, ages: &[u16])` for
  opening files.

### Tests
- Place in both modes: ages of cells already alive kept; newly live cells at
  age 0; *Copy* kills the dead part of the rectangle without trail.
- A placement across each wrapping edge and across a corner lands on the
  opposite sides. On dead edges, the outside part is dropped. A pattern
  larger than the grid is an error.
- `footprint` agrees with where `place` actually writes.
- Flip twice is identity; flip moves ages; `extract` after `place` gives the
  pattern back.
- `restore(capture)` after random fills, kills, flips and placements restores
  bits, ages and live count exactly.
- A whole-grid capture of a 25 % fill at 2048² stays under 1 MiB of ages
  plus the bits.
- `thumbnail`: a 2048 × 2048 pattern becomes 64 × 64, a single cell stays
  visible, and a 10 × 3 pattern is unchanged.
- The Phase 1 known-pattern checks still pass.

## WP5 — Editing commands and undo
> `agent: host` · `model: opus` · `gate: test` · `reads: §6, §7, §8, §9, Decisions taken › Simulation thread and undo, Interaction, Command contract`

### `src-tauri/src/history.rs`
Entries and the rules from Decisions taken. Rectangle steps capture before
and after. A stroke captures each cell the first time the stroke touches it,
and captures it again after.

### `src-tauri/src/sim.rs`
- New commands: `Edit { cells, alive, stroke }`,
  `Place { pattern: Arc<Pattern>, x, y, mode }`, `KillRect(Rect)`,
  `FlipRect(Rect, Axis)`, `Undo`, `Redo`, `Load(Universe)`,
  `MarkSaved { revision }`.
- `Snapshot` gains `revision`, `dirty`, `can_undo` and `can_redo`. The rule
  for clearing the history is applied in `generation` and in `Step`.

### `src-tauri/src/frame.rs`
Flag bits 1–3.

### `src-tauri/src/commands.rs`, `main.rs`
- The WP5 rows of the command contract.
- `Host` gains the pending-pattern slot, `Mutex<Option<Arc<Pattern>>>`. `place`
  flips a copy as asked and sends `Place` with the top-left computed from the
  centre anchor.

### Tests
All on `Sim` with the fake clock:
- A stroke of three `edit` batches is one undo step; a new stroke id or any
  command between them starts a new one.
- Undoing a random fill restores the cells, the ages and the generation
  count from before it; redo puts the fill back.
- A change after an undo discards the redo; the 51st step drops the oldest.
- `Step`, and the first generation while playing, clear the history;
  `set_edges` keeps it.
- While playing, an edit is applied but not recorded, and undo does nothing.
- Undoing back to the saved revision clears `dirty`; redoing sets it again;
  a generation after a save sets it.
- A placement across a wrapping corner undoes exactly.
- `MarkSaved` with an older revision while playing leaves the universe dirty.
- Memory: run a 2048² 25 % fill for 1000 generations, then pause and make 50
  whole-grid random fills. The history stays under 256 MiB by
  `Capture::bytes`. Mark it `#[ignore]` if it takes over 5 s in debug.

## WP6 — Formats and libraries
> `agent: host` · `model: sonnet` · `gate: test` · `parallel: yes` · `reads: §4, §5, §9, §10, §13, Decisions taken › Files and formats`

New modules with no Tauri dependency, so they can be tested on their own.
Crates added to `src-tauri/Cargo.toml`: `serde` (derive), `serde_json`,
`toml`, `flate2`, `time` (RFC 3339); `tempfile` as a dev-dependency.

### `src-tauri/src/rle.rs`
`parse(text) -> Result<Rle, String>` (pattern, name, comment, the rule as
written, bounded grid, notices) and `write(&Pattern, name, comment, bounded)`.

### `src-tauri/src/glife_file.rs`
`write(path, &Universe)` (atomic) and `read(path) -> Result<Universe, String>`,
per Decisions taken.

### `src-tauri/src/open_rle.rs`
`universe_from_rle(&Rle) -> (Universe, notices)`, applying the stage §10
sizing.

### `src-tauri/src/pattern_file.rs`
Read and write the pattern-file TOML, both variants.

### `src-tauri/src/library/builtin.rs`
- `load(dir) -> (entries, problems)`: reads every `*.toml`, parses the
  pattern, and runs the check for its category on a dead-edged grid with a
  margin of 2 × period on every side.
  - A still life is unchanged after 1 generation.
  - An oscillator is back to its start after `period` generations and not
    before.
  - A spaceship is back to its start shape, moved by `(dx, dy)`, after
    `period`.
  - A gun's own bounding box is back to its start after `period`. After
    4 × `period`, the live cells outside the box form `4 × gliders` gliders.
- Speed is derived for display: `c/4 diagonal`, `c/2 orthogonal`, and so on.

### `src-tauri/patterns/*.toml`
30 files, among them:
- still lifes: block, beehive, loaf, boat, tub, pond, ship;
- oscillators: blinker, toad, beacon, clock, pulsar, pentadecathlon, figure
  eight, Kok's galaxy, queen bee shuttle;
- spaceships: glider, LWSS, MWSS, HWSS;
- guns: Gosper glider gun, Simkin glider gun;
- methuselahs: R-pentomino, acorn, diehard, B-heptomino, pi-heptomino,
  rabbits.

Each methuselah's description gives its outcome on an unbounded plane and
says the outcome on a bounded grid differs (stage §4).

### `src-tauri/src/library/personal.rs`
- `Library::open(dir)`, `list() -> (entries, problems)`,
  `save(name, comment, &Pattern, replace)`, `rename`, `set_comment` and
  `delete`.
- Name collisions ignore case. Replacing keeps the old comment when none is
  given, keeps `created` and updates `changed`. Every write is a temporary
  file plus a rename.

### Tests
- RLE:
  - a round trip keeps cells, size (dead margin included), name and comment;
  - text before the pattern is skipped, and text without a pattern is an
    error;
  - a multi-state file is refused, and a side over 2048 is refused;
  - a `B36/S23` file loads with a notice;
  - `:T100,80`, `:P100,80` and `:K100,80` are read;
  - Golly-written samples in `fixtures/rle/` load, and the output for a whole
    universe carries `:T`/`:P`.
- glife file:
  - save, load, then 100 steps match 100 steps of the original, cells and
    ages;
  - `fixtures/universe-v1.glife`, committed by this WP, loads to its known
    content;
  - an unknown section is skipped; `format = 2` is refused with the
    newer-glife message;
  - a truncated file is an error;
  - a write into a read-only directory fails and leaves the old file
    unchanged.
- RLE opening: a bounded torus, a bounded plane, a Klein bottle (dead edges,
  with a notice), a 10 × 10 pattern opening at 256 × 256, a 1000 × 10 pattern
  at 2048 × 256, and 2049 refused.
- Built-in library:
  - every file in `src-tauri/patterns/` loads and passes its check;
  - a temporary folder with a broken file and an oscillator with a wrong
    period yields both as problems, with the rest loaded.
- Personal library: same-name save without `replace` fails; replace keeps the
  comment and `created`; rename to a taken name fails; an unreadable file is
  listed and not touched; `fixtures/pattern-v1.toml` loads.

## WP7 — Selection, placing and undo in the UI
> `agent: ui` · `model: sonnet` · `gate: test` · `parallel: yes` · `reads: §6, §7, §8, Decisions taken › Interaction, Rendering, Command contract`

### `src/ui/edit/tools.ts`
A pure state machine over tool (`draw`/`select`), placing (pattern size and
bits, flips, mode, the tool to return to), and selection rect. It has
transitions for pointer down, move and up, the keys, Escape, New and Open,
and returns the backend calls to make. Selections are clamped to the grid
and never wrap.

### `src/ui/view/GridView.tsx`, `gridShader.ts`
- Pointer input goes through `tools.ts`. Each press of a draw stroke takes a
  new `stroke` id.
- The placement overlay and the selection outline per Decisions taken
  › Rendering. A DOM label shows the selection's W × H.

### `src/ui/backend.ts`, `src/ui/frame.ts`
- The WP5 and WP8 rows of the command contract that this WP uses: `edit`,
  `undo`, `redo`, `delete_selection`, `flip_selection`, `place`,
  `pending_cells`, `copy_selection`, `cut_selection`, `paste`.
- `decodePattern(buffer)` for the pattern buffer.

### `src/ui/controls/`
- `actions.ts` gains undo, redo, cut, copy, paste, delete, select all and both
  flips. Each acts on the placement first, then the selection, as the stage
  doc says.
- `Toolbar.tsx`: Draw/Select, Undo/Redo (enabled from the header flags), Flip
  H/V, and the *Or*/*Copy* switch, shown while placing.
- `shortcuts.ts`: the keys in Decisions taken.
- `Notice.tsx`: one line for errors and notices, dismissible.

### Tests
- `tools.ts`:
  - Escape ends placing first, then the selection;
  - placing returns to the previous tool;
  - a click outside the selection with the select tool ends it;
  - drawing and undo leave the selection in place;
  - a drag is normalized whatever its direction, and clamped at every grid
    edge.
- Delete and Backspace call `delete_selection` with a selection and `clear`
  without one.
- H and V flip the placement while placing and the selection otherwise.
- Undo and Redo buttons follow the flags, and are disabled while playing.
- A simulated drag sends batches with one stroke id; the next press uses a
  new one.
- A paste error shows a notice and does not start placing.
- `decodePattern` on a hand-built buffer.

## WP8 — Files, clipboard, menus
> `agent: host` · `model: sonnet` · `gate: test` · `parallel: yes` · `reads: §4, §5, §9, §10, §13, Decisions taken › Platform integration, Files and formats, Command contract`

### `src-tauri/Cargo.toml`, `main.rs`, `capabilities/default.json`, `tauri.conf.json`
- Add the dialog, clipboard-manager and opener plugins and register them.
- Bundle `patterns/` as a resource.
- Grant `core:window:allow-set-title`.
- Window close and `ExitRequested` go through the unsaved-changes flow.

### `src-tauri/src/menu.rs`
The menus, accelerators, `menu` events, the Edit-menu swap and
`set_menu_state` enabling, per Decisions taken.

### `src-tauri/src/commands.rs` (or split into `files.rs`, `library_cmds.rs`, `clipboard.rs`)
- The WP8 rows of the command contract. The commands that open dialogs are
  `async`.
- `Host` gains the current file path, the built-in entries loaded at startup
  from the resource folder, and the personal `Library` in
  `app_data_dir()/patterns`.
- `cut_selection` writes the clipboard, then sends `KillRect`.
- Open sends `Load`, then `MarkSaved`.

### Tests
- Command logic sits in functions that take paths and a `Sim` handle, so it
  can be tested without a window:
  - save, then open, gives the same universe and a clean mark;
  - Save As on an RLE-opened universe suggests `<name>.glife`;
  - a paste larger than the grid gives a message with both sizes;
  - copying an empty selection gives an RLE of the selection's size.
- The built-in folder resolves to the same path under `tauri dev` and from
  the unbundled release binary. The WP reports the resolved path.

## WP9 — Library panels and file actions
> `agent: ui` · `model: sonnet` · `gate: human` · `parallel: yes` · `reads: §4, §5, §6, §9, §10, §13, Decisions taken › Platform integration, Command contract`

### `src/ui/library/`
- `LibraryPanel.tsx`: a side panel with Built-in (grouped by category) and
  Personal, plus one filter box for both.
  - Each entry shows its preview, name, W × H, live cells, and period and
    speed where given. The selected entry shows its description or comment,
    plus dates for personal entries.
  - Actions: Place; Copy to personal (built-in); Rename, Edit comment,
    Delete, Export (personal); Show folder.
  - An entry larger than the current universe says so and cannot be placed.
  - The panel shows an empty-library hint, and lists problem files with their
    reasons.
- `PatternPreview.tsx`: the SVG path from `pattern_preview`.
- `SavePatternDialog.tsx`: name and multi-line comment. On `nameTaken` it
  offers Replace or Choose another name. It is used for a selection, the
  pending pattern and Copy to personal.

### `src/ui/controls/`
- `menu.ts`: routes `menu` events to `actions.ts`. It sends `set_edit_target`
  on `focusin`/`focusout` of text fields, and `set_menu_state` when the flags
  or the selection change.
- File actions: New (`confirm_discard`, then the dialog), Open, Save, Save
  As, Import RLE (starts placing), Export RLE (the selection if any, else the
  universe).
  - New and Open end placing and the selection, and Open fits the view.
  - Notices from Open and Import are shown.
- Window title through a `setTitle` wrapper in `backend.ts`, per Decisions
  taken. The panel's toggle button goes in the toolbar.

### Tests
- Filtering narrows both lists by name.
- An oversized entry's Place button is disabled.
- A taken name offers Replace, and Replace resends with `replace: true`.
- Each menu id calls its action.
- Focus in the filter box sends `set_edit_target("text")`, and blur sends
  `"universe"`.
- The title follows the file name and the dirty flag.
- Cancelling the discard prompt keeps the New dialog closed.

### Human gate — macOS click-through
Run every item in Verification on macOS with `npm run tauri dev`, except the
performance items, which need the release build as in WP3.

## WP9b — UI polish
> `agent: ui` · `model: sonnet` · `gate: human` · `reads: Decisions taken › Palettes and view settings, Interaction`

Changes from the WP9 click-through.

### `src/ui/controls/Toolbar.tsx`
- Two rows. Paired actions stack vertically: Play / Step, Undo / Redo,
  Draw / Select, Cut / Copy, Paste / Delete, Flip H / Flip V.
- Cut, Copy, Paste and Delete buttons, enabled by the same flags as the Edit
  menu.
- *Or | Copy* is one segmented switch, always shown, enabled while placing.
- Every button with an on/off state (Draw, Select, Play, Trail, Library, the
  *Or | Copy* switch) shows its active state.

### `src/ui/library/LibraryPanel.tsx`
- Clicking a row places its pattern; the Place button goes. An entry larger
  than the grid is not clickable and keeps its note.
- Copy to personal is an icon button with a tooltip.
- Rename and Edit comment edit in place in the row: Enter or leaving the field
  saves, Escape cancels. A taken name offers Replace or Choose another name in
  the row.
- Problem files are listed at the top of the panel when there are any.

### `src/ui/color/palettes.ts`, `Legend.tsx`
Age 0 is white in every palette, per Decisions taken › Palettes; the legend
shows it.

### Tests
- Every palette at every size has entry 0 white and entry 1 equal to its
  first stop.
- A row click begins placing; an oversized row does not.
- In-place rename: Enter saves, Escape cancels, a taken name offers Replace.
- Toolbar buttons expose their state (`aria-pressed`).

### Human gate — macOS click-through
The session rebuilds the release binary. The person checks each change above
by eye and answers yes or no per item.

## WP10 — Docs and rules
> `agent: kit:docs-writer` · `gate: human`

Proposes, for approval:
- `docs/architecture.md`:
  - the data model gains the age and trail layers, the frame layout v1
    additions, the pattern buffer, the glife file, the pattern file and the
    two library folders;
  - topology gains the plugins, menus and pending slot;
  - the components table gains history, formats, libraries and the library
    panel;
  - "Not built yet" drops Phase 2.
- `docs/adr.md`, new entries:
  - age and trail layers, `u16` ages, `u8` on the wire;
  - undo history on the simulation thread, cleared by any generation;
  - read-only commands read the latest snapshot. This supersedes ADR-007,
    whose guardrail names `frame` as the only such command; ADR-007 is marked
    superseded;
  - the glife file format and its versioning;
  - one TOML file per pattern; the built-in library as a folder checked on
    load;
  - dialogs, the clipboard, file access and menus in the backend, with the
    Edit-menu swap.
- Root `CLAUDE.md`, under Cross-cutting rules:
  - keep `fixtures/universe-v1.glife` and `fixtures/pattern-v1.toml`
    loading;
  - a new built-in pattern is one file in `src-tauri/patterns/`.
- `.claude/agents/engine.md`: the age and trail semantics.
- `.claude/agents/host.md`: write files atomically; the UI never sees paths.
- `.claude/agents/ui.md`: previews are SVG.
- `docs/roadmap.md`: Phase 2 to `Done` once Verification has passed on macOS.

## Verification

On macOS at WP9 (Windows and Linux are deferred; see the roadmap):

- **Built-in library**:
  - every category lists its patterns with preview, size, live count and,
    where they apply, period and speed;
  - the filter narrows the list;
  - a methuselah's description mentions bounded grids.
- **Adding a built-in pattern without a rebuild**: drop a new pattern file
  into the built-in folder of the release build and restart; it appears.
  A file with a wrong period appears under problems instead.
- **Placing**:
  - the preview follows the pointer; clicks place repeated copies; Escape
    returns to the previous tool;
  - H and V flip before placing;
  - *Or* keeps existing cells, and *Copy* clears the rectangle;
  - across a wrapping edge the pattern continues on the other side, and on
    dead edges it is cut off;
  - placing during a run joins from the next generation.
- **Selection**:
  - drag, the size label, Select All;
  - copy, then paste in *Copy* mode, reproduces the region exactly, its dead
    margin included;
  - cut, Delete and flip work, flip keeping ages;
  - Delete with no selection clears;
  - copied RLE pastes into Golly, and Golly's RLE pastes back.
- **Clipboard text**: RLE with forum text before it pastes; plain prose is
  refused with a message.
- **Undo**:
  - strokes, placements, cuts, deletes, flips, fills and clears each undo
    and redo as one step;
  - undoing a fill brings back its generation count;
  - Step or Play clears the history, and undo is disabled while playing;
  - undoing back to the saved state clears the title's mark.
- **Personal library**:
  - save a selection with a comment, then rename it, edit its comment,
    export it and re-import it, keeping name and comment;
  - saving under a taken name asks;
  - deleting asks first;
  - Show folder opens the folder;
  - the library survives a restart;
  - a hand-damaged file is listed while the rest still load.
- **Files**:
  - Save, quit, Open: same cells, ages and generation, paused, fitted;
  - New, Open, closing the window and Cmd+Q all ask when there are unsaved
    changes;
  - Open of an RLE with a `:T` grid, of a plain pattern and of a `B36/S23`
    file (with a notice) behaves as stage §10 says;
  - a damaged `.glife` is refused, and the current universe stays;
  - export of the whole universe opens in Golly at the same size and edges.
- **Menus**:
  - every File and Edit item works, with its shortcut;
  - in the filter box and the comment field, Cmd+C, Cmd+V, Cmd+Z and Cmd+A
    act on the text.
- **Coloring**:
  - age coloring shows true ages the moment it is switched on during a run;
  - the trail fades over its set length;
  - the palette and its size survive a restart;
  - the legend matches.
- **Performance** (release build, `GLIFE_BENCH=1`): 60 fps and at least
  30 gen/s in plain coloring and in age coloring with the trail on. Undo of a
  2048² fill and Save of a 2048² universe leave the window responsive.

## Deliberately out of scope

- Everything stage 2 lists as Out.
- Validating built-in patterns by anything other than the stage §13 checks.
  Methuselahs are loaded unchecked.
- Reloading the built-in folder while the app runs; a restart picks up new
  files.
- A built-in folder outside the app's resources, for example in the user's
  data directory. A file added to the shipped folder of an installed app is
  lost when the app is replaced.
- Compressing the undo history beyond run-length ages.
- Automated end-to-end, menu, dialog or WebGL tests; the human gates cover
  them.
