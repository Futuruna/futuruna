---
type: source
status: summarized
source_kind: repo-doc
source_path: "docs/state-and-roadmap.md"
created: 2026-07-18
updated: 2026-07-18
tags:
  - source
  - docs
  - roadmap
related:
  - "[[current-state]]"
  - "[[verification-lanes]]"
  - "[[repo-docs]]"
---

# State And Roadmap

This source note summarizes the contributor-facing map in `docs/state-and-roadmap.md`.

## What The Doc Establishes

- Futuruna has a real assurance stack instead of relying on local confidence.
- Assurance comes from tests, canaries and differential lanes; there is no proof kernel.
- The next work is not feature sprawl. It is semantic closure and broader realistic coverage.

## Assurance Stack In One View

- [[verification-lanes]] for the blocking mint gate, authored canaries, differential search, and FIR snapshots.
- [[mint-ratchet]] for the contributor discipline around semantic changes.

## Trust Boundary Summary

All of Futuruna is conventional trusted compiler/runtime code:

- parser
- type checker
- interpreter
- Rust codegen
- emitted-Rust integration

## Milestones It Sets

1. Close remaining semantic contract gaps in compiled/runtime behavior.
2. Expand realistic authored coverage and internal compiler visibility.

## Best Companion Notes

- [[current-state]]
- [[verification-lanes]]

