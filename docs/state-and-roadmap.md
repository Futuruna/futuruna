# Futuruna State and Roadmap

This document is the short high-level map for contributors.

It answers three questions:

1. Where Futuruna stands now.
2. What is trusted today.
3. What the next three milestones are.

For the detailed contracts behind each lane, see:

- [docs/mint-gate.md](mint-gate.md)
- [docs/production-readiness-scorecard.md](production-readiness-scorecard.md)
- [docs/new-user-stability-packet.md](new-user-stability-packet.md)
- [docs/compatibility-policy.md](compatibility-policy.md)
- [docs/feature-stages.md](feature-stages.md)
- [docs/canary-suite.md](canary-suite.md)
- [docs/canary-matrix.md](canary-matrix.md)
- [docs/expectation-suites.md](expectation-suites.md)
- [docs/library-hygiene.md](library-hygiene.md)
- [docs/differential-testing.md](differential-testing.md)
- [CONTRIBUTING.md](../CONTRIBUTING.md)

## Current State

The project has a real assurance stack:

- A blocking mint gate in [scripts/mint.sh](../scripts/mint.sh) that checks interpreted execution, compiled execution, Rust codegen validation, roundtrip parity, and real example programs.
- A compiletest-style expectation lane for narrow diagnostics, run/fail
  behavior, and phase-specific compiler markers.
- An authored canary suite for realistic user-shaped workflows, with the current coverage tracked in [docs/canary-matrix.md](canary-matrix.md).
- A blocking downstream consumer lane for importable local libraries, import
  hygiene, and library-shaped user workflows.
- A stable differential lane for replayable semantic bugs, seed-stable
  generative search, generated import-aware pressure, and failure artifacts.
- A named stable FRSS-v0 contract for `runa from-rust`: deterministic
  single-file Rust programs are validated by exact stdout matching across the
  example corpus, a clean-directory downstream canary, and a generated
  supported-subset differential lane. `from-rust --verify` has stable summary
  lines for supported matches and recognized failures, while arbitrary crate
  translation remains outside the claim.
- FIR phase validation snapshots that make compiler-structure drift visible instead of silent.
- A contributor ratchet in [CONTRIBUTING.md](../CONTRIBUTING.md) that requires semantic changes to land with permanent coverage and documented follow-up tasks.
- A published compatibility policy in [docs/compatibility-policy.md](compatibility-policy.md) that names source, behavioral, verification, and artifact-facing change categories and defines feature stages.
- A surfaced stage matrix in [docs/feature-stages.md](feature-stages.md) so users and contributors can see which major surfaces are stable, preview, or experimental.
- A new-user stability packet in
  [docs/new-user-stability-packet.md](new-user-stability-packet.md) that
  states production claims, conjectures, trust boundaries, first-hour
  diagnostics, fail-closed unsupported paths, and the formal-strengthening
  ranking in one place.

On the language side, the recent focus has been semantic parity and determinism:

- interpreter vs compiled behavior
- codegen vs declared language contract
- top-level/module visibility
- collection ordering and deterministic semantics
- invariant checks inside ordinary programs
- runtime error behavior for partial builtins such as indexing and empty-list access
- explicit scope-owned lifetime rules for live stream subscriptions, instead of
  detached function-local async work
- importable local library contracts with flat/qualified imports, exported
  values/types/functions, import-hygiene linting, and downstream consumer
  canaries

On the verification side, `runa verify` (Preview) checks `|` invariants with an
SMT backend (Z3). PROVED means that for every value of the invariant's free variables (Int ranges over the 64-bit values) the predicate is true and no Int operation it evaluates overflows or divides by zero. Claims involving Float are reported as unsupported, never proved. The command exits non-zero unless every
invariant is PROVED.

## What Is Trusted Today

### Trusted conventional compiler/runtime code

These are still ordinary Rust implementation, defended by tests and gates rather than formal proof:

- parsing
- type checking
- interpreter execution
- Rust code generation
- build/test runners and emitted Rust integration

This is why the mint, canary, differential, and snapshot lanes matter so much. They are how we keep the non-proved majority of the system professional.

### Trusted verification pipeline

`runa verify` trusts:

- the translation of each invariant into an SMT query
- the Z3 solver

If the translation asks the solver the wrong question, a PROVED result does not
describe the running program. Runtime `?` checks and the test lanes remain the
evidence for behavior that `runa verify` does not cover.

## How the Assurance Stack Fits Together

The lanes are meant to complement each other, not compete:

- `./scripts/mint.sh`
  Fast blocking contract for "is Futuruna mint right now?"
- `./scripts/canary.sh`
  Authored realistic workflows that combine language subsystems the way users do.
- `./scripts/downstream-canary.sh`
  Authored library-consumer workflows that keep stable local import and
  import-hygiene behavior production-ready.
- `./scripts/expectations.sh`
  Narrow compiler expectations for diagnostics, command pass/fail behavior, and
  phase-specific markers.
- `./scripts/differential.sh`
  Seed-stable search for edge cases and unknown semantic bugs, plus generated
  import-aware codegen/run expectations.
- FIR snapshots and focused regressions
  Guard compiler-internal invariants and keep every discovered bug permanent.

The professional move is hybrid assurance:

- prove what is worth proving
- ratchet everything else with tests, canaries, differential checks, and review discipline

For Rust-to-Futuruna translation, the production route is explicit: keep the
stable FRSS-v0 single-file subset green as a release-line contract, keep growing
downstream and generated differential evidence inside that subset, keep
unsupported Rust fail-closed, preserve stable `from-rust --verify` summaries,
and only promote crate-level translation if it gets its own contract and
canaries.

## What Futuruna Is Working Toward

The long-term goal is not just "more features."

It is:

1. a language whose semantics stay stable under change
2. a compiler whose risky passes are increasingly checked rather than merely trusted
3. a project that another contributor can pick up without reintroducing old bugs

In short: keep Futuruna mint, then make more of it provable.

## Next Three Milestones

### 1. Keep the first hour boring

Tracked now by `td-1c5b00` and the stability packet.

The next assurance growth is to make a first-time user's expected path stable
and predictable:

- `runa init`, `check`, `fmt --check`, `run`, and `build`
- tutorial snippets
- feature-stage visibility
- local import/library use
- intentional first-hour mistakes that fail with Futuruna diagnostics instead
  of raw Rust, Cargo, or misleading success

Success looks like:

- `./scripts/first-run-canary.sh` stays mint-blocking
- every new first-hour bug lands in an exact expectation or canary
- unsupported paths fail closed with explicit messages

### 2. Make production claims evidence-addressable

The production-readiness table says what is strong. The stability packet says
why users should believe it and where the conjectures still are.

Success looks like:

- every production-facing claim has an evidence class
- every weak claim has a named strengthening path or is explicitly outside
  scope
- README, feature stages, CLI help, and roadmap do not contradict each other

### 3. Keep `runa verify` claims true of the running program

The practical direction is:

1. compare every PROVED claim against the runtime on generated inputs
2. report every construct the translation cannot model as unsupported
3. keep `runa verify` Preview until its guarantee is backed by that comparison

Success looks like:

- no invariant is PROVED while the interpreter or compiled program violates it
- unsupported claims fail the command instead of passing silently

## Bottom Line

Futuruna is now in a much better place than a few weeks ago:

- the project has a real mint contract
- it has curated canaries and a differential lane
- it has a contributor ratchet

The next step is not to relax because of that.

The next step is to keep tightening the semantic surface, expand realistic coverage, and gradually convert the highest-risk compiler logic from "trusted Rust" into "checked Futuruna."
