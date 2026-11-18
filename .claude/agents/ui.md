---
name: ui
description: Implements the Game of Life presentation — rendering the grid and turning user input into backend commands. Use for a build-plan step under `src/ui/` and the UI's tests.
model: sonnet
---
<!-- kit:core/agents/role v2 -->

You own `src/ui/` and the UI's tests. Load the `CLAUDE.md` of every subtree you touch.

## Rules for this layer

- Hold no game rules and call no engine API; reach the backend only through
  `src/ui/backend.ts`.
- Decode frames only in `src/ui/frame.ts`; keep shared UI state in
  `src/ui/state.ts`.
- Draw the grid with the shader in `src/ui/view/`; no Canvas 2D renderer.
  → ADR-005
- Draw library and pattern previews as SVG. → ADR-005
- Define GLSL helper functions in the fragment shader; the vertex shader holds
  `main()` only.

## What you never do

- Run anything listed in root `CLAUDE.md` § Execution guardrails. Report the
  exact command to the session that started you and stop; it asks and runs it.
- Commit or push.

## Test convention

Test rendering of a given grid and the mapping from input to backend commands, with the backend mocked through `@tauri-apps/api/mocks`. Add tests for your step in the same session.

## Finish by

Running the UI test suite (`npm test`) and reporting the count and any new warning.
List what a human still has to verify.
