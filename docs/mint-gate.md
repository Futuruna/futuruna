# Mint Gate

The canonical local and CI command for proving Futuruna is mint is:

```bash
./scripts/mint.sh
```

It runs the regression-prone lanes that have historically caught user-facing breakage:

```bash
cargo test --quiet
cargo build --release
./scripts/first-run-canary.sh
./scripts/rust-interop-canary.sh
./scripts/from-rust-downstream-canary.sh
./scripts/from-rust-differential.sh
./target/release/runa test
./target/release/runa test --run
./target/release/runa expect tests/expect
./target/release/runa test --check-codegen
./target/release/runa test --roundtrip tests
./target/release/runa run tests/codegen_integration_regression_test.runa
./scripts/storage-canary.sh
./scripts/wasm-canary.sh
./target/release/runa check examples/danish-constitution-legacy/kapitel-02.runa
./target/release/runa check examples/danish-constitution-legacy/kapitel-03.runa
./target/release/runa check examples/danish-constitution-legacy/kapitel-04.runa
./target/release/runa check examples/danish-constitution-legacy/kapitel-05.runa
./target/release/runa check examples/danish-constitution-legacy/kapitel-06.runa
./target/release/runa check examples/danish-constitution-legacy/kapitel-07.runa
```

These lanes are the core mint contract because they cover:

- Rust unit and integration tests
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
- offline compiled persisted-storage transaction runtime canaries
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
- `./target/release/runa fmt --check tests/`
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
gate.

The storage canary lane runs compiled persisted transaction fixtures from a
temporary directory with `CARGO_NET_OFFLINE=true`. It is inside mint because the
generic `runa test --run` and phase expectations do not prove the SQLite-backed
transaction guard's commit, rollback, and nested savepoint behavior at runtime.

The WASM canary lane discovers fixtures marked with `-- wasm-build-canary` and
runs `runa wasm` for each one. By default, a missing `wasm-pack` is reported as
a skip so local mint remains usable on machines without the optional toolchain.
Set `FUTURUNA_WASM_CANARY_REQUIRED=1` in CI when missing `wasm-pack` should fail
the job.
