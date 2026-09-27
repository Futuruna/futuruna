# Mint Gate

The canonical local and CI command for proving Futuruna is mint is:

```bash
./scripts/mint.sh
```

It runs the regression-prone lanes that have historically caught user-facing breakage:

```bash
cargo build --release
FUTURUNA_MODEL_TEST_RUNA="$PWD/target/release/runa" cargo test --quiet
./scripts/first-run-canary.sh
./scripts/rust-interop-canary.sh
./scripts/from-rust-downstream-canary.sh
./scripts/from-rust-differential.sh
./scripts/compiler-cross-product-canary.sh
./target/release/runa test
./target/release/runa test --run
./target/release/runa expect tests/expect
./target/release/runa test --check-codegen
./target/release/runa test --roundtrip tests
./target/release/runa run tests/codegen_integration_regression_test.runa
./scripts/wasm-canary.sh
./target/release/runa check examples/danish-constitution-legacy/kapitel-02.runa
./target/release/runa check examples/danish-constitution-legacy/kapitel-03.runa
./target/release/runa check examples/danish-constitution-legacy/kapitel-04.runa
./target/release/runa check examples/danish-constitution-legacy/kapitel-05.runa
./target/release/runa check examples/danish-constitution-legacy/kapitel-06.runa
./target/release/runa check examples/danish-constitution-legacy/kapitel-07.runa
```

Mint builds the optimized compiler from the current checkout first, then pins
`FUTURUNA_MODEL_TEST_RUNA` to that artifact for calculation CLI and model tests
that support this override. It overwrites any inherited override: an older
release or corpus binary must not validate changed compiler source. This keeps
large model/template checks practical without dropping tests or assertions.
Rust unit tests still use Cargo's test profile. Ordinary focused `cargo test`
runs without the override continue to use `CARGO_BIN_EXE_runa` (the debug CLI).
When changing calculation runtime behavior, exercise a focused regression in
that default mode too; mint then checks the optimized executable.

These lanes are the core mint contract because they cover:

- Rust unit and integration tests
- formatting and source-preservation regressions across `tests/`, including
  required rejection of intentionally malformed parser fixtures, through
  `tests/formatter_preservation.rs`
- the first-run golden path: `runa init`, generated project metadata/source,
  `check`, `fmt --check`, `run`, `build`, feature-stage metadata visibility,
  a local qualified import/library smoke, intentional first-hour diagnostic
  failures, and tutorial 01 `.runa` examples, as documented in
  [first-run-contract.md](first-run-contract.md)
- the Rust-facing integration canary: `runa lib` output compiled into a plain
  Rust consumer that calls exported structs, enums, borrowed params, lists,
  `Option`, and `Result`, plus an offline Cargo consumer for external-crate
  integration through `@ depend`, `@ use`, and regex-backed generated Rust, plus
  a downstream Cargo package consuming generated `src/lib.rs` by path dependency
  and an intentional missing-dependency consumer that proves dependency guidance
  appears in generated source/stderr
- the from-rust downstream canary: deterministic consumer-shaped Rust fixtures
  copied into a fresh temp directory, exact-matched against translated
  Futuruna, plus a fail-closed unsupported ownership fixture
- the from-rust supported-subset differential canary: generated seed-stable
  Rust programs inside FRSS-v0 exact-match Rust vs translated Futuruna output
  from a checked-in search manifest and leave replay/minimization artifacts on
  failure
- interpreted Futuruna execution
- compiled Futuruna execution
- compiletest-style diagnostic, run/fail, and phase expectations
- Rust codegen validation across the test corpus
- interpreter-vs-compiled roundtrip parity across the test corpus
- the blocking codegen regression program
- database-retirement diagnostics and preserved host-boundary behavior
- WASM export build canaries, with an explicit skip when `wasm-pack` is unavailable
- real example programs outside `tests/` that have previously exposed compiler bugs

Intentionally omitted from the core mint gate:

- `./scripts/canary.sh`
- `./scripts/downstream-canary.sh`
- `./scripts/differential.sh`
- the full `./target/release/runa from-rust --test examples/from-rust/`
  example corpus, which is a separate CI-blocking translational-tooling lane
  documented in
  [from-rust-contract.md](from-rust-contract.md)
- standalone solver-dependent flows such as `runa verify file.runa`
- tests that the `runa test` runner already skips because they require optional external crates

CI should call `./scripts/mint.sh` for the core language health gate, then run any omitted lanes as separate jobs or steps. The canary suite is the curated middle lane for realistic authored programs, while differential testing is the deeper search lane that exercises seed-stable generative programs and replayable minimized repros without slowing every core mint run.

The full from-rust example corpus remains separate from mint for runtime budget
and is documented in the stable FRSS-v0 contract. Mint still runs the narrower
downstream canary and the generated supported-subset differential lane so the
production-readiness signal includes clean-directory consumer-shaped exact
matching, fail-closed unsupported-shape diagnostics, and generated FRSS-v0 exact
matching.

Passing machine lanes set `FUTURUNA_SUPPRESS_COMPTIME_DIAGNOSTICS=1` so
informational `@ comptime` and auto-comptime comments do not obscure the
structured step output. Comptime assertion failures and ordinary compiler
diagnostics still print because they are real failures, not progress chatter.

The downstream lane includes `runa lint-library tests/downstream` and
`runa lint-library --imports tests/downstream`, so the stable importable-library
contract is enforced there even though it remains outside the fast core mint
gate. It runs `runa test tests/downstream` as well as the native checks, so
imported subject initialization is exercised by the interpreter even when a
fixture is excluded from generic roundtrip testing.

Database retirement regressions run in the ordinary Rust test lane. They check
explicit errors for removed persistence syntax and builtins, plus preserved
in-memory logic, typed calculations, ordinary assertions, and scope behavior.

The WASM canary lane discovers fixtures marked with `-- wasm-build-canary` and
runs `runa wasm` for each one. By default, a missing `wasm-pack` is reported as
a skip so local mint remains usable on machines without the optional toolchain.
Set `FUTURUNA_WASM_CANARY_REQUIRED=1` in CI when missing `wasm-pack` should fail
the job.
