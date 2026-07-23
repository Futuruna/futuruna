---
type: source
status: summarized
source_kind: repo-doc
source_path: "docs/proof-kernel.md"
created: 2026-07-18
updated: 2026-07-18
tags:
  - source
  - docs
  - proof
  - kernel
related:
  - "[[proof-kernel]]"
  - "[[verified-bootstrap]]"
  - "[[repo-docs]]"
---

# Proof Kernel Spec

This source note summarizes the v1 design contract in `docs/proof-kernel.md`.

## Core Claim

The proof kernel is meant to be the small auditable trust anchor for explicit proof terms. The design spec treats it like cryptographic code: closed, pure, conservative, and reviewable in one sitting.

## Design Principles

- Small enough to stay auditable.
- Closed over compiler state: no I/O, no globals, no solver calls.
- Decidable and conservative: reject when unsure, let automation live outside the kernel.
- Trust is explicit: primitive axioms are named and recognized by the kernel rather than extracted from source files.

## Scope Of The Kernel

The spec gives the proposition fragment and proof-term grammar for v1, including:

- equality and integer order propositions
- conjunction, implication, negation, and `False`
- proof forms like `refl`, `apply`, `rewrite`, `cases`, `induction_on`, `contra`, `assume`, and hypothesis lookup

Out of scope in v1 are existentials, disjunction, higher-order predicates, floats, and anything that would force a substantially larger trusted core.

## Important Trust Detail

The kernel does not unfold arbitrary function bodies. Instead, the compiler can generate trusted computation lemmas for simple total `match`-based functions. That keeps the kernel small, but it means the computation-lemma pass is outside the tiny trusted core.

## Why This Matters To The Wiki

- [[proof-kernel]] is the conceptual summary.
- [[verified-bootstrap]] depends on the kernel being small and honest.
- [[current-state]] should never overclaim beyond this trust boundary.

