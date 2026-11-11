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
| Test command | TODO(kit): the command that runs every test suite, once the stack is chosen |
| Subtrees | none yet |

## Non-negotiable rules

The bullets tagged **(non-negotiable)** under Cross-cutting rules below and in
`.claude/rules/kit-*.md`, plus everything under Execution guardrails. They hold
in every subtree and no subtree file can override them.

## Cross-cutting rules

Rules here apply repo-wide. Where a subtree or an ADR owns the details, point at
it instead of restating them — `→ ADR-0NN` refers to
[docs/adr.md](docs/adr.md).

- TODO(kit): the repo-wide rules. Tag the ones no subtree may override
  *(non-negotiable)*.

## Command index

| I want to... | See |
|---|---|
| Understand the system as a whole | [docs/architecture.md](docs/architecture.md) |
| Check what's already been decided | [docs/adr.md](docs/adr.md) |
| See what's planned and in what order | [docs/roadmap.md](docs/roadmap.md) |
| TODO(kit): one row per task | the README that owns it |

## Execution guardrails

Ask before:

- TODO(kit): every command that touches shared or real infrastructure —
  deploys, `docker push`, `terraform apply`, migrations against a live
  database, `git push`.
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
