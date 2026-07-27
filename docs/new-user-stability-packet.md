# New-User Stability Packet

Tracked by `td-1c5b00`.

This page is the user-facing stability ledger for Futuruna. It names what a new
user can rely on, what is still a conjecture, where the trust boundary is, and
which gates turn those statements into evidence.

Read this with:

- [first-run-contract.md](first-run-contract.md)
- [production-readiness-scorecard.md](production-readiness-scorecard.md)
- [feature-stages.md](feature-stages.md)
- [proof-backed-checking.md](proof-backed-checking.md)
- [mint-gate.md](mint-gate.md)

## Evidence Classes

| Evidence class | Meaning |
|----------------|---------|
| Kernel-checked | A proof term is checked by `src/proof_kernel.rs` inside the documented kernel boundary. |
| Translation-checked slice | A compiler output or normalization report is checked against an independently derived source obligation for a narrow shape. |
| Blocking gate | The behavior is exercised by `./scripts/mint.sh` or another required CI/local gate. |
| Exact expectation | `runa expect tests/expect` asserts exact pass/fail output, diagnostics, phase markers, or artifacts. |
| Realistic canary | Authored canaries or downstream fixtures exercise a user-shaped workflow across subsystems. |
| Differential/generative | Replayable or seed-stable generated programs compare interpreter, compiled, or translated behavior. |
| Documented preview | Useful and supported, but the contract is intentionally still hardening. |
| Outside claim | Futuruna explicitly does not promise this behavior yet. |

## First-Hour Stability Matrix

| User path | State | Evidence | Strengthening action |
|-----------|-------|----------|----------------------|
| `runa init`, `check`, `fmt --check`, `run`, `build` on a new project | Production-ready | Blocking `./scripts/first-run-canary.sh` inside mint | Keep as mint-blocking. |
| Tutorial 01 `.runa` snippets | Production-ready | Blocking first-run canary extracts, checks, formats, and runs snippets | Add tutorial snippets to this lane when the tutorial expands. |
| `runa feature-stages --json` | Stable/production-facing | Blocking first-run canary checks schema and stable core CLI metadata | Keep CLI help, JSON, and docs synchronized. |
| Local qualified import/library use | Production-ready for documented import contract | Blocking first-run canary now checks/runs/builds a local library consumer; downstream canary remains the deeper lane | Add a first-hour or downstream fixture for every future import-consumer bug. |
| Common syntax mistake: `=>` return arrow | Stable diagnostic | Exact expectation `tests/expect/diagnostics/parse_bad_arrow.runa`; first-run intentional failure check | Keep Futuruna-level help text stable. |
| Missing local import file | Stable diagnostic | Exact expectation `tests/expect/diagnostics/missing_import.runa`; first-run intentional failure check | Must fail before codegen and must not print `check ok`. |
| Private qualified import member | Stable diagnostic | Exact expectation `tests/expect/imports/import_private_symbol_fail.runa`; first-run intentional failure check | Must fail before rustc and suggest `@ export`. |
| Undefined function/name, wrong arity, literal type mismatch, heterogeneous lists, branch literal mismatch | Stable diagnostics for covered shapes | Existing diagnostics expectations under `tests/expect/diagnostics/` | Expand when another first-hour mistake leaks into rustc or a vague message. |
| Malformed `@ depend` | Stable diagnostic for covered shapes | Existing dependency expectations | Keep dependency syntax errors out of raw Cargo/Rust output. |
| Live stream use inside ordinary functions | Stable diagnostic for covered shapes | Existing named-scope lifetime expectations | Keep new stateful lifetime failures in expectations or stateful canaries. |

## Conjecture And Trust-Boundary Ledger

| Area | User-facing claim | Evidence class | Trust boundary | Weak conjecture or next strengthening |
|------|-------------------|----------------|----------------|--------------------------------------|
| Core syntax and ordinary expression semantics | Production-ready inside documented syntax | Blocking gate, exact expectations, realistic canaries, differential/generative | Parser/typechecker/interpreter/codegen are trusted Rust, not proved | Keep every semantic bug in the narrowest permanent lane. |
| Documented stdlib semantics | Production-ready for documented builtins | Blocking gate, exact expectations, regression tests | Builtin implementations and type tables are trusted Rust/templates | Add expectation cases for newly promised edge diagnostics. |
| Core CLI workflow | Production-ready for `run`, `check`, `emit`, `build`, `test`, `fmt`, `hashes`, `feature-stages` | Blocking mint and first-run canary | CLI orchestration, rustc/Cargo invocation, and filesystem behavior are trusted conventional code | Keep first-hour and mint gates authoritative. |
| Pure/core Rust codegen | Production-ready for stable pure/core programs | Blocking check-codegen/roundtrip, artifact expectations, translation-checked ownership slice | General Rust emission is trusted; exact helper layout is internal outside named artifacts | Add artifact or translation checks for every newly promised emitted shape. |
| Interpreter-vs-compiled parity | Production-ready for pure/core corpus | Blocking roundtrip, core canaries, differential/generative | Both implementations may share source assumptions; rustc remains external | Keep minimized differential replay growing. |
| Importable local libraries | Production-ready for flat/qualified local import contract | Downstream canary, import expectations, first-run import smoke, lint-library gates, exact `emit-imports` normalization expectation, mixed-alias codegen expectation | Import normalization/export filtering is trusted code with an exact public graph snapshot and diagnostics | Extend the translation-check slice when nested import shapes or new exported declaration forms are added. |
| Reactive/stateful workflows | Production-ready for documented named-scope contract | Stateful canaries, async artifact expectations, lifetime diagnostics | Async scheduling and runtime teardown are trusted Rust behavior | Keep deterministic canaries broad; prove only small algebraic stream laws later. |
| Rust interop via `runa lib` | Production-ready inside documented Rust-facing library contract | Rust interop canary, artifact expectation, downstream Cargo consumers | Cargo packaging and richer FFI patterns remain outside this contract | Require canaries before expanding generated manifests or FFI shapes. |
| FRSS-v0 `runa from-rust` | Production-ready only for the documented single-file supported subset | Exact stdout matches, downstream unsupported fixtures, generated differential lane, stable verify summaries | Rust parsing/lowering is trusted outside checked source-shape fixtures | Arbitrary Rust crate translation, module trees, broad macros, unsafe, async/threading, and effectful APIs remain outside claim. |
| Differential/generative testing | Production-ready as a hardening lane | Blocking/reproducible generated and replay lanes | Generator coverage is evidence, not proof of absence | Scale only while runtime stays predictable; every found bug gets a permanent fixture. |
| Compiletest-style expectations | Preview as a corpus shape, production-useful as a gate | Mint expectation runner and exact fixtures | Fixture selection is human-reviewed; corpus is still growing | Grow first-hour and artifact snapshots. |
| FIR and phase snapshots | Preview | Phase expectations | Phase marker stability is selective | Document which markers are stable and expand reviewed snapshots. |
| Explicit proof terms and proof kernel | Stable compatibility for documented kernel forms, production-readiness still preview | Kernel tests and proof canaries | Kernel core is trusted; elaboration around it is not part of the kernel | Keep kernel boundary audits and adversarial tests current. |
| `runa verify` theorem elaboration | Preview | Proof workflows, computation-lemma checker, selected tests | The compiler still decides which theorem to ask the kernel/solver to check | Add theorem-shape validation and snapshots before production promotion. |
| Proof-backed compiler checking | Research-grade but strategically important | Computation-lemma validation and ownership-reuse translation check | Most compiler passes remain trusted Rust | Add the next translation-checked slice for import normalization/export preservation. |
| WASM artifacts | Preview | WASM tests and canary with explicit missing-tool skip | JS/ABI/package shape is not frozen | Decide where `wasm-pack` is required and add package-shape expectations. |
| `@ persist` and SQL-backed storage | Research-grade | Storage canaries and phase expectations | SQL schema/migration/runtime behavior still needs broader contracts | Finish storage phase work before any production claim. |
| Law/constitution examples | Research-grade examples | Example checks and semantic pressure tests | Not legal advice, not legal-production reasoning | Keep as stress/examples; do not market as production legal semantics. |
| `runa audit`, LSP, exploratory tooling | Research-grade or preview | Limited tests and feature-stage labels | Output/interface shape is not frozen | Add contracts and canaries before treating as first-user-critical. |

## Formal Strengthening Ranking

| Rank | Conjecture to strengthen | Current status | Next concrete move |
|------|--------------------------|----------------|--------------------|
| 1 | Generated proof obligations match the source they claim to model | Computation-lemma validation is implemented and documented in [proof-backed-checking.md](proof-backed-checking.md) | Reduce shared lowering trust by adding independent theorem-shape snapshots for proof canaries. |
| 2 | Import normalization preserves exported declarations and excludes script-only/import-time behavior | Downstream canaries, import expectations, first-hour private/missing import diagnostics, `tests/expect/imports/import_normalization_contract.runa`, and `tests/expect/imports/import_mixed_alias_codegen_pass.runa` defend behavior | Extend source import graph -> normalized module/export graph snapshots for nested imports, hash imports, and future exported declaration forms. |
| 3 | Pure expression/FIR lowering preserves source semantics | Differential, roundtrip, phase expectations, typed lowering tests | Add small source/target model checks for arithmetic, ADTs, tuples/lists, and simple matches. |
| 4 | Ownership-sensitive Rust emission keeps later source reads valid | Narrow translation check covers direct branch/list reuse; artifact expectations cover selected shapes | Extend obligations to match arms, destructuring, helper-call returns, and nested branch/list reuse. |
| 5 | `runa verify` asks the kernel/solver the intended theorem | Preview; computation lemmas are checked but broader elaboration remains trusted | Snapshot theorem shapes for representative invariants and reject unsupported automation cleanly. |
| 6 | Storage and law examples preserve high-level domain meaning | Research-grade | Keep them as pressure tests until contracts, canaries, and failure modes are explicit. |

## Fail-Closed Inventory

| Surface | Failure that must not look successful | Current status |
|---------|---------------------------------------|----------------|
| `@ import ./missing` | Missing file printed a warning and still allowed `check ok` | Fixed by Futuruna diagnostic and expectation `missing_import.runa`. |
| `@ import M from ./lib` private member access | Raw rustc privacy error leaked through `runa check` | Fixed by Futuruna diagnostic and expectation `import_private_symbol_fail.runa`. |
| `=>` in function signature | Parser error could be unclear | Covered by exact diagnostic and first-run intentional failure. |
| Malformed `@ depend` | Cargo/Rust dependency confusion | Covered by exact diagnostics. |
| Unsupported WASM exports | Invalid package/export shape should not build garbage | Covered by explicit unsupported WASM export diagnostics and WASM canary. |
| Missing optional `wasm-pack` | Local mint should not pretend the WASM build passed | Reported as an explicit skip unless required by env. |
| Unsupported Rust in FRSS-v0 | Unsupported source should not translate into broken Futuruna | Covered by expected-unsupported fixtures and stable categories. |

## Rule For Future Bugs

When a new user can hit a confusing failure in the first hour, reduce it into
one of these lanes before expanding the language surface:

1. exact expectation for diagnostics/pass-fail behavior
2. first-run canary step if it belongs to the golden first-hour path
3. authored canary for realistic multi-subsystem workflow
4. downstream fixture for import/library-consumer behavior
5. differential or minimized replay case for semantic drift
6. proof-backed or translation-checked slice when the issue exposes a trust-boundary assumption

The default posture is: fail early, fail in Futuruna terms, and state the
remaining conjecture openly.
