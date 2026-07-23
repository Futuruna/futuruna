---
type: module
path: "tests/"
status: active
language: futuruna
purpose: "Authored canaries, regressions, property tests, and user-facing language coverage."
maintainer: "Futuruna"
last_updated: 2026-07-18
tags:
  - module
  - testing
created: 2026-07-18
updated: 2026-07-18
related:
  - "[[verification-lanes]]"
  - "[[mint-ratchet]]"
---

# Test Surface

Futuruna’s quality strategy is increasingly centered on realistic test surfaces instead of isolated toy examples.

## Current Layers

- unit and regression tests embedded in Rust
- ordinary Futuruna test programs under `tests/`
- authored canary tiers under `tests/canary/`
- roundtrip and codegen parity checks
- differential and downstream-style hardening work

## Current Pressure

The next important expansion is better downstream library-consumer coverage, because that is where several recent external failures surfaced.

