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
  - "[[verified-bootstrap]]"
  - "[[proof-kernel-spec]]"
---

# Proof Kernel

The proof kernel is the small trusted checker for explicit proof terms. It is the narrowest part of Futuruna’s formal trust story and the thing that has to stay boring, small, and auditable.

## What It Checks

- explicit proof terms against proposition goals
- proof forms such as `refl`, `apply`, `rewrite`, `cases`, `induction_on`, `contra`, and hypothesis use
- primitive equality, order, arithmetic, and propositional reasoning through a small hard-coded axiom table

## Design Constraints

- small enough to review in one sitting
- no I/O or ambient compiler state
- conservative rejection instead of “trying harder”
- automation lives outside the kernel, not inside it

## What Sits Outside The Kernel

- proof parsing and invariant elaboration
- computation-lemma generation for simple `>` functions
- constructor metadata seeding for `cases` and `induction_on`
- theorem construction around `runa verify`

That surrounding machinery is still trusted compiler code today. The kernel can only check the theorem it is asked to check.

## What It Does Not Yet Mean

It does not mean the production compiler is proved. It means explicit proof terms are checked by a real small core instead of being accepted on faith.

## Where It Leads

The kernel matters because it enables [[verified-bootstrap]]: small semantics-preserving compiler fragments proved in Futuruna itself. The long-term goal is not just “have a proof kernel,” but “use it to shrink trusted compiler logic where it pays off.”

## Main References

- [[current-state]]
- [[repo-docs]]
- [[proof-kernel-spec]]
- [[verified-bootstrap]]
