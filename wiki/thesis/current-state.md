---
type: thesis
status: developing
created: 2026-07-18
updated: 2026-07-18
tags:
  - thesis
  - state
related:
  - "[[overview]]"
  - "[[verification-lanes]]"
  - "[[wiki/sources/state-and-roadmap|state-and-roadmap]]"
---

# Current State

Futuruna has a real assurance stack:

- a blocking mint gate
- authored canary tiers
- differential and replayable bug-finding lanes
- compiler-internal snapshot validation
- a contributor ratchet for semantic changes

## Honest Position

- Downstream-user failures still matter because they expose usage shapes the in-repo surface may not yet represent.
- The compiler is tested, not formally verified; there is no proof kernel.

## What Is Trusted Today

### Conventional trusted compiler/runtime

- parser
- type checker
- interpreter
- Rust codegen
- build and emitted-Rust integration

## Current Strategic Threads

- keep Futuruna mint through explicit lanes instead of intuition
- expand authored workflow coverage toward downstream-user shapes
- burn down semantic contract gaps before they become issue churn

## Near-Term Direction

1. close remaining semantic contract gaps in compiled/runtime behavior
2. broaden realistic authored coverage and compiler visibility

## Primary Sources

- [[wiki/sources/state-and-roadmap|state-and-roadmap]]
- [[wiki/sources/mint-gate|mint-gate]]
- [[wiki/sources/canary-matrix|canary-matrix]]
