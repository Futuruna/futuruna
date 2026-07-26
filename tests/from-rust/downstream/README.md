# From-Rust Downstream Fixtures

This directory is the downstream-style canary corpus for `runa from-rust`.
`scripts/from-rust-downstream-canary.sh` copies these fixtures into a fresh
temporary directory, then runs `runa from-rust --test` there so the lane does
not rely on generated files, ambient working-directory state, or the broader
`examples/from-rust` corpus.

- `supported/` contains deterministic single-file Rust programs that must
  exact-match Rust stdout after translation to Futuruna.
- `unsupported/` contains intentionally out-of-boundary Rust programs with
  `runa-from-rust: expect-unsupported` directives. These must fail closed with
  stable diagnostics instead of silently entering the supported subset.

The supported corpus currently covers config validation, deterministic event
aggregation, invoice arithmetic, text command parsing, enum/reference loop
aggregation with conditional accumulator rebinding, nested customer/order data,
error-row pipelines, deterministic inventory reporting, and text normalization.
The unsupported corpus currently covers general borrowed-reference returns,
async/threading, unsafe blocks, external crate imports, unchecked associated
types, unchecked `impl Trait`, unsupported `Result::map_err`, unsupported
iterator state machines, unsupported tuple-of-references matches, and
unsupported macro names outside the checked macro subset.
