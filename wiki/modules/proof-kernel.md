---
type: module
path: "src/proof_kernel.rs"
status: active
language: rust
purpose: "Trusted explicit proof checker and kernel for Futuruna proof terms."
maintainer: "Futuruna"
last_updated: 2026-07-18
tags:
  - module
  - proof
created: 2026-07-18
updated: 2026-07-18
related:
  - "[[current-state]]"
  - "[[repo-docs]]"
---

# Proof Kernel

The proof kernel is the trusted checker for explicit proof terms. It is real and already supports meaningful proof workflows, including bootstrap-style proofs over small compiler models.

## What It Does Today

- checks explicit proof terms
- supports core proof forms such as `apply`, `rewrite`, `cases`, and `induction_on`
- backs proof-oriented tests and verified bootstrap fixtures

## What It Does Not Yet Mean

It does not mean the full production compiler is proven. The surrounding elaboration and theorem-construction path is still a trust boundary.

## Main References

- [[current-state]]
- [[repo-docs]]

