# Futuruna Production-Readiness Scorecard

This scorecard answers a narrower question than
[feature stages](feature-stages.md): what can Futuruna responsibly describe as
production-ready today, and what still needs more evidence?

Feature stage is a compatibility promise. Production readiness is an evidence
claim. A stable surface can still need more gates before it is production-ready.

Tracked by `td-4ffe32`.

## Rating Definitions

| Rating | Meaning |
|--------|---------|
| Production-ready | The surface has a documented contract, routine blocking coverage, realistic canaries or downstream coverage where applicable, explicit skip accounting, and no known open blocker against the covered contract. |
| Preview | The surface is intended for real use and has meaningful tests, but some evidence is missing: skips remain material, external/downstream coverage is incomplete, behavior is still being hardened, or the contract is not fully enforced by tooling. |
| Research-grade | The surface is useful for exploration or internal validation, but should not be treated as a production contract. It may be new, broad, performance-sensitive, proof-trust-sensitive, or missing durable coverage. |

## Evidence Snapshot

Evidence source: local scorecard sweeps recorded in `td-f7f0d2`, current CI
wiring, and the current `td` queue through 2026-07-18.

| Lane | Evidence | Readiness Signal |
|------|----------|------------------|
| Mint gate | Latest recorded `./scripts/mint.sh` passed: Rust tests, release build, interpreted tests, compiled tests, expectations, check-codegen, roundtrip, one regression run, and Danish constitution checks. | Strong core health signal. |
| Mint check-codegen | 89 passed, 20 skipped in the latest recorded mint run. | Passing with explicit skips for external-crate and live-async surfaces. |
| Mint roundtrip | 73 matched, 36 skipped in the latest recorded mint run. | Strong for pure/core programs; imported and live-async entrypoints use dedicated lanes instead of hidden roundtrip pass evidence. |
| Expectations | Latest recorded mint expectation run passed 36 cases with 4 dependency-only skips; focused import expectations passed 7 cases with 3 dependency-only skips. | Useful and growing; artifact snapshots, import pass/fail cases, first-hour type diagnostics, and malformed dependency diagnostics now defend selected compiler-contract shapes. |
| Authored canaries | `./scripts/canary.sh` passed across core, stateful, extended, and regressions. | Strong realistic workflow signal. |
| Core canary tier | 10 run passed, 10 check-codegen passed, 10 roundtrip matched, 0 skipped. | Best production-readiness evidence in the project. |
| Stateful canary tier | 7 run passed, 7 check-codegen skipped, 7 roundtrip skipped. | Execution signal is good and live-async skips are explicit; async artifact expectations provide emitted-runtime shape evidence. |
| Extended canary tier | 10 run passed, 5 check-codegen passed, 5 skipped, 2 roundtrip matched, 8 skipped. | Useful coverage, but not production-grade as a whole. |
| Regression canary tier | 3 run passed, 2 check-codegen passed, 1 skipped, 1 roundtrip matched, 2 skipped. | Good bug-class coverage; skip accounting matters. |
| Downstream consumer lane | `./scripts/downstream-canary.sh` passed: 9 importable files passed hygiene, 13 import graphs passed hygiene, 4 consumer checks passed, 13 downstream tests ran, 11 check-codegen passed, 2 live-async skips were reported precisely, and 13 import roundtrip skips were reported explicitly. | Strong production signal for local import consumers because execution, codegen, linting, and skip accounting are all first-class. |
| Differential lane | Local sweep passed checked-in roundtrip corpus cases, authored import-aware corpus compiled execution and check-codegen with 0 skips, seed-stable generated interpreter-vs-compiled cases, and generated import-aware cases with import hygiene, compiled execution, check-codegen, and exact compiled stdout expectations. | Strong production signal for compiler hardening because replay, generative search, import/codegen pressure, failure artifacts, and skip accounting are all first-class. |
| CI wiring | CI runs mint on Ubuntu/macOS, Rust formatting, differential, authored canaries, and downstream consumers. | Strong project-level signal. |

## Surface Scorecard

| Surface | Rating | Evidence | Next Gate Before Promotion |
|---------|--------|----------|----------------------------|
| Core documented syntax and ordinary expression semantics | Production-ready | Stable docs, mint, core canaries with 0 skips, roundtrip parity, many focused regression tests. | Keep semantic changes under the contributor ratchet; expand expectation coverage when diagnostics or phase markers become stable promises. |
| Documented stdlib builtin semantics | Production-ready | Stable docs, broad tests, typed lowering corpus, string/list/map/set/property tests, core canaries, signature-table audit coverage, and duplicate-evaluation audit coverage. | Keep signature-table and duplicate-evaluation regression tests active; add expectation cases for newly promised contract edges. |
| Core CLI commands: `run`, `check`, `emit`, `build`, `test`, `fmt`, `hashes` | Production-ready | Stable feature stage, mint and CI exercise the core commands. | Keep `mint` authoritative, feature-stage metadata synchronized, and compatibility-guide enforcement active. |
| Rust formatting and repository hygiene | Production-ready | `runa fmt --check` is used by canary/downstream/CI, `cargo fmt --check` is green repo-wide, and the dedicated repo-wide rustfmt parity task (`td-e7d877`) is closed. | Keep `cargo fmt --check`, `runa fmt --check`, and `git diff --check` as routine gates. |
| Compiler Rust codegen for pure/core programs | Production-ready | Stable feature stage for pure/core generated Rust behavior; mint/core canaries check codegen and roundtrip; phase marker cases, typed lowering corpus, artifact golden fixtures, duplicate-evaluation template audit, signature-table audit, minimized differential corpus, ownership-sensitive artifact snapshots, and a source-derived ownership-lowering translation check for direct branch/list reuse are in place. Exact emitted Rust remains internal outside named fixtures. | Keep artifact fixture diffs compatibility-reviewed and add new fixtures or translation checks for every newly promised emitted shape. |
| Interpreter-vs-compiled parity for pure/core programs | Production-ready | Mint roundtrip, core canary roundtrip, and checked-in minimized differential cases are strong, with 0 core canary skips. | Require every future parity bug to land in the narrowest permanent lane. |
| Importable local libraries and downstream consumer shape | Production-ready | Stable feature stage; dedicated downstream lane covers flat and qualified imports, exported values/types/functions, pure/stateful/effect consumer families, library hygiene, import-graph hygiene, consumer `check`, compiled runs, and 11/13 generic codegen passes. The remaining 2 codegen skips are precise live-async imported stream cases covered by the stateful async contract; import roundtrip skips are explicit and not counted as pass evidence. Import-consumer expectations and the import-aware differential subcorpus cover smaller pass/fail and deeper replay shapes. | Keep downstream canary, import expectations, and import-aware differential coverage blocking; add a downstream or expectation fixture for every future import-consumer bug. |
| Streams, subjects, actors, and effect-heavy workflows | Production-ready | Stable feature stage, explicit named-scope lifetime contract, 7 compiled stateful canaries including an adversarial subjects/streams/effects/actors workflow, async runtime artifact expectations, stream lifetime diagnostics, and explicit skip accounting for generic check-codegen/roundtrip live-async skips. | Keep stateful canaries and async artifact expectations blocking; add a new adversarial canary or expectation for every future stateful bug. |
| WASM-facing behavior | Preview | WASM tests, an extended export-surface canary, an automated `runa wasm` build lane, and documented preview artifact boundaries exist. Missing `wasm-pack` is reported as an explicit skip unless CI requires it. | Decide where the WASM lane is required in CI and add package-shape expectations once the JS/ABI boundary is chosen. |
| Rust interop and Rust-facing library integration | Production-ready | Stable feature stage for the covered `runa lib` contract; export shape has an artifact expectation for public/private item boundaries; the Rust interop canary compiles generated `runa lib` output as a plain Rust module, as external-crate generated code inside an offline Cargo consumer, and as `src/lib.rs` in a user-owned Cargo library crate consumed by a downstream Cargo package. The covered lane exercises exported structs, enums, borrowed params, lists, `Option`, `Result`, `@ depend`, `@ use`, raw Rust helpers, regex-backed stdlib codegen, and missing-dependency guidance in generated source/stderr. `from-rust`, automatic manifest generation, and richer FFI patterns remain outside this production-ready claim. | Keep the interop canary blocking; add a new canary before expanding the Rust-facing contract to generated manifests, richer FFI, or new package layouts. |
| `runa lint-library` import-hygiene tooling | Production-ready | Stable feature stage; downstream lane enforces marked importable helpers and imported helper graphs; expectations cover pass/fail import hygiene, script leakage, unmarked imports, and local helper-call-chain impurity rejection. | Keep import-hygiene negative expectations and downstream lint checks blocking; expand the lint only with compatibility-guide notes when the stable policy changes. |
| `runa stress-gen` and differential testing | Production-ready | Stable feature stage; CI has a differential job; the canonical script replays minimized corpus cases, runs seed-stable generated interpreter-vs-compiled programs, writes replay artifacts for failures, exercises authored import-aware corpus with compiled execution/check-codegen, and generates per-seed import graphs with import hygiene, compiled execution, check-codegen, and exact compiled stdout expectations. Imported helper roundtrip skips are explicit and not counted as pass evidence. | Keep every differential-found compiler bug landing in the narrowest permanent corpus, expectation, or canary lane; scale stress count only when runtime remains predictable. |
| Compiletest-style expectations | Preview | Expectation runner is in mint and has diagnostic/run/phase cases plus exact golden-file checks. The corpus now covers first-hour type mistakes and malformed dependency directives, but is still small. | Grow the golden snapshot corpus and require exact expectations for new diagnostics where stable. |
| FIR and phase snapshots | Preview | Phase validation exists and has already caught drift classes, including cross-binding/module marker cases. | Add larger reviewed golden snapshots and document which phase markers are stable enough for tests. |
| Explicit proof terms and proof kernel surface | Preview | Kernel exists in `src/proof_kernel.rs`; `td-f8f162` records the trusted-boundary audit and honest size budget; proof canaries exist. | Add more adversarial kernel tests and require boundary-budget updates for new rule or axiom growth before promotion. |
| `runa verify` elaboration and solver-assisted verification | Preview | Feature stage is preview; useful proof workflows exist. Elaboration remains trusted compiler machinery. | Add more proof-backed validation around generated theorem shape and keep solver fallback outside the kernel trust boundary. |
| Proof-backed compiler checking | Research-grade | Computation-lemma translation checking and a first ownership-sensitive Rust-lowering translation check exist, but the compiler is not verified and most high-risk passes remain trusted Rust. | Grow the ownership checker beyond direct branch/list reuse and add checked slices for another high-risk lowering pass before promotion to preview. |
| `@ persist`, SQL-backed facts, watches, migrations, and transactional storage | Research-grade | Storage canaries exist and M26b is active; typed columns, SQL-backed `findall`, persisted retract codegen coverage, and scoped transaction codegen coverage exist, but phase-B children remain open. | Finish M26b phase B (`td-c0a7a1`, `td-13a997`, `td-25667e`) and add storage canary/roundtrip policy. |
| Law and constitution examples | Research-grade | Useful real programs exist and mint checks legacy Danish constitution chapters. They are stress/examples, not production legal semantics. | Keep them as semantic pressure tests; do not market them as legal-reasoning production support. |
| `runa from-rust` and `from-rust --verify` | Preview | Feature stage is preview for the checked single-file validation boundary, not arbitrary Rust crate translation. CI runs the example transpiler validation with 35 exact supported matches and 0 explicit expected-unsupported adversarial fixtures. Mint also runs a clean-directory downstream canary with 9 consumer-shaped exact matches and 10 expected-unsupported fail-closed diagnostics for ownership, generics, error remapping, iterator state machines, tuple reference patterns, async/threading, unsafe, and external-crate boundaries, plus a generated supported-subset differential lane with 6 exact matches and replay artifacts on failure. The supported set includes three example-corpus consumer-shaped single-file workflows for config parsing/validation, invoice totals, and event rollups, plus downstream production-corpus growth for nested customer/order data, error-row pipelines, deterministic inventory reporting, text normalization, and enum/reference loop aggregation with conditional accumulator rebinding. It also includes the checked-in nested pattern fixture (recursive `Box<T>` enum constructors and ordered two-reference tuple matches lowered to nested Futuruna matches), generic trait fixture (`Functor` associated-type calls over `Option`/`Result`, generic higher-order functions, `impl Fn` composition, and generic `Pair` methods), recursive ownership fixture (`Vec<Box<T>>`, inherent impl methods, functional `&mut self` field-push lowering, and narrow recursive `Option<&T>` search translated as value `Option(T)`), and stateful iterator/map grouping subset for tuple-key `sort_by`, Fibonacci-style `scan(...).collect()`, and `entry(...).or_insert_with(Vec::new).push(...)`. Unsupported shapes outside these checked subsets still fail closed with stable diagnostic categories documented in `docs/from-rust-contract.md`. | Work through the Production Promotion Checklist in `docs/from-rust-contract.md`: version the supported subset as a stable contract, keep growing external/downstream corpus evidence and supported-subset differential coverage, and keep arbitrary Rust crate translation outside the claim unless separately promoted. |
| `runa audit`, LSP, and other exploratory tooling | Research-grade | Feature stage is experimental or preview; output/interface shape is not frozen. | Add explicit contracts and canaries for any tool that becomes user-facing. |

## Upgrade Plan

Effort is the expected size of the next state transition:

- `S`: mostly docs, small tests, or lane wiring.
- `M`: focused implementation plus permanent coverage.
- `L`: cross-lane/compiler work with new fixtures or CI behavior.
- `XL`: multi-milestone design and implementation.

Impact is scored from `1` to `5`, where `5` means the next iteration materially
changes Futuruna's production-readiness claim.

| Area | State | Next 1-3 Milestones | Effort for next state | Impact score |
|------|-------|----------------------|-----------------------|--------------|
| Core syntax and ordinary expression semantics | Production-ready | Keep mint green; expand diagnostics/phase expectations when they become stable promises; keep compatibility guide updates enforced. | M | 4 |
| Documented stdlib builtins | Production-ready | Keep signature-table audit tests green; add expectation cases for remaining contract edges; keep duplicate-evaluation fixtures in the artifact lane. | M | 4 |
| Core CLI workflow | Production-ready | Keep `mint` authoritative; keep feature-stage metadata and doc frontmatter synchronized; keep compatibility guide updates enforced. | M | 4 |
| Pure/core interpreter-vs-compiled parity | Production-ready | Keep minimized differential corpus growing; keep core canaries 0-skip; promote every new semantic bug into the narrowest permanent lane. | M | 5 |
| Rust formatting and repository hygiene | Production-ready | Keep `cargo fmt --check`, `runa fmt --check`, and `git diff --check` routine; keep formatting-only cleanups separate from semantic changes. | S | 3 |
| Rust codegen for pure/core programs | Production-ready | Keep artifact fixture diffs compatibility-reviewed; add fixtures or translation checks for newly promised emitted shapes; keep pure/core canaries 0-skip. | M | 5 |
| Importable local libraries and downstream consumer shape | Production-ready | Keep downstream canary, import-consumer expectations, and import-aware differential subcorpus green; add fixtures for every future import-consumer bug; keep live-async import skips tied to the stateful async artifact contract. | M | 5 |
| Streams, subjects, actors, and effect-heavy workflows | Production-ready | Keep named-scope lifetime diagnostics stable; keep compiled stateful canaries and async artifact expectations green; add adversarial fixtures for future stateful bug classes. | M | 5 |
| WASM-facing behavior | Preview | Keep automated WASM build canary green; require the WASM lane in CI where `wasm-pack` is installed; add package-shape expectations after choosing the JS/ABI boundary. | M | 4 |
| Rust interop and integration | Production-ready | Keep the Rust consumer, offline external-crate, and package-layout canaries blocking; require new canaries and compatibility-guide entries before expanding manifest generation, richer FFI, or package layouts. | M | 4 |
| Library hygiene tooling | Production-ready | Keep `lint-library` and `lint-library --imports` blocking in downstream; preserve negative expectation coverage for script leakage, unmarked imports, and helper-chain impurity; document any policy expansion as compatibility-facing. | S | 3 |
| Differential and generative testing | Production-ready | Keep minimized replay corpus growing; keep generated import-aware cases and exact compiled-output expectations green; scale CI stress count only when runtime remains predictable. | M | 5 |
| Compiletest-style expectations | Preview | Grow diagnostics and golden snapshot corpus; require exact expectations for new diagnostics where stable; keep expectation lanes fast enough for mint. | M | 4 |
| FIR and phase snapshots | Preview | Add larger reviewed golden snapshots; connect snapshots to import normalization; document which phase markers are stable enough for tests. | M | 4 |
| Explicit proof terms and proof kernel | Preview | Trusted-core size/surface audit recorded; add more adversarial kernel tests; keep solver fallback outside the kernel trust boundary. | M | 4 |
| `runa verify` elaboration and solver-assisted verification | Preview | Add validation for generated theorem shape; expand proof-backed checking around computation lemmas; document unsupported automation cases. | XL | 4 |
| Proof-backed compiler checking | Research-grade | Extend the ownership-lowering checker beyond direct branch/list reuse; add independent source/target models for another high-risk lowering pass; keep docs explicit about checked vs trusted compiler machinery. | L | 5 |
| `@ persist` and SQL-backed storage | Research-grade | Finish M26b phase B (`td-c0a7a1`); harden assert/retract/findall/transaction/watch/migrate tasks; add storage-specific canary and skip policy. | XL | 4 |
| Law and constitution examples | Research-grade | Keep as semantic pressure tests; document non-legal-production status in README/docs; add expectation/canary distillations only when they expose language bugs. | S | 2 |
| `runa from-rust` tooling | Preview | Work through the Production Promotion Checklist in `docs/from-rust-contract.md`: keep the 35-fixture supported subset, 9-fixture mint-blocking downstream canary, and 6-case generated supported-subset differential lane matching Rust output exactly; version the supported subset as a stable contract; keep broadening external/downstream and differential evidence; and keep arbitrary Rust crate translation outside the claim unless separately promoted. | L | 4 |
| `runa audit`, LSP, and exploratory tooling | Research-grade | Pick which tools are user-facing; add explicit contracts for those; keep the rest marked experimental. | M | 2 |

## Promotion Rules

A surface can move to production-ready only when all of these are true:

1. The source and behavior contract is documented or explicitly linked from
   `docs/feature-stages.md`.
2. The surface has at least one blocking gate that exercises the behavior users
   rely on.
3. Skips are counted separately from passes; no required evidence is hidden
   behind "skipped" output.
4. Multi-subsystem behavior has an authored canary or downstream fixture, not
   only a minimized unit test.
5. Historical bugs for that surface have permanent regression coverage in the
   narrowest matching lane: expectation, canary, downstream, differential, or
   proof-backed checking.
6. No open P2/P3 issue describes wrong-code, unsoundness, nondeterminism, or data
   loss for that surface.
7. Compatibility-guide and feature-stage updates are part of the change when a
   stable surface moves.

## Current Production Claim

The safe production claim is:

> Futuruna's stable core language, core CLI, repository hygiene gates,
> pure/core Rust codegen behavior, documented reactive/stateful workflows,
> importable local-library consumer shape, and differential/generative compiler
> hardening lane are production-ready for small to medium programs that stay
> inside documented syntax, stdlib, named-scope stream lifetime, actor,
> effect-handler, local import, export, and import-hygiene contracts, with
> strong compiled/canary/downstream/artifact/differential evidence and explicit
> skip accounting.

The unsafe production claim is:


Those surfaces are valuable and tested, but they remain preview or
research-grade until the next gates above are closed.

## Next Evaluation Tasks

1. Keep exact diagnostic and phase expectations growing as new stable
   diagnostics or phase markers are promised.
2. Decide where `wasm-pack` should be required rather than skipped and add
   package-shape expectations for the chosen WASM boundary.
3. Keep every future differential-found compiler bug landing in the checked-in
   replay corpus or a narrower expectation/canary when that better captures the
   contract.
4. Distill future compiler bugs into in-repo downstream, expectation, canary,
   or differential fixtures.
