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
  - "[[verified-bootstrap]]"
  - "[[repo-docs]]"
---

# State And Roadmap

This source note summarizes the contributor-facing map in `docs/state-and-roadmap.md`.

## What The Doc Establishes

- Futuruna now has a real assurance stack instead of relying on local confidence.
- The proof story is real, but still stage-1: a trusted kernel plus a wider trusted elaboration pipeline.
- The next work is not feature sprawl. It is semantic closure, broader realistic coverage, and a narrower proof trust boundary.

## Assurance Stack In One View

- [[verification-lanes]] for the blocking mint gate, authored canaries, differential search, and FIR snapshots.
- [[mint-ratchet]] for the contributor discipline around semantic changes.
- [[proof-kernel]] for the small trusted checker.
- [[verified-bootstrap]] for the path from tiny proved compiler fragments to larger proof-carrying slices.

## Trust Boundary Summary

The document is explicit that most of Futuruna is still conventional trusted compiler/runtime code:

- parser
- type checker
- interpreter
- Rust codegen
- emitted-Rust integration

The narrow trusted proof core is `src/proof_kernel.rs` plus the primitive kernel axioms. The proof-elaboration machinery around `runa verify` is still trusted compiler code.

## Milestones It Sets

1. Close remaining semantic contract gaps in compiled/runtime behavior.
2. Expand realistic authored coverage and internal compiler visibility.
3. Shrink the proof trust boundary around real compiler slices.

## Best Companion Notes

- [[current-state]]
- [[verification-lanes]]
- [[proof-kernel]]
- [[verified-bootstrap]]

