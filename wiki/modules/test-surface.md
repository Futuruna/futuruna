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
  - "[[canary-matrix]]"
---

# Test Surface

Futuruna’s quality strategy is increasingly centered on realistic test surfaces instead of isolated toy examples.

## Current Layers

- unit and regression tests embedded in Rust
- ordinary Futuruna test programs under `tests/`
- authored canary tiers under `tests/canary/`
- roundtrip and codegen parity checks
- differential and downstream-style hardening work

## Authored Canary Shape

- `core`: stable blocking workflows that combine subsystems users actually mix
- `stateful`: subjects, streams, actors, lifecycle, and effectful flows
- `extended`: runtime-heavy surfaces like JSON, regex, DB, HTTP, and WASM
- `regressions`: authored workflows distilled from real user bug classes

## Current Pressure

The biggest remaining pressure is downstream library-consumer behavior: nested imports, imported smoke leakage, ownership-heavy helper chains, and other shapes that are less visible in self-contained in-repo programs.

## Key Reference

- [[canary-matrix]]
