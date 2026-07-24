---
type: source
source_type: repo-doc-batch
status: summarized
source_paths:
  - "docs/milestones/m27-error-reporting.md"
  - "docs/milestones/m28-negative-tests-cli.md"
  - "docs/milestones/m29-fir.md"
  - "docs/milestones/m30-passes.md"
  - "docs/milestones/m33-trait-resolution.md"
  - "docs/milestones/m38-ci.md"
  - "docs/milestones/m41-codegen-parity.md"
  - "docs/milestones/m43-rust-transpiler.md"
  - "docs/milestones/m45-perf-baseline.md"
created: 2026-07-18
updated: 2026-07-18
tags:
  - source
  - docs
  - milestones
related:
  - "[[board]]"
  - "[[compiler-pipeline]]"
  - "[[test-surface]]"
  - "[[verification-lanes]]"
---

# Milestone Docs

This source note summarizes selected high-value milestone documents under
`docs/milestones/`.

> [!warning] Staleness Boundary
> Some milestone docs are historical snapshots and their checklists lag the live
> repo. Use [[board]], `td critical-path`, and current source/tests as the live
> operational state.

## Compiler Quality Milestones

M27 error reporting delivered structured diagnostics, spans, color handling,
type-checker breadcrumbs, LSP conversion, parse warning behavior, many dangerous
unwrap fixes, and expression spans. Remaining deferred work is statement and
pattern spans.

M28 negative tests and CLI polish defines the target shape for bad-input
coverage: negative fixtures, parse/type/runtime errors, version output, unknown
command handling, and integration into `runa test`. The live repo has moved
past the original "zero negative tests" premise, so treat the doc as design
intent rather than current count.

M29 FIR introduced `TypeRegistry`, `OwnershipAnalysis`, typed/ownership
annotated FIR nodes, AST-to-FIR lowering, FIR-to-Rust emission, and
`runa emit --fir`. It separated analysis data from Rust emission.

M30 split RustCodegen into explicit passes: declaration/import scanning,
borrow-flag computation, and emission. The doc still says "in progress", but
its checklist is complete.

M33 trait resolution remains in progress: TypeChecker should collect trait and
impl declarations, check impl completeness, and report missing methods as
Futuruna diagnostics rather than rustc failures.

M41 codegen parity records the principle "if it runs in the interpreter, it
compiles to Rust." Its historical counts are now stale, but the core result
matters: `--check-codegen` became a CI gate.

## Tooling And Ecosystem Milestones

M38 CI/CD delivered push/PR CI over Rust builds, Rust tests, release build,
Futuruna tests, formatting, version checks, and release artifacts for tagged
versions.

M43 Rust-to-Futuruna transpiler delivered `runa from-rust`, verification, and a
batch test lane over example Rust files.

M45 performance baseline delivered `runa bench` and baseline measurements for
interpreter, parse/type-check, codegen, test suite, from-rust transpiler, and
binary size.

## Wiki Implication

The milestone docs are useful as history and design intent, but the production
readiness guide should prefer current lanes: [[mint-gate]], [[canary-suite]],
[[differential-testing]], [[expectation-suites]], and `td critical-path`.
