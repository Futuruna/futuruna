---
type: thesis
status: developing
created: 2026-07-18
updated: 2026-07-18
tags:
  - thesis
  - proof
  - bootstrap
related:
  - "[[current-state]]"
  - "[[proof-kernel]]"
  - "[[verified-bootstrap-doc]]"
  - "[[proof-backed-checking]]"
---

# Verified Bootstrap

Futuruna’s verified-bootstrap story is real, but staged.

## What Exists Now

The language can already express and check tiny semantics-preservation proofs inside ordinary `.runa` files. The canonical fixture proves toy lowerings preserve evaluation, including a recursive `let` model over an explicit environment.

That gives Futuruna a genuine proof-carrying compiler fragment.

## What Is Still Trusted

- theorem construction around `runa verify`
- computation-lemma generation
- ADT constructor metadata seeding
- the rest of the production compiler and runtime

So the current state is not “compiler verified.” It is “small trusted checker plus trusted elaboration, with real proved slices inside that boundary.”

## Why This Matters

The bootstrap track is how Futuruna avoids leaving the proof kernel as an isolated research toy. The goal is to push proof-backed checking into real compiler logic gradually, where it replaces ordinary trust with checked transformations or translation checks.

## Near-Term Direction

1. keep proving small source-to-target preservation results in Futuruna
2. grow the modeled core so more realistic passes fit
3. narrow the elaboration trust boundary over time

The next concrete compiler slice is translation-checking generated computation
lemmas against their source function arms. That targets the current trusted
proof-elaboration boundary directly without broadening Futuruna into a general
theorem prover.

## Primary Sources

- [[verified-bootstrap-doc]]
- [[proof-backed-checking]]
- [[proof-kernel-spec]]
- [[state-and-roadmap]]
