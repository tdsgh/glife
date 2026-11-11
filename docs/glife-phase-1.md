# glife — stage 1 feature description: Classic Life

Requirements for a person deciding whether these are the right ones. Technical
design is a later document: `build-plan-glife-phase-1.md`.

## 1. Purpose

Give glife its first working version: Conway's Game of Life on a bounded square
grid, fully interactive on the desktop. This stage also sets up the window, the
controls and the view that every later phase builds on.

## 2. What we are building

A desktop app with one window holding one universe. A universe is a grid of
square cells, each either alive or dead, that moves from one generation to the
next by Conway's rule. The operator creates a universe, draws on it or fills it
at random, runs it at a chosen speed, steps through it one generation at a time,
and moves around it by zooming and panning. A status area shows where the run
stands.

## 3. Scope of stage 1

**In.**
- Creating a universe of a chosen width and height, up to 2048 × 2048 cells.
- Wrapping or dead edges, switchable at any time.
- The rule B3/S23.
- Play, pause, single step and speed control.
- Drawing and erasing cells with the mouse, during a run or while paused.
- Random fill and clear.
- Zoom, pan and fit-to-window, including a readable view of the whole grid when
  it is larger than the window.
- Generation count, live-cell count and achieved speed.
- Unsigned builds that run on macOS, Windows and Linux.

**Out.**
- Saving and opening universes; pattern files of any format.
- The pattern library, selection, copy and paste, rotate and flip.
- Undo and redo.
- Any rule other than B3/S23, and a way to choose the rule at all.
- Resizing an existing universe.
- More than one universe or window at a time.
- Non-square cells, closed surfaces, 3D, unbounded grids.
- Signed or notarised installers.

## 4. The universe

**Can see.** The grid's width and height, its edge behaviour and its rule
(B3/S23). These are always visible.

**Can change.** The operator creates a new universe by choosing a width and a
height (each from 3 to 2048 cells, chosen independently) and an edge behaviour.
The new universe starts empty, at generation 0, paused and fitted to the
window. When the app launches, it opens a 256 × 256 empty universe with
wrapping edges, so that no dialog stands in the way of starting.

**Cannot.** Resize an existing universe. Changing its size means creating a new
one, and the current cells are lost. Change the rule.

A side shorter than 3 cells is not allowed: on a wrapping grid that narrow, a
cell would count the same neighbour twice.

## 5. Edges

**Can see.** Whether the edges wrap or are dead. The grid's boundary is drawn in
both modes.

**Can change.** The operator switches between the two modes at any time,
including during a run. The switch takes effect from the next generation.

- **Wrapping.** The grid is a torus: the left edge borders the right, the top
  borders the bottom. A pattern that leaves on one side comes back on the
  opposite side.
- **Dead.** Every position outside the grid counts as a dead cell. Nothing
  crosses the edge; a pattern that reaches it is cut off by it.

**Cannot.** Wrap in one direction only, or wrap with a twist.

The view shows a single copy of the grid in both modes. It does not tile the
torus visually.

## 6. The rule and the generation step

**Can see.** The rule as text, B3/S23, in the status area.

**Behaviour.** In each generation, every cell updates at the same moment from
the state of its eight neighbours in the previous generation:

- A dead cell with exactly 3 live neighbours becomes alive.
- A live cell with 2 or 3 live neighbours stays alive.
- Every other cell is dead in the next generation.

The same starting cells, edge mode and sequence of edits always produce the same
history.

**Cannot.** Change or replace the rule in this stage.

## 7. Running

**Can see.** Whether the universe is playing or paused. The chosen speed. While
playing, the achieved generations per second.

**Can change.**
- **Play / pause.** Starts and stops the run.
- **Step.** Advances exactly one generation. Available while paused.
- **Speed.** Chosen from 1 generation per second up to "as fast as possible",
  and changeable during a run with immediate effect.

When the run goes faster than the screen redraws, the view shows the latest
generation on each redraw, and the generations in between are not drawn. The
generation count still counts every one of them.

At the largest grid size, 2048 × 2048, "as fast as possible" reaches at least 30
generations per second on a typical current laptop, while the view stays at 60
frames per second.

**Cannot.** Step backwards, or run a set number of generations and stop.

## 8. Editing cells

**Can change.**
- **Draw and erase.** Pressing on a dead cell starts a drawing stroke; pressing on
  a live cell starts an erasing stroke. Dragging extends the stroke to every
  cell the pointer passes over, with no gaps, however fast the pointer moves.
- **Editing during a run.** Strokes work while playing or paused. While playing,
  each edited cell takes its new state from the next generation on.
- **Random fill.** Replaces the whole grid with randomly placed live cells at a
  chosen density (1–99 %, default 25 %). Each fill is different.
- **Clear.** Kills every cell.

Random fill and clear reset the generation count to 0 and leave the run playing
or paused as it was. Drawing does not reset the count.

**Cannot.** Undo an edit, a fill or a clear in this stage. Draw a line, rectangle
or shape in one gesture. Edit more than one cell under a single click.

## 9. The view

**Can see.** The grid, at any zoom from "the whole grid fits the window" to a
cell 64 pixels across. Grid lines between cells appear once cells are large
enough to tell apart, and fade out below that.

When the grid is zoomed out so far that a cell is smaller than a screen pixel,
each pixel is shaded by the share of live cells it covers. A lone live cell is
still visible as a faint pixel; it never disappears from the zoomed-out view.

**Can change.**
- **Zoom.** Mouse wheel or trackpad pinch, centred on the pointer.
- **Pan.** Trackpad two-finger swipe, or dragging with the right or middle mouse
  button, or holding Space while dragging with the left button.
- **Fit to window.** Returns to the whole grid, centred.

Resizing the window keeps the point at the centre of the view in place.

**Cannot.** Rotate the view. Pan past the grid more than half a window beyond
its edge.

## 10. Status and controls

**Can see.** Generation count, live-cell count, achieved generations per second
while playing, grid size, edge mode and rule. While running, the counts update
at least several times per second.

**Can change.** Every control is a button in the window. Play/pause, step,
clear, random fill and fit-to-window also have keyboard shortcuts.

## 11. Cross-cutting requirements

- The app runs on macOS, Windows and Linux from unsigned builds. It works
  offline, needs no account and sends no telemetry.
- The window keeps responding during a run at any grid size and speed:
  controls, drawing, zoom and pan react within a tenth of a second.
- The performance target holds on all three operating systems: 2048 × 2048 at 60
  frames per second and at least 30 generations per second, including while
  the operator draws, zooms or pans.
- Known patterns behave correctly:
  - a blinker alternates between its two phases every generation;
  - a block stays unchanged;
  - a glider moves one cell diagonally every 4 generations, and on a wrapping
    N × N grid it returns to its starting position after 4 × N generations;
  - with dead edges, no live cell ever appears outside the grid's boundary.

## 12. Feasibility

- **Already proven.** Computing B3/S23 over a few million cells many times per
  second is routine for native code. Drawing a few million cells per frame on
  the GPU is routine.
- **New but straightforward.** The window and its controls, mouse drawing,
  zoom and pan, the status area.
- **Known costs.**
  - The zoomed-out shaded view: at the largest grid, the cells outnumber the
    screen's pixels, so the view needs a summary of the grid rather than the
    cells themselves.
  - Moving up to four million cells from the simulation to the screen 60 times
    a second. This has not been measured in the Windows webview, which may be
    the slowest of the three.
- **Blocked.** Nothing.

## 13. Open questions

- Should clear, random fill and creating a new universe ask before discarding
  live cells, given that there is no undo or save in this stage?
- Is 256 × 256 with wrapping edges the right universe to open at launch?
- Is 25 % the right default density for random fill?
- Which machine is "a typical current laptop" for the performance target on
  each operating system?

## 14. Beyond stage 1

Phase 2 adds saving and opening, the pattern library, selection with copy, cut,
paste, rotate and flip, undo and redo, and RLE import and export. Its universes
must open the grids this stage creates: the width, height, edge mode and cells
defined here are the minimum a saved universe holds.

The zoomed-out shaded view is a first step towards the summary view the infinite
universe of Phase 7 needs.

## 15. Notes for implementation planning

The repository holds documentation and the definitions of the engine and UI
agents; there is no code yet, and no test command. The stack is settled in the
roadmap and the decision log. The build plan starts from an empty project:
it needs the app skeleton for all three operating systems, and it needs the
repository's test command recorded.

Still to decide is the shape of the data that crosses from the simulation to
the screen on each frame: what is sent when the whole grid is in view, what is
sent when only part of it is, and which side produces the zoomed-out shading.
The same choice decides whether the 60-frames target holds on Windows, so the
plan should measure it early, on a full 2048 × 2048 grid, before the UI is
built out around it.

Also to decide: how the run's pace is kept when generations take longer than
the chosen speed allows, and how edits made during a run reach the simulation
so that a fast stroke loses no cells.
