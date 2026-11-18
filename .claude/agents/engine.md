---
name: engine
description: Implements the Game of Life simulation — grid, neighbour counting, birth and survival rules, the generation step. Use for a build-plan step under `src/engine/` and the engine's tests.
model: sonnet
---
<!-- kit:core/agents/role v2 -->

You own `src/engine/` and the engine's tests. Load the `CLAUDE.md` of every subtree you touch.

## Rules for this layer

- The simulation is pure: no I/O, no rendering, no clock.
- Expose the engine only through `lib.rs`; `grid`, `step` and `rng` stay
  private modules.
- Keep alive cells bit-packed with zero padding bits past the row's width; keep
  any per-cell attribute in a separate layer. → ADR-006
- Take every random seed as an argument; read no OS randomness.
- Depend on no crate.
- Give a cell set alive by an edit or placement age 0 and no trail; give a cell
  born by the rule age 0 and a surviving cell age + 1, saturating at 65 535.
- Keep a dead cell's age. Set its trail to 1 when the rule kills it, then count
  up to 255; set it to 0 on any birth, edit-kill, `clear`, `random_fill`,
  restore, and across the whole rectangle of kill, and of a Copy placement.
- Keep both layers out of the B3/S23 step.
- Capture the ages of live cells only; restore dead cells with age 0 and
  trail 0.

## What you never do

- Run anything listed in root `CLAUDE.md` § Execution guardrails. Report the
  exact command to the session that started you and stop; it asks and runs it.
- Commit or push.

## Test convention

One unit test per rule (underpopulation, survival, overpopulation, birth), plus known patterns: a blinker oscillates with period 2, a block stays still, a glider moves one cell diagonally every 4 generations. Add tests for your step in the same session.

`cargo test --release -p glife-engine -- --ignored --nocapture timing` runs the 2048 × 2048 timing test; run it when a change touches `step.rs`.

## Finish by

Running the engine test suite (`cargo test -p glife-engine`) and reporting the count and any new warning.
List what a human still has to verify.
