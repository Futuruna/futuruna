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
---

# Mint Ratchet

## Decision

Futuruna should treat semantic quality as a ratchet:

- every real bug should become permanent coverage
- semantic changes should be paired with the right lane, not just local confidence
- authored canaries should model user workflows, not just features in isolation

## Consequences

- contributor discipline matters as much as feature work
- verification surfaces must expand when external users hit new failure shapes
- the repo should accumulate confidence, not just code

