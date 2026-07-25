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
fixtures that model Futuruna code being consumed as local libraries.

It is not part of the tiered canary corpus because the emphasis is different:

- library-consumer entrypoints rather than standalone workflows
- nested and qualified import paths
- exported type/value/function usage across files
- direct `runa check` coverage on consumer programs

The fixtures live in `tests/downstream/` and should stay authored and owned by
this repository.

The lane also runs:

```bash
./target/release/runa lint-library tests
```

That keeps marked importable helper files honest and stops script/demo leakage
from creeping into library-shaped fixtures.

## WASM Build Canary Lane

`./scripts/wasm-canary.sh` discovers `.runa` fixtures marked with
`-- wasm-build-canary` and runs `runa wasm` for each one. The default target is
`tests/canary`, so the WASM export-surface fixture participates without a
hand-maintained file list.

If `wasm-pack` is unavailable, the lane reports an explicit skip and exits
successfully by default. Set `FUTURUNA_WASM_CANARY_REQUIRED=1` in CI when the
optional WASM toolchain should be mandatory.
