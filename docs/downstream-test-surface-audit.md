# Downstream Test-Surface Audit

This document audits where Futuruna's current quality gates model real
downstream use well, and where they still leave gaps.

The trigger for this audit was straightforward: external user programs broke in
five different ways even though the in-repo suite was largely green. The root
cause was not lack of testing effort. It was a shape mismatch between what the
repo was exercising and how downstream users were actually consuming Futuruna.

## What Counts As "Downstream" Here

For this audit, "downstream" does not mean "another repository checked into CI."
It means authored Futuruna programs that are:

- consumed as libraries rather than only run directly
- imported through flat, qualified, or content-addressed imports
- depended on for exported values, types, and functions across files
- exercised through the same codegen/check path a real consumer would hit

This is why the new `./scripts/downstream-canary.sh` lane exists alongside the
general canary tiers.

## Current Lane Coverage

### Mint Gate

`./scripts/mint.sh` is still the main "is Futuruna mint right now?" contract.

What it covers well:

- core interpreter and compiled execution across `tests/`
- codegen validation over the main corpus
- roundtrip parity over the main corpus
- a few real non-test examples that historically exposed breakage

What it misses for downstream use:

- multi-file library-consumer entrypoints as a named first-class lane
- imported library hygiene as a distinct policy surface
- import-heavy authored workflows outside the main `tests/` corpus shape

### Tiered Canary Suite

`./scripts/canary.sh` covers authored realistic workflows in `tests/canary/`.

What it covers well:

- realistic subsystem combinations
- stateful and extended user workflows
- curated authored regressions broader than narrow compiler probes

What it misses for downstream use:

- library-consumer entrypoints are not the organizing abstraction
- import-heavy authored programs exist, but they are still framed as canaries,
  not as a dedicated consumer contract
- the generic `test --check-codegen` path still skips many local multi-file
  import consumers

### Downstream Consumer Lane

`./scripts/downstream-canary.sh` now covers authored fixtures in
`tests/downstream/`.

What it covers well:

- nested flat imports
- qualified imports
- exported type/value/function usage across files
- explicit `runa check` on consumer programs
- compiled/interpreted execution over local authored library consumers

What it still misses:

- a broader corpus than the current initial fixture family
- direct participation of most consumer fixtures in the generic
  `test --check-codegen` runner
- stateful/effect-heavy downstream consumer families
- import-hygiene policy enforcement beyond authored convention

### Differential Lane

`./scripts/differential.sh` is strong at unknown semantic bugs.

What it covers well:

- replayable roundtrip corpus cases
- seed-stable generated programs
- weird expression/program combinations the authored suite would not write

What it misses for downstream use:

- generated programs are effectively single-file
- it does not synthesize nested import graphs or library-consumer entrypoints
- it does not pressure script-vs-library boundary mistakes

### FIR Snapshots And Import-Normalization Snapshots

Compiler snapshots now cover both FIR structure and post-import normalization.

What they cover well:

- silent compiler drift in internal lowering shape
- dropped transitive imports
- leaked top-level imported smoke binds
- module export-set drift

What they miss:

- they are structural guardrails, not behavioral consumer programs
- they do not prove imported workflows still compile and run end to end

### Proof / Verify Lanes

The proof stack is valuable, but it is not the primary tool for downstream
consumer hardening right now.

What it covers well:

- explicit proof correctness
- some ordinary workflow invariants
- future proof-backed compiler slices

What it misses:

- import-heavy library-consumer codegen behavior
- script-vs-library hygiene
- emitted Rust validity for cross-file consumer programs

## Current Gap Map

These are the important remaining downstream gaps after the new consumer lane.

### 1. Script-vs-Library Boundary Is Still Conventional, Not Enforced

Imported top-level smoke leakage was one of the real user failures. We now test
that class better, but the project still relies on convention rather than a
hard rule for importable library files.

Consequence:

- a `lib`-shaped file can still accumulate top-level smoke/demo code
- the break may only surface when another file imports it

Needed:

- a hygiene rule, lint, or explicit surface distinction for importable library
  files vs runnable scripts

### 2. Generic Codegen Validation Is Still Weaker Than The Explicit Consumer Lane

The new downstream lane uses explicit `runa check` for entrypoints because the
generic `runa test --check-codegen` runner still skips most local multi-file
consumer fixtures.

Consequence:

- downstream coverage exists, but in a parallel path rather than the generic
  codegen validation machinery

Tracked by:

- `td-35b4e3`

### 3. Constructor And Inference Corners Still Leak Through Consumer Shapes

Two concrete consumer-shaped bugs are now tracked:

- same-name single-constructor exported ADT lowering
- tuple helper parameter inference in higher-order calls

Consequence:

- the downstream lane is useful, but some authored fixtures still need local
  workarounds

Tracked by:

- `td-8ec837`
- `td-007fbc`

### 4. Import-Heavy Coverage Is Still Too Small

We now have one real downstream consumer family, but not enough breadth yet.

Still missing:

- stateful downstream consumers
- effect-heavy downstream consumers
- module consumers with richer exported ADT/trait surfaces
- consumer fixtures that stress diagnostics and failure modes, not just success

### 5. Differential Testing Still Has No Import Graph Story

The differential lane is still effectively a single-file semantic fuzzer.

Consequence:

- it is excellent at expression/runtime edge cases
- it is weak at the exact import/module/library-consumer shape that hurt us

Needed:

- an import-aware downstream corpus or generated import-graph cases

## Recommendations

### Blocking Lanes

These should remain or become blocking:

- `./scripts/mint.sh`
- `./scripts/canary.sh`
- `./scripts/downstream-canary.sh`

Reason:

- mint protects the broad core contract
- canary protects authored realistic workflows
- downstream-canary protects the library-consumer surface specifically

### Near-Term Engineering Order

1. Make the downstream lane broader, not just present.
2. Enforce script-vs-library hygiene.
3. Make generic codegen validation understand local multi-file import consumers.
4. Add import-aware downstream corpus pressure to the deeper lanes.

## Task Queue

Already tracked and still relevant:

- `td-35b4e3` teach the generic `check-codegen` lane to cover local import consumers
- `td-8ec837` lower single-constructor exported ADTs correctly
- `td-007fbc` infer tuple helper params in higher-order calls
- `td-c812f4` design compiletest-style expectation suites

Created from this audit:

- `td-14f86b` define and enforce library-vs-script import hygiene
- `td-b26732` expand downstream consumer fixtures to stateful and effect-heavy families
- `td-a70b05` add import-aware downstream cases to deeper search lanes

## Bottom Line

The main oversight was not "we forgot tests." It was that the old assurance
stack mostly exercised Futuruna as standalone programs, while users were
hitting it as imported libraries.

That mismatch is now much smaller:

- there is a dedicated downstream consumer lane
- there are import-normalization snapshots
- there is an import-heavy authored canary

But the surface is not fully professional yet until library hygiene, generic
codegen coverage, and deeper import-aware search are all pulled forward.
