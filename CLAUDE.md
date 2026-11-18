<!-- kit:core/CLAUDE.root v1 -->
# glife — orientation for Claude Code

A Game of Life implementation, used as a test project for wscontext.
The system itself is described in
[docs/architecture.md](docs/architecture.md).

This file owns the rules that hold everywhere in this repo. Subtrees
(none yet) each have a `CLAUDE.md` that adds rules for that subtree and does
not repeat these. Precedence and conflict handling:
[.claude/rules/kit-precedence.md](.claude/rules/kit-precedence.md).

## Project facts

Skills and agents read these by slot name. Change a value freely; keep the slot
names.

| Slot | Value |
|---|---|
| Docs dir | `docs/` |
| Build plans | `docs/build-plan-*.md` |
| Test command | `cargo test --workspace && npm test` |
| Subtrees | none |

## Non-negotiable rules

The bullets tagged **(non-negotiable)** under Cross-cutting rules below and in
`.claude/rules/kit-*.md`, plus everything under Execution guardrails. They hold
in every subtree and no subtree file can override them.

## Cross-cutting rules

Rules here apply repo-wide. Where a subtree or an ADR owns the details, point at
it instead of restating them — `→ ADR-0NN` refers to
[docs/adr.md](docs/adr.md).

- Change `src/ui/backend.ts` and the Tauri command handlers registered in
  `src-tauri/src/main.rs` together.
- Keep `fixtures/universe-v1.glife` and `fixtures/pattern-v1.toml` loading.
- Add a built-in pattern as one file in `src-tauri/patterns/`.

## Command index

| I want to... | See |
|---|---|
| Understand the system as a whole | [docs/architecture.md](docs/architecture.md) |
| Check what's already been decided | [docs/adr.md](docs/adr.md) |
| See what's planned and in what order | [docs/roadmap.md](docs/roadmap.md) |
| Run the app in development | `npm run tauri dev` ([package.json:10](package.json)) |
| Build the release binary | `npm run tauri build -- --no-bundle`; output `target/release/glife-app` |
| Run the release binary on the benchmark universe | `GLIFE_BENCH=1 target/release/glife-app` ([src-tauri/src/main.rs:38](src-tauri/src/main.rs)) |
| Time the engine on a 2048 × 2048 grid | `cargo test --release -p glife-engine -- --ignored --nocapture timing` ([src/engine/src/tests.rs:1051](src/engine/src/tests.rs)) |

## Execution guardrails

Ask before:

- `git push`.
- Touching anything matching the repo's secret-shaped gitignore patterns.

<!-- wscontext:begin -->
## wscontext

Past agent sessions in this repository are captured and linked to commits.
Before changing code you did not write, run `ctx why <file>:<line>` to see the
session that produced it.
Before starting on a topic, or when asked whether something was done or
discussed before, run `ctx find <words>`.
Before searching for where something is defined, run `ctx code <name>`.
Before changing a function that other code calls, run `ctx impact <name>`.
<!-- wscontext:end -->
