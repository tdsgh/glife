# glife — stage 2 feature description: Patterns

Requirements for a person deciding whether these are the right ones. Technical
design is a later document: `build-plan-glife-phase-2.md`.

## 1. Purpose

Let the operator work with patterns rather than single cells: pick well-known
patterns from a library, keep their own, move groups of cells around, take back
mistakes while building a pattern, and keep their work between sessions. This
stage also gives cells a color that tells their story, starting with their age.

## 2. What we are building

The Phase 1 app, plus:

- two pattern libraries side by side: a built-in one of well-known patterns,
  and a personal one the operator fills;
- a selection tool that copies, cuts, deletes and flips a rectangle of cells,
  and a clipboard that speaks RLE, the text format the Life community shares
  patterns in;
- undo and redo for the changes made while paused, since the universe last
  advanced;
- saving and opening universes, and importing and exporting RLE files;
- coloring live cells by age, with an optional fading trail behind dying cells,
  drawn from palettes of 16, 32 or 256 colors.

The universe itself is unchanged: one bounded square grid, wrapping or dead
edges, the rule B3/S23.

## 3. Scope of stage 2

**In.**
- A built-in library of well-known patterns: still lifes, oscillators,
  spaceships, guns and methuselahs.
- A personal pattern library on the operator's machine: save, name, comment,
  rename, delete, place, export, and a way to reach its folder.
- Placing a pattern by clicking, with a preview under the pointer, flipped
  first if wanted.
- A rectangular selection; copy, cut, delete, flip horizontally and
  vertically, save to the personal library, export as RLE.
- Copy and paste through the system clipboard as RLE text.
- Undo and redo of drawing, fills, clears, placements, and cuts, deletes and
  flips of a selection, made while paused, back to the last generation step.
- Saving and opening universes in a glife file, ages included.
- Importing and exporting RLE files: a pattern, a selection or a whole
  universe.
- Coloring live cells by age, an optional trail after death, a choice of
  built-in palettes at 16, 32 or 256 colors.
- File and Edit menus with each platform's usual shortcuts.

**Out.**
- Rotating patterns or selections.
- Online pattern catalogs; downloading patterns from the internet.
- Pattern search and analysis: recognizing objects in a universe, detecting
  period or speed.
- Pattern formats other than RLE (plaintext `.cells`, Life 1.06, macrocell).
- Any rule other than B3/S23. RLE files written for other rules are read for
  their cells only (see §10).
- Coloring by any attribute other than age; palettes larger than 256 colors.
- Editing the colors of a palette.
- Resizing an existing universe; more than one universe or window at a time.
- Several selections at once; selections of any shape but a rectangle;
  selections across a wrapping edge.
- Undoing changes made during a run, or going back in time.
- Exporting or importing the personal library as a whole; syncing it between
  machines.

## 4. The built-in pattern library

**Can see.** A panel listing the built-in patterns by category: still lifes,
oscillators, spaceships, guns, methuselahs. Each entry shows its name, a
preview picture, its bounding box (width × height) and live-cell count, and,
where it applies, its period and speed (a glider: period 4, speed c/4
diagonally). Selecting an entry shows a short description. A filter box
narrows the list by name.

The library holds a few dozen patterns, the ones a newcomer to Life meets
first: among them the block, beehive, blinker, toad, beacon, pulsar,
pentadecathlon, glider, the lightweight, middleweight and heavyweight
spaceships, the Gosper glider gun, the R-pentomino, the acorn and diehard.

A methuselah's description gives its outcome on an unbounded plane (the
R-pentomino settles after 1103 generations) and says that on a bounded grid
its escaping gliders hit the edge or come back, so the outcome differs.

**Can change.** Place a pattern into the universe (§6). Copy a built-in
pattern into the personal library, to annotate it; its description becomes the
copy's comment.

**Cannot.** Edit, rename or delete a built-in pattern. Add to the built-in
library from within the app; the personal library is where the operator's own
and imported patterns go (§5).

**Where it comes from.** The built-in library ships with the app. Each pattern
is its published RLE together with its category, period, speed and a
description written for glife. The library grows by adding a pattern to the
app's source and building a new version. The correctness check in §13 then
confirms its stated period and speed.

## 5. The personal pattern library

**Can see.** A second list next to the built-in one, with the same layout: name,
preview, bounding box, live-cell count, comment, and when the pattern was first
saved and last changed. The same filter box applies. An empty library shows how
to add the first pattern.

**Can change.**
- **Save.** Save the current selection as a new pattern, giving it a name and
  optionally a comment. A pattern from the clipboard or an RLE file can be
  saved the same way.
- **Same name.** Names are unique within the personal library. Saving or
  importing under a name already taken asks whether to replace that pattern or
  pick another name. Replacing keeps the old comment unless a new one is given,
  keeps the first-saved date and updates the last-changed date.
- **Comment.** Add or edit a comment of any length, as plain text over several
  lines.
- **Rename.**
- **Delete.** Asks before deleting; a deleted pattern cannot be brought back.
- **Place.** Place it into any universe, exactly like a built-in pattern.
- **Export.** Write it out as an RLE file. Its name and comment travel with it,
  so a pattern exported and imported again keeps both.
- **Show folder.** Open the folder that holds the library in the system's file
  manager, to back it up or move it by hand.

The library lives on the operator's machine, survives restarts and app updates,
and needs no account. It is one library per user of the machine. Its patterns
are kept as files a person can read.

If part of the library cannot be read, the panel shows the patterns it could
read and names the ones it could not; nothing is deleted. If a change cannot be
written, the operator is told and the library stays as it was.

**Cannot.** Edit a saved pattern's cells; a changed pattern is saved again under
a new or the same name. Sort patterns into folders. Undo a library change: undo
covers the universe only (§8).

## 6. Placing a pattern

**Can see.** After choosing a pattern from either library, or pasting from the
clipboard, a translucent copy of it follows the pointer over the grid, so the
operator sees where it will land before committing.

**Can change.**
- **Place.** A click places the pattern there. The pattern stays under the
  pointer, so several copies can be placed in a row. Escape, or choosing
  another tool, ends placing, and the tool used before placing returns.
- **Flip.** Before placing, flip the pattern horizontally or vertically, with a
  button or a key.
- **Paste mode.** *Or*, the default: the pattern's live cells are added, and
  the live cells already there stay, keeping their ages. *Copy*: the pattern's
  whole rectangle replaces what lies beneath it, dead cells included. The
  choice is visible while placing and stays until changed.

Placing works while playing or paused. While playing, a placed pattern takes
part from the next generation on. While paused, each placement is one undo
step.

On a wrapping grid, a pattern placed across an edge continues on the opposite
side. Such a pattern cannot later be selected as one piece (§7). On a grid
with dead edges, the part outside the grid is cut off. A pattern wider or
taller than the grid cannot be placed: its library entry says so, and a paste
or import is refused with a message giving both sizes.

**Cannot.** Rotate a pattern. Move a pattern after it has been placed, other
than by selecting and cutting it.

## 7. Selection and the clipboard

**Can see.** The selected rectangle, outlined over the grid, with its width and
height. The selection stays where it is while cells move through it during a
run.

**Can change.**
- **Select.** With the select tool, drag a rectangle. Select all takes the
  whole grid.
- **Copy.** Puts the selected rectangle on the system clipboard as RLE text,
  dead margin included, so a pattern pasted in *Copy* mode reproduces the
  region exactly. An empty selection copies an empty pattern of its size.
- **Cut.** Copy, then kill every cell in the selection.
- **Delete.** With a selection, Delete and Backspace kill every cell in it,
  without copying. Without a selection, they clear the universe, as in
  Phase 1.
- **Flip.** Mirror the selection's cells horizontally or vertically in place;
  each cell keeps its age.
- **Paste.** Reads RLE text from the system clipboard and starts placing it
  (§6). The text may hold other lines before the pattern, as text copied from
  a forum does; text with no RLE pattern in it is refused with a message
  saying so.
- **Save to library** and **Export as RLE** act on the same rectangle as Copy.

Copy captures the cells as they are at that moment, also during a run. Cut,
delete and flip take effect from the next generation on while playing; while
paused, each is one undo step.

The selection ends with Escape (after placing has ended, if both were
active), with a click outside it using the select tool, with New or Open, or
when the operator selects again. Drawing outside it, switching tools and undo
leave it in place.

The Draw tool from Phase 1 stays the default. Draw and select are switched with
a toolbar button or a key. New keys do not take over Phase 1's keys. While a
text field such as the filter box or a comment has focus, keys and Edit
shortcuts act on its text, not on the universe.

**Cannot.** Select across a wrapping edge. Select a non-rectangular region. Fill
a selection at random.

## 8. Undo and redo

Undo is for building a starting pattern. It covers the changes made while the
universe is paused, since it last advanced.

**Can change.** Undo takes back the most recent change to the cells; redo puts
it back. A new change after an undo discards what could have been redone.

Each of these is one undo step:
- a drawing stroke, from press to release;
- a placement or paste;
- a cut, delete or flip of a selection;
- a random fill or a clear.

Undo restores the cells, their ages and the generation count exactly as they
were, so undoing a random fill brings back the generation count it reset.
Undoing back to the last saved state removes the unsaved-changes mark.

Advancing the universe, by play or by a single step, clears the history. While
playing, undo and redo are unavailable, and changes made during the run are
not recorded. Changing the edge mode keeps the history.

The history holds at least the last 50 steps on the largest grid.

**Can see.** Whether undo and redo are available, in the menu and the toolbar.

**Cannot.** Undo a run or a step: there is no going back in time. Undo a change
of edge mode. Undo a new universe or opening a file; both start an empty
history. Undo changes to the personal library.

## 9. Saving and opening universes

**Can change.**
- **Save / Save as.** Writes the universe to a glife file the operator names
  and places. The file holds the grid's width and height, edge mode, rule,
  generation count, cells and the age of every live cell. Saving during a run
  saves the generation current at that moment; the run continues.
- **Open.** Reads a glife file, or an RLE file (§10), into a new universe that
  replaces the current one, paused and fitted to the window.
- **Unsaved changes.** New universe, Open, closing the window and quitting the
  app ask whether to save first when the universe differs from the file it was
  last saved to or opened from. Every change counts, generations run included.

**Can see.** The file name in the window title, marked when there are unsaved
changes.

The file does not hold the undo history, whether the universe was playing, the
speed, the view, the palette or the trail. A reopened universe starts paused,
with the same ages it was saved with and no trail.

When saving fails, for example on a full disk, the operator is told why, the
universe is unchanged and a file that existed before is left intact. When a
file cannot be opened, because it is damaged, not a glife or RLE file, or
written by a newer glife, the operator is told why and the current universe
stays.

**Cannot.** Open more than one universe at a time.

## 10. RLE import and export

**Can change.**
- **Import as pattern.** An RLE file or the clipboard's RLE text starts
  placing (§6), or is saved to the personal library (§5).
- **Open as universe.** An RLE file that declares a bounded grid of 3 to 2048
  cells per side opens with that grid's size; a plane grid opens with dead
  edges and a torus with wrapping edges. A declared grid of a kind glife does
  not have, such as a Klein bottle, opens with dead edges and a notice. Any
  other RLE file opens in a new universe with dead edges, so that escaping
  gliders do not come back: the pattern centered, with a margin at least as
  wide as the pattern on every side, at least 256 × 256 and at most
  2048 × 2048. The universe starts at generation 0, every cell at age 0.
- **Export.** A selection, a personal or built-in pattern, or the whole
  universe, to an RLE file. A whole universe is written with its grid size and
  edges as a bounded grid, which Golly opens with the same size and edges.

**Can see.** The pattern's name and comments from the file, when it has them.
When a file states a rule other than B3/S23, a notice says that its cells were
loaded and will run under B3/S23.

A pattern exported and imported again has the same cells, size, name and
comment. Golly opens any RLE glife writes, and glife reads the RLE Golly writes
for B3/S23 patterns.

**Cannot.** Open an RLE pattern larger than 2048 × 2048. Read a pattern with
more than two cell states; it is refused with a message. Import other formats.

## 11. Coloring by age

**Can see.** In age coloring, each live cell takes a color from the palette by
its age: the number of generations it has stayed alive. Ages past the palette's
last color take the last color. A legend shows the palette and the ages its
colors stand for.

Every edit that sets a cell alive gives it age 0: drawing, placing, pasting,
random fill. A cell that is already alive and stays alive keeps its age, as
under an *Or* placement or a flip.

With the trail on, a cell that the rule kills keeps a fading color for a chosen
number of generations, from its last color down to the background. Cells killed
by an edit leave no trail. The trail fades only as generations advance. It is
drawn only in age coloring; it never counts as a live cell, and it never
changes what the rule does. Its length is 1 to 100 generations, 10 by default.

When a cell is smaller than a screen pixel, the pixel blends the colors of the
cells it covers with the background, weighted by how many are alive or
trailing. Old regions and young regions stay distinguishable at any zoom, and
a lone live cell is still visible as a faint pixel, as in Phase 1.

**Can change.** Switch between plain coloring (Phase 1's look) and age coloring
at any time, during a run as well. Ages are kept all along, so switching on
shows the true ages at once. Switch the trail on or off and set its length.

**Cannot.** Color by any other attribute. Set a different age per color step.

## 12. Palettes

**Can see.** The chosen palette, in the legend and the view.

**Can change.** Choose one of at least three built-in palettes, and its size:
16, 32 or 256 colors. Every built-in palette comes in all three sizes. At
least one palette keeps its colors distinguishable for people with common
color-vision deficiencies. The choice is kept between sessions and does not
depend on the universe.

**Cannot.** Edit a palette or make a new one. Use more than 256 colors.

## 13. Cross-cutting requirements

- The app still works offline, needs no account and sends no telemetry. The
  personal library and saved universes are files on the operator's machine.
- The Phase 1 targets hold with plain coloring and with age coloring and the
  trail on: 2048 × 2048 at 60 frames per second and at least 30 generations
  per second. The window keeps responding within a tenth of a second while
  placing, selecting, undoing and saving.
- A saved universe, reopened and run, produces the same history as the
  original would have from the moment it was saved.
- Universes and personal-library patterns saved in this stage open in every
  later stage.
- File and Edit commands are in the app's menu bar as well as the toolbar, with
  each platform's usual shortcuts for new, open, save, undo, redo, cut, copy,
  paste and select all.
- The built-in library is correct: each still life stays unchanged; each
  oscillator returns to its starting state after its stated period; each
  spaceship returns to its starting shape, moved by its stated displacement,
  after its period; each gun returns to its starting shape after its period and
  has emitted its stated output. Methuselahs are not checked, since their
  outcome depends on the grid.
- Phase 1's known-pattern checks still pass.

## 14. Feasibility

- **Already proven.** RLE is a simple, well-documented text format, and the
  well-known patterns are freely published in it. Keeping undo history for
  grids of at most 2048 × 2048 cells is a matter of memory, not of technique.
- **New but straightforward.** The libraries' panels, placement preview,
  selection, flipping, the clipboard, file dialogs, menus and storing the
  personal library on disk.
- **Known costs.**
  - Age coloring sends far more data to the screen than alive-or-dead: at
    least a byte per cell instead of a bit, so up to 4 MiB per frame for a
    whole 2048 × 2048 grid in view. Holding the Phase 1 targets with age
    coloring on is the stage's main technical risk.
  - Keeping ages up to date costs time in every generation, also while age
    coloring is off.
  - Blending colors in the zoomed-out view is more work than shading by the
    share of live cells.
  - Undo restores ages as well as cells. At the largest grid, 50 steps of
    whole-grid changes, such as random fills, can take a few hundred MiB
    unless the history is stored compactly.
  - Saved universes carry ages: up to a few MiB more per file at the largest
    grid, before compression.
- **Blocked.** Nothing.

## 15. Open questions

None at present.

## 16. Beyond stage 2

Phase 3 brings hexagonal and triangular grids. Their patterns cannot be written
in RLE, so the glife file this stage defines must have room for a tiling, every
personal pattern must record the tiling it belongs to, and the clipboard and the
library will need glife's own pattern format next to RLE. A pattern can then be
placed only into a universe of its own tiling. The selection, placement, flip,
undo and library tools of this stage carry over to those grids.

Later phases add to what a file and a pattern must hold: surfaces and three
dimensions (Phases 4 and 5), cells with more than two states (Phase 6), and
universes with no edges and patterns far larger than 2048 × 2048 (Phase 7).

Age coloring is the first per-cell attribute shown with a palette. Phase 6's
multi-state rules, where each state has its own color, reuse it. Palettes
larger than 256 colors may follow; the 256 limit is not fixed for good.

Rotation is left out. Hexagonal grids may bring it back as turns of 60°.

## 17. Notes for implementation planning

Phase 1 is built and verified on macOS. Every change to the universe is applied
between generations, and the screen receives only the cells in view, once per
frame. Phase 2 is the first stage where cells must also travel on request: to
the clipboard, to files and to the personal library.

Still to decide: where the undo history is kept and how it learns that the
universe advanced; how ages are stored and how wide an age value is, given that
palettes above 256 colors may follow; how ages reach the screen at the full
speed target when the whole grid is in view; and how the zoomed-out blend is
made. These choices decide whether the speed targets survive age coloring, so
the plan should measure them early, on a full 2048 × 2048 grid.

The glife file needs a version and room for what later phases add (§16). The
personal library's on-disk form is open; it must stay readable by a person,
since its folder is shown to the operator, and it must record each pattern's
tiling. The built-in library needs one place in the source where a pattern and
its facts are added, so that a new pattern is one addition and the correctness
check picks it up without further changes.

File dialogs, clipboard access, menus and storage in the user's app-data folder
are new to the app. The keys for the select tool and for flipping are still to
be chosen; Phase 1 already uses Enter, N, R, F, Delete and Backspace.
