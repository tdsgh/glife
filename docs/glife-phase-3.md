# glife — stage 3 feature description: Non-square tilings

Requirements for a person deciding whether these are the right ones. Technical
design is a later document: `build-plan-glife-phase-3.md`.

## 1. Purpose

Give the operator the same experience on grids whose cells are not squares:
hexagons and triangles. Everything that works on the square grid today works
on these grids too, so the operator can compare how Life-like rules behave
when cells meet their neighbors differently.

## 2. What we are building

The Phase 2 app, plus a choice of grid when a universe is created:

| Grid | Cell | Neighbors per cell |
|---|---|---|
| Square | square | 8, as today |
| Hexagonal | hexagon | 6, the cells sharing an edge |
| Triangular, 12 neighbors | triangle | 3 sharing an edge and 9 sharing only a corner |
| Triangular, 3 neighbors | triangle | 3, the cells sharing an edge |

Each grid has one rule of its own, fixed in this stage. Drawing, selection,
placing, flipping, undo, both pattern libraries, saving and opening, age
coloring and the trail all work on every grid.

A universe keeps the grid it was created with. Its edges wrap or are dead, as
on the square grid.

## 3. Scope of stage 3

**In.**
- Choosing the grid when creating a universe: square, hexagonal, triangular
  with 12 neighbors, triangular with 3 neighbors.
- One fixed rule per grid, shown with the grid's name.
- Hexagonal and triangular universes shaped as rectangles, with wrapping or
  dead edges, 3 to 2048 cells per side.
- Drawing and erasing cells, random fill and clear, play, step, speed, zoom,
  pan and fit, on every grid.
- Selection, copy, cut, delete, flip and placing with a preview, on every grid.
- Undo and redo, as in Phase 2.
- Saving and opening universes of every grid in the glife file.
- Patterns that record the grid they belong to, in both libraries, on the
  clipboard and in pattern files.
- A small built-in set of published patterns for each new grid, checked on
  load like the square ones.
- Age coloring, the trail and palettes on every grid.

**Out.**
- Changing the grid of an existing universe; converting a pattern from one
  grid to another.
- Editing or choosing the rule. Each grid has one rule in this stage; Phase 6
  makes rules editable.
- Rotating patterns or selections, including turns of 60° on the hexagonal grid.
- Hexagonal universes shaped as a rhombus.
- Other neighborhoods on the hexagonal grid, such as the 12 cells within two
  steps.
- Aperiodic tilings such as Penrose; tilings mixing cell shapes.
- Reading or writing RLE for hexagonal and triangular patterns (see §10).
- Several universes or windows at a time, as in Phase 2.

## 4. Choosing a grid

**Can see.** The New universe dialog offers the four grids, with width, height
and edges as today. Square stays the default. The status bar shows the grid's
name next to its size, edges and rule.

**Can change.** Pick the grid, width, height and edges when creating a
universe. Width counts cells along a row; height counts rows. On the
hexagonal grid rows run across the window and every other row is shifted by
half a cell. On the triangular grids triangles in a row point up and down in
turn, and each row starts the other way round from the row above.

The cells of a wrapping grid must line up across each edge:
- a wrapping hexagonal universe has an even height;
- a wrapping triangular universe has an even width and an even height.

The dialog says so next to the sizes and refuses an odd size with wrapping
edges. Switching an existing universe to wrapping edges when its size does not
allow it is refused with a message giving the reason. Dead edges allow any
size from 3 to 2048.

**Cannot.** Change the grid of an existing universe. New starts from the
dialog; Open takes the grid from the file.

## 5. Rules

**Can see.** The rule of the current universe, in birth/survival notation, in
the status bar, as B3/S23 is shown today.

Each grid's rule is taken from published work on Life-like rules for that
grid. It must support at least one still life, one oscillator and, where one
is known, a pattern that travels. The rules:

| Grid | Rule |
|---|---|
| Square | B3/S23 |
| Hexagonal | To be chosen. Candidate: B2/S35, Carter Bays' hexagonal Life rule, which has a glider |
| Triangular, 12 neighbors | To be chosen from Bays' rules for this grid, which include ones with gliders and glider guns |
| Triangular, 3 neighbors | To be chosen |

A rule is chosen together with the built-in patterns that run under it.

A neighbor count on each grid is the number of live cells among exactly the
neighbors in §2, across a wrapping edge as well.

**Cannot.** Change the rule (Phase 6). Run one grid's rule on another grid.

## 6. Viewing and drawing

**Can see.** Cells drawn in their true shape, hexagons or triangles, with grid
lines along the cell edges when zoomed in, and the universe's boundary. When a
cell is smaller than a screen pixel the view shades and blends colors as on
the square grid, so a lone live cell remains a faint pixel.

**Can change.** Draw and erase by clicking or dragging; the cell changed is the
one whose shape lies under the pointer. Zoom, pan and fit work as on the square
grid. Random fill, clear, play, step and speed are unchanged.

**Cannot.** Choose the hexagons' orientation; rows always run across the
window.

## 7. Selection, flipping and placing

**Can see.** A selection covers a range of rows and a range of cells within each
row. Its outline follows the cell edges, so on the hexagonal and triangular
grids its sides are stepped. Its width and height are shown in cells, as
today.

**Can change.**
- **Select, copy, cut, delete, save to library**, with the same rules and the
  same undo steps as in Phase 2.
- **Flip** horizontally or vertically. The result is the true mirror image of
  the pattern. On these grids a mirror image may have to move by half a cell
  or by one row, so that every cell lands on a cell of the same shape and
  orientation; the flipped pattern stays as close as possible to where it
  was. This holds for flipping a selection and for flipping a pattern before
  placing it.
- **Place**, with the preview under the pointer, in *Or* or *Copy* mode. The
  preview moves only to positions where the pattern keeps its shape: on the
  hexagonal grid a pattern may sit half a cell left or right of the pointer, and on
  the triangular grids it moves in steps that keep every triangle pointing
  the same way.

Wrapping and dead edges treat patterns and selections as in Phase 2: a
placement wraps or is cut off, and a selection does not cross a wrapping edge.

**Cannot.** Place a pattern made on another grid (§8). Rotate.

## 8. Patterns and the libraries

Every pattern belongs to one grid: square, hexagonal, triangular with
12 neighbors, or triangular with 3 neighbors. The two triangular grids share
cell shapes but not behavior, so their patterns are kept apart.

**Can see.** Both library panels list the patterns of the current universe's
grid, with the same entries as in Phase 2. A line under each list says how many
patterns belong to other grids. The preview pictures draw hexagons and
triangles in their true shape.

**Can change.**
- **Built-in library.** A small set of published patterns for each new grid:
  still lifes, oscillators and, where known, travelling patterns, each with
  its period, speed and a short description, as for the square grid. Each new
  grid ships with at least a few patterns. Like the square patterns, each one
  is checked against its stated facts every time the app loads the library,
  and a pattern that fails is not offered.
- **Personal library.** Save, name, comment, rename, delete, place and export,
  as in Phase 2. A pattern is saved with the grid of the universe it came
  from. Names stay unique across the whole personal library, whatever the
  grid.
- **Show the other grids' patterns.** A switch lists them too, marked with
  their grid. They can be renamed, commented on, exported or deleted there,
  but not placed.

Personal patterns saved in Phase 2 are square patterns.

**Cannot.** Place a pattern into a universe of another grid. Convert a pattern
to another grid.

## 9. Saving and opening universes

**Can change.** Save and open as in Phase 2. A saved universe records its grid
and rule, and opens into a universe of the same grid. Universe files saved in
Phase 2 open as square universes.

When a file names a grid this version of glife does not know, the operator is
told so and the current universe stays.

**Cannot.** Open an RLE file as a hexagonal or triangular universe (§10).

## 10. Clipboard and pattern exchange

**Can change.**
- **Copy** on a hexagonal or triangular grid puts the pattern on the system
  clipboard as glife's own pattern text, which names the grid, the size and
  the cells, and which a person can read. On the square grid copy still writes
  RLE.
- **Paste** reads either kind of text. Pattern text for another grid is refused
  with a message naming both grids. RLE pasted into a hexagonal or triangular
  universe is refused with a message saying RLE describes square grids.
- **Export and import a pattern file.** A pattern of any grid, a selection or a
  personal pattern is written to, and read from, a glife pattern file. A file
  read in starts placing, or is saved to the personal library, as RLE does in
  Phase 2. Name and comment travel with it.

**Can see.** On a hexagonal or triangular universe, the RLE import and export
commands are unavailable, with a note saying RLE covers square grids only.

**Cannot.** Exchange hexagonal or triangular patterns with Golly or LifeViewer
in this stage.

## 11. Cross-cutting requirements

- The Phase 1 and Phase 2 targets hold on every grid: a 2048 × 2048 universe at
  60 frames per second and at least 30 generations per second, with plain and
  with age coloring and the trail on. The window keeps responding within a
  tenth of a second while drawing, placing, selecting, undoing and saving.
- The simulation stays deterministic on every grid. A saved universe, reopened
  and run, produces the same history as the original from the moment it was
  saved.
- Universe files and personal patterns saved in Phases 1 to 3 open in every
  later stage.
- Each grid's rule is checked on small hand-worked cases, including cells
  across a wrapping edge and, on the triangular grids, both orientations of
  triangle.
- The built-in library is correct on every grid, by the checks of Phase 2.
- Phase 1 and Phase 2 checks still pass on the square grid. No existing key or
  shortcut changes meaning.
- The app still works offline, needs no account and sends no telemetry.

## 12. Feasibility

- **Already proven.**
  - The glife universe file and the pattern file already record a grid and
    refuse any grid but square, so files can carry the new grids without a new
    format.
  - Golly and LifeViewer run hexagonal and triangular Life on a square grid
    of cells, with every other row or cell read differently. A rectangle of
    hexagons or triangles fits the same row-by-row storage glife already
    uses.
  - Bays published Life-like rules with gliders for the hexagonal grid and the
    12-neighbor triangular grid, and glider guns on the triangular one.
- **New but straightforward.** The grid choice in the dialog and status bar;
  drawing hexagons and triangles and their grid lines; finding the cell under
  the pointer; stepped selection outlines; library panels filtered by grid;
  pattern text on the clipboard; pattern file import and export.
- **Known costs.**
  - The 12-neighbor triangular grid counts half as many neighbors again as the
    square grid, and does so differently for triangles pointing up and down.
    Holding 30 generations per second at 2048 × 2048 there is the stage's main
    technical risk.
  - On the hexagonal grid, which cells are neighbors depends on whether a row
    is shifted, so the generation step has two cases where the square grid has
    one.
  - Flipping and placing must keep cells on cells of the same shape and
    orientation, which adds offsets the square grid never needed.
  - Shading the zoomed-out view by the share of live cells is harder when
    cells do not line up with pixel rows and columns.
  - Collecting the built-in patterns is research: few patterns for these grids
    are published in a form glife can read, and for the 3-neighbor triangular
    grid little is published at all.
- **Blocked.** Nothing. The three new rules are not chosen yet (§5, §13).

## 13. Open questions

- The rule for each new grid (§5). It is settled by a research step at the
  start of the build plan, which chooses each rule and collects its published
  patterns, and is approved before the rest is built. The 3-neighbor
  triangular grid has the fewest published rules; if none has published
  patterns, its built-in set may hold only still lifes and oscillators found
  while choosing the rule.
- Exchange with Golly and LifeViewer. Both write hexagonal patterns as RLE with
  an `H` after the rule and triangular ones with an `L`, on a slanted or
  emulated square grid. Whether to read and write these in a later stage is
  open; this stage leaves it out.

## 14. Beyond stage 3

Phase 4 brings closed surfaces; whether they use hexagonal and triangular
cells as well as squares is a roadmap question, and this stage's grids are the
candidates. Phase 6 makes the rule of every grid editable and adds rule
families; each grid's rule from this stage becomes its preset there. Rotation,
including turns of 60° on the hexagonal grid and 120° on the triangular ones,
remains for later, as does exchanging non-square patterns with other programs.

## 15. Notes for implementation planning

Phase 2 is built and verified on macOS. The engine, the generation step, the
transfer of cells to the screen, the shader, selections, placements, flips and
the pattern buffers all assume square cells in rows. The universe file and the
pattern file already carry a grid name and refuse anything but square; how the
two triangular grids are told apart there is still to decide.

Still to decide: how hexagons and triangles map onto the existing row-by-row
storage, and how the step handles shifted rows and the two orientations of
triangle at word speed; how the screen learns a universe's grid, since today
it learns everything about the universe from what each frame carries; how the
pointer finds a cell; and how flips and placements choose their half-cell or
one-row offset. The 12-neighbor triangular step decides whether the speed
target holds, so the plan should measure it early, on a full
2048 × 2048 grid.

The built-in patterns for the new grids need a research step before they can
be written as files: choosing each rule and collecting patterns that run under
it. The clipboard needs a way to tell glife pattern text from RLE. Phase 1 and
2 keys are taken; this stage adds no new keys unless the plan finds one
necessary.
