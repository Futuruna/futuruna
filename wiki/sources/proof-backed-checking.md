---
type: source
status: summarized
source_kind: repo-doc
source_path: "docs/proof-backed-checking.md"
created: 2026-07-18
updated: 2026-07-18
tags:
  - source
  - docs
  - proof
  - compiler
related:
  - "[[verified-bootstrap]]"
  - "[[proof-kernel]]"
  - "[[state-and-roadmap]]"
  - "[[alive2-translation-validation]]"
---

# Proof-Backed Checking

This source note summarizes `docs/proof-backed-checking.md`.

## Core Claim

Futuruna should use proof machinery where it shrinks real compiler trust, not as
a route to becoming a general theorem prover.

The assurance model is hybrid:

- small proof-backed compiler slices
- translation validation where checking is cheaper than proving
- mint, canaries, differential testing, and snapshots for the rest

## First Target

The first proof-adjacent compiler slice should be computation-lemma generation.

Reason: computation lemmas are currently trusted compiler-generated equations
handed to the proof kernel. If they misrepresent source functions, the kernel
can prove the wrong theorem correctly.

Generated computation lemmas now pass through a checked collection path before
they enter the explicit-proof registry. The checker rejects ghost lemmas,
missing source-backed lemmas, and schemas that differ from the source-derived
arm schema.

## Ranked Surfaces

1. proof elaboration and computation-lemma generation
2. import normalization and library boundary preservation
3. pure expression/FIR lowering
4. ownership and borrow-sensitive Rust emission
5. stateful streams and async scope ownership

The last two remain test/canary surfaces until smaller proof-backed slices are
under control.

## Companion Notes

- [[verified-bootstrap]]
- [[proof-kernel]]
- [[state-and-roadmap]]
- [[alive2-translation-validation]]
