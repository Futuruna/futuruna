---
type: source
source_type: repo-doc
status: summarized
source_path: "docs/canary-suite.md"
created: 2026-07-18
updated: 2026-07-18
tags:
  - source
  - testing
  - canary
related:
  - "[[canary-matrix]]"
  - "[[test-surface]]"
  - "[[verification-lanes]]"
  - "[[mint-gate]]"
---

# Canary Suite

This source note summarizes `docs/canary-suite.md`.

The canary suite is the authored middle lane between the fast mint gate and
heavier differential search. It uses programs owned by this repository so the
suite stays realistic, curated, semantically broad, and stable enough to block
CI.

## Commands

- `./scripts/canary.sh` runs authored canary tiers.
- `./scripts/downstream-canary.sh` runs authored local library-consumer fixtures.
- `./scripts/expectations.sh` runs narrow compiler expectations.
- `./scripts/wasm-canary.sh` runs marked WASM build canaries.

## Tier Shape

- `tests/canary/core/` is the blocking authored language workflow lane.
- `tests/canary/stateful/` covers subjects, actors, lifecycle, and effects.
- `tests/canary/extended/` covers heavier JSON, DB, HTTP, WASM, regex, and
  import-heavy workflows.
- `tests/canary/regressions/` keeps broader user-bug classes alive as authored
  workflows.

Each selected tier runs format checking, compiled execution, codegen checking,
and roundtrip comparison. Some stateful cases can roundtrip-skip when the
generic runner excludes their runtime shape, but they still participate in the
other lanes.

## Boundaries

Canaries are for user-shaped workflows and distilled bug patterns broader than
one minimized compiler case. They are not for exact diagnostics, phase-output
assertions, giant downstream applications, or unclear external ports.

Those belong in [[expectation-suites]], `tests/downstream/`, or a tracked
follow-up.

## Downstream Lane

The authored downstream lane models Futuruna files being consumed as local
libraries. It focuses on nested imports, qualified imports, exported
type/value/function use, and direct `runa check` coverage. It also runs
`runa lint-library tests` to keep importable helper files from leaking script
behavior.

