# Futuruna State and Roadmap

This document is the short high-level map for contributors.

It answers three questions:

1. Where Futuruna stands now.
2. What is trusted today.
3. What the next three milestones are.

For the detailed contracts behind each lane, see:

- [docs/mint-gate.md](mint-gate.md)
- [docs/production-readiness-scorecard.md](production-readiness-scorecard.md)
- [docs/compatibility-policy.md](compatibility-policy.md)
- [docs/feature-stages.md](feature-stages.md)
- [docs/compatibility-guides/](compatibility-guides/README.md)
- [docs/canary-suite.md](canary-suite.md)
- [docs/canary-matrix.md](canary-matrix.md)
- [docs/expectation-suites.md](expectation-suites.md)
- [docs/downstream-test-surface-audit.md](downstream-test-surface-audit.md)
- [docs/differential-testing.md](differential-testing.md)
- [docs/verified-bootstrap.md](verified-bootstrap.md)
- [docs/proof-kernel.md](proof-kernel.md)
- [docs/proof-backed-checking.md](proof-backed-checking.md)
- [CONTRIBUTING.md](../CONTRIBUTING.md)

## Current State

Futuruna is no longer in the "add features and hope" phase.

The project now has a real assurance stack:

- A blocking mint gate in [scripts/mint.sh](../scripts/mint.sh) that checks interpreted execution, compiled execution, Rust codegen validation, roundtrip parity, and real example programs.
- A compiletest-style expectation lane for narrow diagnostics, run/fail
  behavior, and phase-specific compiler markers.
- An authored canary suite for realistic user-shaped workflows, with the current coverage tracked in [docs/canary-matrix.md](canary-matrix.md).
- A differential lane for generative and replayable semantic bug finding.
- FIR phase validation snapshots that make compiler-structure drift visible instead of silent.
- A contributor ratchet in [CONTRIBUTING.md](../CONTRIBUTING.md) that requires semantic changes to land with permanent coverage and documented follow-up tasks.
- A published compatibility policy in [docs/compatibility-policy.md](compatibility-policy.md) that names source, behavioral, verification, and artifact-facing change categories and defines feature stages.
- A surfaced stage matrix in [docs/feature-stages.md](feature-stages.md) so users and contributors can see which major surfaces are stable, preview, or experimental.
- A versioned compatibility-guide discipline in [docs/compatibility-guides/](compatibility-guides/README.md) so stable changes and bug-fix exceptions become release-facing history instead of only PR-local context.

On the language side, the recent focus has been semantic parity and determinism:

- interpreter vs compiled behavior
- codegen vs declared language contract
- top-level/module visibility
- collection ordering and deterministic semantics
- proof workflows inside ordinary programs
- runtime error behavior for partial builtins such as indexing and empty-list access
- explicit scope-owned lifetime rules for live stream subscriptions, instead of
  detached function-local async work

On the proof side, Futuruna has a real kernel-backed verification story, but only in stage 1 form:

- explicit proof terms are checked by the audited kernel in `src/proof_kernel.rs`
- `cases`, `induction_on`, `apply`, and `rewrite` are real kernel features
- Futuruna can already host tiny semantics-preservation proofs for toy compiler slices

That is enough to say "native proof-carrying compiler fragments exist."

It is not enough to say "the Futuruna compiler is verified."

## What Is Trusted Today

The trust boundary is deliberately wider than the proof kernel.

### Trusted conventional compiler/runtime code

These are still ordinary Rust implementation, defended by tests and gates rather than formal proof:

- parsing
- type checking
- interpreter execution
- Rust code generation
- build/test runners and emitted Rust integration

This is why the mint, canary, differential, and snapshot lanes matter so much. They are how we keep the non-proved majority of the system professional.

### Trusted proof core

The audited trusted proof core is:

- `src/proof_kernel.rs`
- the primitive hard-coded axiom table recognized by the kernel

That core checks proof terms. Its current trusted boundary is documented in
[proof-kernel.md](proof-kernel.md): syntax datatypes, `Ctx` metadata,
primitive axioms, unification/rewrite/synthesis, and the `check`/`apply`/
`cases`/`induction` judgment code are inside; tests and compiler elaboration are
outside. It does not inspect arbitrary Futuruna code and infer truth by itself.

### Trusted proof-elaboration pipeline

This is still trusted compiler machinery:

- proof parsing and invariant elaboration
- theorem construction for `runa verify`
- computation-lemma generation
- ADT constructor metadata seeding
- local lemma registration for explicit proofs

If this surrounding pipeline asks the kernel to prove the wrong theorem, the kernel can still succeed on the wrong theorem. That is the main current limit of the proof story.

### Outside the small trusted proof story

- Z3 fallback is useful automation, not part of the closed kernel trust boundary.
- proved user lemmas are not primitive trust if they are actually checked by the kernel.
- the rest of the compiler remains conventional code until modeled, translation-checked, or proved separately.

## How the Assurance Stack Fits Together

The lanes are meant to complement each other, not compete:

- `./scripts/mint.sh`
  Fast blocking contract for "is Futuruna mint right now?"
- `./scripts/canary.sh`
  Authored realistic workflows that combine language subsystems the way users do.
- `./scripts/expectations.sh`
  Narrow compiler expectations for diagnostics, command pass/fail behavior, and
  phase-specific markers.
- `./scripts/differential.sh`
  Seed-stable search for edge cases and unknown semantic bugs.
- FIR snapshots and focused regressions
  Guard compiler-internal invariants and keep every discovered bug permanent.
- Verified bootstrap work
  Shrinks the trusted boundary for selected compiler logic instead of relying only on tests.

The professional move is hybrid assurance:

- prove what is worth proving
- ratchet everything else with tests, canaries, differential checks, and review discipline

## What Futuruna Is Working Toward

The long-term goal is not just "more features."

It is:

1. a language whose semantics stay stable under change
2. a compiler whose risky passes are increasingly checked rather than merely trusted
3. a project that another contributor can pick up without reintroducing old bugs

In short: keep Futuruna mint, then make more of it provable.

## Next Three Milestones

### 1. Close the remaining semantic contract gaps

Current open gaps are mostly precise, known compiler/runtime edges rather than vague instability.

Examples:

- `td-88706d`: context-free empty-list expressions stay under-inferred in compiled mode
- `td-a1b98e`: add runtime-error fixtures to the runner so error contracts are tested directly
- `td-3fcc3e` and `td-00f228`: recursive ADT lowering gaps
- `td-4b1694`: list literal ownership can still consume reused strings

Success looks like:

- fewer raw Rust inference failures leaking through codegen
- partial builtins behaving by explicit Futuruna contract
- remaining compiler bugs increasingly concentrated in tracked, narrow edge cases

### 2. Expand realistic authored coverage and internal compiler visibility

The next assurance growth is not another giant feature wave. It is broader coverage of real workflows and more compiler-internal validation.

Key tracked work:

- `td-90448d`: expand the stateful canary wave
- `td-9fdd94`: expand the extended canary wave
- `td-48e5d9`: broaden FIR snapshot coverage to richer cross-binding and module samples
- `td-9e6db1`: reduce machine-lane noise so failures stay high-signal

Success looks like:

- more real user patterns represented in-repo
- better visibility when internal compiler phases drift
- fewer "Claude found this in another repo" surprises

### 3. Shrink the proof trust boundary around real compiler slices

This is the serious formal-methods milestone.

Key tracked work:

- `td-84cd25`: build a proof-carrying Futuruna bootstrap
- `td-165c01`: plan proof-backed or translation-checked handling for higher-risk compiler passes

The practical direction is:

1. keep proving small semantics-preserving compiler models in Futuruna
2. replace trusted compiler-side transformations with proof-producing or translation-checking variants where the payoff is highest
3. narrow the compiler's ability to silently invent the theorem that the kernel checks

The next proof-backed compiler slice is not whole-codegen proof. It is
translation-checking generated computation lemmas against the source functions
they claim to describe, as detailed in
[docs/proof-backed-checking.md](proof-backed-checking.md). That directly shrinks
the proof-elaboration trust boundary without broadening Futuruna into a general
theorem prover.

Success looks like:

- real compiler passes, not just toy fragments, acquiring proof-backed justification
- a smaller trusted elaboration boundary
- a credible path from "kernel-backed proofs exist" to "parts of Futuruna are proved in Futuruna"

## Bottom Line

Futuruna is now in a much better place than a few weeks ago:

- the project has a real mint contract
- it has curated canaries and a differential lane
- it has a contributor ratchet
- it has a real proof kernel
- it already proves tiny compiler slices natively

The next step is not to relax because of that.

The next step is to keep tightening the semantic surface, expand realistic coverage, and gradually convert the highest-risk compiler logic from "trusted Rust" into "checked Futuruna."
