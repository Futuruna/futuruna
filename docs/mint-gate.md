# Mint Gate

The canonical local and CI command for proving Futuruna is mint is:

```bash
./scripts/mint.sh
```

It runs the regression-prone lanes that have historically caught user-facing breakage:

```bash
cargo test --quiet
cargo build --release
./target/release/runa test
./target/release/runa test --run
./target/release/runa test --check-codegen
./target/release/runa run tests/codegen_integration_regression_test.runa
./target/release/runa check examples/danish-constitution-legacy/kapitel-02.runa
./target/release/runa check examples/danish-constitution-legacy/kapitel-03.runa
./target/release/runa check examples/danish-constitution-legacy/kapitel-04.runa
./target/release/runa check examples/danish-constitution-legacy/kapitel-05.runa
./target/release/runa check examples/danish-constitution-legacy/kapitel-06.runa
./target/release/runa check examples/danish-constitution-legacy/kapitel-07.runa
```

These lanes are the core mint contract because they cover:

- Rust unit and integration tests
- interpreted Futuruna execution
- compiled Futuruna execution
- Rust codegen validation across the test corpus
- the blocking codegen regression program
- real example programs outside `tests/` that have previously exposed compiler bugs

Intentionally omitted from the core mint gate:

- `./target/release/runa test --roundtrip`
  This is tracked separately in `td-f220b1`; the current known divergences are
  `comptime_types_test.runa` and `traits_test.runa`.
- `./target/release/runa from-rust --test examples/from-rust/`
- `./target/release/runa fmt --check tests/`
- standalone solver-dependent flows such as `runa verify file.runa`
- tests that the `runa test` runner already skips because they require optional external crates

CI should call `./scripts/mint.sh` for the core language health gate, then run any omitted lanes as separate jobs or steps.
