---
type: source
status: summarized
source_kind: repo-doc
source_path: "docs/canary-matrix.md"
created: 2026-07-18
updated: 2026-07-18
tags:
  - source
  - docs
  - canary
  - testing
related:
  - "[[test-surface]]"
  - "[[verification-lanes]]"
  - "[[repo-docs]]"
  - "[[canary-suite]]"
---

# Canary Matrix

This source note summarizes the authored coverage map in `docs/canary-matrix.md`.

## Suite Shape

The canary suite is meant to cover realistic user-shaped workflows rather than isolated feature demos.

Current tiers:

- `core`
- `stateful`
- `extended`
- `regressions`

## What The Matrix Gives The Wiki

- a concrete map of which usage shapes are already represented in-repo
- a place to see where coverage is growing next
- a link between specific canaries and the broader semantic hardening effort

## Current Coverage Themes

- collection pipelines and deterministic ordering
- recursive ADTs and top-level composition
- subjects, streams, actors, effects, and lifecycle behavior
- JSON, regex, DB, and other extended runtime surfaces
- user-bug classes promoted into realistic authored workflows

## Immediate Reading Path

- [[canary-suite]]
- [[test-surface]]
- [[verification-lanes]]
- [[mint-ratchet]]
