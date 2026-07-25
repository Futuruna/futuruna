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
| Mint check-codegen | 84 passed, 22 skipped in the latest recorded mint run. | Passing but skip-heavy for external-crate surfaces. |
| Mint roundtrip | 71 matched, 35 skipped in the latest recorded mint run. | Strong for pure/core programs; weaker for external/stateful surfaces. |
| Expectations | Latest recorded `./scripts/expectations.sh` passed 20 cases with 1 skipped, including artifact and codegen contract fixtures. | Useful and growing; artifact snapshots now defend selected emitted Rust shapes. |
| Authored canaries | `./scripts/canary.sh` passed across core, stateful, extended, and regressions. | Strong realistic workflow signal. |
| Core canary tier | 10 run passed, 10 check-codegen passed, 10 roundtrip matched, 0 skipped. | Best production-readiness evidence in the project. |
| Stateful canary tier | 6 run passed, 6 check-codegen skipped, 6 roundtrip skipped. | Execution signal is good; generic parity evidence is weak. |
| Extended canary tier | 10 run passed, 5 check-codegen passed, 5 skipped, 2 roundtrip matched, 8 skipped. | Useful coverage, but not production-grade as a whole. |
| Regression canary tier | 3 run passed, 2 check-codegen passed, 1 skipped, 1 roundtrip matched, 2 skipped. | Good bug-class coverage; skip accounting matters. |
| Downstream consumer lane | `./scripts/downstream-canary.sh` passed: 15 importable files passed hygiene, 13 downstream tests ran, 5 check-codegen passed, 8 skipped, 13 roundtrip skipped. | Import/lint coverage is real; generic parity is not yet broad. |
| Differential lane | Local reduced sweep passed generated interpreter-vs-compiled cases, and `tests/differential/corpus` now contains minimized historical codegen cases plus an import-aware subcorpus. | Good generator signal with durable replay artifacts. |
| CI wiring | CI runs mint on Ubuntu/macOS, Rust formatting, differential, authored canaries, and downstream consumers. | Strong project-level signal. |

## Surface Scorecard

| Surface | Rating | Evidence | Next Gate Before Promotion |
|---------|--------|----------|----------------------------|
| Core documented syntax and ordinary expression semantics | Production-ready | Stable docs, mint, core canaries with 0 skips, roundtrip parity, many focused regression tests. | Keep semantic changes under the contributor ratchet; expand expectation coverage when diagnostics or phase markers become stable promises. |
| Documented stdlib builtin semantics | Production-ready | Stable docs, broad tests, typed lowering corpus, string/list/map/set/property tests, core canaries, signature-table audit coverage, and duplicate-evaluation audit coverage. | Keep signature-table and duplicate-evaluation regression tests active; add expectation cases for newly promised contract edges. |
| Core CLI commands: `run`, `check`, `emit`, `build`, `test`, `fmt`, `hashes` | Production-ready | Stable feature stage, mint and CI exercise the core commands. | Keep `mint` authoritative, feature-stage metadata synchronized, and compatibility-guide enforcement active. |
| Rust formatting and repository hygiene | Production-ready | `runa fmt --check` is used by canary/downstream/CI, `cargo fmt --check` is green repo-wide, and the dedicated repo-wide rustfmt parity task (`td-e7d877`) is closed. | Keep `cargo fmt --check`, `runa fmt --check`, and `git diff --check` as routine gates. |
| Compiler Rust codegen for pure/core programs | Production-ready | Stable feature stage for pure/core generated Rust behavior; mint/core canaries check codegen and roundtrip; phase marker cases, typed lowering corpus, artifact golden fixtures, duplicate-evaluation template audit, signature-table audit, minimized differential corpus, and ownership-sensitive artifact snapshots are in place. Exact emitted Rust remains internal outside named fixtures. | Keep artifact fixture diffs compatibility-reviewed and add new fixtures for every newly promised emitted shape. |
| Interpreter-vs-compiled parity for pure/core programs | Production-ready | Mint roundtrip, core canary roundtrip, and checked-in minimized differential cases are strong, with 0 core canary skips. | Require every future parity bug to land in the narrowest permanent lane. |
| Importable local libraries and downstream consumer shape | Preview | Dedicated downstream lane, library hygiene lint, and consumer checks pass. Generic roundtrip skips all downstream fixtures and check-codegen skips 8 of 13. | Teach generic check-codegen to cover local import consumers (`td-35b4e3`) and add import-aware deep-search cases (`td-a70b05`, `td-b4729e`). |
| Streams, subjects, actors, and effect-heavy workflows | Production-ready | Stable feature stage, explicit named-scope lifetime contract, 7 compiled stateful canaries including an adversarial subjects/streams/effects/actors workflow, async runtime artifact expectations, stream lifetime diagnostics, and explicit skip accounting for generic check-codegen/roundtrip live-async skips. | Keep stateful canaries and async artifact expectations blocking; add a new adversarial canary or expectation for every future stateful bug. |
| WASM-facing behavior | Preview | WASM tests, an extended export-surface canary, an automated `runa wasm` build lane, and documented preview artifact boundaries exist. Missing `wasm-pack` is reported as an explicit skip unless CI requires it. | Decide where the WASM lane is required in CI and add package-shape expectations once the JS/ABI boundary is chosen. |
| Rust interop and Rust-facing integration | Preview | Feature stage is preview; CI runs `from-rust --test examples/from-rust/`; `runa lib` export shape has an artifact expectation for public/private item boundaries. | Add canaries for stable integration shapes and broaden exported type/function fixtures before promotion. |
| `runa lint-library` import-hygiene tooling | Preview | Downstream lane enforces hygiene over importable files and imports. | Deepen purity analysis through local helper calls (`td-fd7715`). |
| `runa stress-gen` and differential testing | Preview | CI has a differential job, seed-stable generator, checked-in minimized replay corpus, and import-aware differential subcorpus. | Require every differential-found compiler bug to land in corpus; add more import/ownership pressure as failures are found. |
| Compiletest-style expectations | Preview | Expectation runner is in mint and has diagnostic/run/phase cases plus exact golden-file checks. Corpus is still small. | Grow the golden snapshot corpus and require exact expectations for new diagnostics where stable. |
| FIR and phase snapshots | Preview | Phase validation exists and has already caught drift classes, including cross-binding/module marker cases. | Add larger reviewed golden snapshots and document which phase markers are stable enough for tests. |
| Explicit proof terms and proof kernel surface | Preview | Kernel exists in `src/proof_kernel.rs`; `td-f8f162` records the trusted-boundary audit and honest size budget; proof canaries exist. | Add more adversarial kernel tests and require boundary-budget updates for new rule or axiom growth before promotion. |
| `runa verify` elaboration and solver-assisted verification | Preview | Feature stage is preview; useful proof workflows exist. Elaboration remains trusted compiler machinery. | Add more proof-backed validation around generated theorem shape and keep solver fallback outside the kernel trust boundary. |
| Proof-backed compiler checking | Research-grade | Translation-checking and proof-backed bootstrap work exist, but the compiler is not verified and high-risk passes remain trusted Rust. | Continue `td-a97ed9`; add checked compiler slices beyond toy/bootstrap cases. |
| `@ persist`, SQL-backed facts, watches, migrations, and transactional storage | Research-grade | Storage canaries exist and M26b is active; typed columns, SQL-backed `findall`, persisted retract codegen coverage, and scoped transaction codegen coverage exist, but phase-B children remain open. | Finish M26b phase B (`td-c0a7a1`, `td-13a997`, `td-25667e`) and add storage canary/roundtrip policy. |
| Law and constitution examples | Research-grade | Useful real programs exist and mint checks legacy Danish constitution chapters. They are stress/examples, not production legal semantics. | Keep them as semantic pressure tests; do not market them as legal-reasoning production support. |
| `runa from-rust` and `from-rust --verify` | Research-grade | Feature stage is experimental even though CI runs the example transpiler validation. | Define supported subset and compatibility boundary before promotion. |
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
| Rust codegen for pure/core programs | Production-ready | Keep artifact fixture diffs compatibility-reviewed; add fixtures for newly promised emitted shapes; keep pure/core canaries 0-skip. | M | 5 |
| Importable local libraries and downstream consumer shape | Preview | Cover local import consumers in generic check-codegen (`td-35b4e3`); add import-aware deep-search cases (`td-a70b05`); add import-consumer expectation cases (`td-b4729e`). | L | 5 |
| Streams, subjects, actors, and effect-heavy workflows | Production-ready | Keep named-scope lifetime diagnostics stable; keep compiled stateful canaries and async artifact expectations green; add adversarial fixtures for future stateful bug classes. | M | 5 |
| WASM-facing behavior | Preview | Keep automated WASM build canary green; require the WASM lane in CI where `wasm-pack` is installed; add package-shape expectations after choosing the JS/ABI boundary. | M | 4 |
| Rust interop and integration | Preview | Add stable-shape interop canaries; broaden `runa lib` artifact expectations; document supported crate/import patterns. | L | 4 |
| Library hygiene tooling | Preview | Deepen purity analysis through local helper calls (`td-fd7715`); add negative expectation cases; surface library markers in docs/tooling. | M | 3 |
| Differential and generative testing | Preview | Keep minimized replay corpus growing; add import-aware generation pressure; scale CI stress count once runtime is predictable. | L | 5 |
| Compiletest-style expectations | Preview | Grow diagnostics and golden snapshot corpus; require exact expectations for new diagnostics where stable; keep expectation lanes fast enough for mint. | M | 4 |
| FIR and phase snapshots | Preview | Add larger reviewed golden snapshots; connect snapshots to import normalization; document which phase markers are stable enough for tests. | M | 4 |
| Explicit proof terms and proof kernel | Preview | Trusted-core size/surface audit recorded; add more adversarial kernel tests; keep solver fallback outside the kernel trust boundary. | M | 4 |
| `runa verify` elaboration and solver-assisted verification | Preview | Add validation for generated theorem shape; expand proof-backed checking around computation lemmas; document unsupported automation cases. | XL | 4 |
| Proof-backed compiler checking | Research-grade | Continue `td-a97ed9`; add checked compiler slices beyond bootstrap; choose one high-risk lowering pass for translation validation. | XL | 5 |
| `@ persist` and SQL-backed storage | Research-grade | Finish M26b phase B (`td-c0a7a1`); harden assert/retract/findall/transaction/watch/migrate tasks; add storage-specific canary and skip policy. | XL | 4 |
| Law and constitution examples | Research-grade | Keep as semantic pressure tests; document non-legal-production status in README/docs; add expectation/canary distillations only when they expose language bugs. | S | 2 |
| `runa from-rust` tooling | Research-grade | Define supported subset; add compatibility boundary docs; add fixture categories for accepted vs rejected Rust shapes. | L | 3 |
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
> pure/core Rust codegen behavior, and documented reactive/stateful workflows
> are production-ready for small to medium programs that stay inside documented
> syntax, stdlib, named-scope stream lifetime, actor, and effect-handler
> contracts, with strong compiled/canary/artifact evidence and explicit skip
> accounting.

The unsafe production claim is:


Those surfaces are valuable and tested, but they remain preview or
research-grade until the next gates above are closed.

## Next Evaluation Tasks

2. Keep exact diagnostic and phase expectations growing as new stable
   diagnostics or phase markers are promised.
3. Decide where `wasm-pack` should be required rather than skipped and add
   package-shape expectations for the chosen WASM boundary.
4. Require every future differential-found compiler bug to land in the
   checked-in replay corpus.
