# Canary Suite

Futuruna's canary suite is the authored middle lane between the fast mint gate
and the heavier differential lane.

The canonical command is:

```bash
./scripts/canary.sh
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

## Recommended shape

Each canary should stress more than one area at once, for example:

- lists + maps + sets
- streams + windows + reductions
- subjects + stream operators + collection projection
- ownership-sensitive list or string transforms in realistic pipelines

## Current contract

`./scripts/canary.sh` runs:

```bash
./target/release/runa fmt --check tests/canary
./target/release/runa test --run tests/canary
./target/release/runa test --check-codegen tests/canary
./target/release/runa test --roundtrip tests/canary
```

The roundtrip lane will naturally skip canaries that use constructs the generic
roundtrip runner already excludes, such as `subject()`. Those canaries still
participate in interpreted and compiled execution plus codegen validation.
