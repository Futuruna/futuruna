---
type: source
status: summarized
source_kind: repo-doc
source_path: "docs/mint-gate.md"
created: 2026-07-18
updated: 2026-07-18
tags:
  - source
  - docs
  - quality
  - verification
related:
  - "[[verification-lanes]]"
  - "[[mint-ratchet]]"
  - "[[repo-docs]]"
---

# Mint Gate

This source note summarizes the contract in `docs/mint-gate.md`.

## Canonical Command

`./scripts/mint.sh`

The important point is not just the script name. The document defines which regression-prone lanes are considered the minimum blocking health contract for Futuruna.

## What Mint Covers

- Rust unit and integration tests
- release build health
- interpreted Futuruna execution
- compiled Futuruna execution
- Rust codegen validation
- roundtrip parity
- high-value example programs that have already exposed compiler bugs

## What Mint Intentionally Does Not Cover

- full authored canary execution
- differential search
- optional dependency lanes
- standalone solver-heavy verification flows

Those belong in adjacent jobs, not the fastest blocking gate.

## Why It Matters

This note is the operational anchor for [[verification-lanes]] and the policy anchor for [[mint-ratchet]]. It says what "Futuruna is mint" means in practice instead of leaving that as team folklore.

