---
type: source
status: summarized
source_kind: repo-doc
source_path: "docs/verified-bootstrap.md"
created: 2026-07-18
updated: 2026-07-18
tags:
  - source
  - docs
  - proof
  - bootstrap
related:
  - "[[verified-bootstrap]]"
  - "[[proof-kernel]]"
  - "[[current-state]]"
  - "[[repo-docs]]"
---

# Verified Bootstrap Doc

This source note summarizes the honest bootstrap claim in `docs/verified-bootstrap.md`.

## Current Claim

Futuruna can already host tiny semantics-preservation proofs in `.runa` files. The canonical example is `tests/verified_bootstrap_test.runa`, which proves toy lowering passes preserve evaluation, including the stronger stage-2 `let` model.

That is enough to say Futuruna has native proof-carrying compiler fragments.

It is not enough to say the production compiler is verified.

## Trusted Boundary Today

- Small trusted core: [[proof-kernel]]
- Still-trusted elaboration: proof parsing, theorem construction, computation-lemma generation, constructor metadata seeding, local-lemma registration
- Not part of the closed kernel story: Z3 fallback and the rest of the compiler

## Long-Term Shape

The bootstrap plan is staged:

1. model source and target semantics
2. define a compiler pass
3. prove preservation in Futuruna
4. replace trusted compiler-side transformations with proof-producing or translation-checking variants where it pays off

## Best Companion Notes

- [[verified-bootstrap]]
- [[proof-kernel]]
- [[state-and-roadmap]]

