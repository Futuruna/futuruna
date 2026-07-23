---
type: flow
status: active
created: 2026-07-18
updated: 2026-07-18
tags:
  - flow
  - verification
related:
  - "[[mint-ratchet]]"
  - "[[test-surface]]"
  - "[[mint-gate]]"
  - "[[canary-matrix]]"
---

# Verification Lanes

Futuruna’s verification stack is layered on purpose. The lanes are meant to complement each other, not duplicate each other.

## Fast Blocking Gate

[[mint-gate]] defines the minimum blocking contract:

- Rust tests
- release build
- interpreted execution
- compiled execution
- codegen validation
- roundtrip parity
- selected real example checks

## Authored Realistic Workflows

[[canary-matrix]] tracks the curated authored suite:

- `core` for blocking language workflows
- `stateful` for subjects, actors, lifecycle, and effects
- `extended` for JSON, regex, DB, HTTP, WASM, and import-heavy behavior
- `regressions` for user-found bug classes promoted into realistic workflows

## Deep Search And Internal Visibility

- differential testing hunts unknown semantic bugs and preserves replayable seeds
- FIR snapshots make internal compiler drift visible instead of silent
- focused Rust regressions keep every discovered compiler bug permanent

## Reading External Failures

When a downstream user finds a bug, ask in this order:

1. did the mint gate miss a core contract it should have caught?
2. did the canary surface miss the usage shape entirely?
3. do we need a deeper regression or snapshot for a compiler-internal invariant?
4. is the failure revealing heuristic codegen rather than a declared language contract?

## Policy Link

[[mint-ratchet]] is the human side of these lanes: every semantic change should land with the right lane, not just with local confidence.
