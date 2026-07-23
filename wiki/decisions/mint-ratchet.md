---
type: decision
status: active
date: 2026-07-18
owner: "Futuruna"
context: "The project needed to move from ad hoc confidence to enforceable semantic discipline."
tags:
  - decision
  - quality
created: 2026-07-18
updated: 2026-07-18
related:
  - "[[verification-lanes]]"
  - "[[current-state]]"
  - "[[mint-gate]]"
  - "[[canary-matrix]]"
---

# Mint Ratchet

## Decision

Futuruna should treat semantic quality as a ratchet:

- every real bug should become permanent coverage
- semantic changes should be paired with the right lane, not just local confidence
- authored canaries should model user workflows, not just features in isolation

## Operational Meaning

- [[mint-gate]] defines the smallest blocking health claim.
- [[canary-matrix]] grows the realistic workflow surface over time.
- contributor review should ask which lane proves the change is safe, not merely whether it “seems fine”.

## Consequences

- contributor discipline matters as much as feature work
- verification surfaces must expand when external users hit new failure shapes
- the repo should accumulate confidence, not just code
