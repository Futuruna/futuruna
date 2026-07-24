---
type: module
path: "src/bin/runa.rs"
status: active
language: rust
purpose: "Main compiler, transpiler, runtime emission, and verification entrypoint."
maintainer: "Futuruna"
last_updated: 2026-07-18
tags:
  - module
  - compiler
created: 2026-07-18
updated: 2026-07-18
related:
  - "[[verification-lanes]]"
  - "[[test-surface]]"
  - "[[milestone-docs]]"
---

# Compiler Pipeline

The main compiler flow is concentrated in `src/bin/runa.rs`, which currently carries parsing, scanning, lowering, codegen, runtime helpers, verification entrypoints, and a large embedded regression suite.

## Responsibilities

- transpile Futuruna to Rust
- drive interpreter and compiled execution paths
- emit runtime support code
- integrate proof checking and SMT fallback behavior
- host focused compiler regression tests

## Current Risk

> [!warning] Concentration Risk
> A large amount of compiler behavior still lives in one Rust file. This makes cross-cutting fixes fast, but increases the risk of heuristic drift and missed interactions.

## Refactoring History

[[milestone-docs]] records the key compiler-structure milestones:

- M29 introduced FIR, `TypeRegistry`, and `OwnershipAnalysis`.
- M30 split declaration/import scanning, borrow-flag computation, and emission
  into explicit passes.
- M41 made codegen parity a blocking gate through `--check-codegen`.

## Relevant Project Docs

- [[current-state]]
- [[verification-lanes]]
- [[repo-docs]]
- [[milestone-docs]]
