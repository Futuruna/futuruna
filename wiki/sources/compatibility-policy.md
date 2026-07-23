---
type: source
status: summarized
source_kind: repo-doc
source_path: "docs/compatibility-policy.md"
created: 2026-07-18
updated: 2026-07-18
tags:
  - source
  - docs
  - compatibility
  - policy
related:
  - "[[compatibility-discipline]]"
  - "[[mint-ratchet]]"
  - "[[repo-docs]]"
---

# Compatibility Policy

This source note summarizes `docs/compatibility-policy.md`.

## What The Policy Adds

- named compatibility categories: source, behavioral, verification, and artifact/codegen
- feature stages: experimental, preview, stable
- a preferred deprecation path for stable changes
- explicit bug-fix exceptions for wrong-code, nondeterminism, unsoundness, and similar too-wrong-to-preserve behavior

## Important Constraint

The policy explicitly says Futuruna does **not** guarantee exact emitted Rust
text, helper names, or internal compiler layouts unless a doc promises
otherwise. It does treat emitted behavior as part of the stable language
contract.

## Best Companion Notes

- [[compatibility-discipline]]
- [[mint-ratchet]]
- [[state-and-roadmap]]

