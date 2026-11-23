<!-- kit:core/docs/build-plan v2 -->
# <Area> stage N — build plan

Builds [<area>-phase-N.md](<area>-phase-N.md). Run with `/kit:run-build-plan`.

## Context

## Decisions taken

## Step routing at a glance

| Step | Agent | Model | Gate | Parallel | Status |
|---|---|---|---|---|---|
| WP1 | `<agent>` | sonnet | test | no | Not started |
| WPn | `kit:docs-writer` | sonnet | human | no | Not started |
| Verification | session | — | — | — | Not started |

Status is updated in the commit that finishes each step. A step's own commit
can't name its hash, so it records `Done`, and the next step's commit adds the
hash.

## WP1 — <title>
> `agent: <agent>` · `model: sonnet` · `gate: test` · `reads: §4.1, §5`

### `<file or module>`

What changes, at file level.

### Tests

## WPn — Docs and rules
> `agent: kit:docs-writer` · `gate: human`

## Verification

## Deliberately out of scope
