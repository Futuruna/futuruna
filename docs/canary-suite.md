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
- giant downstream applications owned by another repository
- random external ports with unclear maintenance value

## Tier Layout

- `tests/canary/core/`
The blocking authored lane. These canaries should stay green in interpreter,
compiled, codegen, and roundtrip execution.

- `tests/canary/stateful/`
Subjects, actors, lifecycle, and richer effectful workflows. Some of these may
roundtrip-skip selectively, but they are still required to pass interpreted and
compiled execution.

- `tests/canary/extended/`
Heavier authored programs for JSON, DB, HTTP, WASM, regex, or import-heavy
flows.

- `tests/canary/regressions/`
Broader authored workflows distilled from user bug classes.

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

The roundtrip lane will naturally skip canaries that use constructs the generic
roundtrip runner already excludes, such as `subject()`. Those canaries still
participate in interpreted and compiled execution plus codegen validation.

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
