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

This is why `./scripts/downstream-canary.sh` exists alongside the general
canary tiers. It is now the blocking production evidence lane for the stable
importable-local-library surface.

## Current Lane Coverage

### Mint Gate

`./scripts/mint.sh` is still the main "is Futuruna mint right now?" contract.

What it covers well:

- core interpreter and compiled execution across `tests/`
- codegen validation over the main corpus
- roundtrip parity over the main corpus
- a few real non-test examples that historically exposed breakage

What it deliberately leaves to the downstream lane:

- multi-file library-consumer entrypoints as a named first-class lane
- imported library hygiene as a distinct policy surface
- import-heavy authored workflows outside the main `tests/` corpus shape

### Tiered Canary Suite

`./scripts/canary.sh` covers authored realistic workflows in `tests/canary/`.

What it covers well:

- realistic subsystem combinations
- stateful and extended user workflows
- curated authored regressions broader than narrow compiler probes

What it deliberately leaves to the downstream lane:

- library-consumer entrypoints are not the organizing abstraction
- import-heavy authored programs exist, but the production contract lives in the
  dedicated downstream lane
- live-async imported stream helpers still use stateful async artifact evidence
  rather than generic rustc metadata codegen checks

### Downstream Consumer Lane

`./scripts/downstream-canary.sh` now covers authored fixtures in
`tests/downstream/`.

What it covers well:

- nested flat imports
- qualified imports
- exported type/value/function usage across files
- explicit `runa check` on consumer programs
- compiled/interpreted execution over local authored library consumers
- generic `test --check-codegen` coverage for pure and effect consumer shapes
- precise live-async skip reasons for imported stream helpers
- `runa lint-library` and `runa lint-library --imports` enforcement
- expectation coverage for import pass/fail and hygiene diagnostics

Current bounded limits:

- `test --roundtrip` intentionally skips `@ import` entrypoints, so downstream
  consumer parity evidence comes from compiled execution plus codegen checks
- stateful live-async import codegen skips are counted separately and tied to
  the stateful async artifact contract

### Differential Lane

`./scripts/differential.sh` is strong at unknown semantic bugs.

What it covers well:

- replayable roundtrip corpus cases
- replayable import-aware corpus cases under `tests/differential/corpus/imports`
- seed-stable generated programs
- seed-derived generated import graphs with exact compiled-output expectations
- weird expression/program combinations the authored suite would not write

What it still misses for downstream use:

- the core random expression generator is still effectively single-file
- generated import graphs are synthesized around stable seeds, but not yet
  randomized over broad import topologies
- script-vs-library boundary mistakes are primarily covered by `lint-library`
  expectations and the downstream lane

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

These were the important downstream gaps found by the audit and how they are
covered now.

### 1. Script-vs-Library Boundary Has Explicit Tooling

Imported top-level smoke leakage was one of the real user failures. The stable
contract now relies on `runa lint-library`, `runa lint-library --imports`, and
the `-- library-hygiene: importable` marker instead of convention alone.

Evidence:

- downstream canary runs both library-file and import-graph hygiene checks
- expectation cases reject script leakage, unmarked imports, and local helper
  call chains that reach impure behavior

### 2. Generic Codegen Validation Covers Local Import Consumers

Generic `runa test --check-codegen` now recursively follows local plain and
qualified imports before deciding whether a fixture needs an external crate or
async runtime.

Evidence:

- pure and effect downstream consumer fixtures participate in the rustc metadata
  lane
- imported live-async stream fixtures report precise async-runtime skips instead
  of being mistaken for external-crate gaps
- `td-35b4e3` is closed

### 3. Constructor And Inference Corners Have Regressions

Two concrete consumer-shaped bugs from the first downstream wave are closed:

- same-name single-constructor exported ADT lowering
- tuple helper parameter inference in higher-order calls

Evidence:

- same-name single-constructor ADTs are covered by downstream alias fixtures
- named tuple callback inference is covered by the import mesh and focused
  compiled regressions
- `td-8ec837` and `td-007fbc` are closed

### 4. Import-Heavy Coverage Is Authored And Replayable

The in-repo surface now has pure, stateful, and effect-heavy downstream
consumer families plus import-heavy canary and differential corpus coverage.

Evidence:

- `tests/downstream/` has pure, stateful, and effect-heavy consumer families
- `tests/canary/extended/import_mesh_test.runa` covers authored import-heavy
  workflows
- `tests/expect/imports/` covers pass/fail import and hygiene behavior

### 5. Differential Testing Has An Import Graph Story

The random expression generator remains single-file, but the differential lane
now has both an import-aware replay corpus and generated import graphs derived
from the stable seed list.

Evidence:

- `scripts/differential.sh` runs `tests/differential/corpus/imports` with
  compiled execution and `test --check-codegen`
- the import mesh subcorpus covers nested flat imports, qualified imports,
  exported ADTs/functions/values, and named higher-order callbacks
- the generated import cases create per-seed flat and qualified import graphs,
  then run import hygiene, compiled execution, `test --check-codegen`, and exact
  compiled stdout expectations

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

1. Keep the downstream lane green and blocking for local import consumers.
2. Distill every future import-consumer bug into `tests/downstream/`,
   `tests/expect/imports/`, or the import-aware differential corpus.
3. Keep live-async import skips explicit and tied to stateful async artifact
   expectations.

## Task Queue

Closed task trail:

- `td-35b4e3` teach the generic `check-codegen` lane to cover local import consumers
- `td-8ec837` lower single-constructor exported ADTs correctly
- `td-007fbc` infer tuple helper params in higher-order calls
- `td-c812f4` design compiletest-style expectation suites
- `td-14f86b` define and enforce library-vs-script import hygiene
- `td-b26732` expand downstream consumer fixtures to stateful and effect-heavy families
- `td-a70b05` add import-aware downstream cases to deeper search lanes
- `td-b4729e` add import-consumer expectation cases
- `td-fd7715` deepen lint-library purity analysis through local helper calls

## Bottom Line

The main oversight was not "we forgot tests." It was that the old assurance
stack mostly exercised Futuruna as standalone programs, while users were
hitting it as imported libraries.

That mismatch is now small enough for a bounded production-ready claim:

- there is a dedicated downstream consumer lane
- there are import-normalization snapshots
- there is an import-heavy authored canary
- there are import-consumer expectations
- there is an import-aware differential subcorpus
- library hygiene is enforced by stable tooling rather than convention

The stable claim is intentionally local: authored Futuruna libraries imported
through documented local flat or qualified imports, with exported values, types,
functions, stateful/effect consumer families, and import-safe helper files kept
green by the downstream, expectation, and differential lanes.
