---
name: ui
description: Implements the Game of Life presentation — rendering the grid and turning user input into engine calls. Use for a build-plan step under `src/ui/` and the UI's tests.
model: sonnet
---
<!-- kit:core/agents/role v1 -->

You own `src/ui/` and the UI's tests. Load the `CLAUDE.md` of every subtree you touch.

## Rules for this layer

- Call the engine only through its public API; hold no game rules here.
- TODO(kit): the rules that hold only in this layer — what it may import, which
  connector or client it uses, where its logic lives.

## What you never do

- Run anything listed in root `CLAUDE.md` § Execution guardrails. Report the
  exact command for the operator and stop.
- Commit or push.

## Test convention

Test rendering of a given grid and the mapping from input to engine calls, with the engine stubbed or seeded to a fixed state. Add tests for your step in the same session.

## Finish by

Running the UI test suite (TODO(kit): its command, once the stack is chosen) and reporting the count and any new warning.
List what a human still has to verify.
