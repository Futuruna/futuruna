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

Evidence source: local scorecard sweep recorded in `td-f7f0d2` on 2026-05-01,
current CI wiring, and the current open `td` queue.

| Lane | Evidence | Readiness Signal |
|------|----------|------------------|
| Mint gate | Latest recorded `./scripts/mint.sh` passed: Rust tests, release build, interpreted tests, compiled tests, expectations, check-codegen, roundtrip, one regression run, and Danish constitution checks. | Strong core health signal. |
| Mint check-codegen | 84 passed, 22 skipped in the latest recorded mint run. | Passing but skip-heavy for external-crate surfaces. |
| Mint roundtrip | 71 matched, 35 skipped in the latest recorded mint run. | Strong for pure/core programs; weaker for external/stateful surfaces. |
| Expectations | 8 expectation cases passed. | Useful but still small. `td-4d7e81` remains open. |
| Authored canaries | `./scripts/canary.sh` passed across core, stateful, extended, and regressions. | Strong realistic workflow signal. |
| Core canary tier | 10 run passed, 10 check-codegen passed, 10 roundtrip matched, 0 skipped. | Best production-readiness evidence in the project. |
| Stateful canary tier | 6 run passed, 6 check-codegen skipped, 6 roundtrip skipped. | Execution signal is good; generic parity evidence is weak. |
| Extended canary tier | 10 run passed, 5 check-codegen passed, 5 skipped, 2 roundtrip matched, 8 skipped. | Useful coverage, but not production-grade as a whole. |
| Regression canary tier | 3 run passed, 2 check-codegen passed, 1 skipped, 1 roundtrip matched, 2 skipped. | Good bug-class coverage; skip accounting matters. |
| Downstream consumer lane | `./scripts/downstream-canary.sh` passed: 15 importable files passed hygiene, 13 downstream tests ran, 5 check-codegen passed, 8 skipped, 13 roundtrip skipped. | Import/lint coverage is real; generic parity is not yet broad. |
| Differential lane | Local reduced sweep passed 48 generated interpreter-vs-compiled cases, but `tests/differential/corpus` has no checked-in minimized cases. | Good generator signal; durable replay corpus is missing. `td-2061ce` tracks this. |
| CI wiring | CI runs mint on Ubuntu/macOS, Rust formatting, differential, authored canaries, and downstream consumers. | Strong project-level signal. |

## Surface Scorecard

| Surface | Rating | Evidence | Next Gate Before Promotion |
|---------|--------|----------|----------------------------|
| Core documented syntax and ordinary expression semantics | Production-ready | Stable docs, mint, core canaries with 0 skips, roundtrip parity, many focused regression tests. | Keep semantic changes under the contributor ratchet; expand expectation coverage for diagnostics and phase markers (`td-4d7e81`). |
| Documented stdlib builtin semantics | Production-ready | Stable docs, broad tests, typed lowering corpus, string/list/map/set/property tests, core canaries. | Finish builtin contract audits: duplicate evaluation (`td-e5bd06`) and signature-table audit (`td-4f849b`). |
| Core CLI commands: `run`, `check`, `emit`, `build`, `test`, `fmt`, `hashes` | Production-ready | Stable feature stage, mint and CI exercise the core commands. | Keep compatibility-guide enforcement moving (`td-6769d2`). |
| Rust formatting and repository hygiene | Preview | `runa fmt --check` is used by canary/downstream/CI, but repo-wide cargo fmt parity has an open task. | Close `td-e7d877`. |
| Compiler Rust codegen for pure/core programs | Preview | Strong mint/core-canary check-codegen signal; emitted Rust text remains explicitly unstable. | Broaden phase snapshots and expectation cases (`td-48e5d9`, `td-15746e`). |
| Interpreter-vs-compiled parity for pure/core programs | Production-ready | Mint roundtrip and core canary roundtrip are strong, with 0 core canary skips. | Seed minimized differential corpus so historical bugs replay permanently (`td-2061ce`). |
| Importable local libraries and downstream consumer shape | Preview | Dedicated downstream lane, library hygiene lint, and consumer checks pass. Generic roundtrip skips all downstream fixtures and check-codegen skips 8 of 13. | Teach generic check-codegen to cover local import consumers (`td-35b4e3`) and add import-aware deep-search cases (`td-a70b05`, `td-b4729e`). |
| Streams, subjects, actors, and effect-heavy workflows | Preview | Stateful canaries pass execution and docs mark the surface preview. Generic check-codegen and roundtrip skip the whole stateful tier. | Continue stream lifetime hardening (`td-17811d`) and reduce external-crate skip reliance with explicit compiled-codegen lanes. |
| WASM-facing behavior | Preview | WASM tests, an extended export-surface canary, and an automated `runa wasm` build lane exist. Missing `wasm-pack` is reported as an explicit skip unless CI requires it. | Define artifact stability boundaries and run the WASM lane as required in CI where the toolchain is installed. |
| Rust interop and Rust-facing integration | Preview | Feature stage is preview; CI runs `from-rust --test examples/from-rust/`. | Keep artifact/codegen boundaries explicit (`td-e579c9`) and add canaries for stable integration shapes before promotion. |
| `runa lint-library` import-hygiene tooling | Preview | Downstream lane enforces hygiene over importable files and imports. | Deepen purity analysis through local helper calls (`td-fd7715`). |
| `runa stress-gen` and differential testing | Preview | CI has a differential job and seed-stable generator. No minimized checked-in replay corpus exists yet. | Close `td-2061ce`; require every differential-found compiler bug to land in corpus. |
| Compiletest-style expectations | Preview | Expectation runner is in mint and has diagnostic/run/phase cases. Corpus is still small. | Close `td-4d7e81`; add golden snapshot support (`td-15746e`). |
| FIR and phase snapshots | Preview | Phase validation exists and has already caught drift classes. Cross-binding/module breadth remains open. | Close `td-48e5d9`. |
| Explicit proof terms and proof kernel surface | Preview | Small kernel exists in `src/proof_kernel.rs`; docs state the trust boundary honestly; proof canaries exist. Trusted-core audit remains open. | Close `td-f8f162` before calling this production-ready. |
| `runa verify` elaboration and solver-assisted verification | Preview | Feature stage is preview; useful proof workflows exist. Elaboration remains trusted compiler machinery. | Add more proof-backed validation around generated theorem shape and keep solver fallback outside the kernel trust boundary. |
| Proof-backed compiler checking | Research-grade | Translation-checking and proof-backed bootstrap work exist, but the compiler is not verified and high-risk passes remain trusted Rust. | Continue `td-a97ed9`; add checked compiler slices beyond toy/bootstrap cases. |
| `@ persist`, SQL-backed facts, watches, migrations, and transactional storage | Research-grade | Storage canaries exist and M26b is active, but persist work has open children and `td-f4f433` is still in progress pending commit/review. | Finish M26b phase B (`td-c0a7a1`, `td-7b097f`, `td-f4f433`, `td-c4282c`, `td-13a997`, `td-25667e`) and add storage canary/roundtrip policy. |
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
| Core syntax and ordinary expression semantics | Production-ready | Keep mint green; expand diagnostics/phase expectations (`td-4d7e81`); keep compatibility guide updates enforced (`td-6769d2`). | M | 4 |
| Documented stdlib builtins | Production-ready | Audit duplicate evaluation hazards (`td-e5bd06`); audit signature tables against stdlib docs (`td-4f849b`); add expectation cases for contract edges. | M | 4 |
| Core CLI workflow | Production-ready | Keep `mint` authoritative; make feature-stage metadata machine-readable (`td-0bd873`); enforce compatibility guide updates (`td-6769d2`). | M | 4 |
| Pure/core interpreter-vs-compiled parity | Production-ready | Seed minimized differential corpus (`td-2061ce`); keep core canaries 0-skip; promote every new semantic bug into the narrowest permanent lane. | M | 5 |
| Rust codegen for pure/core programs | Preview | Broaden FIR/phase snapshots (`td-48e5d9`); add golden-file support to expectations (`td-15746e`); audit emitted helper/evaluation boundaries. | L | 5 |
| Importable local libraries and downstream consumer shape | Preview | Cover local import consumers in generic check-codegen (`td-35b4e3`); add import-aware deep-search cases (`td-a70b05`); add import-consumer expectation cases (`td-b4729e`). | L | 5 |
| Streams, subjects, actors, and effect-heavy workflows | Preview | Continue stream lifetime epic (`td-17811d`); reduce stateful canary skip reliance with explicit compiled-codegen coverage; infer actor payloads from send sites (`td-ca379f`). | XL | 5 |
| WASM-facing behavior | Preview | Keep automated WASM build canary green; define artifact stability boundary; require the WASM lane in CI where `wasm-pack` is installed. | M | 4 |
| Rust interop and integration | Preview | Formalize artifact/codegen stability (`td-e579c9`); add stable-shape interop canaries; document supported crate/import patterns. | L | 4 |
| Library hygiene tooling | Preview | Deepen purity analysis through local helper calls (`td-fd7715`); add negative expectation cases; surface library markers in docs/tooling. | M | 3 |
| Differential and generative testing | Preview | Seed minimized replay corpus (`td-2061ce`); add import-aware generation pressure; scale CI stress count once runtime is predictable. | L | 5 |
| Compiletest-style expectations | Preview | Expand diagnostics and phase marker corpus (`td-4d7e81`); add golden-file snapshot support (`td-15746e`); require exact expectations for new diagnostics. | M | 4 |
| FIR and phase snapshots | Preview | Expand cross-binding/module samples (`td-48e5d9`); connect snapshots to import normalization; document which phase markers are stable enough for tests. | M | 4 |
| Explicit proof terms and proof kernel | Preview | Audit trusted-core size/surface (`td-f8f162`); add more adversarial kernel tests; keep solver fallback outside the kernel trust boundary. | L | 4 |
| `runa verify` elaboration and solver-assisted verification | Preview | Add validation for generated theorem shape; expand proof-backed checking around computation lemmas; document unsupported automation cases. | XL | 4 |
| Proof-backed compiler checking | Research-grade | Continue `td-a97ed9`; add checked compiler slices beyond bootstrap; choose one high-risk lowering pass for translation validation. | XL | 5 |
| `@ persist` and SQL-backed storage | Research-grade | Finish M26b phase B (`td-c0a7a1`); land assert/retract/findall/transaction/watch/migrate tasks; add storage-specific canary and skip policy. | XL | 4 |
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

> Futuruna's stable core language and core CLI are production-ready for small to
> medium pure/core programs that stay inside documented syntax and stdlib
> contracts, with strong interpreter/compiled/codegen/roundtrip evidence.

The unsafe production claim is:


Those surfaces are valuable and tested, but they remain preview or
research-grade until the next gates above are closed.

## Next Evaluation Tasks

2. Close `td-4d7e81` to grow exact diagnostic and phase expectations.
3. Define a WASM artifact stability boundary and decide where `wasm-pack` should
   be required rather than skipped.
4. Close `td-2061ce` so differential testing has a checked-in replay corpus.
5. Close `td-0bd873` so stage metadata can be queried mechanically instead of
   read from prose tables.
