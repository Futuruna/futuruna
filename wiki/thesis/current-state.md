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
  - "[[proof-kernel]]"
  - "[[verified-bootstrap]]"
  - "[[state-and-roadmap]]"
---

# Current State

Futuruna is no longer in the “add features and hope” phase. It now has a real assurance stack:

- a blocking mint gate
- authored canary tiers
- differential and replayable bug-finding lanes
- compiler-internal snapshot validation
- a contributor ratchet for semantic changes
- a proof kernel that can already justify tiny compiler slices

## Honest Position

- The language and compiler are materially stronger and more disciplined than before.
- Downstream-user failures still matter because they expose usage shapes the in-repo surface may not yet represent.
- The proof story is real, but it is still a staged trust story, not full production-compiler verification.

## What Is Trusted Today

### Conventional trusted compiler/runtime

- parser
- type checker
- interpreter
- Rust codegen
- build and emitted-Rust integration

### Small trusted proof core

- [[proof-kernel]]
- primitive kernel axioms

### Still-trusted proof elaboration

- theorem construction for `runa verify`
- computation-lemma generation
- constructor metadata seeding
- local-lemma registration and proof plumbing

## Current Strategic Threads

- keep Futuruna mint through explicit lanes instead of intuition
- expand authored workflow coverage toward downstream-user shapes
- burn down semantic contract gaps before they become issue churn
- shrink the proof trust boundary deliberately through [[verified-bootstrap]]

## Near-Term Direction

1. close remaining semantic contract gaps in compiled/runtime behavior
2. broaden realistic authored coverage and compiler visibility
3. move from tiny proved fragments toward proof-backed handling of real compiler slices

## Primary Sources

- [[state-and-roadmap]]
- [[mint-gate]]
- [[canary-matrix]]
- [[verified-bootstrap-doc]]
- [[proof-kernel-spec]]
