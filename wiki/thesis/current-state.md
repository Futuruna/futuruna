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
---

# Current State

Futuruna is no longer operating as “write code and hope”. It now has a real quality spine:

- a mint gate
- authored canary layers
- differential and regression work
- a proof kernel that can already prove meaningful bootstrap slices

## Honest Position

- the language and compiler are materially stronger than before
- downstream-user failures still surface gaps in test shape and codegen invariants
- the proof story is real but still staged, not total compiler verification

## Near-Term Direction

- close remaining semantic contract gaps
- harden downstream library-consumer coverage
- keep shrinking the proof trust boundary with deliberate slices

