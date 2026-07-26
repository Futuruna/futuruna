# Canary Suite

Futuruna's canary suite is the authored middle lane between the fast mint gate
and the heavier differential lane.

The canonical command is:

```bash
./scripts/canary.sh
```

For the separate authored library-consumer lane, use:

```bash
./scripts/downstream-canary.sh
```

For narrow compiler expectations such as diagnostics, pass/fail behavior, and
phase-specific structural markers, use:

```bash
./scripts/expectations.sh
```

For persisted-storage runtime coverage, use:

```bash
./scripts/storage-canary.sh
```

For the new-user project scaffold and first tutorial examples, use:

```bash
./scripts/first-run-canary.sh
```

The stable first-run path is documented in
[docs/first-run-contract.md](first-run-contract.md).

For Rust-facing integration through `runa lib`, use:

```bash
./scripts/rust-interop-canary.sh
```

For Rust-to-Futuruna validation through `runa from-rust`, use:

```bash
./scripts/from-rust-downstream-canary.sh
```

It intentionally uses programs written in this repository rather than pulling in
downstream codebases as fixtures. The goal is to keep the suite:

- realistic
- curated
- semantically broad
- stable enough to be blocking in CI

## What belongs here

- small end-to-end programs that mix multiple Futuruna subsystems
- user-shaped workflows such as collection analytics, sliding-window pipelines,
  and subject-backed projections
- distilled versions of bug patterns that are broader than a single minimized
  regression

## What does not belong here

- tiny single-feature probes that are better as ordinary tests
- exact compiler diagnostic or phase-output assertions that are better as
  expectation cases under `tests/expect/`
- giant downstream applications owned by another repository
- random external ports with unclear maintenance value

## Tier Layout

- `tests/canary/core/`
The blocking authored lane. These canaries should stay green in interpreter,
compiled, codegen, and roundtrip execution.

- `tests/canary/stateful/`
Subjects, actors, lifecycle, and richer effectful workflows. This tier is part
of the production evidence for the stable reactive/stateful surface: compiled
execution is blocking, live-async roundtrip/check-codegen skips are reported
explicitly, and async runtime artifact expectations cover emitted Rust shapes
that the generic lanes intentionally skip.

- `tests/canary/extended/`
Heavier authored programs for JSON, DB, HTTP, WASM, regex, or import-heavy
flows.

- `tests/canary/regressions/`
Broader authored workflows distilled from user bug classes.

- `tests/canary/storage/`
Persisted SQLite-backed runtime canaries. This tier has a dedicated script
because it runs from a temporary directory with `CARGO_NET_OFFLINE=true` and one
fixture is intentionally expected to fail so a follow-up fixture can verify
rollback state.

- `tests/canary/interop/`
Rust-facing integration fixtures. These use a dedicated script because the
contract is not just "generated Rust compiles"; it is that an ordinary Rust
consumer can compile the `runa lib` output and call the exported API, including
the case where generated code relies on external Cargo dependencies and the
case where generated output is packaged as `src/lib.rs` in a downstream
Cargo dependency. The same lane also covers the missing-dependency setup error:
generated source and `runa lib` stderr must list required Cargo.toml entries
before an intentionally incomplete consumer fails to compile.

- `tests/from-rust/downstream/`
Rust-to-Futuruna validation fixtures. These use a dedicated script because the
contract is differential rather than authored Futuruna execution: each
supported Rust file is compiled and run with `rustc`, translated with
`runa from-rust`, interpreted as Futuruna, and required to exact-match stdout.
The lane copies fixtures into a fresh temporary directory before running and
also includes expected-unsupported files that prove documented non-goals fail
closed with stable diagnostics.

See [docs/canary-matrix.md](canary-matrix.md) for the current coverage map and
the planned build-out.

## Current contract

`./scripts/canary.sh` runs every non-empty tier by default. It also accepts
explicit targets, for example:

```bash
./scripts/canary.sh core
./scripts/canary.sh core stateful
./scripts/canary.sh tests/canary/regressions
```

For each selected tier it runs:

```bash
./target/release/runa fmt --check <tier>
./target/release/runa test --run <tier>
./target/release/runa test --check-codegen <tier>
./target/release/runa test --roundtrip <tier>
```

The roundtrip and generic check-codegen lanes will naturally skip canaries that
use constructs the generic runners intentionally exclude, such as live
`subject()`-backed async streams. Those skips are not counted as pass evidence
for the stable stateful surface; the required evidence is compiled execution,
stateful canary invariants, stream lifetime diagnostics, and emitted-Rust
artifact expectations under `tests/expect/artifact/`.

## Storage Runtime Lane

`./scripts/storage-canary.sh` is the blocking lane for persisted transaction
runtime behavior. It runs compiled fixtures from `tests/canary/storage/` in a
temporary working directory so generated `.runa-build/` projects and SQLite
databases do not dirty the repository.

The script sets `CARGO_NET_OFFLINE=true` by default. With the generated Cargo
dependencies already present in the local Cargo cache, this proves the runtime
canary does not rely on ambient network access. The lane checks:

- committed persisted writes are visible after a transactional `| scope`
- nested persisted scopes use savepoint/release behavior
- a failing scope rolls back its persisted write
- the rollback proof is read back through a separate compiled Futuruna program

## Downstream Consumer Lane

`./scripts/downstream-canary.sh` is the companion blocking lane for authored
fixtures that model Futuruna code being consumed as local libraries. It is part
of the production evidence for the stable importable-local-library surface.

It is not part of the tiered canary corpus because the emphasis is different:

- library-consumer entrypoints rather than standalone workflows
- nested and qualified import paths
- exported type/value/function usage across files
- direct `runa check` coverage on consumer programs
- compiled execution and generic codegen coverage for pure/effect consumers
- explicit skip accounting for live-async imported stream helpers that belong to
  the stateful async artifact contract rather than the generic rustc metadata
  lane

The fixtures live in `tests/downstream/` and should stay authored and owned by
this repository.

The lane also runs:

```bash
./target/release/runa lint-library tests/downstream
./target/release/runa lint-library --imports tests/downstream
```

Those checks keep marked importable helper files honest, reject imported
script/demo leakage, and verify helper-call-chain impurity before downstream
consumers run.

## From-Rust Downstream Lane

`./scripts/from-rust-downstream-canary.sh` is the mint-blocking lane for
consumer-shaped `runa from-rust` validation. It is separate from the full
`examples/from-rust` corpus because it models what a downstream user expects
from a small deterministic Rust workflow: Rust stdout and translated Futuruna
stdout must match exactly from a clean directory.

The supported fixtures live in `tests/from-rust/downstream/supported/`; the lane
currently requires 9 exact Rust-vs-Futuruna matches from a clean temporary
directory. The unsupported fixtures live in
`tests/from-rust/downstream/unsupported/` and must carry
`runa-from-rust: expect-unsupported` directives so accidental promotion is
reported as an `XPASS` failure.

## WASM Build Canary Lane

`./scripts/wasm-canary.sh` discovers `.runa` fixtures marked with
`-- wasm-build-canary` and runs `runa wasm` for each one. The default target is
`tests/canary`, so the WASM export-surface fixture participates without a
hand-maintained file list.

If `wasm-pack` is unavailable, the lane reports an explicit skip and exits
successfully by default. Set `FUTURUNA_WASM_CANARY_REQUIRED=1` in CI when the
optional WASM toolchain should be mandatory.
